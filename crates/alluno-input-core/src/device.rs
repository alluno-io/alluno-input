//! The device contracts a host hands out.
//!
//! None of these is `Send`: a device lives on the thread that opened the host.

use crate::{GamepadState, Key, MouseButton, OutputSink, PenState, Result, TouchState};

/// A keyboard the OS believes in.
pub trait Keyboard {
    /// Presses (`true`) or releases (`false`) a key.
    fn key(&mut self, key: Key, pressed: bool) -> Result<()>;

    /// Types literal text through the OS text path where one exists.
    ///
    /// The default answers [`crate::Error::Unsupported`], which is what a
    /// kernel-level keyboard says: it has scan codes, not characters.
    fn text(&mut self, text: &str) -> Result<()> {
        let _ = text;
        Err(crate::Error::Unsupported)
    }
}

/// A mouse the OS believes in.
pub trait Mouse {
    /// Moves to an absolute position in the driver's 0..=65535 space.
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()>;

    /// Moves by a delta in device units.
    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()>;

    /// Presses (`true`) or releases (`false`) a button.
    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()>;

    /// Scrolls by whole notches, horizontal then vertical; positive is right and up.
    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()>;
}

/// A stylus the OS believes in.
pub trait Pen {
    /// Reports one sample; the backend derives hover, contact and lift from consecutive samples.
    fn report(&mut self, state: &PenState) -> Result<()>;
}

/// A touch digitiser the OS believes in.
pub trait Touch {
    /// Reports every current contact; a finger absent from a report is lifted.
    fn report(&mut self, state: &TouchState) -> Result<()>;
}

/// A game controller the OS believes in.
pub trait Gamepad {
    /// Applies one full snapshot.
    fn submit(&mut self, state: &GamepadState) -> Result<()>;

    /// Installs where rumble, lamps and raw reports from the game are delivered.
    fn on_output(&mut self, sink: OutputSink) -> Result<()>;

    /// The player slot the OS assigned, 0-based, where the profile has one.
    fn slot(&mut self) -> Option<u8>;
}
