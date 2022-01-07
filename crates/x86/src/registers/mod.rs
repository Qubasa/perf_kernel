//! Access to various system and model specific registers.

pub mod control;
pub mod model_specific;
pub mod xcontrol;
pub mod eflags;

#[cfg(all(feature = "instructions", feature = "inline_asm"))]
pub use crate::instructions::read_eip;
