#![allow(unused_imports)]

use crate::memory::{id_map_nocache, map_and_read_phys};
use core::mem::size_of;
use core::ptr::{read_volatile, write_volatile};
use modular_bitfield::prelude::*;
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
        let head: Header = map_and_read_phys(mapper, frame_allocator, addr.as_u64());

        let table_len = head
            .length
            .checked_sub(size_of::<Header>() as u32)
            .expect("Integer underflow on table");

        // Checksum the table
        let mut sum: u8 = 0;
        for i in addr.as_u64()..addr.as_u64() + head.length as u64 {
            let byte: u8 = map_and_read_phys(mapper, frame_allocator, i);
            sum = sum.wrapping_add(byte);
        }


        if sum != 0 {
            panic!("Checksum invalid: {}", sum);
        }

        (head, addr + size_of::<Header>() as u64, table_len as usize)
    }

    pub unsafe fn init(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        // Map 0x40e and read ebda
        let ebda_ptr: u16 = map_and_read_phys(mapper, frame_allocator, 0x40e);

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

                let table: Rsdp = map_and_read_phys(mapper, frame_allocator, addr);
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
                        map_and_read_phys(mapper, frame_allocator, addr);
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
    }
}
