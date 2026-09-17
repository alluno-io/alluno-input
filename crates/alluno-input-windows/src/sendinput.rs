//! The user-mode layer: `SendInput` with scan codes, Unicode text and
//! virtual-desktop mouse packets.
//!
//! Everything sent this way carries `LLKHF_INJECTED`, which is why the kernel
//! filter exists; this is the layer for a machine without it, and for the two
//! things the filter cannot do: type a character by code point, and press
//! `Pause`, which is an E1 sequence rather than a stroke.

use std::marker::PhantomData;
use std::mem::size_of;

use alluno_input_core::{Error, Key, Keyboard, Mouse, MouseButton, Result};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
    MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SendInput, VIRTUAL_KEY,
};

use crate::kernel::WHEEL_DELTA;
use crate::scan;

/// The virtual key `Pause` is sent as, since it has no single scan code.
const VK_PAUSE: VIRTUAL_KEY = VIRTUAL_KEY(0x13);
const XBUTTON1: i32 = 1;
const XBUTTON2: i32 = 2;

/// What one keyboard call turns into, before it is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStroke {
    /// The scan code, or 0 when the key goes by virtual key instead.
    pub scan: u16,
    /// The virtual key, or 0 when the key goes by scan code.
    pub vk: u16,
    pub extended: bool,
    pub up: bool,
}

/// How a key is spelled to `SendInput`; `None` for a key neither table knows.
pub fn stroke_for(key: Key, pressed: bool) -> Option<KeyStroke> {
    if key == Key::Pause {
        return Some(KeyStroke {
            scan: 0,
            vk: VK_PAUSE.0,
            extended: false,
            up: !pressed,
        });
    }
    let (scan, extended) = scan::of(key)?;
    Some(KeyStroke {
        scan,
        vk: 0,
        extended,
        up: !pressed,
    })
}

/// The mouse packet one call turns into, before it is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MousePacket {
    pub dx: i32,
    pub dy: i32,
    pub data: i32,
    pub flags: u32,
}

/// An absolute move across the virtual desktop, 0..=65535 on each axis.
pub fn packet_move_abs(x: u16, y: u16) -> MousePacket {
    MousePacket {
        dx: i32::from(x),
        dy: i32::from(y),
        data: 0,
        flags: (MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK).0,
    }
}

/// A relative move in device units.
pub fn packet_move_rel(dx: i32, dy: i32) -> MousePacket {
    MousePacket {
        dx,
        dy,
        data: 0,
        flags: MOUSEEVENTF_MOVE.0,
    }
}

/// A button transition.
pub fn packet_button(button: MouseButton, pressed: bool) -> Option<MousePacket> {
    let (flags, data) = match (button, pressed) {
        (MouseButton::Left, true) => (MOUSEEVENTF_LEFTDOWN, 0),
        (MouseButton::Left, false) => (MOUSEEVENTF_LEFTUP, 0),
        (MouseButton::Right, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
        (MouseButton::Right, false) => (MOUSEEVENTF_RIGHTUP, 0),
        (MouseButton::Middle, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
        (MouseButton::Middle, false) => (MOUSEEVENTF_MIDDLEUP, 0),
        (MouseButton::Back, true) => (MOUSEEVENTF_XDOWN, XBUTTON1),
        (MouseButton::Back, false) => (MOUSEEVENTF_XUP, XBUTTON1),
        (MouseButton::Forward, true) => (MOUSEEVENTF_XDOWN, XBUTTON2),
        (MouseButton::Forward, false) => (MOUSEEVENTF_XUP, XBUTTON2),
        _ => return None,
    };
    Some(MousePacket {
        dx: 0,
        dy: 0,
        data,
        flags: flags.0,
    })
}

/// A wheel turn of `notches`, vertical when `vertical`, else horizontal.
pub fn packet_wheel(notches: i32, vertical: bool) -> MousePacket {
    MousePacket {
        dx: 0,
        dy: 0,
        data: notches
            .saturating_mul(i32::from(WHEEL_DELTA))
            .clamp(i32::from(i16::MIN), i32::from(i16::MAX)),
        flags: if vertical {
            MOUSEEVENTF_WHEEL.0
        } else {
            MOUSEEVENTF_HWHEEL.0
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<()> {
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(Error::backend(format!(
            "SendInput accepted {sent} of {} events",
            inputs.len()
        )))
    }
}

fn keyboard_input(scan: u16, vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_input(packet: MousePacket) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: packet.dx,
                dy: packet.dy,
                mouseData: packet.data as u32,
                dwFlags: MOUSE_EVENT_FLAGS(packet.flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// A keyboard over `SendInput`.
pub struct UserKeyboard {
    _thread: PhantomData<*const ()>,
}

impl UserKeyboard {
    /// Always available on Windows.
    pub fn open() -> Self {
        Self {
            _thread: PhantomData,
        }
    }
}

impl Keyboard for UserKeyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        let stroke = stroke_for(key, pressed).ok_or(Error::Unsupported)?;
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if stroke.vk == 0 {
            flags |= KEYEVENTF_SCANCODE;
        }
        if stroke.extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if stroke.up {
            flags |= KEYEVENTF_KEYUP;
        }
        send(&[keyboard_input(stroke.scan, stroke.vk, flags)])
    }

    fn text(&mut self, text: &str) -> Result<()> {
        let mut inputs = Vec::with_capacity(text.len() * 2);
        for unit in text.encode_utf16() {
            inputs.push(keyboard_input(unit, 0, KEYEVENTF_UNICODE));
            inputs.push(keyboard_input(unit, 0, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
        if inputs.is_empty() {
            return Ok(());
        }
        send(&inputs)
    }
}

/// A mouse over `SendInput`, in the same 0..=65535 virtual-desktop space the
/// kernel filter uses.
pub struct UserMouse {
    _thread: PhantomData<*const ()>,
}

impl UserMouse {
    /// Always available on Windows.
    pub fn open() -> Self {
        Self {
            _thread: PhantomData,
        }
    }
}

impl Mouse for UserMouse {
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()> {
        send(&[mouse_input(packet_move_abs(x, y))])
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        send(&[mouse_input(packet_move_rel(dx, dy))])
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let packet = packet_button(button, pressed).ok_or(Error::Unsupported)?;
        send(&[mouse_input(packet)])
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        let mut inputs = Vec::with_capacity(2);
        if dy != 0 {
            inputs.push(mouse_input(packet_wheel(dy, true)));
        }
        if dx != 0 {
            inputs.push(mouse_input(packet_wheel(dx, false)));
        }
        if inputs.is_empty() {
            return Ok(());
        }
        send(&inputs)
    }
}
