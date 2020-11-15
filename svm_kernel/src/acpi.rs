#![allow(unused_imports)]

use crate::memory::{id_map_nocache, map_and_read_phys};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use modular_bitfield::prelude::*;
use rangeset::{Range, RangeSet};
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::PageSize;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

/// In-memory representation of an RSDP ACPI structure
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct Rsdp {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct RsdpExtended {
    descriptor: Rsdp,
    length: u32,
    xsdt_addr: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

/// In-memory representation of an ACPI table header
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct Header {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: u64,
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IoApic {
    typ: u8,
    length: u8,
    id: u8,
    res0: u8,
    address: u32,
    interrupt_base: u32,
}

impl fmt::Debug for IoApic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe { write!(f, "IoApic address: {:#x}", self.address) }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct LocalApic {
    typ: u8,
    length: u8,
    processor_uid: u8,
    id: u8,
    flags: u32,
}

impl fmt::Debug for LocalApic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LApic id: {}", self.id)
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IntOverride {
    typ: u8,
    length: u8,
    bus: u8, // always 0
    source: u8,
    mapped_to: u32,
    flags: u16,
}
// TODO: Misaligned reads from packed struct in Debug could cause problems?
impl fmt::Debug for IntOverride {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            write!(
                f,
                "IntOverride src: {} mapped to: {}",
                self.source, self.mapped_to
            )
        }
    }
}

/// Different states for APICs to be in
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ApicState {
    /// The core has checked in with the kernel and is actively running
    Online = 1,
    /// The core has been launched by the kernel, but has not yet registered
    /// with the kernel
    Launched = 2,
    /// The core is present but has not yet been launched
    Offline = 3,
    /// This APIC ID does not exist
    None = 4,
    /// This APIC ID has disabled interrupts and halted forever
    Halted = 5,
}

impl From<u8> for ApicState {
    /// Convert a raw `u8` into an `ApicState`
    fn from(val: u8) -> ApicState {
        match val {
            1 => ApicState::Online,
            2 => ApicState::Launched,
            3 => ApicState::Offline,
            4 => ApicState::None,
            5 => ApicState::Halted,
            _ => panic!("Invalid ApicState from `u8`"),
        }
    }
}

/// Maximum number of cores allowed on the system
pub const MAX_CORES: usize = 1024;

pub struct Acpi {}

impl Acpi {
    pub fn new() -> Self {
        Acpi {}
    }

    unsafe fn parse_header(
        &self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        addr: PhysAddr,
    ) -> (Header, PhysAddr, usize) {
        let head: Header = map_and_read_phys(mapper, frame_allocator, addr);

        let table_len = head
            .length
            .checked_sub(size_of::<Header>() as u32)
            .expect("Integer underflow on table");

        // Checksum the table
        let mut sum: u8 = 0;
        for i in addr.as_u64()..addr.as_u64() + head.length as u64 {
            let byte: u8 = map_and_read_phys(mapper, frame_allocator, PhysAddr::new(i));
            sum = sum.wrapping_add(byte);
        }

        if sum != 0 {
            panic!("Checksum invalid: {}", sum);
        }

        (head, addr + size_of::<Header>() as u64, table_len as usize)
    }

    unsafe fn search_rsdp(
        &self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Option<Rsdp> {
        // Map 0x40e and read ebda
        let ebda_ptr: u16 = map_and_read_phys(mapper, frame_allocator, PhysAddr::new(0x40e));

        // Compute the regions we need to scan for the RSDP
        let regions = [
            // First 1 KiB of the EBDA
            (ebda_ptr as u64, ebda_ptr as u64 + 1024 - 1),
            // From 0xe0000 to 0xfffff
            (0xe0000, 0xfffff),
        ];

        // Holds the RSDP structure if found
        for &(start, end) in &regions {
            let start = x86_64::addr::align_up(start, 16);
            for addr in (start..=end).step_by(16) {
                // Compute the end address of RSDP structure
                let struct_end = start + size_of::<Rsdp>() as u64 - 1;

                // Break out of the scan if we are out of bounds of this region
                if struct_end > end {
                    break;
                }

                let table: Rsdp = map_and_read_phys(mapper, frame_allocator, PhysAddr::new(addr));
                if &table.signature != b"RSD PTR " {
                    continue;
                }

                // Checksum table
                let table_bytes: &[u8; size_of::<Rsdp>()] = core::intrinsics::transmute(&table);
                let sum = table_bytes
                    .iter()
                    .fold(0_u8, |acc, &elem| acc.wrapping_add(elem));
                if sum != 0 {
                    log::warn!("Checksum is incorrect: {}", sum);
                    continue;
                }

                // Checksum the extended RSDP if needed
                if table.revision > 0 {
                    // Read the tables bytes so we can checksum it
                    let extended_rsdp: RsdpExtended =
                        map_and_read_phys(mapper, frame_allocator, PhysAddr::new(addr));
                    let extended_bytes: &[u8; core::mem::size_of::<RsdpExtended>()] =
                        core::intrinsics::transmute(&extended_rsdp);

                    // Checksum the table
                    let sum = extended_bytes
                        .iter()
                        .fold(0_u8, |acc, &x| acc.wrapping_add(x));
                    if sum != 0 {
                        continue;
                    }
                }

                return Some(table);
            }
        }
        return None;
    }

    pub unsafe fn init(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        // Search for RSDP pointer
        let rsdp = self
            .search_rsdp(mapper, frame_allocator)
            .expect("Failed to find RSDP for ACPI");

        // Parse out the RSDT
        let (rsdt, rsdt_payload, rsdt_size) = self.parse_header(
            mapper,
            frame_allocator,
            PhysAddr::new(rsdp.rsdt_addr.into()),
        );

        // Check the signature of rsdt
        if &rsdt.signature != b"RSDT" {
            panic!("RSDT signature mismatch");
        }
        if rsdt_size % size_of::<u32>() != 0 {
            panic!("Invalid table size for RSDT");
        }
        let rsdt_entries = rsdt_size / size_of::<u32>();

        // Set up the structures we're interested as parsing out as `None` as some
        // of them may or may not be present.
        let mut apics = None;
        let mut ioapics = None;
        let mut int_overrides = None;
        let mut apic_domains = None;
        let mut memory_domains = None;

        for entry in 0..rsdt_entries {
            // Get the physical address of the RSDP table entry
            let entry_paddr = rsdt_payload + entry * size_of::<u32>();

            let table_ptr: u32 = map_and_read_phys(mapper, frame_allocator, entry_paddr);
            let signature: [u8; 4] =
                map_and_read_phys(mapper, frame_allocator, PhysAddr::new(table_ptr as u64));

            // Parse MADT
            if &signature == b"APIC" {
                if !apics.is_none() {
                    panic!("Multiple SRAT ACPI table entrie");
                }

                let result =
                    self.parse_madt(mapper, frame_allocator, PhysAddr::new(table_ptr as u64));
                apics = Some(result.0);
                ioapics = Some(result.1);
                int_overrides = Some(result.2);
                // log::set_max_level(LevelFilter::Info);

            // Parse SRAT
            } else if &signature == b"SRAT" {
                log::info!("FOUND SRAT STRUCTURE");
                if !apic_domains.is_none() || !memory_domains.is_none() {
                    panic!("Multiple SRAT entries");
                }
                let (ad, md) =
                    self.parse_srat(mapper, frame_allocator, PhysAddr::new(table_ptr as u64));
                apic_domains = Some(ad);
                memory_domains = Some(md);
            }
        } // enf for rsdt_entries

        log::info!("apics: {:?}", apics);
        log::info!("ioapcis: {:?}", ioapics);
        log::info!("int_overrides: {:?}", int_overrides);
        log::info!("apic domains: {:?}", apic_domains);
        log::info!("memory domains: {:?}", memory_domains);
    } // end fn init

    /// Parse the MADT out of the ACPI tables
    /// Returns a vector of all usable APIC IDs
    unsafe fn parse_madt(
        &self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        ptr: PhysAddr,
    ) -> (Vec<LocalApic>, Vec<IoApic>, Vec<IntOverride>) {
        let (_header, payload, size) = self.parse_header(mapper, frame_allocator, ptr);

        // Skip the local interrupt controller address and the flags to get the
        // physical address of the ICS
        let mut ics = payload + 4u64 + 4u64;
        let end = payload + size as u64;

        // Create a new structure to hold the APICs that are usable
        let mut lapics = Vec::new();
        let mut ioapcis = Vec::new();
        let mut int_overrides = Vec::new();

        loop {
            /// Processor is ready for use
            const APIC_ENABLED: u32 = 1 << 0;

            /// Processor may be enabled at runtime (IFF ENABLED is zero),
            /// otherwise this bit is RAZ
            const APIC_ONLINE_CAPABLE: u32 = 1 << 1;

            // Make sure there's room for the type and the length
            if ics + 2_u64 > end {
                break;
            }

            // Parse out the type and the length of the ICS entry
            let typ: u8 = map_and_read_phys(mapper, frame_allocator, ics + 0_u64);
            let len: u8 = map_and_read_phys(mapper, frame_allocator, ics + 1_u64);

            // Make sure there's room for this structure
            if ics + len as u64 > end {
                break;
            }

            if len < 2 {
                panic!("Bad length for MADT ICS entry");
            }

            match typ {
                // LAPIC entry
                0 => {
                    if len != 8 {
                        panic!("Invalid LAPIC ICS entry");
                    }
                    // Read the struct
                    let lapic: LocalApic = map_and_read_phys(mapper, frame_allocator, ics);

                    // If the processor is enabled, or can be enabled, log it as
                    // a valid APIC
                    if (lapic.flags & APIC_ENABLED) != 0 || (lapic.flags & APIC_ONLINE_CAPABLE) != 0
                    {
                        lapics.push(lapic);
                    }
                }
                // I/O APIC
                1 => {
                    if len != 12 {
                        panic!("Invalid I/O apic entry");
                    }

                    let ioapic: IoApic = map_and_read_phys(mapper, frame_allocator, ics);
                    ioapcis.push(ioapic);
                }
                // Interrupt overrides
                2 => {
                    if len != 10 {
                        panic!("Invalid interrupt override entry");
                    }

                    let int_override: IntOverride = map_and_read_phys(mapper, frame_allocator, ics);

                    // Filter out identity mappings
                    if int_override.source as u32 != int_override.mapped_to {
                        int_overrides.push(int_override);
                    }
                }
                // x2apic entry
                9 => {
                    if len != 16 {
                        panic!("Invalid x2apic ICS entry");
                    }

                    // Read the struct
                    let lapic: LocalApic = map_and_read_phys(mapper, frame_allocator, ics);

                    // If the processor is enabled, or can be enabled, log it as
                    // a valid APIC
                    if (lapic.flags & APIC_ENABLED) != 0 || (lapic.flags & APIC_ONLINE_CAPABLE) != 0
                    {
                        lapics.push(lapic);
                    }
                }
                _ => {
                    // Don't really care for now
                }
            }
            // Go to the next ICS entry
            ics = ics + len as u64;
        } // end loop

        return (lapics, ioapcis, int_overrides);
    } // end function

    unsafe fn parse_srat(
        &self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        ptr: PhysAddr,
    ) -> (BTreeMap<u32, u32>, BTreeMap<u32, RangeSet>) {
        // Parse the SRAT header
        let (_header, payload, size) = self.parse_header(mapper, frame_allocator, ptr);

        // Skip the 12 reserved bytes to get to the SRA structure
        let mut sra = payload + 4_u64 + 8_u64;
        let end = payload + size as u64;

        // Mapping of proximity domains to their memory ranges
        let mut memory_affinities: BTreeMap<u32, RangeSet> = BTreeMap::new();

        // Mapping of APICs to their proximity domains
        let mut apic_affinities: BTreeMap<u32, u32> = BTreeMap::new();

        loop {
            /// The entry is enabled and present. Some BIOSes may staticially
            /// allocate these table regions, thus the flags indicate whether the
            /// entry is actually present or not.
            const FLAGS_ENABLED: u32 = 1 << 0;

            // Make sure there's room for the type and the length
            if sra + 2_u64 > end {
                break;
            }

            // Parse out the type and the length of the ICS entry
            let typ: u8 = map_and_read_phys(mapper, frame_allocator, sra + 0_u64);
            let len: u8 = map_and_read_phys(mapper, frame_allocator, sra + 1_u64);

            // Make sure there's room for this structure
            if sra + len as u64 > end {
                break;
            }
            if len < 2 {
                panic!("Bad length for SRAT SRA entry");
            }

            match typ {
                0 => {
                    // Local APIC
                    if len != 16 {
                        panic!("Invalid APIC SRA entry");
                    }

                    // Extract the fields we care about
                    let domain_low: u8 = map_and_read_phys(mapper, frame_allocator, sra + 2_u64);
                    let domain_high: [u8; 3] =
                        map_and_read_phys(mapper, frame_allocator, sra + 9_u64);
                    let apic_id: u8 = map_and_read_phys(mapper, frame_allocator, sra + 3_u64);
                    let flags: u32 = map_and_read_phys(mapper, frame_allocator, sra + 4_u64);

                    // Parse the domain low and high parts into an actual `u32`
                    let domain = [domain_low, domain_high[0], domain_high[1], domain_high[2]];
                    let domain = u32::from_le_bytes(domain);

                    // Log the affinity record
                    if (flags & FLAGS_ENABLED) != 0 {
                        if !apic_affinities.insert(apic_id as u32, domain).is_none() {
                            panic!("Duplicate LAPIC affinity domain");
                        }
                    }
                }
                1 => {
                    // Memory affinity
                    if len != 40 {
                        panic!("Invalid memory affinity SRA entry");
                    }

                    // Extract the fields we care about
                    let domain: u32 = map_and_read_phys(mapper, frame_allocator, sra + 2_u64);
                    let base: PhysAddr = map_and_read_phys(mapper, frame_allocator, sra + 8_u64);
                    let size: u64 = map_and_read_phys(mapper, frame_allocator, sra + 16_u64);
                    let flags: u32 = map_and_read_phys(mapper, frame_allocator, sra + 28_u64);

                    // Only process ranges with a non-zero size (observed on
                    // polar and grizzly that some ranges were 0 size)
                    if size > 0 {
                        // Log the affinity record
                        if (flags & FLAGS_ENABLED) != 0 {
                            memory_affinities
                                .entry(domain)
                                .or_insert_with(|| RangeSet::new())
                                .insert(Range {
                                    start: base.as_u64(),
                                    end: base
                                        .as_u64()
                                        .checked_add(size.checked_sub(1).unwrap())
                                        .unwrap(),
                                });
                        }
                    }
                }
                2 => {
                    // Local x2apic
                    if len != 24 {
                        panic!("Invalid x2apic SRA entry");
                    }

                    // Extract the fields we care about
                    let domain: u32 = map_and_read_phys(mapper, frame_allocator, sra + 4_u64);
                    let apic_id: u32 = map_and_read_phys(mapper, frame_allocator, sra + 8_u64);
                    let flags: u32 = map_and_read_phys(mapper, frame_allocator, sra + 12_u64);

                    // Log the affinity record
                    if (flags & FLAGS_ENABLED) != 0 {
                        assert!(
                            apic_affinities.insert(apic_id, domain).is_none(),
                            "Duplicate APIC affinity domain"
                        );
                    }
                }
                _ => {}
            } // end match

            sra = sra + len as u64;
        } // end loop
        (apic_affinities, memory_affinities)
    } // end func
} // end impl Apic
