#![feature(bench_black_box)]
use dyn_frame_alloc::*;
//use tracy_client::*;



const TWOMB: usize = 0x200000;
const FOURMB: usize = TWOMB * 2;
//const PAGE: usize = 0x1000;

fn main() {
    let mut f = FrameAllocator::<FOURMB, 1>::new(128);
//    span!("some span");
    let mut addr = 0;
    for _ in 0..FOURMB {
        
        addr = f.alloc(1).unwrap();
    }
    println!("last addr: {}", addr);

}