
/// Maximum number of cores allowed on the system
pub const MAX_CORES: usize = 1024;


//TODO: Understand PCID in CR3 for TLB sharing in smp
// https://stackoverflow.com/questions/47116141/why-each-logical-cpu-has-its-own-cr3-register-in-case-of-multithreading

pub static mut BSPCORE_STATE: Option<CoreState> = None;

/// Different states for APICs to be in
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ApicState {
    /// The core has checked in with the kernel and is actively running
    Online = 1,

    /// The core has been launched by the kernel, but has not yet registered
    /// with the kernel
    Launched = 2,

    /// The core is present but has not yet been launched
    Offline = 3,

    /// This APIC ID does not exist
    None = 4,

    /// This APIC ID has disabled interrupts and halted forever
    Halted = 5,
}

impl From<u8> for ApicState {
    /// Convert a raw `u8` into an `ApicState`
    fn from(val: u8) -> ApicState {
        match val {
            1 => ApicState::Online,
            2 => ApicState::Launched,
            3 => ApicState::Offline,
            4 => ApicState::None,
            5 => ApicState::Halted,
            _ => panic!("Invalid ApicState from `u8`"),
        }
    }
}

use x86_64::registers::model_specific::*;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::registers::control::*;
use x86_64::registers::model_specific::Efer;
use x86_64::registers::rflags::RFlags;
use x86_64::addr::VirtAddr;
use x86_64::structures::paging::frame::PhysFrame;
use x86_64::registers::xcontrol::*;

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct CoreState {

    pub xcr0: XCr0Flags,
    // CR0
    pub cr0: Cr0Flags,
    // CR2
    pub cr2: VirtAddr,
    // CR3
    pub cr3: (PhysFrame, Cr3Flags),
    // CR4
    pub cr4: Cr4Flags,
    // CR8

    // RFLAGS
    pub rflags: RFlags,
    // GDTR
    // IDTR
    // LDTR
    // TR
    pub efer: EferFlags,
    // SYSCFG
    pub star: (SegmentSelector, SegmentSelector, SegmentSelector, SegmentSelector),
    pub lstar: VirtAddr,
    // CSTAR
    pub sfmask: RFlags,
    pub fsbase: VirtAddr,
    pub gsbase: VirtAddr,
    pub kernel_gsbase: VirtAddr,
    // SYSENTER_CS
    // SYSENTER_ESP
    // SYSENTER_EIP

    /*MEMORY TYPING REGISTERS*/
    //MTRRcap
    //MTRRdefType
    //MTRRphysBasen
    //MTRRphysMaskn
    //MTRRfixn
    //PAT
    //TOP_MEM
    //TOP_MEM2
}

impl CoreState {
    pub fn new() -> Self {
        Self {
            xcr0: XCr0::read(),
            cr0: Cr0::read(),
            cr2: Cr2::read(),
            cr3: Cr3::read(),
            cr4: Cr4::read(),
            rflags: x86_64::registers::rflags::read(),
            efer: Efer::read(),
            star: Star::read(),
            lstar: LStar::read(),
            sfmask: SFMask::read(),
            fsbase: FsBase::read(),
            gsbase: GsBase::read(),
            kernel_gsbase: KernelGsBase::read(),   
        }
    }

    pub fn diff_print(&self, s: &CoreState) {
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
        if self.rflags != s.rflags {
            log::info!("rflags:\n {:#?} \n{:#?}", self.rflags, s.rflags);
        }
        if self.efer != s.efer {
            log::info!("efer:\n {:#?} \n {:#?}", self.efer, s.efer);
        }

        if self.star != s.star {
            log::info!("star:\n {:#?} \n {:#?}", self.star, s.star);
        }
        if self.lstar != s.lstar {
            log::info!("lstar:\n {:#?} \n {:#?}", self.lstar, s.lstar);
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
            log::info!("kernel_gsbase:\n {:#?} \n {:#?}", self.kernel_gsbase, s.kernel_gsbase);
        }
    }
}


pub fn apic_id() -> u8 {
    unsafe {
        let res = core::arch::x86_64::__cpuid(0x0000_0001);
        let core_id = (res.ebx >> 24) as u8;
        return core_id;
    };
}

