#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(perf_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(dead_code)]
#![allow(unused_imports)]

extern crate alloc;

use bootloader::bootinfo::BootInfo;
use bootloader::entry_point;
use core::hint::black_box;
use core::panic::PanicInfo;
use perf_kernel::{
    allocator::ALLOCATOR, allocator::HEAP_START, bench::Bench, klog, print, println,
};

entry_point!(main);

static mut TEST_LOCK: spin::Mutex<u8> = spin::Mutex::new(0);

fn main(boot_info: &'static BootInfo) -> ! {
    klog::init();
    log::set_max_level(log::LevelFilter::Debug);

    unsafe {
        perf_kernel::init(boot_info);
    }
    println!("===== heap_allocator test =====");

    test_main();
    perf_kernel::hlt_loop();
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
    perf_kernel::test_panic_handler(info)
}

use alloc::alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout};
use alloc::boxed::Box;
use core::intrinsics::copy;

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);

    black_box(&heap_value_1);
    black_box(&heap_value_2);
}

//#[test_case]
fn zero_alloc() {
    unsafe {
        let lock = TEST_LOCK.lock();
        let layout = Layout::new::<u16>();
        let ptr = alloc_zeroed(layout);

        assert_eq!(*(ptr as *mut u16), 0);

        dealloc(ptr, layout);
        black_box(lock);
    }
}

//#[test_case]
fn realloc_grow_forward() {
    unsafe {
        let lock = TEST_LOCK.lock();

        let layout = Layout::from_size_align(32, 16).unwrap();
        let old_ptr = alloc(layout);

        let n: u32 = 0xdeadbeef;
        copy::<u32>(&n as *const u32, old_ptr as *mut u32, 1);

        let new_ptr = realloc(old_ptr, layout, 640);

        // assert_eq!(new_ptr, old_ptr);
        assert_eq!(*(new_ptr as *mut u16), 0xbeef);
        assert_eq!(*(new_ptr as *mut u16).offset(1), 0xdead);

        dealloc(new_ptr, layout);

        black_box(lock);
    }
}

//#[test_case]
fn realloc_copy_grow() {
    unsafe {
        let lock = TEST_LOCK.lock();
        let mut bench = Bench::start();
        let layout = Layout::from_size_align(32, 16).unwrap();
        let old_ptr = alloc(layout);
        let obstacle_ptr = alloc(layout);
        black_box(obstacle_ptr);

        let n: u32 = 0xdeadbeef;
        copy::<u32>(&n as *const u32, old_ptr as *mut u32, 1);

        let new_ptr = realloc(old_ptr, layout, 640);

        assert_ne!(new_ptr, old_ptr);
        assert_eq!(*(new_ptr as *mut u16), 0xbeef);
        assert_eq!(*(new_ptr as *mut u16).offset(1), 0xdead);

        dealloc(new_ptr, layout);
        dealloc(obstacle_ptr, layout);
        bench.end();
        black_box(lock);
    }
}

//#[test_case]
fn realloc_copy_shrink() {
    let lock = unsafe { TEST_LOCK.lock() };
    unsafe {
        let mut bench = Bench::start();
        let layout = Layout::from_size_align(32, 16).unwrap();
        let old_ptr = alloc(layout);
        let obstacle_ptr = alloc(layout);
        black_box(obstacle_ptr);

        let n: u32 = 0xdeadbeef;
        copy::<u32>(&n as *const u32, old_ptr as *mut u32, 1);

        let new_ptr = realloc(old_ptr, layout, 16);

        assert_eq!(new_ptr, old_ptr);
        assert_eq!(*(new_ptr as *mut u16), 0xbeef);

        dealloc(new_ptr, layout);
        dealloc(obstacle_ptr, layout);
        bench.end();
    }
    black_box(lock);
}

#[test_case]
fn heap_full_alloc() {
    let mut vec: Vec<usize> = Vec::new();
    let mut i = 0;
    let len = vec.len();
    let new_len = (HEAP_SIZE - 512) / core::mem::size_of::<usize>();
    log::info!("Start resize...");
    vec.resize_with(new_len, || {
        i += 1;
        i
    });
    log::info!("Done resizing.");
    let sum: usize = vec.iter().sum();

    let n = new_len - len;
    assert_eq!(sum, (n.pow(2) + n) / 2);

    let mut vec2: Vec<usize> = Vec::new();
    let mut i = 0;
    let len = vec2.len();
    let new_len = (HEAP_SIZE - 512) / core::mem::size_of::<usize>();
    log::info!("Start resize2...");
    vec2.resize_with(new_len, || {
        i += 1;
        i
    });
    log::info!("Done resizing2.");
    let sum: usize = vec.iter().sum();

    let n = new_len - len;
    assert_eq!(sum, (n.pow(2) + n) / 2);
    black_box(vec);
}

#[test_case]
fn mult_alloc() {
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
}

use alloc::vec::Vec;

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

use perf_kernel::allocator::HEAP_SIZE;

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        black_box(&x);
        assert_eq!(*x, i);
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Test0 {
    typ: u8,
    length: u8,
    processor_uid: u8,
    id: u8,
    flags: u32,
}
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Test1 {
    typ: u8,
    length: u8,
    bus: u8, // always 0
    source: u8,
    mapped_to: u32,
    flags: u16,
}
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Test2 {
    typ: u8,
    length: u8,
    id: u8,
    res0: u8,
    address: u32,
    interrupt_base: u32,
}

#[test_case]
fn multiple_vecs() {
    let mut vec0 = Vec::new();
    let mut vec1 = Vec::new();
    let mut vec2 = Vec::new();

    let test0 = Test0 {
        typ: 0,
        length: 32,
        processor_uid: 0,
        id: 0,
        flags: 0xff,
    };
    let test1 = Test1 {
        typ: 1,
        length: 10,
        bus: 0,
        source: 0,
        mapped_to: 2,
        flags: 0xff,
    };
    let test2 = Test2 {
        typ: 2,
        length: 12,
        id: 1,
        res0: 0,
        address: 0xbaab,
        interrupt_base: 0xc,
    };

    vec0.push(test0);
    vec1.push(test1);
    vec2.push(test2);

    black_box(&vec1);
    black_box(&vec2);
    black_box(&vec0);
}
