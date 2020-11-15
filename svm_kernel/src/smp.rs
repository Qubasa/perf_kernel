#![allow(unused_imports)]
use crate::memory::map_and_read_phys;
use alloc::vec::Vec;
use core::mem::size_of;
use modular_bitfield::prelude::*;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct MpFloatingPoint {
    signature: [u8; 4],
    mp_addr: u32,
    length: u8,
    revision: u8,
    checksum: u8,
    config_type: u8,
    imcrp: u8,
    res0: u16,
    res1: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Header {
    signature: [u8; 4],
    length: u16,
    revision: u8,
    checksum: u8,
    oemid: [u8; 8],
    product_id: [u8; 12],
    oem_table_ptr: u32,
    oem_table_size: u16,
    entry_count: u16,
    lapic_ptr: u32,
    extended_table_len: u16,
    extended_checksum: u8,
    res0: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct IoApic {
    typ: u8,
    id: u8,
    version: u8,
    enabled: u8,
    address: u32,
}

pub struct Smp {}

impl Smp {
    pub fn new() -> Self {
        Smp {}
    }

    unsafe fn search_mpfloat(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Option<MpFloatingPoint> {
        // Map 0x40e and read ebda
        let ebda_ptr: u16 = map_and_read_phys(mapper, frame_allocator, PhysAddr::new(0x40e));

        // Compute the regions we need to scan for the RSDP
        let regions = [
            // First 1 KiB of the EBDA
            (ebda_ptr as u64, ebda_ptr as u64 + 1024 - 1),
            // From 0xe0000 to 0xfffff
            (0xe0000, 0xfffff),
        ];

        for &(start, end) in &regions {
            let start = x86_64::addr::align_up(start, 16);
            for addr in (start..=end).step_by(16) {
                // Compute the end address of MP float structure
                let struct_end = start + size_of::<MpFloatingPoint>() as u64 - 1;

                // Break out of the scan if we are out of bounds of this region
                if struct_end > end {
                    break;
                }

                let table: MpFloatingPoint =
                    map_and_read_phys(mapper, frame_allocator, PhysAddr::new(addr));
                if &table.signature != b"_MP_" {
                    continue;
                }

                // Checksum table
                let table_bytes: &[u8; size_of::<MpFloatingPoint>()] =
                    core::intrinsics::transmute(&table);
                let sum = table_bytes
                    .iter()
                    .fold(0_u8, |acc, &elem| acc.wrapping_add(elem));
                if sum != 0 {
                    log::warn!("Checksum is incorrect: {}", sum);
                    continue;
                }

                if table.mp_addr == 0 {
                    panic!("Mp config table does not exist");
                }
                return Some(table);
            }
        }
        return None;
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
            .checked_sub(size_of::<Header>() as u16)
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

        // Add extended entries to length (if not present is zero)
        let table_len = table_len + head.extended_table_len;

        log::info!("OEM ID: {}", core::str::from_utf8(&head.oemid).unwrap());

        match head.revision {
            1 => {
                log::warn!("MP revision: 1.1");
                log::warn!("Never tested on this revision");
            }
            4 => {
                log::info!("MP revision: 1.4");
            }
            _ => {
                panic!("Uknown revision");
            }
        }

        (head, addr + size_of::<Header>() as u64, table_len as usize)
    }

    pub unsafe fn init(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        // search for mp float table ptr
        let mp_float = self
            .search_mpfloat(mapper, frame_allocator)
            .expect("Could not find MpFloatingPoint structure");

        // parse header of mp float table
        let (header, table_ptr, _table_len) = self.parse_header(
            mapper,
            frame_allocator,
            PhysAddr::new(mp_float.mp_addr.into()),
        );

        // Parse entries in mp float table
        let mut cur_entry_ptr: PhysAddr = table_ptr;
        let mut counter_arr = [0; 5];
        let mut ioapics = Vec::new();
        for _ in 0..header.entry_count {
            let typ: u8 = map_and_read_phys(mapper, frame_allocator, cur_entry_ptr);

            counter_arr[typ as usize] += 1;

            match typ {
                0 => {
                    cur_entry_ptr += 20_u64;
                }
                1 => {
                    cur_entry_ptr += 8_u64;
                }
                2 => {
                    let ioapic: IoApic = map_and_read_phys(mapper, frame_allocator, cur_entry_ptr);
                    ioapics.push(ioapic);
                    cur_entry_ptr += 8_u64;
                }
                3 => {
                    cur_entry_ptr += 8_u64;
                }
                4 => {
                    cur_entry_ptr += 8_u64;
                }
                _ => {
                    panic!("Entry type {} does not exist", typ);
                }
            }
        }

        log::info!("Num of: \n Processors: {}\n Buses: {}\n I/O APICs: {}\n I/O Interrupt Assignemts: {}\n Local Interrupt Assignments: {} ", counter_arr[0], counter_arr[1], counter_arr[2], counter_arr[3], counter_arr[4]);
        for io in &ioapics {
            log::info!("IO Apic addr: {:#x}, enabled: {}", io.address, io.enabled);
        }

        //TODO: The imcrp flag should also be available in the acpi table but where?
        use crate::interrupts::{InterruptIndex, PICS};
        PICS.lock().initialize();
        if mp_float.imcrp & (1 << 7) == 0 {
            log::info!("Virtual wire mode is active");
            let keyboard_enable = InterruptIndex::Keyboard.as_pic_enable_mask();
            let serial_enable = InterruptIndex::COM1.as_pic_enable_mask()
                & InterruptIndex::COM2.as_pic_enable_mask();
            PICS.lock().mask(keyboard_enable & serial_enable, 0xff);
        } else {
            use x86_64::instructions::port::Port;
            let mut imcr_low: Port<u8> = Port::new(0x22);
            let mut imcr_high: Port<u8> = Port::new(0x23);
            imcr_low.write(0x70); // Select imcr register
            imcr_high.write(0x01); // go through apic
            PICS.lock().mask_all();
            log::warn!("Redirecting PIC to io acpi this has not been tested");
        }
    }
}
