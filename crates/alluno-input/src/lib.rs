//! Virtual input devices, one host per platform.
//!
//! ```no_run
//! use alluno_input::{Host, Key, Options, Input};
//!
//! let caps = Input::probe();
//! println!("keyboard: {:?}", caps.keyboard);
//!
//! let host = Input::open(Options::default())?;
//! let mut keyboard = host.keyboard()?;
//! keyboard.key(Key::A, true)?;
//! keyboard.key(Key::A, false)?;
//! # Ok::<(), alluno_input::Error>(())
//! ```
//!
//! The host and every device it hands out stay on the thread that opened it:
//!
//! ```compile_fail
//! fn assert_send<T: Send>() {}
//! assert_send::<alluno_input::Input>();
//! ```

pub use alluno_input_core::*;

#[cfg(target_os = "windows")]
pub use alluno_input_windows::Input;

#[cfg(target_os = "linux")]
pub use alluno_input_linux::Input;

#[cfg(target_os = "macos")]
pub use alluno_input_macos::Input;

/// What this machine can emulate, without opening anything.
pub fn probe() -> Capabilities {
    Input::probe()
}

/// [`probe`] under the name some callers expect.
pub fn check() -> Capabilities {
    Input::probe()
}
