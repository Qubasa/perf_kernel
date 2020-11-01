use super::{HEAP_SIZE, HEAP_START};
use alloc::alloc::Layout;
use core::ptr;

const ALLOC_STEPS: usize = 16;

/* This allocator needs HEAP_SIZE / (ALLOC_STEPS / 4) of memory
 * as overhead but it uses a fixed size array
 * which is cache coherent
 * Max alloc size is (2^32-1)*ALLOC_STEPS
 */
/* In this case:
 * max alloc: 64G
 * mem overhead: 25Kb
 * dealloc: O(1)
 * alloc worst: O(HEAP_SIZE / ALLOC_STEPS)
 * realloc best: O(1)
 * realloc worst: O(HEAP_SIZE / ALLOC_STEPS)
 */

// TODO: Use a generic here?
pub struct FixedSizeBlockAllocator {
    arr: [Option<u32>; HEAP_SIZE / ALLOC_STEPS],
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        FixedSizeBlockAllocator {
            arr: [None; HEAP_SIZE / ALLOC_STEPS],
        }
    }

    pub fn print_array(&self, size: usize) {
        log::debug!("==== FixedSizeBlockAllocator ====");
        for i in 0..size {
            log::debug!("{}: {:#?}, ", i, self.arr[i]);
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: &Layout) {
        let index = (ptr as usize - HEAP_START) / ALLOC_STEPS;

        match self.arr[index].take() {
            None => {
                panic!("dealloced invalid ptr! {:#?}, index: {}", ptr, index);
            }
            Some(i) => {
                log::trace!(
                    "dealloced {:#x} bytes at addr: {:#?}",
                    i as usize * ALLOC_STEPS,
                    ptr
                );
                self.arr[index] = None;
            }
        }
    }

    unsafe fn alloc(&mut self, layout: &Layout) -> *mut u8 {
        let needed_size = layout.align_to(ALLOC_STEPS).unwrap().pad_to_align().size();
        let mut accumulator = 0;
        let mut spot = 0;

        log::trace!("Searching for size: {}", needed_size);
        // Iterate over arr
        let mut i = 0;
        while i < self.arr.len() {
            log::trace!("i = {}", i);

            // Check if mem used at this index
            // if so reset accumulator and skip next
            // values
            if let Some(offset) = self.arr[i] {
                accumulator = 0;
                spot = i + 1;
                log::trace!("offset by: {}", offset);
                i += offset as usize;
                continue;

            // If not increase accumulator
            // and check if needed size reached
            } else {
                accumulator += ALLOC_STEPS;
                if needed_size == accumulator {
                    self.arr[spot] = Some((needed_size / ALLOC_STEPS) as u32);
                    let mem_ptr = spot * ALLOC_STEPS + HEAP_START;
                    log::trace!(
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

//TODO: Benchmark your implementation
//TODO: Use sse instructions to make it faster
//TODO: Use bitarray instead of u32 array
unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut alloc = self.lock();
        alloc.alloc(&layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut alloc = self.lock();
        alloc.dealloc(ptr, &layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let mut alloc = self.lock();
        let index = (ptr as usize - HEAP_START) / ALLOC_STEPS;
        let new_layout = Layout::from_size_align(new_size, ALLOC_STEPS)
            .unwrap()
            .pad_to_align();
        let new_size = new_layout.size();
        let old_size = layout.align_to(ALLOC_STEPS).unwrap().pad_to_align().size();

        log::trace!(
            "realloc ptr: {:#?},
            prev_size: {},
            new_size: {}",
            ptr,
            old_size,
            new_size,
        );

        // Make buffer smaller
        if old_size > new_size {
            alloc.arr[index] = Some((new_size as usize / ALLOC_STEPS) as u32);
            return ptr;

        // Make buffer bigger
        } else if old_size < new_size {
            alloc.dealloc(ptr, &layout);
            let new_ptr = alloc.alloc(&new_layout);

            // ptr is the same == buffer increased forward
            if new_ptr == ptr {
                log::trace!("realloc: ptr is the same == buffer increased forward");
            } else {
                core::intrinsics::copy_nonoverlapping(ptr, new_ptr, layout.size());
            }
            return new_ptr;

        // After aligning new_size to ALLOC_STEPS buffer remains at the same size
        } else {
            log::warn!("Called realloc with same size as previous buffer");
            return ptr::null_mut();
        }
    }
}
