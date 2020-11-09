use x86_64::registers::control::Cr3;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::Mapper;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::{OffsetPageTable, PageTable};
use x86_64::VirtAddr;

//
// The bootloader maps the page table to a very high offset
// in memory and this function returns the page table type
// with the offset applied
// Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();

    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};

//TODO: If rust allows it in the future save the iterator in struct
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}


// Identity maps the phys address + type size and volatile reads the type from
// memory. Does not unmap the page
pub unsafe fn map_and_read_phys<T: Copy>(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    addr: PhysAddr,
) -> T
{
    // Map the start address
    id_map_nocache(mapper, frame_allocator, addr).unwrap();

    // Add type size and map if on new page
    let size = core::mem::size_of::<T>() as u64;
    id_map_nocache(mapper, frame_allocator, addr + size).unwrap();

    // NOTE: Can't use read_volatile because pointer is not necesseraly aligned
    // like in acpi when searching for tables (as by spec)
    // core::ptr::read_volatile(addr.as_u64() as *const T)
    let ptr = addr.as_u64() as *const T;
    *ptr
}

// Identity map page and return page
// if already identity mapped succeed and return page
// if mapped but not as identity then return error
pub unsafe fn id_map_nocache(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    addr: PhysAddr,
) -> Result<Page, MapToError<Size4KiB>> {
    // Seek for page & phys frame containing address
    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(addr.as_u64()));
    use x86_64::structures::paging::PageTableFlags as Flags;
    let frame = PhysFrame::containing_address(addr);
    let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_CACHE | Flags::NO_EXECUTE;

    // Identity map both and do not fail if they are already id mapped
    let map_to_result = match mapper.map_to(page, frame, flags, frame_allocator) {
        Ok(i) => i,
        Err(MapToError::PageAlreadyMapped(mapped_frame)) => {
            if mapped_frame != frame {
                return Err(MapToError::PageAlreadyMapped(mapped_frame));
            }
            return Ok(page);
        }
        Err(e) => return Err(e),
    };

    // Flush TLB
    map_to_result.flush();
    Ok(page)
}

use bootloader::bootinfo::MemoryMap;
/// A FrameAllocator that returns usable frames from the bootloader's memory map.
#[derive(Debug)]
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

use bootloader::bootinfo::MemoryRegionType;
impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    pub fn test(&self) {
        log::info!("TEST!!");
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}
