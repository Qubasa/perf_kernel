use crate::apic;
use core::mem::MaybeUninit;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering;

static mut CORES: Option<[MaybeUninit<AtomicU8>; bootloader::MAX_CORES]> = None;
static mut NUM_CORES_ONLINE: AtomicU8 = AtomicU8::new(0);

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
            x => panic!("Invalid ApicState from `u8` {}", x),
        }
    }
}

pub unsafe fn init() {
    if CORES.is_none() {
        CORES = Some(MaybeUninit::uninit_array());
        for core in CORES.as_mut().unwrap().iter_mut() {
            core.write(AtomicU8::new(ApicState::Offline as u8));
        }
    }
}

pub fn set_core_ready() {
    let id = apic::apic_id();
    set_state(id as usize, ApicState::Online);
    unsafe {
        NUM_CORES_ONLINE.fetch_add(1, Ordering::SeqCst);
    }
}

// pub fn are_all_cores_online(acpi: &Acpi) -> bool {
//      unsafe {
//         if NUM_CORES_ONLINE.load(Ordering::SeqCst) != ;
//     }
// }

fn set_state(id: usize, state: ApicState) {
    unsafe {
        CORES
            .as_mut()
            .unwrap()
            .get_mut(id)
            .unwrap()
            .assume_init_mut()
            .store(state as u8, Ordering::SeqCst);
    }
}

pub fn get_state(id: usize) -> ApicState {
    unsafe {
        ApicState::from(
            CORES
                .as_ref()
                .unwrap()
                .get(id)
                .unwrap()
                .assume_init_ref()
                .load(Ordering::SeqCst),
        )
    }
}
