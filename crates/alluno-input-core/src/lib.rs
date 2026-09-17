//! The alluno-input port.
//!
//! Everything a consumer touches: the device contracts, the state a device is
//! fed, the feedback it returns, and the capability answer a machine gives before
//! anything is opened. Nothing here knows a platform; the host crates implement
//! [`Host`] over their drivers and the `alluno-input` facade re-exports the one for the
//! target being built.
//!
//! Devices are deliberately `!Send`. Every OS binds an input handle to the
//! thread and desktop that created it (`SetThreadDesktop` and the AllunoInput control
//! devices on Windows, a `CGEventSource` on macOS), so a consumer opens the host
//! on the thread that will inject and keeps every device there.
//!
//! [`hid`] holds the report descriptors and codecs every bus-backed host
//! publishes, so a pad, pen or touch screen is the same device on every
//! platform.

mod capabilities;
mod device;
mod error;
mod gamepad;
pub mod hid;
mod host;
mod key;
mod mouse;
mod passthrough;
mod pen;
mod touch;

pub use capabilities::{Backing, Capabilities};
pub use device::{Gamepad, Keyboard, Mouse, Pen, Touch};
pub use error::{Error, Result};
pub use gamepad::{GamepadOutput, GamepadProfile, GamepadState, OutputSink, buttons};
pub use host::{Host, Options, PASSTHROUGH_PENDING};
pub use key::Key;
pub use mouse::MouseButton;
pub use passthrough::{AudioFrame, Camera, Microphone, Midi, PixelFormat, VideoFrame};
pub use pen::PenState;
pub use touch::{MAX_CONTACTS, TouchContact, TouchState};
