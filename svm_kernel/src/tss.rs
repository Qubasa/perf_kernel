use core::convert::TryInto;

use crate::apic;
use x86_64::instructions::segmentation::{Segment, CS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub static mut TSS_STACK_ITER: Option<StackIter> = None;

pub struct StackIter {
    curr: u16,
    max: u16,
}

impl StackIter {
    pub fn new(max: u16) -> StackIter {
        Self { curr: 0, max: max }
    }
}

impl core::iter::Iterator for StackIter {
    type Item = u16;
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.max {
            self.curr += 1;
            return Some(self.curr);
        }
        return None;
    }
}

static mut GDT_ARR: [Option<GlobalDescriptorTable>; bootloader::MAX_CORES] =
    [None; bootloader::MAX_CORES];
static mut TSS_ARR: [Option<TaskStateSegment>; bootloader::MAX_CORES] =
    [None; bootloader::MAX_CORES];

pub unsafe fn init(boot_info: &'static bootloader::bootinfo::BootInfo) {
    let apic_id = apic::local_apic_id() as usize;
    log::info!("Local Apic id is: {}", apic_id);

    TSS_STACK_ITER = Some(StackIter::new(
        bootloader::TSS_STACKS_PER_CPU.try_into().unwrap(),
    ));

    let mut tss = TaskStateSegment::new();
    for i in 0..bootloader::TSS_STACKS_PER_CPU {
        let core = &boot_info.cores[apic_id as usize];
        let stack_start = core.tss.get_stack_start(i).unwrap();

        if i < 7 {
            tss.interrupt_stack_table[i] = VirtAddr::new(stack_start as u64);
        }
    }

    TSS_ARR[apic_id] = Some(tss);
    GDT_ARR[apic_id] = Some(GlobalDescriptorTable::new());
    let code_selector = GDT_ARR[apic_id]
        .as_mut()
        .unwrap()
        .add_entry(Descriptor::kernel_code_segment());
    let tss_selector = GDT_ARR[apic_id]
        .as_mut()
        .unwrap()
        .add_entry(Descriptor::tss_segment(TSS_ARR[apic_id].as_ref().unwrap()));
    GDT_ARR[apic_id].as_ref().unwrap().load();
    CS::set_reg(code_selector);
    load_tss(tss_selector);
}
