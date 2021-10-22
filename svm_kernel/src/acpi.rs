use crate::acpi_regs::*;

use crate::memory::read_phys;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use rangeset::{Range, RangeSet};
use x86_64::PhysAddr;
static mut ACPI_TABLES: Option<Acpi> = None;

pub unsafe fn init() -> &'static Acpi {
    if ACPI_TABLES.is_none() {
        let mut acpi = Acpi::new();
        acpi.init();
        ACPI_TABLES = Some(acpi);
    }
    ACPI_TABLES.as_ref().unwrap()
}

pub struct Acpi {
    pub apics: Option<Vec<LocalApic>>,
    pub ioapics: Option<Vec<IoApic>>,
    pub int_overrides: Option<Vec<IntOverride>>,
    pub nmis: Option<Vec<NonMaskableInts>>,
    pub apic_domains: Option<BTreeMap<u32, u32>>,
    pub memory_domains: Option<BTreeMap<u32, RangeSet>>,
    pub mask_pics: bool,
}

impl fmt::Debug for Acpi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Acpi tables:\n").unwrap();
        write!(f, "apics: {:?}\n", self.apics).unwrap();
        write!(f, "ioapics: {:?}\n", self.ioapics).unwrap();
        write!(f, "int overrides: {:?}\n", self.int_overrides).unwrap();
        write!(f, "non maskable ints: {:?}\n", self.nmis).unwrap();
        write!(f, "apic domains: {:?}\n", self.apic_domains).unwrap();
        write!(f, "memory domains: {:?}\n", self.memory_domains).unwrap();
        write!(f, "mask pics: {:?}\n", self.mask_pics)
    }
}

impl Acpi {
    pub const fn new() -> Self {
        Acpi {
            mask_pics: false,
            apics: None,
            ioapics: None,
            int_overrides: None,
            apic_domains: None,
            nmis: None,
            memory_domains: None,
        }
    }

    unsafe fn parse_header(&self, addr: PhysAddr) -> (Header, PhysAddr, usize) {
        let head: Header = read_phys(addr);

        let table_len = head
            .length
            .checked_sub(size_of::<Header>() as u32)
            .expect("Integer underflow on table");

        // Checksum the table
        let mut sum: u8 = 0;
        for i in addr.as_u64()..addr.as_u64() + head.length as u64 {
            let byte: u8 = read_phys(PhysAddr::new(i));
            sum = sum.wrapping_add(byte);
        }

        if sum != 0 {
            panic!("Checksum invalid: {}", sum);
        }

        (head, addr + size_of::<Header>() as u64, table_len as usize)
    }

    unsafe fn search_rsdp(&self) -> Option<Rsdp> {
        // Map 0x40e and read ebda
        let ebda_ptr: u16 = read_phys(PhysAddr::new(0x40e));

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

                let table: Rsdp = read_phys(PhysAddr::new(addr));
                if &table.signature != b"RSD PTR " {
                    continue;
                }

                // Checksum table
                let table_bytes: &[u8; size_of::<Rsdp>()] = core::intrinsics::transmute(&table);
                let sum = table_bytes
                    .iter()
                    .fold(0_u8, |acc, &elem| acc.wrapping_add(elem));
                if sum != 0 {
                    log::warn!("Rsdp checksum is incorrect: {}", sum);
                    continue;
                }

                // Checksum the extended RSDP if needed
                if table.revision > 0 {
                    // Read the tables bytes so we can checksum it
                    let extended_rsdp: RsdpExtended = read_phys(PhysAddr::new(addr));
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

    pub unsafe fn init(&mut self) {
        // Search for RSDP pointer
        let rsdp = self.search_rsdp().expect("Failed to find RSDP for ACPI");

        // Parse out the RSDT
        let (rsdt, rsdt_payload, rsdt_size) =
            self.parse_header(PhysAddr::new(rsdp.rsdt_addr.into()));

        // Check the signature of rsdt
        if &rsdt.signature != b"RSDT" {
            panic!("RSDT signature mismatch");
        }
        if rsdt_size % size_of::<u32>() != 0 {
            panic!("Invalid table size for RSDT");
        }
        let rsdt_entries = rsdt_size / size_of::<u32>();

        for entry in 0..rsdt_entries {
            // Get the physical address of the RSDP table entry
            let entry_paddr = rsdt_payload + entry * size_of::<u32>();

            let table_ptr: u32 = read_phys(entry_paddr);
            let signature: [u8; 4] = read_phys(PhysAddr::new(table_ptr as u64));

            // Parse MADT
            if &signature == b"APIC" {
                if !self.apics.is_none() {
                    panic!("Multiple SRAT ACPI table entrie");
                }

                let result = self.parse_madt(PhysAddr::new(table_ptr as u64));

                if result.0.len() != 0 {
                    self.apics = Some(result.0);
                }
                if result.1.len() != 0 {
                    self.ioapics = Some(result.1);
                }

                if result.2.len() != 0 {
                    self.int_overrides = Some(result.2);
                }

                if result.3.len() != 0 {
                    self.nmis = Some(result.3);
                }

                self.mask_pics = result.4;

            // Parse SRAT
            } else if &signature == b"SRAT" {
                log::info!("FOUND SRAT STRUCTURE");
                if !self.apic_domains.is_none() || !self.memory_domains.is_none() {
                    panic!("Multiple SRAT entries");
                }
                let (ad, md) = self.parse_srat(PhysAddr::new(table_ptr as u64));
                self.apic_domains = Some(ad);
                self.memory_domains = Some(md);
            }
        } // enf for rsdt_entries

        log::info!("{:?}", self);
    } // end fn init

    /// Parse the MADT out of the ACPI tables
    /// Returns a vector of all usable APIC IDs
    unsafe fn parse_madt(
        &self,
        ptr: PhysAddr,
    ) -> (
        Vec<LocalApic>,
        Vec<IoApic>,
        Vec<IntOverride>,
        Vec<NonMaskableInts>,
        bool,
    ) {
        let (_header, payload, size) = self.parse_header(ptr);

        let flags: u32 = read_phys(ptr + 4_u64);

        // If the first bit is set the spec says we have to mask the pic interrupts
        let mask_pics: bool = flags & 1 == 1;

        // Skip the local interrupt controller address and the flags to get the
        // physical address of the ICS
        let mut ics = payload + 4u64 + 4u64;
        let end = payload + size as u64;

        // Create a new structure to hold the APICs that are usable
        let mut lapics = Vec::new();
        let mut ioapcis = Vec::new();
        let mut int_overrides = Vec::new();
        let mut nmis = Vec::new();

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
            let typ: u8 = read_phys(ics + 0_u64);
            let len: u8 = read_phys(ics + 1_u64);

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
                    let lapic: LocalApic = read_phys(ics);

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

                    let ioapic: IoApic = read_phys(ics);
                    ioapcis.push(ioapic);
                }
                // NonMaskableInts
                3 => {
                    if len != 8 {
                        panic!("Invalid NonMaskableInts entry");
                    }
                    let nmi: NonMaskableInts = read_phys(ics);
                    nmis.push(nmi);
                }
                // Interrupt overrides
                2 => {
                    if len != 10 {
                        panic!("Invalid interrupt override entry");
                    }

                    let int_override: IntOverride = read_phys(ics);

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
                    let lapic: LocalApic = read_phys(ics);

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

        return (lapics, ioapcis, int_overrides, nmis, mask_pics);
    } // end function

    unsafe fn parse_srat(&self, ptr: PhysAddr) -> (BTreeMap<u32, u32>, BTreeMap<u32, RangeSet>) {
        // Parse the SRAT header
        let (_header, payload, size) = self.parse_header(ptr);

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
            let typ: u8 = read_phys(sra + 0_u64);
            let len: u8 = read_phys(sra + 1_u64);

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
                    let domain_low: u8 = read_phys(sra + 2_u64);
                    let domain_high: [u8; 3] = read_phys(sra + 9_u64);
                    let apic_id: u8 = read_phys(sra + 3_u64);
                    let flags: u32 = read_phys(sra + 4_u64);

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
                    let domain: u32 = read_phys(sra + 2_u64);
                    let base: PhysAddr = read_phys(sra + 8_u64);
                    let size: u64 = read_phys(sra + 16_u64);
                    let flags: u32 = read_phys(sra + 28_u64);

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
                    let domain: u32 = read_phys(sra + 4_u64);
                    let apic_id: u32 = read_phys(sra + 8_u64);
                    let flags: u32 = read_phys(sra + 12_u64);

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
