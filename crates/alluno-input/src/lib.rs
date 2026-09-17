//! Virtual input devices, one runtime per platform.
//!
//! ```no_run
//! use alluno_input::{Host, Key, Options, Runtime};
//!
//! let caps = Runtime::probe();
//! println!("keyboard: {:?}", caps.keyboard);
//!
//! let host = Runtime::open(Options::default())?;
//! let mut keyboard = host.keyboard()?;
//! keyboard.key(Key::A, true)?;
//! keyboard.key(Key::A, false)?;
//! # Ok::<(), alluno_input::Error>(())
//! ```
//!
//! The runtime and every device it hands out stay on the thread that opened it:
//!
//! ```compile_fail
//! fn assert_send<T: Send>() {}
//! assert_send::<alluno_input::Runtime>();
//! ```

pub use alluno_input_core::*;

#[cfg(target_os = "windows")]
pub use alluno_input_windows::Runtime;

#[cfg(target_os = "linux")]
pub use alluno_input_linux::Runtime;

#[cfg(target_os = "macos")]
pub use alluno_input_macos::Runtime;

/// What this machine can emulate, without opening anything.
pub fn probe() -> Capabilities {
    Runtime::probe()
}
