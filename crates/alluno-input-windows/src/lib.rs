//! The Windows host: keyboard and mouse through the AllunoInput KMDF filter
//! driver with `SendInput` underneath it, pen and touch through the AllunoVHID
//! bus when it is installed and the Synthetic Pointer API otherwise, gamepads
//! through AllunoVHID for the HID profiles and a ViGEmBus-compatible bus for
//! the Xbox 360 identity.
//!
//! The filter driver sits above `kbdclass` and `mouclass`, so what it injects
//! arrives the way hardware does: no `LLKHF_INJECTED`, no extra-info marker,
//! nothing for a game or a security product to tell apart. AllunoVHID is our
//! own KMDF bus on the Virtual HID Framework: every node it publishes is a real
//! HID device with the descriptor and feature reports the profile declares.

#![cfg(target_os = "windows")]

pub mod kernel;
pub mod pointer;
pub mod scan;
pub mod sendinput;
pub mod vhid;
pub mod vigem;

mod report;
mod runtime;

pub use runtime::Runtime;
