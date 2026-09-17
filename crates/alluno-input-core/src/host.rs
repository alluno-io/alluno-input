//! The one contract a platform host fulfils.

use crate::{
    Camera, Capabilities, Error, Gamepad, GamepadProfile, Keyboard, Microphone, Midi, Mouse, Pen,
    Result, Touch,
};

/// How a host is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    /// The name a bus-backed device announces itself under, where the bus lets it.
    pub device_name: Option<String>,
}

/// The answer a host gives for a passthrough device it has no backend for yet.
pub const PASSTHROUGH_PENDING: &str = "passthrough has no backend on this host yet";

/// A platform's host: probe what it can do, open it on the injecting thread,
/// hand out devices.
///
/// Implementors are `!Send` by construction; see the crate docs for why.
pub trait Host: Sized {
    /// Answers what this machine can emulate without opening anything.
    fn probe() -> Capabilities;

    /// [`Host::probe`] under the name some callers expect.
    fn check() -> Capabilities {
        Self::probe()
    }

    /// Opens the host on the calling thread.
    fn open(options: Options) -> Result<Self>;

    /// [`Host::open`] under the name some callers expect.
    fn new(options: Options) -> Result<Self> {
        Self::open(options)
    }

    fn keyboard(&self) -> Result<Box<dyn Keyboard>>;
    fn mouse(&self) -> Result<Box<dyn Mouse>>;
    fn pen(&self) -> Result<Box<dyn Pen>>;
    fn touch(&self) -> Result<Box<dyn Touch>>;
    fn gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>>;

    /// A MIDI port; a host without a backend answers [`Error::Unavailable`].
    fn midi(&self) -> Result<Box<dyn Midi>> {
        Err(Error::unavailable(PASSTHROUGH_PENDING))
    }

    /// A camera; a host without a backend answers [`Error::Unavailable`].
    fn camera(&self) -> Result<Box<dyn Camera>> {
        Err(Error::unavailable(PASSTHROUGH_PENDING))
    }

    /// A microphone; a host without a backend answers [`Error::Unavailable`].
    fn microphone(&self) -> Result<Box<dyn Microphone>> {
        Err(Error::unavailable(PASSTHROUGH_PENDING))
    }
}
