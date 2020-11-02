#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(svm_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use svm_kernel::{
    allocator::HEAP_START, bench::black_box, bench::Bench, mylog::LOGGER, print, println,
};

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use svm_kernel::allocator;
    use svm_kernel::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    svm_kernel::init();
    println!("===== heap_allocator test =====");
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    loop {}
}
use core::fmt::LowerHex;
fn print_heap<T>(offset: isize, size: isize)
where
    T: LowerHex,
{
    unsafe {
        log::debug!("==== HEAP CONTENT ====");
        let ptr = HEAP_START as *const T;
        for i in offset..size {
            print!("{:#x}, ", *ptr.offset(i));
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print_heap::<u16>(0, 10);
    svm_kernel::test_panic_handler(info)
}

use alloc::alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout};
use alloc::boxed::Box;
use core::intrinsics::copy;

#[test_case]
fn simple_allocation() {
    let mut bench = Bench::start();
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
    bench.end();
    black_box(&heap_value_1);
    black_box(&heap_value_2);
}

#[test_case]
fn zero_alloc() {
    unsafe {
        let layout = Layout::new::<u16>();
        let ptr = alloc_zeroed(layout);

        assert_eq!(*(ptr as *mut u16), 0);

        dealloc(ptr, layout);
    }
}

#[test_case]
fn realloc_grow_forward() {
    unsafe {
        let mut bench = Bench::start();
        let layout = Layout::array::<u16>(4).unwrap();
        let old_ptr = alloc(layout);

        let n: u32 = 0xdeadbeef;
        copy::<u32>(&n as *const u32, old_ptr as *mut u32, 1);

        let new_ptr = realloc(old_ptr, layout, 64);

        // assert_eq!(new_ptr, old_ptr);
        assert_eq!(*(new_ptr as *mut u16), 0xbeef);
        assert_eq!(*(new_ptr as *mut u16).offset(1), 0xdead);

        dealloc(new_ptr, layout);
        bench.end();
    }
}

#[test_case]
fn realloc_copy_grow() {
    unsafe {
        let mut bench = Bench::start();
        let layout = Layout::array::<u16>(4).unwrap();
        let old_ptr = alloc(layout);
        let obstacle_ptr = alloc(layout);
        black_box(obstacle_ptr);

        let n: u32 = 0xdeadbeef;
        copy::<u32>(&n as *const u32, old_ptr as *mut u32, 1);

        let new_ptr = realloc(old_ptr, layout, 64);

        assert_ne!(new_ptr, old_ptr);
        assert_eq!(*(new_ptr as *mut u16), 0xbeef);
        assert_eq!(*(new_ptr as *mut u16).offset(1), 0xdead);

        dealloc(new_ptr, layout);
        dealloc(obstacle_ptr, layout);
        bench.end();
    }
}

//TODO
// #[test_case]
// fn realloc_copy_shrink() {
//     unsafe {
//         let mut bench = Bench::start();
//         let layout = Layout::array::<u16>(4).unwrap();
//         let old_ptr = alloc(layout);
//         let obstacle_ptr = alloc(layout);

//         let n: u32 = 0xdeadbeef;
//         copy::<u32>(&n as *const u32, old_ptr as *mut u32, 1);

//         let new_ptr = realloc(old_ptr, layout, 64);

//         assert_ne!(new_ptr, old_ptr);
//         assert_eq!(*(new_ptr as *mut u16), 0xbeef);
//         assert_eq!(*(new_ptr as *mut u16).offset(1), 0xdead);

//         dealloc(new_ptr, layout);
//         dealloc(obstacle_ptr, layout);
//         bench.end();
//     }
// }

#[test_case]
fn heap_full_alloc() {
    unsafe {
        let mut bench = Bench::start();
        let layout = Layout::array::<u8>(HEAP_SIZE).unwrap();
        let ptr = black_box(alloc(layout));

        // let failed_layout = Layout::array::<u8>(1).unwrap();
        // let failed_ptr = alloc(failed_layout);
        // assert_eq!(failed_ptr, core::ptr::null_mut());
        dealloc(ptr, layout);
        bench.end();
    }
}

#[test_case]
fn mult_alloc() {
    let mut bench = Bench::start();
    {
        let heap_value_1 = Box::new(41);
        let heap_value_2 = Box::new(13);
        black_box(&heap_value_1);
        black_box(&heap_value_2);
        assert_eq!(*heap_value_1, 41);
        assert_eq!(*heap_value_2, 13);
    }
    let heap_value_1 = Box::<u64>::new(0xdeadbeef);
    black_box(&heap_value_1);
    assert_eq!(*heap_value_1, 0xdeadbeef);
    bench.end();
}

use alloc::vec::Vec;

#[test_case]
fn large_vec() {
    let mut bench = Bench::start();
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
    bench.end();
}

use svm_kernel::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    let mut bench = Bench::start();
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        black_box(&x);
        assert_eq!(*x, i);
    }
    bench.end();
}
