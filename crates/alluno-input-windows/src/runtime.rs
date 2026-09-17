//! The Windows [`Host`].

use std::marker::PhantomData;
use std::sync::Arc;

use alluno_input_core::hid::PadCodec;
use alluno_input_core::{
    Backing, Capabilities, Error, Gamepad, GamepadOutput, GamepadProfile, GamepadState, Host, Key,
    Keyboard, Mouse, MouseButton, Options, OutputSink, PASSTHROUGH_PENDING, Pen, Result, Touch,
};

use crate::kernel::{KernelKeyboard, KernelMouse, WHEEL_DELTA, mouse_button_flags};
use crate::pointer::{SyntheticPen, SyntheticTouch};
use crate::report::GamepadNotification;
use crate::scan;
use crate::sendinput::{UserKeyboard, UserMouse};
use crate::vhid::{self, BusXusbPad, VhidKeyboard, VhidMouse, VhidPad, VhidPen, VhidTouch};
use crate::vigem::{Ds4Target, GamepadBus, XusbTarget};

const KEYBOARD_MISSING: &str = "the AllunoInput keyboard filter is not installed";
const MOUSE_MISSING: &str = "the AllunoInput mouse filter is not installed";
const XUSB_MISSING: &str =
    "no Xbox 360 bus is installed: neither AllunoVHID nor a ViGEmBus-compatible bus";
const VHID_MISSING: &str = "the AllunoVHID bus is not installed";

/// XInput exposes four player slots and the bus refuses a fifth.
const XINPUT_SLOTS: u8 = 4;

/// The Windows runtime.
pub struct Runtime {
    options: Options,
    _thread: PhantomData<*const ()>,
}

impl Runtime {
    /// The kernel keyboard alone, for a host that layers it over the user one.
    pub fn kernel_keyboard(&self) -> Result<Box<dyn Keyboard>> {
        KernelKeyboard::open()
            .map(|device| Box::new(device) as Box<dyn Keyboard>)
            .ok_or_else(|| Error::unavailable(KEYBOARD_MISSING))
    }

    /// The `SendInput` keyboard alone.
    pub fn user_keyboard(&self) -> Result<Box<dyn Keyboard>> {
        Ok(Box::new(UserKeyboard::open()))
    }

    /// The kernel mouse alone.
    pub fn kernel_mouse(&self) -> Result<Box<dyn Mouse>> {
        KernelMouse::open()
            .map(|device| Box::new(device) as Box<dyn Mouse>)
            .ok_or_else(|| Error::unavailable(MOUSE_MISSING))
    }

    /// The `SendInput` mouse alone.
    pub fn user_mouse(&self) -> Result<Box<dyn Mouse>> {
        Ok(Box::new(UserMouse::open()))
    }

    /// An AllunoVHID keyboard node alone: its own device identity, for a host
    /// without the filter or with more than one seat.
    pub fn bus_keyboard(&self) -> Result<Box<dyn Keyboard>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidKeyboard::plug(bus)?))
    }

    /// An AllunoVHID mouse node alone: relative motion, buttons and wheels
    /// only, since a HID mouse cannot place the cursor across every monitor.
    pub fn bus_mouse(&self) -> Result<Box<dyn Mouse>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidMouse::plug(bus)?))
    }

    /// The AllunoVHID pen node alone.
    pub fn bus_pen(&self) -> Result<Box<dyn Pen>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidPen::plug(bus)?))
    }

    /// The Synthetic Pointer pen alone.
    pub fn user_pen(&self) -> Result<Box<dyn Pen>> {
        Ok(Box::new(SyntheticPen::open()?))
    }

    /// The AllunoVHID touch node alone.
    pub fn bus_touch(&self) -> Result<Box<dyn Touch>> {
        let bus = Arc::new(vhid::Bus::connect()?);
        Ok(Box::new(VhidTouch::plug(bus)?))
    }

    /// The Synthetic Pointer touch digitiser alone.
    pub fn user_touch(&self) -> Result<Box<dyn Touch>> {
        Ok(Box::new(SyntheticTouch::open()?))
    }

    /// Whether the AllunoVHID bus is installed.
    pub fn vhid_installed() -> bool {
        vhid::installed()
    }

    fn xusb_bus() -> Result<Arc<GamepadBus>> {
        GamepadBus::connect()
            .map(Arc::new)
            .map_err(|_| Error::unavailable(XUSB_MISSING))
    }
}

impl Host for Runtime {
    fn probe() -> Capabilities {
        let xusb = GamepadBus::connect().is_ok();
        let vhid = vhid::installed();
        let pad = |profile: GamepadProfile| {
            let backing = match profile {
                GamepadProfile::Xbox360 if vhid || xusb => Backing::Bus,
                GamepadProfile::Xbox360 => Backing::Unavailable(XUSB_MISSING.to_string()),
                GamepadProfile::DualShock4 if vhid || xusb => Backing::Bus,
                _ if vhid => Backing::Bus,
                _ => Backing::Unavailable(VHID_MISSING.to_string()),
            };
            (profile, backing)
        };
        let digitiser = if vhid { Backing::Bus } else { Backing::UserApi };
        Capabilities {
            keyboard: if !KernelKeyboard::available() && vhid {
                Backing::Bus
            } else {
                layered(KernelKeyboard::available())
            },
            mouse: layered(KernelMouse::available()),
            pen: digitiser.clone(),
            touch: digitiser,
            gamepads: GamepadProfile::ALL.iter().copied().map(pad).collect(),
            max_gamepads: Some(XINPUT_SLOTS),
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
        self.kernel_keyboard()
            .or_else(|_| self.bus_keyboard())
            .or_else(|_| self.user_keyboard())
    }

    fn mouse(&self) -> Result<Box<dyn Mouse>> {
        self.kernel_mouse().or_else(|_| self.user_mouse())
    }

    fn pen(&self) -> Result<Box<dyn Pen>> {
        self.bus_pen().or_else(|_| self.user_pen())
    }

    fn touch(&self) -> Result<Box<dyn Touch>> {
        self.bus_touch().or_else(|_| self.user_touch())
    }

    fn gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>> {
        let _ = &self.options;
        match profile {
            GamepadProfile::Xbox360 => match vhid::Bus::connect() {
                Ok(bus) => Ok(Box::new(BusXusbPad::plug(Arc::new(bus))?)),
                Err(_) => {
                    let target = XusbTarget::plugin(Self::xusb_bus()?).map_err(Error::backend)?;
                    let _ = target.wait_ready();
                    Ok(Box::new(XusbPad { target }))
                }
            },
            GamepadProfile::DualShock4 => match vhid::Bus::connect() {
                Ok(bus) => Ok(Box::new(VhidPad::plug(Arc::new(bus), profile)?)),
                Err(_) => {
                    let target = Ds4Target::plugin(Self::xusb_bus()?).map_err(Error::backend)?;
                    let _ = target.wait_ready();
                    Ok(Box::new(Ds4Pad { target }))
                }
            },
            other if PadCodec::supports(other) => {
                let bus = Arc::new(vhid::Bus::connect()?);
                Ok(Box::new(VhidPad::plug(bus, other)?))
            }
            other => Err(Error::unavailable(format!("{other:?} has no backend"))),
        }
    }
}

fn layered(kernel: bool) -> Backing {
    if kernel {
        Backing::Kernel
    } else {
        Backing::UserApi
    }
}

impl Keyboard for KernelKeyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        let (code, extended) = scan::of(key).ok_or(Error::Unsupported)?;
        let sent = if pressed {
            self.press(code, extended)
        } else {
            self.release(code, extended)
        };
        sent.map(drop).map_err(Error::backend)
    }
}

impl Mouse for KernelMouse {
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()> {
        KernelMouse::move_abs(self, x, y)
            .map(drop)
            .map_err(Error::backend)
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        KernelMouse::move_rel(self, dx, dy)
            .map(drop)
            .map_err(Error::backend)
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        use mouse_button_flags::*;
        let flags = match (button, pressed) {
            (MouseButton::Left, true) => LEFT_BUTTON_DOWN,
            (MouseButton::Left, false) => LEFT_BUTTON_UP,
            (MouseButton::Right, true) => RIGHT_BUTTON_DOWN,
            (MouseButton::Right, false) => RIGHT_BUTTON_UP,
            (MouseButton::Middle, true) => MIDDLE_BUTTON_DOWN,
            (MouseButton::Middle, false) => MIDDLE_BUTTON_UP,
            (MouseButton::Back, true) => BUTTON_4_DOWN,
            (MouseButton::Back, false) => BUTTON_4_UP,
            (MouseButton::Forward, true) => BUTTON_5_DOWN,
            (MouseButton::Forward, false) => BUTTON_5_UP,
            _ => return Err(Error::Unsupported),
        };
        self.buttons(flags).map(drop).map_err(Error::backend)
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        if dy != 0 {
            KernelMouse::wheel(self, notches(dy))
                .map(drop)
                .map_err(Error::backend)?;
        }
        if dx != 0 {
            KernelMouse::hwheel(self, notches(dx))
                .map(drop)
                .map_err(Error::backend)?;
        }
        Ok(())
    }
}

fn notches(count: i32) -> i16 {
    count
        .saturating_mul(i32::from(WHEEL_DELTA))
        .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn deliver(sink: &mut OutputSink, notification: GamepadNotification) {
    sink(GamepadOutput::Rumble {
        large: notification.large_motor,
        small: notification.small_motor,
    });
    sink(GamepadOutput::PlayerLed(notification.led_number));
}

struct XusbPad {
    target: XusbTarget,
}

impl Gamepad for XusbPad {
    fn submit(&mut self, state: &GamepadState) -> Result<()> {
        self.target.update(state).map_err(Error::backend)
    }

    fn on_output(&mut self, mut sink: OutputSink) -> Result<()> {
        self.target
            .spawn_notification(move |n| deliver(&mut sink, n))
            .map_err(Error::backend)
    }

    fn slot(&mut self) -> Option<u8> {
        self.target.get_user_index().ok().map(|i| i as u8)
    }
}

struct Ds4Pad {
    target: Ds4Target,
}

impl Gamepad for Ds4Pad {
    fn submit(&mut self, state: &GamepadState) -> Result<()> {
        self.target.update(state).map_err(Error::backend)
    }

    fn on_output(&mut self, mut sink: OutputSink) -> Result<()> {
        self.target
            .spawn_notification(move |n| deliver(&mut sink, n))
            .map_err(Error::backend)
    }

    fn slot(&mut self) -> Option<u8> {
        None
    }
}
