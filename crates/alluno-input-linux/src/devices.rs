//! The uinput keyboard, mouse, pen and touch devices.

use std::marker::PhantomData;

use alluno_input_core::{
    Error, Key, Keyboard, MAX_CONTACTS, Mouse, MouseButton, Pen, PenState, Result, Touch,
    TouchState,
};

use crate::keymap;
use crate::uinput::{self, Axis, Device, InputId};

const KEYBOARD_PRODUCT: u16 = 0x5679;
const MOUSE_PRODUCT: u16 = 0x5678;
const PEN_PRODUCT: u16 = 0x5680;
const TOUCH_PRODUCT: u16 = 0x5681;

/// The absolute range every pointing device declares, so the port's 0..=65535
/// lands on the device unchanged.
pub const ABS_MAX: i32 = 65535;

/// The pressure range a tablet declares.
pub const PRESSURE_MAX: i32 = 8191;

/// Units per millimetre a tablet declares; libinput refuses one without a resolution.
const TABLET_RESOLUTION: i32 = 200;

/// A keyboard over uinput.
pub struct UinputKeyboard {
    device: Device,
    _thread: PhantomData<*const ()>,
}

impl UinputKeyboard {
    /// Creates the device with every key the map knows.
    pub fn open(name: &str) -> Result<Self> {
        let device = Device::open()?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_KEY)?;
        for key in Key::ALL {
            device.set_bit(uinput::UI_SET_KEYBIT, keymap::to_evdev(*key))?;
        }
        device.create(
            name,
            InputId {
                bustype: uinput::BUS_USB,
                vendor: uinput::VENDOR,
                product: KEYBOARD_PRODUCT,
                version: 1,
            },
        )?;
        uinput::settle();
        Ok(Self {
            device,
            _thread: PhantomData,
        })
    }
}

impl Keyboard for UinputKeyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        self.device
            .emit(uinput::EV_KEY, keymap::to_evdev(key), i32::from(pressed))?;
        self.device.sync()
    }
}

/// A mouse over uinput, absolute across the whole desktop and relative in device units.
pub struct UinputMouse {
    device: Device,
    _thread: PhantomData<*const ()>,
}

impl UinputMouse {
    /// Creates the device.
    pub fn open(name: &str) -> Result<Self> {
        let device = Device::open()?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_KEY)?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_REL)?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_ABS)?;
        for button in [
            uinput::BTN_LEFT,
            uinput::BTN_RIGHT,
            uinput::BTN_MIDDLE,
            uinput::BTN_SIDE,
            uinput::BTN_EXTRA,
        ] {
            device.set_bit(uinput::UI_SET_KEYBIT, button)?;
        }
        for axis in [
            uinput::REL_X,
            uinput::REL_Y,
            uinput::REL_WHEEL,
            uinput::REL_HWHEEL,
        ] {
            device.set_bit(uinput::UI_SET_RELBIT, axis)?;
        }
        for code in [uinput::ABS_X, uinput::ABS_Y] {
            device.abs_setup(Axis {
                code,
                min: 0,
                max: ABS_MAX,
                resolution: 0,
            })?;
        }
        device.create(
            name,
            InputId {
                bustype: uinput::BUS_USB,
                vendor: uinput::VENDOR,
                product: MOUSE_PRODUCT,
                version: 1,
            },
        )?;
        uinput::settle();
        Ok(Self {
            device,
            _thread: PhantomData,
        })
    }
}

/// The evdev button for a mouse button.
pub fn button_code(button: MouseButton) -> Option<u16> {
    Some(match button {
        MouseButton::Left => uinput::BTN_LEFT,
        MouseButton::Right => uinput::BTN_RIGHT,
        MouseButton::Middle => uinput::BTN_MIDDLE,
        MouseButton::Back => uinput::BTN_SIDE,
        MouseButton::Forward => uinput::BTN_EXTRA,
        _ => return None,
    })
}

impl Mouse for UinputMouse {
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()> {
        self.device
            .emit(uinput::EV_ABS, uinput::ABS_X, i32::from(x))?;
        self.device
            .emit(uinput::EV_ABS, uinput::ABS_Y, i32::from(y))?;
        self.device.sync()
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        self.device.emit(uinput::EV_REL, uinput::REL_X, dx)?;
        self.device.emit(uinput::EV_REL, uinput::REL_Y, dy)?;
        self.device.sync()
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let code = button_code(button).ok_or(Error::Unsupported)?;
        self.device.emit(uinput::EV_KEY, code, i32::from(pressed))?;
        self.device.sync()
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        if dx != 0 {
            self.device.emit(uinput::EV_REL, uinput::REL_HWHEEL, dx)?;
        }
        if dy != 0 {
            self.device.emit(uinput::EV_REL, uinput::REL_WHEEL, dy)?;
        }
        self.device.sync()
    }
}

/// Tablet pressure from the port's 0..=65535.
pub fn tablet_pressure(pressure: u16) -> i32 {
    (i32::from(pressure) * PRESSURE_MAX) / 65535
}

/// A stylus over uinput, presented as a direct-input tablet so libinput accepts it.
pub struct UinputPen {
    device: Device,
    last: Option<PenState>,
    _thread: PhantomData<*const ()>,
}

impl UinputPen {
    /// Creates the tablet.
    pub fn open(name: &str) -> Result<Self> {
        let device = Device::open()?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_SYN)?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_KEY)?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_ABS)?;
        for button in [
            uinput::BTN_TOUCH,
            uinput::BTN_STYLUS,
            uinput::BTN_STYLUS2,
            uinput::BTN_TOOL_PEN,
            uinput::BTN_TOOL_RUBBER,
        ] {
            device.set_bit(uinput::UI_SET_KEYBIT, button)?;
        }
        for axis in [
            Axis {
                code: uinput::ABS_X,
                min: 0,
                max: ABS_MAX,
                resolution: TABLET_RESOLUTION,
            },
            Axis {
                code: uinput::ABS_Y,
                min: 0,
                max: ABS_MAX,
                resolution: TABLET_RESOLUTION,
            },
            Axis {
                code: uinput::ABS_PRESSURE,
                min: 0,
                max: PRESSURE_MAX,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_DISTANCE,
                min: 0,
                max: 63,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_TILT_X,
                min: -90,
                max: 90,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_TILT_Y,
                min: -90,
                max: 90,
                resolution: 0,
            },
        ] {
            device.abs_setup(axis)?;
        }
        device.set_bit(uinput::UI_SET_PROPBIT, uinput::INPUT_PROP_DIRECT)?;
        device.create(
            name,
            InputId {
                bustype: uinput::BUS_VIRTUAL,
                vendor: uinput::VENDOR,
                product: PEN_PRODUCT,
                version: 1,
            },
        )?;
        uinput::settle();
        Ok(Self {
            device,
            last: None,
            _thread: PhantomData,
        })
    }
}

impl Pen for UinputPen {
    fn report(&mut self, state: &PenState) -> Result<()> {
        let was = self.last.unwrap_or_default();
        let tool = if state.eraser {
            uinput::BTN_TOOL_RUBBER
        } else {
            uinput::BTN_TOOL_PEN
        };
        let was_tool = if was.eraser {
            uinput::BTN_TOOL_RUBBER
        } else {
            uinput::BTN_TOOL_PEN
        };
        let in_range = state.in_range || state.down;
        let was_in_range = self.last.is_some() && (was.in_range || was.down);
        if was_in_range && was_tool != tool {
            self.device.emit(uinput::EV_KEY, was_tool, 0)?;
        }
        if in_range && (!was_in_range || was_tool != tool) {
            self.device.emit(uinput::EV_KEY, tool, 1)?;
        }
        self.device
            .emit(uinput::EV_ABS, uinput::ABS_X, i32::from(state.x))?;
        self.device
            .emit(uinput::EV_ABS, uinput::ABS_Y, i32::from(state.y))?;
        self.device.emit(
            uinput::EV_ABS,
            uinput::ABS_PRESSURE,
            if state.down {
                tablet_pressure(state.pressure)
            } else {
                0
            },
        )?;
        self.device.emit(
            uinput::EV_ABS,
            uinput::ABS_DISTANCE,
            if state.down { 0 } else { 10 },
        )?;
        self.device
            .emit(uinput::EV_ABS, uinput::ABS_TILT_X, i32::from(state.tilt_x))?;
        self.device
            .emit(uinput::EV_ABS, uinput::ABS_TILT_Y, i32::from(state.tilt_y))?;
        if state.down != was.down {
            self.device
                .emit(uinput::EV_KEY, uinput::BTN_TOUCH, i32::from(state.down))?;
        }
        if state.barrel != was.barrel {
            self.device
                .emit(uinput::EV_KEY, uinput::BTN_STYLUS, i32::from(state.barrel))?;
        }
        if !in_range && was_in_range {
            self.device.emit(uinput::EV_KEY, tool, 0)?;
        }
        self.device.sync()?;
        self.last = Some(*state);
        Ok(())
    }
}

/// Which slot each live contact id occupies, protocol B style.
#[derive(Debug, Default)]
pub struct Slots {
    ids: [Option<u8>; MAX_CONTACTS],
}

impl Slots {
    /// The slot for `id`, claiming a free one when it is new; `None` when all are taken.
    pub fn claim(&mut self, id: u8) -> Option<usize> {
        if let Some(slot) = self.ids.iter().position(|s| *s == Some(id)) {
            return Some(slot);
        }
        let free = self.ids.iter().position(|s| s.is_none())?;
        self.ids[free] = Some(id);
        Some(free)
    }

    /// The slot for `id`, without claiming.
    pub fn find(&self, id: u8) -> Option<usize> {
        self.ids.iter().position(|s| *s == Some(id))
    }

    /// Frees the slot for `id`.
    pub fn release(&mut self, id: u8) {
        if let Some(slot) = self.find(id) {
            self.ids[slot] = None;
        }
    }

    /// Every live id.
    pub fn live(&self) -> Vec<u8> {
        self.ids.iter().flatten().copied().collect()
    }
}

/// A touch digitiser over uinput, multitouch protocol B.
pub struct UinputTouch {
    device: Device,
    slots: Slots,
    _thread: PhantomData<*const ()>,
}

impl UinputTouch {
    /// Creates the digitiser.
    pub fn open(name: &str) -> Result<Self> {
        let device = Device::open()?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_SYN)?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_KEY)?;
        device.set_bit(uinput::UI_SET_EVBIT, uinput::EV_ABS)?;
        device.set_bit(uinput::UI_SET_KEYBIT, uinput::BTN_TOUCH)?;
        for axis in [
            Axis {
                code: uinput::ABS_MT_SLOT,
                min: 0,
                max: MAX_CONTACTS as i32 - 1,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_MT_TRACKING_ID,
                min: 0,
                max: 65535,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_MT_POSITION_X,
                min: 0,
                max: ABS_MAX,
                resolution: TABLET_RESOLUTION,
            },
            Axis {
                code: uinput::ABS_MT_POSITION_Y,
                min: 0,
                max: ABS_MAX,
                resolution: TABLET_RESOLUTION,
            },
            Axis {
                code: uinput::ABS_MT_PRESSURE,
                min: 0,
                max: PRESSURE_MAX,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_MT_TOUCH_MAJOR,
                min: 0,
                max: ABS_MAX,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_MT_TOUCH_MINOR,
                min: 0,
                max: ABS_MAX,
                resolution: 0,
            },
            Axis {
                code: uinput::ABS_X,
                min: 0,
                max: ABS_MAX,
                resolution: TABLET_RESOLUTION,
            },
            Axis {
                code: uinput::ABS_Y,
                min: 0,
                max: ABS_MAX,
                resolution: TABLET_RESOLUTION,
            },
        ] {
            device.abs_setup(axis)?;
        }
        device.set_bit(uinput::UI_SET_PROPBIT, uinput::INPUT_PROP_DIRECT)?;
        device.create(
            name,
            InputId {
                bustype: uinput::BUS_VIRTUAL,
                vendor: uinput::VENDOR,
                product: TOUCH_PRODUCT,
                version: 1,
            },
        )?;
        uinput::settle();
        Ok(Self {
            device,
            slots: Slots::default(),
            _thread: PhantomData,
        })
    }
}

impl Touch for UinputTouch {
    fn report(&mut self, state: &TouchState) -> Result<()> {
        if !state.fits() {
            return Err(Error::Unsupported);
        }
        let before = self.slots.live();
        for contact in &state.contacts {
            if contact.down {
                let Some(slot) = self.slots.claim(contact.id) else {
                    continue;
                };
                self.device
                    .emit(uinput::EV_ABS, uinput::ABS_MT_SLOT, slot as i32)?;
                self.device.emit(
                    uinput::EV_ABS,
                    uinput::ABS_MT_TRACKING_ID,
                    i32::from(contact.id),
                )?;
                self.device.emit(
                    uinput::EV_ABS,
                    uinput::ABS_MT_POSITION_X,
                    i32::from(contact.x),
                )?;
                self.device.emit(
                    uinput::EV_ABS,
                    uinput::ABS_MT_POSITION_Y,
                    i32::from(contact.y),
                )?;
                self.device.emit(
                    uinput::EV_ABS,
                    uinput::ABS_MT_PRESSURE,
                    tablet_pressure(contact.pressure),
                )?;
                self.device.emit(
                    uinput::EV_ABS,
                    uinput::ABS_MT_TOUCH_MAJOR,
                    i32::from(contact.width.max(contact.height)),
                )?;
                self.device.emit(
                    uinput::EV_ABS,
                    uinput::ABS_MT_TOUCH_MINOR,
                    i32::from(contact.width.min(contact.height)),
                )?;
            } else if let Some(slot) = self.slots.find(contact.id) {
                self.device
                    .emit(uinput::EV_ABS, uinput::ABS_MT_SLOT, slot as i32)?;
                self.device
                    .emit(uinput::EV_ABS, uinput::ABS_MT_TRACKING_ID, -1)?;
                self.slots.release(contact.id);
            }
        }
        for id in before {
            if !state.contacts.iter().any(|c| c.id == id)
                && let Some(slot) = self.slots.find(id)
            {
                self.device
                    .emit(uinput::EV_ABS, uinput::ABS_MT_SLOT, slot as i32)?;
                self.device
                    .emit(uinput::EV_ABS, uinput::ABS_MT_TRACKING_ID, -1)?;
                self.slots.release(id);
            }
        }
        let any_down = !self.slots.live().is_empty();
        if let Some(first) = state.contacts.iter().find(|c| c.down) {
            self.device
                .emit(uinput::EV_ABS, uinput::ABS_X, i32::from(first.x))?;
            self.device
                .emit(uinput::EV_ABS, uinput::ABS_Y, i32::from(first.y))?;
        }
        self.device
            .emit(uinput::EV_KEY, uinput::BTN_TOUCH, i32::from(any_down))?;
        self.device.sync()
    }
}
