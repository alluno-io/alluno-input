//! The Linux [`Host`].

use std::marker::PhantomData;

use alluno_input_core::hid::PadCodec;
use alluno_input_core::{
    Backing, Capabilities, Error, Gamepad, GamepadOutput, GamepadProfile, GamepadState, Host,
    Keyboard, Mouse, Options, OutputSink, PASSTHROUGH_PENDING, Pen, Result, Touch,
};

use crate::devices::{UinputKeyboard, UinputMouse, UinputPen, UinputTouch};
use crate::report::GamepadKind;
use crate::uhid;
use crate::uhid_pad::UhidGamepad;
use crate::uinput;
use crate::uinput_pad::UinputGamepad;
use crate::x11::{self, X11Keyboard, X11Mouse};

const UINPUT_MISSING: &str =
    "/dev/uinput is not writable; load the uinput module and join the input group";
const UHID_MISSING: &str = "/dev/uhid is not writable; load the uhid module or grant access to it";
const DEFAULT_NAME: &str = "Alluno Virtual";

/// The Linux host.
pub struct Input {
    options: Options,
    _thread: PhantomData<*const ()>,
}

impl Input {
    fn name(&self, kind: &str) -> String {
        match &self.options.device_name {
            Some(name) => format!("{name} {kind}"),
            None => format!("{DEFAULT_NAME} {kind}"),
        }
    }

    fn require_uinput() -> Result<()> {
        if uinput::available() {
            Ok(())
        } else {
            Err(Error::unavailable(UINPUT_MISSING))
        }
    }

    /// The uinput keyboard, a device the whole machine sees.
    pub fn bus_keyboard(&self) -> Result<Box<dyn Keyboard>> {
        Self::require_uinput()?;
        Ok(Box::new(UinputKeyboard::open(&self.name("Keyboard"))?))
    }

    /// The uinput mouse, a device the whole machine sees.
    pub fn bus_mouse(&self) -> Result<Box<dyn Mouse>> {
        Self::require_uinput()?;
        Ok(Box::new(UinputMouse::open(&self.name("Mouse"))?))
    }

    /// The XTest keyboard, events that exist only inside the X server in `DISPLAY`.
    pub fn user_keyboard(&self) -> Result<Box<dyn Keyboard>> {
        Ok(Box::new(
            X11Keyboard::open().map_err(|_| Error::unavailable(x11::X11_MISSING))?,
        ))
    }

    /// The XTest mouse, events that exist only inside the X server in `DISPLAY`.
    pub fn user_mouse(&self) -> Result<Box<dyn Mouse>> {
        Ok(Box::new(
            X11Mouse::open().map_err(|_| Error::unavailable(x11::X11_MISSING))?,
        ))
    }

    fn require_uhid() -> Result<()> {
        if uhid::available() {
            Ok(())
        } else {
            Err(Error::unavailable(UHID_MISSING))
        }
    }

    /// A pad on `uhid` for any HID profile, DualShock 4 included, so the
    /// kernel's own controller driver binds to the exact device.
    pub fn uhid_gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>> {
        if !PadCodec::supports(profile) {
            return Err(Error::Unsupported);
        }
        Self::require_uhid()?;
        Ok(Box::new(UhidGamepad::create(
            &self.name("Gamepad"),
            profile,
        )?))
    }

    /// A pad on `uinput` for one of the two evdev identities.
    pub fn uinput_gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>> {
        let kind = match profile {
            GamepadProfile::Xbox360 => GamepadKind::Xbox360,
            GamepadProfile::DualShock4 => GamepadKind::DualShock4,
            _ => return Err(Error::Unsupported),
        };
        Self::require_uinput()?;
        let pad = UinputGamepad::create(&self.name("Gamepad"), kind).map_err(Error::backend)?;
        Ok(Box::new(UinputPad { pad }))
    }
}

impl Host for Input {
    fn probe() -> Capabilities {
        let uinput = uinput::available();
        let uhid = uhid::available();
        let backing = || {
            if uinput {
                Backing::Bus
            } else {
                Backing::Unavailable(UINPUT_MISSING.to_string())
            }
        };
        let pointer = || {
            if uinput {
                Backing::Bus
            } else if x11::available() {
                Backing::UserApi
            } else {
                Backing::Unavailable(format!("{UINPUT_MISSING}; {}", x11::X11_MISSING))
            }
        };
        let pad = |profile: GamepadProfile| {
            let backing = match profile {
                GamepadProfile::Xbox360 | GamepadProfile::DualShock4 if uinput => Backing::Bus,
                GamepadProfile::Xbox360 => Backing::Unavailable(UINPUT_MISSING.to_string()),
                _ if PadCodec::supports(profile) && uhid => Backing::Bus,
                _ => Backing::Unavailable(UHID_MISSING.to_string()),
            };
            (profile, backing)
        };
        Capabilities {
            keyboard: pointer(),
            mouse: pointer(),
            pen: backing(),
            touch: backing(),
            gamepads: GamepadProfile::ALL.iter().copied().map(pad).collect(),
            max_gamepads: None,
            midi: Backing::Unavailable(PASSTHROUGH_PENDING.to_string()),
            camera: Backing::Unavailable(PASSTHROUGH_PENDING.to_string()),
            microphone: Backing::Unavailable(PASSTHROUGH_PENDING.to_string()),
        }
    }

    fn open(options: Options) -> Result<Self> {
        Ok(Self {
            options,
            _thread: PhantomData,
        })
    }

    fn keyboard(&self) -> Result<Box<dyn Keyboard>> {
        match self.bus_keyboard() {
            Err(Error::Unavailable(_)) => self.user_keyboard(),
            other => other,
        }
    }

    fn mouse(&self) -> Result<Box<dyn Mouse>> {
        match self.bus_mouse() {
            Err(Error::Unavailable(_)) => self.user_mouse(),
            other => other,
        }
    }

    fn pen(&self) -> Result<Box<dyn Pen>> {
        Self::require_uinput()?;
        Ok(Box::new(UinputPen::open(&self.name("Pen"))?))
    }

    fn touch(&self) -> Result<Box<dyn Touch>> {
        Self::require_uinput()?;
        Ok(Box::new(UinputTouch::open(&self.name("Touch"))?))
    }

    fn gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>> {
        match profile {
            GamepadProfile::Xbox360 | GamepadProfile::DualShock4 => self.uinput_gamepad(profile),
            other => self.uhid_gamepad(other),
        }
    }
}

struct UinputPad {
    pad: UinputGamepad,
}

impl Gamepad for UinputPad {
    fn submit(&mut self, state: &GamepadState) -> Result<()> {
        self.pad.update(state).map_err(Error::backend)
    }

    fn on_output(&mut self, mut sink: OutputSink) -> Result<()> {
        self.pad
            .spawn_notification(move |n| {
                sink(GamepadOutput::Rumble {
                    large: n.large_motor,
                    small: n.small_motor,
                });
            })
            .map_err(Error::backend)
    }

    fn slot(&mut self) -> Option<u8> {
        None
    }
}
