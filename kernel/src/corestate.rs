use core::{convert::TryFrom, ops::BitAnd};
use x86_64::addr::VirtAddr;
use x86_64::instructions::tables::{sgdt, sidt};
use x86_64::registers::control::*;
use x86_64::registers::model_specific::Efer;
use x86_64::registers::model_specific::*;
use x86_64::registers::mtrr::*;
use x86_64::registers::rflags::RFlags;
use x86_64::registers::xcontrol::*;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::paging::frame::PhysFrame;

//TODO: Understand PCID in CR3 for TLB sharing in smp
// https://stackoverflow.com/questions/47116141/why-each-logical-cpu-has-its-own-cr3-register-in-case-of-multithreading

static mut BSPCORE_STATE: Option<CoreState> = None;

pub fn check_corestate() {
    let curr_core_state = CoreState::new();
    let bsp_state = unsafe { BSPCORE_STATE.as_ref().unwrap() };
    if &curr_core_state != bsp_state {
        log::info!("First one is BSP second one is core 1");
        bsp_state.diff_print(&curr_core_state);
        panic!("Different core states. This will create issues.");
    }
}

pub fn save_corestate() {
    unsafe {
        BSPCORE_STATE = Some(CoreState::new());

        let corestate = BSPCORE_STATE.as_ref().unwrap();
        if log::log_enabled!(log::Level::Debug) {
            corestate.print_fixed_mtrrs();
        }
        corestate.print_variable_mtrrs();
    }
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct CoreState {
    pub xcr0: XCr0Flags,
    pub cr0: Cr0Flags,
    pub cr2: VirtAddr,
    pub cr3: (PhysFrame, Cr3Flags),
    pub cr4: Cr4Flags,
    pub cr8: Cr8Flags,
    pub gdtr: VirtAddr,
    pub idtr: VirtAddr,
    // TR
    pub efer: EferFlags,
    pub syscfg: SyscfgFlags,
    pub star: (
        SegmentSelector,
        SegmentSelector,
        SegmentSelector,
        SegmentSelector,
    ),
    pub lstar: VirtAddr,
    pub cstar: VirtAddr,
    pub sfmask: RFlags,
    pub fsbase: VirtAddr,
    pub gsbase: VirtAddr,
    pub kernel_gsbase: VirtAddr,
    // SYSENTER_CS
    // SYSENTER_ESP
    // SYSENTER_EIP

    /*MEMORY TYPING REGISTERS*/
    pub mtrrcap: MTRRcapFlags,
    pub mtrrdeftype: MTRRdefTypeFlags,
    pub mtrrphysbase0: MTRRphysBaseFlags,
    pub mtrrphysbase1: MTRRphysBaseFlags,
    pub mtrrphysbase2: MTRRphysBaseFlags,
    pub mtrrphysbase3: MTRRphysBaseFlags,
    pub mtrrphysbase4: MTRRphysBaseFlags,
    pub mtrrphysbase5: MTRRphysBaseFlags,
    pub mtrrphysbase6: MTRRphysBaseFlags,
    pub mtrrphysbase7: MTRRphysBaseFlags,

    //MTRRphysMaskn
    pub mtrrphysmask0: MTRRphysMaskFlags,
    pub mtrrphysmask1: MTRRphysMaskFlags,
    pub mtrrphysmask2: MTRRphysMaskFlags,
    pub mtrrphysmask3: MTRRphysMaskFlags,
    pub mtrrphysmask4: MTRRphysMaskFlags,
    pub mtrrphysmask5: MTRRphysMaskFlags,
    pub mtrrphysmask6: MTRRphysMaskFlags,
    pub mtrrphysmask7: MTRRphysMaskFlags,

    //MTRRfixn
    pub mtrrfix64k00000: FixMemRangeReg,
    pub mtrrfix16k80000: FixMemRangeReg,
    pub mtrrfix16ka0000: FixMemRangeReg,
    pub mtrrfix4kc0000: FixMemRangeReg,
    pub mtrrfix4kc8000: FixMemRangeReg,
    pub mtrrfix4kd0000: FixMemRangeReg,
    pub mtrrfix4kd8000: FixMemRangeReg,
    pub mtrrfix4ke0000: FixMemRangeReg,
    pub mtrrfix4ke8000: FixMemRangeReg,
    pub mtrrfix4kf0000: FixMemRangeReg,
    pub mtrrfix4kf8000: FixMemRangeReg,
}

impl CoreState {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            xcr0: XCr0::read(),
            cr0: Cr0::read(),
            cr2: Cr2::read(),
            cr3: Cr3::read(),
            cr4: Cr4::read(),
            cr8: Cr8::read(),
            gdtr: sgdt().base,
            idtr: sidt().base,
            efer: Efer::read(),
            syscfg: Syscfg::read(),
            star: Star::read(),
            lstar: LStar::read(),
            cstar: CStar::read(),
            sfmask: SFMask::read(),
            fsbase: FsBase::read(),
            gsbase: GsBase::read(),
            kernel_gsbase: KernelGsBase::read(),
            mtrrcap: MTRRcap::read(),
            mtrrdeftype: MTRRdefType::read(),
            mtrrfix64k00000: MTRRfix64K00000::read(),
            mtrrfix16k80000: MTRRfix16K80000::read(),
            mtrrfix16ka0000: MTRRfix16KA0000::read(),
            mtrrfix4kc0000: MTRRfix4KC0000::read(),
            mtrrfix4kc8000: MTRRfix4KC8000::read(),
            mtrrfix4kd0000: MTRRfix4KD0000::read(),
            mtrrfix4kd8000: MTRRfix4KD8000::read(),
            mtrrfix4ke0000: MTRRfix4KD8000::read(),
            mtrrfix4ke8000: MTRRfix4KE8000::read(),
            mtrrfix4kf0000: MTRRfix4KF0000::read(),
            mtrrfix4kf8000: MTRRfix4KF8000::read(),

            mtrrphysmask0: MTRRphysMask0::read(),
            mtrrphysmask1: MTRRphysMask0::read(),
            mtrrphysmask2: MTRRphysMask0::read(),
            mtrrphysmask3: MTRRphysMask0::read(),
            mtrrphysmask4: MTRRphysMask0::read(),
            mtrrphysmask5: MTRRphysMask0::read(),
            mtrrphysmask6: MTRRphysMask0::read(),
            mtrrphysmask7: MTRRphysMask0::read(),

            mtrrphysbase0: MTRRphysBase0::read(),
            mtrrphysbase1: MTRRphysBase1::read(),
            mtrrphysbase2: MTRRphysBase2::read(),
            mtrrphysbase3: MTRRphysBase3::read(),
            mtrrphysbase4: MTRRphysBase4::read(),
            mtrrphysbase5: MTRRphysBase5::read(),
            mtrrphysbase6: MTRRphysBase6::read(),
            mtrrphysbase7: MTRRphysBase7::read(),
        }
    }

    pub fn print_fixed_mtrrs(&self) {
        if !self.mtrrdeftype.contains(MTRRdefTypeFlags::FIXED_ENABLE) {
            log::info!("Fixed MTRRs are disabled!");
            return;
        }
        log::info!("Fixed MTRRs are enabled:");
        let arr = [
            self.mtrrfix64k00000,
            self.mtrrfix16k80000,
            self.mtrrfix16ka0000,
            self.mtrrfix4kc0000,
            self.mtrrfix4kc8000,
            self.mtrrfix4kd0000,
            self.mtrrfix4kd8000,
            self.mtrrfix4ke0000,
            self.mtrrfix4ke8000,
            self.mtrrfix4kf8000,
        ];

        let mut blast: Option<FixMemRange> = None;
        crate::print!("0x0 {:>5} ", "-");
        for (i, fixreg) in arr.iter().enumerate() {
            for (z, (&prange, &range)) in fixreg.iter().zip(fixreg.iter().skip(1)).enumerate() {
                if let Some(last) = blast {
                    if range.memory_type != last.memory_type {
                        crate::println!(
                            "{:#x}: {:?}",
                            prange.range.end.start_address().as_u64(),
                            prange.memory_type
                        );
                        crate::print!("{:#x} - ", range.range.start.start_address().as_u64());
                        blast = Some(range);
                    }

                    if i + 1 == arr.len() && z + 1 == 7 {
                        crate::println!(
                            "{:#x}: {:?}",
                            range.range.end.start_address().as_u64(),
                            range.memory_type
                        );
                    }
                } else {
                    blast = Some(range);
                }
            }
        }
    }

    pub fn print_variable_mtrrs(&self) {
        if !self.mtrrdeftype.contains(MTRRdefTypeFlags::MTRR_ENABLE) {
            panic!("MTRRs are not enabled. Everything is set to uncacheable!");
        }

        let default_mem_type =
            MTRRtype::try_from(self.mtrrdeftype.bitand(MTRRdefTypeFlags::TYPE).bits()).unwrap();
        if default_mem_type != MTRRtype::WriteBack {
            panic!(
                "Default memory type should be WriteBack is however {:?}",
                default_mem_type
            );
        }

        let arr = [
            (self.mtrrphysbase0, self.mtrrphysmask0),
            (self.mtrrphysbase1, self.mtrrphysmask1),
            (self.mtrrphysbase2, self.mtrrphysmask2),
            (self.mtrrphysbase3, self.mtrrphysmask3),
            (self.mtrrphysbase4, self.mtrrphysmask4),
            (self.mtrrphysbase5, self.mtrrphysmask5),
            (self.mtrrphysbase6, self.mtrrphysmask6),
            (self.mtrrphysbase7, self.mtrrphysmask7),
        ];

        let mut valid_once = false;
        for (i, (base, mask)) in arr.iter().enumerate() {
            let physbase = base.bitand(MTRRphysBaseFlags::PHYS_BASE).bits();
            let physmask = mask.bitand(MTRRphysMaskFlags::PHYS_MASK).bits();
            let enabled: bool = mask.bitand(MTRRphysMaskFlags::VALID).bits() == 1;
            let mem_type = MTRRtype::try_from(base.bitand(MTRRphysBaseFlags::TYPE).bits()).unwrap();
            let startrange = physmask & physbase;
            let a = startrange.trailing_zeros();
            let endmask = !(u64::MAX.overflowing_shl(a - a % 4).0);
            let endrange = startrange | endmask;
            if enabled {
                log::info!(
                    "Physbase{}: {:#x} - {:#x} Type: {:?}",
                    i,
                    startrange,
                    endrange,
                    mem_type
                );
                valid_once = true;
            }
        }

        if valid_once {
            panic!("Variable mtrrs are enabled and valid. Make *very* sure that you want this. In combination with PAT this is a recipe for desaster.");
        }
    }

    pub fn diff_print(&self, s: &CoreState) {
        if self.mtrrphysmask0 != s.mtrrphysmask0 {
            log::info!(
                "mtrrphysmask0:\n {:#?} \n {:#?}",
                self.mtrrphysmask0,
                s.mtrrphysmask0
            );
        }
        if self.mtrrphysmask1 != s.mtrrphysmask1 {
            log::info!(
                "mtrrphysmask1:\n {:#?} \n {:#?}",
                self.mtrrphysmask1,
                s.mtrrphysmask1
            );
        }
        if self.mtrrphysmask2 != s.mtrrphysmask2 {
            log::info!(
                "mtrrphysmask2:\n {:#?} \n {:#?}",
                self.mtrrphysmask2,
                s.mtrrphysmask2
            );
        }
        if self.mtrrphysmask3 != s.mtrrphysmask3 {
            log::info!(
                "mtrrphysmask3:\n {:#?} \n {:#?}",
                self.mtrrphysmask3,
                s.mtrrphysmask3
            );
        }
        if self.mtrrphysmask4 != s.mtrrphysmask4 {
            log::info!(
                "mtrrphysmask4:\n {:#?} \n {:#?}",
                self.mtrrphysmask4,
                s.mtrrphysmask4
            );
        }
        if self.mtrrphysmask5 != s.mtrrphysmask5 {
            log::info!(
                "mtrrphysmask5:\n {:#?} \n {:#?}",
                self.mtrrphysmask5,
                s.mtrrphysmask5
            );
        }
        if self.mtrrphysmask6 != s.mtrrphysmask6 {
            log::info!(
                "mtrrphysmask6:\n {:#?} \n {:#?}",
                self.mtrrphysmask6,
                s.mtrrphysmask6
            );
        }
        if self.mtrrphysmask7 != s.mtrrphysmask7 {
            log::info!(
                "mtrrphysmask7:\n {:#?} \n {:#?}",
                self.mtrrphysmask7,
                s.mtrrphysmask7
            );
        }
        if self.mtrrphysbase0 != s.mtrrphysbase0 {
            log::info!(
                "mtrrphysbase0:\n {:#?} \n {:#?}",
                self.mtrrphysbase0,
                s.mtrrphysbase0
            );
        }
        if self.mtrrphysbase1 != s.mtrrphysbase1 {
            log::info!(
                "mtrrphysbase1:\n {:#?} \n {:#?}",
                self.mtrrphysbase1,
                s.mtrrphysbase1
            );
        }
        if self.mtrrphysbase2 != s.mtrrphysbase2 {
            log::info!(
                "mtrrphysbase2:\n {:#?} \n {:#?}",
                self.mtrrphysbase2,
                s.mtrrphysbase2
            );
        }
        if self.mtrrphysbase3 != s.mtrrphysbase3 {
            log::info!(
                "mtrrphysbase3:\n {:#?} \n {:#?}",
                self.mtrrphysbase3,
                s.mtrrphysbase3
            );
        }
        if self.mtrrphysbase4 != s.mtrrphysbase4 {
            log::info!(
                "mtrrphysbase4:\n {:#?} \n {:#?}",
                self.mtrrphysbase4,
                s.mtrrphysbase4
            );
        }
        if self.mtrrphysbase5 != s.mtrrphysbase5 {
            log::info!(
                "mtrrphysbase5:\n {:#?} \n {:#?}",
                self.mtrrphysbase5,
                s.mtrrphysbase5
            );
        }
        if self.mtrrphysbase6 != s.mtrrphysbase6 {
            log::info!(
                "mtrrphysbase6:\n {:#?} \n {:#?}",
                self.mtrrphysbase6,
                s.mtrrphysbase6
            );
        }
        if self.mtrrphysbase7 != s.mtrrphysbase7 {
            log::info!(
                "mtrrphysbase7:\n {:#?} \n {:#?}",
                self.mtrrphysbase7,
                s.mtrrphysbase7
            );
        }
        if self.mtrrcap != s.mtrrcap {
            log::info!("mtrrcap:\n {:#?} \n {:#?}", self.mtrrcap, s.mtrrcap);
        }
        if self.mtrrdeftype != s.mtrrdeftype {
            log::info!(
                "mtrrdeftype:\n {:#?} \n {:#?}",
                self.mtrrdeftype,
                s.mtrrdeftype
            );
        }
        if self.mtrrfix64k00000 != s.mtrrfix64k00000 {
            print_mtrrfix_diff("mtrrfix64k00000", &self.mtrrfix64k00000, &s.mtrrfix64k00000);
        }
        if self.mtrrfix16k80000 != s.mtrrfix16k80000 {
            print_mtrrfix_diff("mtrrfix16k80000", &self.mtrrfix16k80000, &s.mtrrfix16k80000);
        }
        if self.mtrrfix16ka0000 != s.mtrrfix16ka0000 {
            print_mtrrfix_diff("mtrrfix16ka0000", &self.mtrrfix16ka0000, &s.mtrrfix16ka0000);
        }
        if self.mtrrfix4kc0000 != s.mtrrfix4kc0000 {
            print_mtrrfix_diff("mtrrfix16ka0000", &self.mtrrfix4kc0000, &s.mtrrfix4kc0000);
        }
        if self.mtrrfix4kc8000 != s.mtrrfix4kc8000 {
            print_mtrrfix_diff("mtrrfix4kc8000", &self.mtrrfix4kc8000, &s.mtrrfix4kc8000);
        }
        if self.mtrrfix4kd0000 != s.mtrrfix4kd0000 {
            print_mtrrfix_diff("mtrrfix4kd0000", &self.mtrrfix4kd0000, &s.mtrrfix4kd0000);
        }
        if self.mtrrfix4kd8000 != s.mtrrfix4kd8000 {
            print_mtrrfix_diff("mtrrfix4kd8000", &self.mtrrfix4kd8000, &s.mtrrfix4kd8000);
        }
        if self.mtrrfix4ke0000 != s.mtrrfix4ke0000 {
            print_mtrrfix_diff("mtrrfix4ke0000", &self.mtrrfix4ke0000, &s.mtrrfix4ke0000);
        }
        if self.mtrrfix4ke8000 != s.mtrrfix4ke8000 {
            print_mtrrfix_diff("mtrrfix4ke8000", &self.mtrrfix4ke8000, &s.mtrrfix4ke8000);
        }
        if self.mtrrfix4kf8000 != s.mtrrfix4kf8000 {
            print_mtrrfix_diff("mtrrfix4kf8000", &self.mtrrfix4kf8000, &s.mtrrfix4kf8000);
        }
        if self.xcr0 != s.xcr0 {
            log::info!("xcr0:\n {:#?} \n {:#?}", self.xcr0, s.xcr0);
        }
        if self.cr0 != s.cr0 {
            log::info!("cr0:\n {:#?} \n {:#?}", self.cr0, s.cr0);
        }
        if self.cr2 != s.cr2 {
            log::info!("cr2:\n {:#?} \n {:#?}", self.cr2, s.cr2);
        }
        if self.cr3 != s.cr3 {
            log::info!("cr3:\n {:#?} \n {:#?}", self.cr3, s.cr3);
        }
        if self.cr4 != s.cr4 {
            log::info!("cr4:\n {:#?} \n {:#?}", self.cr4, s.cr4);
        }
        if self.cr8 != s.cr8 {
            log::info!("cr8:\n {:#?} \n {:#?}", self.cr8, s.cr8);
        }
        if self.idtr != s.idtr {
            log::info!("idtr:\n {:#?} \n {:#?}", self.idtr, s.idtr);
        }
        if self.gdtr != s.gdtr {
            log::info!("gdtr:\n {:#?} \n {:#?}", self.gdtr, s.gdtr);
        }
        if self.efer != s.efer {
            log::info!("efer:\n {:#?} \n {:#?}", self.efer, s.efer);
        }
        if self.syscfg != s.syscfg {
            log::info!("syscfg:\n {:#?} \n {:#?}", self.syscfg, s.syscfg);
        }
        if self.star != s.star {
            log::info!("star:\n {:#?} \n {:#?}", self.star, s.star);
        }
        if self.lstar != s.lstar {
            log::info!("lstar:\n {:#?} \n {:#?}", self.lstar, s.lstar);
        }
        if self.cstar != s.cstar {
            log::info!("cstar:\n {:#?} \n {:#?}", self.cstar, s.cstar);
        }
        if self.sfmask != s.sfmask {
            log::info!("sfmask:\n {:#?} \n {:#?}", self.sfmask, s.sfmask);
        }
        if self.fsbase != s.fsbase {
            log::info!("sfbase:\n {:#?} \n {:#?}", self.fsbase, s.fsbase);
        }
        if self.gsbase != s.gsbase {
            log::info!("gsbase:\n {:#?} \n {:#?}", self.gsbase, s.gsbase);
        }
        if self.kernel_gsbase != s.kernel_gsbase {
            log::info!(
                "kernel_gsbase:\n {:#?} \n {:#?}",
                self.kernel_gsbase,
                s.kernel_gsbase
            );
        }
    }
}

fn print_mtrrfix_diff(name: &str, a: &FixMemRangeReg, b: &FixMemRangeReg) {
    for (_i, (c, d)) in a.iter().zip(b.iter()).enumerate() {
        if c != d {
            log::info!("{}: \n {:#?} \n {:#?}", name, c, d);
        }
    }
}
