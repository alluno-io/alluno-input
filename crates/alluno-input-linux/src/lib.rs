//! The Linux host: virtual devices over `/dev/uinput` and `/dev/uhid`.
//!
//! Keyboard, mouse, pen and touch are plain uinput devices; the pen and touch
//! digitisers declare a resolution and `INPUT_PROP_DIRECT` so libinput takes
//! them for what they are. The Xbox 360 and DualShock 4 pads are uinput
//! devices carrying the VID, PID and version SDL keys its mappings on, with
//! force feedback arriving as `EV_FF` uploads. Every other pad profile is a
//! uhid node publishing the shared HID descriptor, so the kernel binds its own
//! controller driver and rumble arrives as HID output reports.

#![cfg(target_os = "linux")]

pub mod devices;
pub mod keymap;
pub mod uhid;
pub mod uhid_pad;
pub mod uinput;
pub mod uinput_pad;

mod report;
mod runtime;

pub use report::GamepadKind;
pub use runtime::Runtime;
