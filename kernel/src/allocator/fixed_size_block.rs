use super::HEAP_SIZE;
use alloc::alloc::Layout;
use core::convert::TryFrom;
use core::ptr;

const ALLOC_STEPS: usize = 16;


/* This allocator needs HEAP_SIZE / (ALLOC_STEPS / sizeof(typeof(arr))) of memory
 * as overhead but it uses a fixed size array
 * which is cache coherent
 * Max alloc size is (2^16-1)*ALLOC_STEPS
 * Max alloc can be increased to 64Gb by changing the Option<u16> to
 * an Option<u32>. Doubles the size of the memory overhead
 */
/* In this case:
 * alloc buffer: 100K
 * max alloc: 1Mb
 * mem overhead: 13Kb
 * dealloc: O(1)
 * alloc worst: O(HEAP_SIZE / ALLOC_STEPS)
 * realloc best: O(1)
 * realloc worst: O(HEAP_SIZE / ALLOC_STEPS)
 */

// TODO: Use a generic here?
pub struct FixedSizeBlockAllocator {
    arr: [Option<u16>; HEAP_SIZE / ALLOC_STEPS],
    heap_start: usize,
}

impl FixedSizeBlockAllocator {
    pub const fn new(heap_start: usize) -> Self {
        FixedSizeBlockAllocator {
            arr: [None; HEAP_SIZE / ALLOC_STEPS],
            heap_start,
        }
    }

    pub fn print_array(&self, size: usize) {
        log::debug!("==== FixedSizeBlockAllocator ====");
        for i in 0..size {
            log::debug!("{}: {:#?}, ", i, self.arr[i]);
        }
    }
    pub fn print_non_empty(&self) {
        log::debug!("==== FixedSizeBlockAllocator ====");
        for (i, val) in self.arr.iter().enumerate() {
            if let Some(val) = val {
                log::debug!("{}: {:#?}, ", i, val);
            }
        }
    }
    pub fn num_non_empty(&self) -> usize {
        let mut count  = 0;
        for (_i, val) in self.arr.iter().enumerate() {
            if let Some(_val) = val {
                count += 1;
            }
        }
        count
    }

    pub fn num_bytes_allocated(&self) -> usize {
        let mut count  = 0;
        for (_i, val) in self.arr.iter().enumerate() {
            if let Some(val) = val {
                count += *val as usize;
            }
        }
        count
    }


    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: &Layout) {
        let index = (ptr as usize - self.heap_start) / ALLOC_STEPS;

        match self.arr[index].take() {
            None => {
                panic!("dealloced invalid ptr! {:#?}, index: {}", ptr, index);
            }
            Some(_i) => {
                log::info!(
                    "dealloced {:#x} bytes at addr: {:#?}",
                    _i as usize * ALLOC_STEPS,
                    ptr
                );
                self.arr[index] = None;
            }
        }
    }

    unsafe fn alloc(&mut self, layout: &Layout) -> *mut u8 {
        let needed_size = layout.align_to(ALLOC_STEPS).unwrap().pad_to_align().size();
        log::info!("alloc needed_size {:#x}", needed_size);
        let mut accumulator = 0;
        let mut spot = 0;

        // log::trace!("Searching for size: {}", needed_size);
        // Iterate over arr
        let mut i = 0;
        while i < self.arr.len() {
            // log::trace!("i = {}, spot = {}", i, spot);

            // Check if mem used at this index
            // if so reset accumulator and skip next
            // values
            if let Some(offset) = self.arr[i] {
                accumulator = 0;
                // log::trace!("offset by: {}", offset);
                i += offset as usize;
                spot = i;
                continue;

            // If not increase accumulator
            // and check if needed size reached
            } else {
                accumulator += ALLOC_STEPS;
                if needed_size == accumulator {
                    let arr_data =
                        u16::try_from(needed_size / ALLOC_STEPS).expect("alloc size is too big");
                    self.arr[spot] = Some(arr_data);
                    let mem_ptr = spot * ALLOC_STEPS + self.heap_start;
                    log::info!(
                        "alloc_ptr: {:#x}, size: {:#x}, spot: {}",
                        mem_ptr,
                        accumulator,
                        spot
                    );
                    return mem_ptr as *mut u8;
                }
            }

            i += 1;
        }

        log::error!("Heap is full!");
        ptr::null_mut()
    }
}

use super::Locked;
use alloc::alloc::GlobalAlloc;

//TODO: Use sse instructions to make it faster
//TODO: Use bitarray instead of u16 array
unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut alloc = self.lock();
        alloc.alloc(&layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut alloc = self.lock();
        alloc.dealloc(ptr, &layout);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let mut alloc = self.lock();
        let ptr = alloc.alloc(&layout);
        core::intrinsics::write_bytes::<u8>(
            ptr,
            0,
            layout.align_to(ALLOC_STEPS).unwrap().pad_to_align().size(),
        );
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let mut alloc = self.lock();
        let index = (ptr as usize - alloc.heap_start) / ALLOC_STEPS;
        let new_layout = Layout::from_size_align(new_size, ALLOC_STEPS)
            .unwrap()
            .pad_to_align();
        let new_size = new_layout.size();
        let old_size = layout.align_to(ALLOC_STEPS).unwrap().pad_to_align().size();

        log::info!(
            "realloc ptr: {:#?},
            prev_size: {},
            new_size: {}",
            ptr,
            old_size,
            new_size,
        );

        use core::cmp::Ordering;
        // Make buffer smaller
        match old_size.cmp(&new_size) {
            Ordering::Equal => {
                log::warn!("Called realloc with same size as previous buffer");
                ptr
            }
            Ordering::Greater => {
                let arr_data =
                    u16::try_from(new_size as usize / ALLOC_STEPS).expect("alloc size is too big");
                alloc.arr[index] = Some(arr_data);
                ptr
            }
            Ordering::Less => {
                alloc.dealloc(ptr, &layout);
                let new_ptr = alloc.alloc(&new_layout);

                // ptr is the same == buffer increased forward
                if new_ptr == ptr {
                    log::trace!("realloc: ptr is the same == buffer increased forward");
                } else {
                    core::intrinsics::copy_nonoverlapping(ptr, new_ptr, layout.size());
                }
                new_ptr
            }
        }
    }
}
