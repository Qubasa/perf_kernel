#![allow(unused_imports)]

use crate::memory::{id_map_nocache, map_and_read_phys};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::{read_volatile, write_volatile};
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

        log::info!("header addr: {:#x}", addr);
        (head, addr + size_of::<Header>() as u64, table_len as usize)
    }

    pub unsafe fn init(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
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
        let mut rsdp: Option<Rsdp> = None;
        'rsdp_search: for &(start, end) in &regions {
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
                let table_bytes: &[u8; core::mem::size_of::<Rsdp>()] =
                    core::intrinsics::transmute(&table);
                let sum = table_bytes
                    .iter()
                    .fold(0_u8, |acc, &elem| acc.wrapping_add(elem));
                if sum != 0 {
                    log::warn!("Checksum is incorrect: {}", sum);
                    continue;
                }

                log::info!("ACPI revision: {}", table.revision + 1);

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

                rsdp = Some(table);
                break 'rsdp_search;
            }
        }

        let rsdp = rsdp.expect("Failed to find RSDP for ACPI");

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

                log::info!("FOUND APIC STRUCTURE");
                apics =
                    Some(self.parse_madt(mapper, frame_allocator, PhysAddr::new(table_ptr as u64)));

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
    ) -> Vec<u32> {
        let (_header, payload, size) = self.parse_header(mapper, frame_allocator, ptr);

        // Skip the local interrupt controller address and the flags to get the
        // physical address of the ICS
        let mut ics = payload + 4u64 + 4u64;
        let end = payload + size as u64;

        // Create a new structure to hold the APICs that are usable
        let mut apics = Vec::new();

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
                0 => {
                    // LAPIC entry
                    if len != 8 {
                        panic!("Invalid LAPIC ICS entry");
                    }

                    // Read the APIC ID
                    let apic_id: u8 = map_and_read_phys(mapper, frame_allocator, ics + 3_u64);
                    let flags: u32 = map_and_read_phys(mapper, frame_allocator, ics + 4_u64);

                    // If the processor is enabled, or can be enabled, log it as
                    // a valid APIC
                    if (flags & APIC_ENABLED) != 0 || (flags & APIC_ONLINE_CAPABLE) != 0 {
                        apics.push(apic_id as u32);
                    }
                }
                9 => {
                    // x2apic entry
                    if len != 16 {
                        panic!("Invalid x2apic ICS entry");
                    }

                    // Read the APIC ID
                    let apic_id: u32 = map_and_read_phys(mapper, frame_allocator, ics + 4u64);
                    let flags: u32 = map_and_read_phys(mapper, frame_allocator, ics + 8u64);

                    // If the processor is enabled, or can be enabled, log it as
                    // a valid APIC
                    if (flags & APIC_ENABLED) != 0 || (flags & APIC_ONLINE_CAPABLE) != 0 {
                        apics.push(apic_id);
                    }
                }
                _ => {
                    // Don't really care for now
                }
            }
            // Go to the next ICS entry
            ics = ics + len as u64;
        } // end loop

        return apics;
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
