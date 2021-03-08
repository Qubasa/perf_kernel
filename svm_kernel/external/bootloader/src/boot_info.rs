#![no_std]

use core::slice;
use core::convert::TryFrom;

mod bootinfo;
pub use bootinfo::*;

pub fn create_from(memory_map_addr: u64, entry_count: u64) -> MemoryMap {
    let memory_map_start_ptr = memory_map_addr as *const E820MemoryRegion;
    let e820_memory_map =
        unsafe { slice::from_raw_parts(memory_map_start_ptr, usize::try_from(entry_count).unwrap()) };

    let mut memory_map = MemoryMap::new();
    for region in e820_memory_map {
        memory_map.add_region(MemoryRegion::from(*region));
    }

    memory_map.sort();

    let mut iter = memory_map.iter_mut().peekable();
    while let Some(region) = iter.next() {
        if let Some(next) = iter.peek() {
            if region.range.end_frame_number > next.range.start_frame_number
                && region.region_type == MemoryRegionType::Usable
            {
                region.range.end_frame_number = next.range.start_frame_number;
            }
        }
    }

    memory_map
}

