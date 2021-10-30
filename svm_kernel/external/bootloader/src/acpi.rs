use crate::acpi_regs::*;

use crate::mmu::read_phys;
use core::convert::TryInto;
use core::mem::size_of;
use x86::PhysAddr;

unsafe fn parse_header(addr: PhysAddr) -> (Header, PhysAddr, usize) {
    let head: Header = read_phys(addr);

    let table_len = head
        .length
        .checked_sub(size_of::<Header>() as u32)
        .expect("Integer underflow on table");

    // Checksum the table
    let mut sum: u8 = 0;
    for i in addr.as_u32()..addr.as_u32() + head.length as u32 {
        let byte: u8 = read_phys(PhysAddr::new(i));
        sum = sum.wrapping_add(byte);
    }

    if sum != 0 {
        panic!("Checksum invalid: {}", sum);
    }

    (head, addr + size_of::<Header>() as u32, table_len as usize)
}

unsafe fn search_rsdp() -> Option<Rsdp> {
    // Map 0x40e and read ebda
    let ebda_ptr: u16 = read_phys(PhysAddr::new(0x40e));

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
    None
}

pub unsafe fn init(lapic_arr: &mut [LocalApic]) {
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

        let table_ptr: u32 = read_phys(entry_paddr);
        let signature: [u8; 4] = read_phys(PhysAddr::new(table_ptr));

        // Parse MADT
        if &signature == b"APIC" {
            parse_madt(PhysAddr::new(table_ptr), lapic_arr);
        }
    } // enf for rsdt_entries
} // end fn init

/// Parse the MADT out of the ACPI tables
/// Returns a vector of all usable APIC IDs
unsafe fn parse_madt(ptr: PhysAddr, lapic_arr: &mut [LocalApic]) {
    let (_header, payload, size) = parse_header(ptr);

    // Skip the local interrupt controller address and the flags to get the
    // physical address of the ICS
    let mut ics = payload + 4u32 + 4u32;
    let end = payload + size as u32;

    // Create a new structure to hold the APICs that are usable
    let mut lapics_iter = lapic_arr.iter_mut();

    loop {
        /// Processor is ready for use
        const APIC_ENABLED: u32 = 1 << 0;

        /// Processor may be enabled at runtime (IFF ENABLED is zero),
        /// otherwise this bit is RAZ
        const APIC_ONLINE_CAPABLE: u32 = 1 << 1;

        // Make sure there's room for the type and the length
        if ics + 2_u32 > end {
            break;
        }

        // Parse out the type and the length of the ICS entry
        let typ: u8 = read_phys(ics);
        let len: u8 = read_phys(ics + 1_u32);

        // Make sure there's room for this structure
        if ics + len as u32 > end {
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
                if (lapic.flags & APIC_ENABLED) != 0 || (lapic.flags & APIC_ONLINE_CAPABLE) != 0 {
                    *lapics_iter.next().expect("lapic buffer is too small") = lapic;
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
                if (lapic.flags & APIC_ENABLED) != 0 || (lapic.flags & APIC_ONLINE_CAPABLE) != 0 {
                    *lapics_iter.next().expect("lapic buffer is too small") = lapic;
                }
            }
            _ => {
                // Don't really care for now
            }
        }
        // Go to the next ICS entry
        ics += len as u32;
    } // end loop
} // end function
