use crate::acpi_regs::*;

use crate::mmu::read_phys;
use core::convert::TryInto;
use core::mem::size_of;
use x86::PhysAddr;

fn parse_header(addr: PhysAddr) -> (Header, PhysAddr, usize) {
    let head: Header = unsafe { read_phys(addr) };

    let table_len = head
        .length
        .checked_sub(size_of::<Header>() as u32)
        .expect("Integer underflow on table");

    // Checksum the table
    let mut sum: u8 = 0;
    for i in addr.as_u32()..addr.as_u32() + head.length as u32 {
        let byte: u8 = unsafe { read_phys(PhysAddr::new(i)) };
        sum = sum.wrapping_add(byte);
    }

    if sum != 0 {
        panic!("Checksum invalid: {}", sum);
    }

    (head, addr + size_of::<Header>() as u32, table_len as usize)
}

fn search_rsdp() -> Option<Rsdp> {
    // Map 0x40e and read ebda
    let ebda_ptr: u16 = unsafe { read_phys(PhysAddr::new(0x40e)) };

    // Compute the regions we need to scan for the RSDP
    let regions = [
        // First 1 KiB of the EBDA
        (ebda_ptr as u32, ebda_ptr as u32 + 1024 - 1),
        // From 0xe0000 to 0xfffff
        (0xe0000, 0xfffff),
    ];

    // Holds the RSDP structure if found
    for &(start, end) in &regions {
        let start = x86::addr::align_up(start, 16);
        for addr in (start..=end).step_by(16) {
            // Compute the end address of RSDP structure
            let struct_end = start + size_of::<Rsdp>() as u32 - 1;

            // Break out of the scan if we are out of bounds of this region
            if struct_end > end {
                break;
            }

            let table: Rsdp = unsafe { read_phys(PhysAddr::new(addr)) };
            if &table.signature != b"RSD PTR " {
                continue;
            }

            // Checksum table
            let table_bytes: &[u8; size_of::<Rsdp>()] =
                unsafe { core::intrinsics::transmute(&table) };
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
                let extended_rsdp: RsdpExtended = unsafe { read_phys(PhysAddr::new(addr)) };
                let extended_bytes: &[u8; core::mem::size_of::<RsdpExtended>()] =
                    unsafe { core::intrinsics::transmute(&extended_rsdp) };

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
    None
}

#[derive(Debug, Clone)]
pub struct LapicIter {
    current: PhysAddr,
    end: PhysAddr,
}

impl LapicIter {
    pub fn new() -> Option<Self> {
        // Search for RSDP pointer
        let rsdp = search_rsdp().expect("Failed to find RSDP for ACPI");

        // Parse out the RSDT
        let (rsdt, rsdt_payload, rsdt_size) = parse_header(PhysAddr::new(rsdp.rsdt_addr));

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
            let entry_paddr = rsdt_payload + (entry * size_of::<u32>()).try_into().unwrap();

            let table_ptr: u32 = unsafe { read_phys(entry_paddr) };
            let signature: [u8; 4] = unsafe { read_phys(PhysAddr::new(table_ptr)) };

            // Parse MADT
            if &signature == b"APIC" {
                let (_header, payload, size) = parse_header(PhysAddr::new(table_ptr));

                // Skip the local interrupt controller address and the flags to get the
                // physical address of the ICS
                let start = payload + 4u32 + 4u32;

                return Some(Self {
                    current: start,
                    end: payload + size as u32,
                });
            }
        } // enf for rsdt_entries
        None
    } // end fn init
}

impl Iterator for LapicIter {
    type Item = LocalApic;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            /// Processor is ready for use
            const APIC_ENABLED: u32 = 1 << 0;

            /// Processor may be enabled at runtime (IFF ENABLED is zero),
            /// otherwise this bit is RAZ
            const APIC_ONLINE_CAPABLE: u32 = 1 << 1;

            // Make sure there's room for the type and the length
            if self.current + 2_u32 > self.end {
                break;
            }

            // Parse out the type and the length of the ICS entry
            let typ: u8 = unsafe { read_phys(self.current) };
            let len: u8 = unsafe { read_phys(self.current + 1_u32) };

            // Make sure there's room for this structure
            if self.current + len as u32 > self.end {
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
                    let lapic: LocalApic = unsafe { read_phys(self.current) };
                    // Go to the next ICS entry
                    self.current += len as u32;
                    // If the processor is enabled, or can be enabled, log it as
                    // a valid APIC
                    if (lapic.flags & APIC_ENABLED) != 0 || (lapic.flags & APIC_ONLINE_CAPABLE) != 0
                    {
                        return Some(lapic);
                    }
                }
                // x2apic entry
                9 => {
                    if len != 16 {
                        panic!("Invalid x2apic ICS entry");
                    }

                    // Read the struct
                    let lapic: LocalApic = unsafe { read_phys(self.current) };
                    // Go to the next ICS entry
                    self.current += len as u32;
                    // If the processor is enabled, or can be enabled, log it as
                    // a valid APIC
                    if (lapic.flags & APIC_ENABLED) != 0 || (lapic.flags & APIC_ONLINE_CAPABLE) != 0
                    {
                        return Some(lapic);
                    }
                }
                _ => {
                    // Don't really care for now
                    // Go to the next ICS entry
                    self.current += len as u32;
                }
            }
        }
        None
    }
}
