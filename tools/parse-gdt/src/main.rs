#![allow(dead_code)]

use modular_bitfield::prelude::*;
#[bitfield]
struct GdtEntry {
    limit0: B16,
    baddr0: B24,
    typ: B4,
    s: B1,
    dpl: B2,
    p: B1,
    limit1: B4,
    avl: B1,
    #[skip] __: B1,
    db: B1,
    g: B1,
    baddr1: B8
}


#[bitfield(filled = false)]
struct CodeEntry {
    a: B1,
    r: B1,
    c: B1,
    typ: B1,
}

#[bitfield(filled = false)]
struct DataEntry {
    a: B1,
    w: B1,
    e: B1,
    typ: B1,
}

fn print_gdt(entry: u64) {
    println!("==== {:#x} ====", entry);

    let g = GdtEntry::from_bytes(entry.to_le_bytes());
    let mut base_addr:u32 = (g.baddr1() as u32) << 24;
    base_addr |= g.baddr0();
    let mut limit = (g.limit1() as u32) << 16;
    limit |= g.limit0() as u32;
    println!("Base address: {:#x}", base_addr);
    println!("Segment limit: {:#x}", limit);
    if g.s() == 1 {
        println!("User segment");
    }else {
        println!("System segment");
    }

    if g.p() == 1 {
        println!("Segment is present in memory");
    } else {
        println!("Segment is stored on disk");
    }

    println!("Privilege level: {}", g.dpl());

    if g.g() == 1 {
        println!("Limit scaling: 1 limit byte = 4k memory");
    } else {
        println!("Limit scaling: 1 limit byte = 1 memory byte");
    }

    if g.db() == 0 {
        println!("Operand size in code and data: 16 byte");
    }else {
        println!("Operand size in code and data: 32 byte");
    }

    let raw_t = g.typ().to_le_bytes();
    let t =  CodeEntry::from_bytes(raw_t).unwrap();

    if g.s() == 1 && t.typ() == 1 {
        println!("Code segment");

        println!("Conforming: {}", t.c());
        println!("Readable: {}", t.r());
        println!("Accessed: {}", t.a());
    }

    if g.s() == 1 && t.typ() == 0 {
        println!("Data segment");
        let t =  DataEntry::from_bytes(raw_t).unwrap();

        if t.e() == 1 {
            println!("Expand down");

            if g.db() == 1 {
                println!("Upper bound is 0x0_FFFF_FFFF");
            } else {
                println!("Upper bound is 0x0_FFFF");
            }
        }
        if t.w() == 1 {
            println!("Readable & writable");
        } else {
            println!("Only readable");
        }

        println!("Accessed: {}", t.a());
    }
}

use std::env;

fn main() {
    for argument in env::args().skip(1) {
        println!("arg: {}", argument);
        let without_pref = argument.trim_start_matches("0x");
        print_gdt(u64::from_str_radix(&without_pref, 16).unwrap());
    }
}
