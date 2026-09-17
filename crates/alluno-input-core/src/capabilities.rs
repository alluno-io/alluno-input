//! What a machine can emulate, answered before anything is opened.

use crate::GamepadProfile;

/// How a device kind is backed on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Backing {
    /// A kernel driver: the OS cannot tell the input from hardware.
    Kernel,
    /// A bus or virtual-device driver that publishes a real device node.
    Bus,
    /// A user-mode injection API; visible to software that looks.
    UserApi,
    /// Nothing on this machine can do it; the message says what is missing.
    Unavailable(String),
}

impl Backing {
    /// Whether the device kind can be opened at all.
    pub fn available(&self) -> bool {
        !matches!(self, Backing::Unavailable(_))
    }
}

/// The capability answer for one machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub keyboard: Backing,
    pub mouse: Backing,
    pub pen: Backing,
    pub touch: Backing,
    /// One entry per [`GamepadProfile`], available or not.
    pub gamepads: Vec<(GamepadProfile, Backing)>,
    /// How many pads may be plugged at once, where the bus has a cap.
    pub max_gamepads: Option<u8>,
    pub midi: Backing,
    pub camera: Backing,
    pub microphone: Backing,
}

impl Capabilities {
    /// How a profile is backed here.
    pub fn gamepad(&self, profile: GamepadProfile) -> &Backing {
        static NONE: Backing = Backing::Unavailable(String::new());
        self.gamepads
            .iter()
            .find(|(p, _)| *p == profile)
            .map(|(_, b)| b)
            .unwrap_or(&NONE)
    }

    /// Every profile that can be plugged here.
    pub fn available_gamepads(&self) -> Vec<GamepadProfile> {
        self.gamepads
            .iter()
            .filter(|(_, b)| b.available())
            .map(|(p, _)| *p)
            .collect()
    }
}
