use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size2MiB, Size4KiB,
    },
    VirtAddr,
};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 1000 * 1024; // 2Mib
                                          // TODO: Bug arises when all 4KiB pages are exausted and then 4KiB pages are allocated in 2MiB
                                          // pages? (i didn't understand it fully)

pub mod fixed_size_block;

use fixed_size_block::FixedSizeBlockAllocator;
#[global_allocator]
pub static ALLOCATOR: Locked<FixedSizeBlockAllocator> =
    Locked::new(FixedSizeBlockAllocator::new(HEAP_START));

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

pub fn init_heap(
    mapper: &mut impl Mapper<Size2MiB>,
    frame_allocator: &mut (impl FrameAllocator<Size2MiB> + FrameAllocator<Size4KiB>),
) -> Result<(), MapToError<Size2MiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);

        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    log::debug!("Start init heap");
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        log::debug!("Mapping virtual page: {:x?} to {:x?}", page, frame);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    log::debug!("Done init heap");
    Ok(())
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
