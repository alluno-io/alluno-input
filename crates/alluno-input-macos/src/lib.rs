//! The macOS host: keyboard, mouse and pen over Core Graphics events, and
//! gamepads and touch through the AllunoVHID DriverKit extension when it is
//! activated.
//!
//! macOS has no virtual HID device without a DriverKit extension, and shipping
//! one needs Apple's `com.apple.developer.driverkit.family.hid.device`
//! entitlement, notarization and a user approval flow. The extension's source
//! lives in `driver/macos`; until it is activated on a machine, a gamepad and
//! a touch digitiser answer `Unavailable` and say why.

#![cfg(target_os = "macos")]

pub mod cg;
pub mod keymap;
pub mod vhid;

use std::marker::PhantomData;
use std::sync::Arc;

use alluno_input_core::hid::PadCodec;
use alluno_input_core::{
    Backing, Capabilities, Error, Gamepad, GamepadProfile, Host, Keyboard, Mouse, Options,
    PASSTHROUGH_PENDING, Pen, Result, Touch,
};

use cg::{CgKeyboard, CgMouse, CgPen};
use vhid::{VhidKeyboard, VhidMouse, VhidPad, VhidPen, VhidTouch};

const NO_EXTENSION: &str =
    "the AllunoVHID DriverKit extension is not activated; it needs Apple's HID entitlement";
const NO_ACCESSIBILITY: &str = "the Accessibility permission is not granted; switch this app on \
     under System Settings, Privacy & Security, Accessibility";
const XINPUT_NONE: &str = "the Xbox 360 identity is XUSB, not HID; ask for XboxSeries";

/// The macOS host.
pub struct Input {
    _thread: PhantomData<*const ()>,
}

impl Input {
    /// The Core Graphics pen alone.
    pub fn user_pen(&self) -> Result<Box<dyn Pen>> {
        Ok(Box::new(CgPen::open()?))
    }

    /// The AllunoVHID pen node alone.
    pub fn bus_pen(&self) -> Result<Box<dyn Pen>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidPen::plug(bus)?))
    }

    /// An AllunoVHID keyboard node alone: a second keyboard identity.
    pub fn bus_keyboard(&self) -> Result<Box<dyn Keyboard>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidKeyboard::plug(bus)?))
    }

    /// An AllunoVHID mouse node alone: relative motion, buttons and wheels.
    pub fn bus_mouse(&self) -> Result<Box<dyn Mouse>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidMouse::plug(bus)?))
    }

    /// Whether the AllunoVHID extension is activated.
    pub fn vhid_installed() -> bool {
        vhid::installed()
    }

    /// Whether the system lets this process post keyboard, mouse and pen events.
    ///
    /// Without the permission every Core Graphics post is dropped silently, so
    /// [`Host::probe`] reports those three as unavailable until it is granted.
    /// The devices open either way; a grant takes effect on the next event.
    pub fn accessibility_trusted() -> bool {
        cg::accessibility_trusted()
    }

    /// Asks the system for that permission: the dialog that points at the
    /// Accessibility pane opens once, and this app is listed there for the user
    /// to switch on. Answers whether the permission is granted now.
    pub fn request_accessibility() -> bool {
        cg::request_accessibility()
    }
}

impl Host for Input {
    fn probe() -> Capabilities {
        let vhid = vhid::installed();
        let trusted = cg::accessibility_trusted();
        let user = || {
            if trusted {
                Backing::UserApi
            } else {
                Backing::Unavailable(NO_ACCESSIBILITY.to_string())
            }
        };
        let pad = |profile: GamepadProfile| {
            let backing = if !PadCodec::supports(profile) {
                Backing::Unavailable(XINPUT_NONE.to_string())
            } else if vhid {
                Backing::Bus
            } else {
                Backing::Unavailable(NO_EXTENSION.to_string())
            };
            (profile, backing)
        };
        Capabilities {
            keyboard: user(),
            mouse: user(),
            pen: user(),
            touch: if vhid {
                Backing::Bus
            } else {
                Backing::Unavailable(NO_EXTENSION.to_string())
            },
            gamepads: GamepadProfile::ALL.iter().map(|p| pad(*p)).collect(),
            max_gamepads: None,
            midi: Backing::Unavailable(PASSTHROUGH_PENDING.to_string()),
            camera: Backing::Unavailable(PASSTHROUGH_PENDING.to_string()),
            microphone: Backing::Unavailable(PASSTHROUGH_PENDING.to_string()),
        }
    }

    fn open(_options: Options) -> Result<Self> {
        Ok(Self {
            _thread: PhantomData,
        })
    }

    fn keyboard(&self) -> Result<Box<dyn Keyboard>> {
        Ok(Box::new(CgKeyboard::open()?))
    }

    fn mouse(&self) -> Result<Box<dyn Mouse>> {
        Ok(Box::new(CgMouse::open()?))
    }

    fn pen(&self) -> Result<Box<dyn Pen>> {
        self.user_pen()
    }

    fn touch(&self) -> Result<Box<dyn Touch>> {
        let bus = Arc::new(vhid::Bus::connect().map_err(|_| Error::unavailable(NO_EXTENSION))?);
        Ok(Box::new(VhidTouch::plug(bus)?))
    }

    fn gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>> {
        if !PadCodec::supports(profile) {
            return Err(Error::unavailable(XINPUT_NONE));
        }
        let bus = Arc::new(vhid::Bus::connect().map_err(|_| Error::unavailable(NO_EXTENSION))?);
        Ok(Box::new(VhidPad::plug(bus, profile)?))
    }
}
