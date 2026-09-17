//! The user-space layer for X11: XTest fake input on the display in `DISPLAY`.
//!
//! This is the path for a machine, or a container, whose `/dev/uinput` is out
//! of reach: the events exist only inside that X server, which is what keeps a
//! container's injection off the host. Keys go by keysym through the server's
//! own keyboard mapping, and a character the mapping lacks borrows a spare
//! keycode for the length of one tap. Wayland compositors do not take XTest
//! input, so the probe answers only for a real X server.

use std::marker::PhantomData;

use alluno_input_core::{Error, Key, Keyboard, Mouse, MouseButton, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, ConnectionExt as _, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT, Keycode, Keysym, MOTION_NOTIFY_EVENT, Window,
};
use x11rb::protocol::xtest;
use x11rb::rust_connection::RustConnection;

pub const X11_MISSING: &str = "no X11 server with the XTest extension answers on DISPLAY";

const UNICODE_KEYSYM: Keysym = 0x0100_0000;
const XK_BACKSPACE: Keysym = 0xff08;
const XK_TAB: Keysym = 0xff09;
const XK_RETURN: Keysym = 0xff0d;
const XK_PAUSE: Keysym = 0xff13;
const XK_ESCAPE: Keysym = 0xff1b;
const XK_HOME: Keysym = 0xff50;
const XK_LEFT: Keysym = 0xff51;
const XK_UP: Keysym = 0xff52;
const XK_RIGHT: Keysym = 0xff53;
const XK_DOWN: Keysym = 0xff54;
const XK_PAGE_UP: Keysym = 0xff55;
const XK_PAGE_DOWN: Keysym = 0xff56;
const XK_END: Keysym = 0xff57;
const XK_INSERT: Keysym = 0xff63;
const XK_KP_ENTER: Keysym = 0xff8d;
const XK_KP_MULTIPLY: Keysym = 0xffaa;
const XK_KP_ADD: Keysym = 0xffab;
const XK_KP_SUBTRACT: Keysym = 0xffad;
const XK_KP_DECIMAL: Keysym = 0xffae;
const XK_KP_DIVIDE: Keysym = 0xffaf;
const XK_KP_0: Keysym = 0xffb0;
const XK_F1: Keysym = 0xffbe;
const XK_SHIFT_L: Keysym = 0xffe1;
const XK_SHIFT_R: Keysym = 0xffe2;
const XK_CONTROL_L: Keysym = 0xffe3;
const XK_CONTROL_R: Keysym = 0xffe4;
const XK_CAPS_LOCK: Keysym = 0xffe5;
const XK_ALT_L: Keysym = 0xffe9;
const XK_ALT_R: Keysym = 0xffea;
const XK_SUPER_L: Keysym = 0xffeb;
const XK_DELETE: Keysym = 0xffff;

const BUTTON_LEFT: u8 = 1;
const BUTTON_MIDDLE: u8 = 2;
const BUTTON_RIGHT: u8 = 3;
const BUTTON_WHEEL_UP: u8 = 4;
const BUTTON_WHEEL_DOWN: u8 = 5;
const BUTTON_WHEEL_LEFT: u8 = 6;
const BUTTON_WHEEL_RIGHT: u8 = 7;
const BUTTON_BACK: u8 = 8;
const BUTTON_FORWARD: u8 = 9;

/// Whether an X server with XTest answers on `DISPLAY`.
pub fn available() -> bool {
    std::env::var_os("DISPLAY").is_some() && Server::connect().is_ok()
}

/// The keysym a key is spelled as, in the server's own vocabulary.
pub fn keysym_of(key: Key) -> Keysym {
    match key {
        Key::A => 0x61,
        Key::B => 0x62,
        Key::C => 0x63,
        Key::D => 0x64,
        Key::E => 0x65,
        Key::F => 0x66,
        Key::G => 0x67,
        Key::H => 0x68,
        Key::I => 0x69,
        Key::J => 0x6a,
        Key::K => 0x6b,
        Key::L => 0x6c,
        Key::M => 0x6d,
        Key::N => 0x6e,
        Key::O => 0x6f,
        Key::P => 0x70,
        Key::Q => 0x71,
        Key::R => 0x72,
        Key::S => 0x73,
        Key::T => 0x74,
        Key::U => 0x75,
        Key::V => 0x76,
        Key::W => 0x77,
        Key::X => 0x78,
        Key::Y => 0x79,
        Key::Z => 0x7a,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::F1 => XK_F1,
        Key::F2 => XK_F1 + 1,
        Key::F3 => XK_F1 + 2,
        Key::F4 => XK_F1 + 3,
        Key::F5 => XK_F1 + 4,
        Key::F6 => XK_F1 + 5,
        Key::F7 => XK_F1 + 6,
        Key::F8 => XK_F1 + 7,
        Key::F9 => XK_F1 + 8,
        Key::F10 => XK_F1 + 9,
        Key::F11 => XK_F1 + 10,
        Key::F12 => XK_F1 + 11,
        Key::Backspace => XK_BACKSPACE,
        Key::Tab => XK_TAB,
        Key::Enter => XK_RETURN,
        Key::Pause => XK_PAUSE,
        Key::CapsLock => XK_CAPS_LOCK,
        Key::Escape => XK_ESCAPE,
        Key::Space => 0x20,
        Key::PageUp => XK_PAGE_UP,
        Key::PageDown => XK_PAGE_DOWN,
        Key::End => XK_END,
        Key::Home => XK_HOME,
        Key::Left => XK_LEFT,
        Key::Up => XK_UP,
        Key::Right => XK_RIGHT,
        Key::Down => XK_DOWN,
        Key::Insert => XK_INSERT,
        Key::Delete => XK_DELETE,
        Key::Minus => 0x2d,
        Key::Equal => 0x3d,
        Key::BracketLeft => 0x5b,
        Key::BracketRight => 0x5d,
        Key::Semicolon => 0x3b,
        Key::Quote => 0x27,
        Key::Backslash => 0x5c,
        Key::Comma => 0x2c,
        Key::Period => 0x2e,
        Key::Slash => 0x2f,
        Key::Backquote => 0x60,
        Key::Numpad0 => XK_KP_0,
        Key::Numpad1 => XK_KP_0 + 1,
        Key::Numpad2 => XK_KP_0 + 2,
        Key::Numpad3 => XK_KP_0 + 3,
        Key::Numpad4 => XK_KP_0 + 4,
        Key::Numpad5 => XK_KP_0 + 5,
        Key::Numpad6 => XK_KP_0 + 6,
        Key::Numpad7 => XK_KP_0 + 7,
        Key::Numpad8 => XK_KP_0 + 8,
        Key::Numpad9 => XK_KP_0 + 9,
        Key::NumpadMultiply => XK_KP_MULTIPLY,
        Key::NumpadAdd => XK_KP_ADD,
        Key::NumpadEnter => XK_KP_ENTER,
        Key::NumpadSubtract => XK_KP_SUBTRACT,
        Key::NumpadDecimal => XK_KP_DECIMAL,
        Key::NumpadDivide => XK_KP_DIVIDE,
        Key::ShiftLeft => XK_SHIFT_L,
        Key::ShiftRight => XK_SHIFT_R,
        Key::ControlLeft => XK_CONTROL_L,
        Key::ControlRight => XK_CONTROL_R,
        Key::AltLeft => XK_ALT_L,
        Key::AltRight => XK_ALT_R,
        Key::Meta => XK_SUPER_L,
        _ => 0,
    }
}

/// The keysym a character is typed as: Latin-1 directly, everything else by code point.
pub fn keysym_of_char(ch: char) -> Keysym {
    match ch {
        '\n' => XK_RETURN,
        '\t' => XK_TAB,
        ' '..='~' | '\u{a0}'..='\u{ff}' => ch as Keysym,
        other => UNICODE_KEYSYM | other as Keysym,
    }
}

/// The X button number for a mouse button, `None` for one X has no number for.
pub fn button_number(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(BUTTON_LEFT),
        MouseButton::Right => Some(BUTTON_RIGHT),
        MouseButton::Middle => Some(BUTTON_MIDDLE),
        MouseButton::Back => Some(BUTTON_BACK),
        MouseButton::Forward => Some(BUTTON_FORWARD),
        _ => None,
    }
}

/// The button taps one wheel call turns into: vertical notches first, then horizontal.
pub fn wheel_taps(dx: i32, dy: i32) -> Vec<u8> {
    let vertical = if dy > 0 {
        BUTTON_WHEEL_UP
    } else {
        BUTTON_WHEEL_DOWN
    };
    let horizontal = if dx > 0 {
        BUTTON_WHEEL_RIGHT
    } else {
        BUTTON_WHEEL_LEFT
    };
    let mut taps = vec![vertical; dy.unsigned_abs() as usize];
    taps.extend(std::iter::repeat_n(horizontal, dx.unsigned_abs() as usize));
    taps
}

/// Maps a 0..=65535 desktop coordinate onto a root window extent in pixels.
pub fn to_root(value: u16, extent: u16) -> i16 {
    if extent == 0 {
        return 0;
    }
    (u32::from(value) * u32::from(extent - 1) / 65535) as i16
}

struct Server {
    conn: RustConnection,
    root: Window,
    width: u16,
    height: u16,
}

impl Server {
    fn connect() -> Result<Self> {
        let (conn, screen) = x11rb::connect(None).map_err(Error::backend)?;
        xtest::get_version(&conn, 2, 2)
            .map_err(Error::backend)?
            .reply()
            .map_err(Error::backend)?;
        let screen = &conn.setup().roots[screen];
        let (root, width, height) = (screen.root, screen.width_in_pixels, screen.height_in_pixels);
        Ok(Self {
            conn,
            root,
            width,
            height,
        })
    }

    fn fake(&self, kind: u8, detail: u8, x: i16, y: i16) -> Result<()> {
        xtest::fake_input(
            &self.conn,
            kind,
            detail,
            x11rb::CURRENT_TIME,
            self.root,
            x,
            y,
            0,
        )
        .map_err(Error::backend)?;
        self.conn.flush().map_err(Error::backend)
    }
}

/// A keyboard on the X server's own mapping.
pub struct X11Keyboard {
    server: Server,
    min_keycode: Keycode,
    keysyms_per_keycode: usize,
    keysyms: Vec<Keysym>,
    spare: Option<Keycode>,
    _thread: PhantomData<*const ()>,
}

impl X11Keyboard {
    pub fn open() -> Result<Self> {
        let server = Server::connect()?;
        let (min_keycode, max_keycode) = {
            let setup = server.conn.setup();
            (setup.min_keycode, setup.max_keycode)
        };
        let reply = server
            .conn
            .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)
            .map_err(Error::backend)?
            .reply()
            .map_err(Error::backend)?;
        let keysyms_per_keycode = usize::from(reply.keysyms_per_keycode.max(1));
        let spare = reply
            .keysyms
            .chunks(keysyms_per_keycode)
            .position(|row| row.iter().all(|&keysym| keysym == 0))
            .map(|index| min_keycode + index as Keycode);
        Ok(Self {
            server,
            min_keycode,
            keysyms_per_keycode,
            keysyms: reply.keysyms,
            spare,
            _thread: PhantomData,
        })
    }

    fn keycode_of(&self, keysym: Keysym) -> Option<Keycode> {
        self.keysyms
            .chunks(self.keysyms_per_keycode)
            .position(|row| row.contains(&keysym))
            .map(|index| self.min_keycode + index as Keycode)
    }

    fn unshifted_keycode_of(&self, keysym: Keysym) -> Option<Keycode> {
        self.keysyms
            .chunks(self.keysyms_per_keycode)
            .position(|row| row.first() == Some(&keysym))
            .map(|index| self.min_keycode + index as Keycode)
    }

    fn press(&self, keycode: Keycode, pressed: bool) -> Result<()> {
        let kind = if pressed {
            KEY_PRESS_EVENT
        } else {
            KEY_RELEASE_EVENT
        };
        self.server.fake(kind, keycode, 0, 0)
    }

    fn remap(&self, keycode: Keycode, keysym: Keysym) -> Result<()> {
        let row = vec![keysym; self.keysyms_per_keycode];
        self.server
            .conn
            .change_keyboard_mapping(1, keycode, self.keysyms_per_keycode as u8, &row)
            .map_err(Error::backend)?
            .check()
            .map_err(Error::backend)
    }

    fn borrow_spare(
        &self,
        keysym: Keysym,
        tap: impl FnOnce(&Self, Keycode) -> Result<()>,
    ) -> Result<()> {
        let spare = self.spare.ok_or(Error::Unsupported)?;
        self.remap(spare, keysym)?;
        let outcome = tap(self, spare);
        let restored = self.remap(spare, 0);
        outcome.and(restored)
    }
}

impl Keyboard for X11Keyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        let keysym = keysym_of(key);
        if keysym == 0 {
            return Err(Error::Unsupported);
        }
        match self.keycode_of(keysym) {
            Some(keycode) => self.press(keycode, pressed),
            None => self.borrow_spare(keysym, |keyboard, keycode| keyboard.press(keycode, pressed)),
        }
    }

    fn text(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            let keysym = keysym_of_char(ch);
            match self.unshifted_keycode_of(keysym) {
                Some(keycode) => {
                    self.press(keycode, true)?;
                    self.press(keycode, false)?;
                }
                None => self.borrow_spare(keysym, |keyboard, keycode| {
                    keyboard.press(keycode, true)?;
                    keyboard.press(keycode, false)
                })?,
            }
        }
        Ok(())
    }
}

/// A mouse on the root window of the X server.
pub struct X11Mouse {
    server: Server,
    _thread: PhantomData<*const ()>,
}

impl X11Mouse {
    pub fn open() -> Result<Self> {
        Ok(Self {
            server: Server::connect()?,
            _thread: PhantomData,
        })
    }

    fn tap(&self, button: u8) -> Result<()> {
        self.server.fake(BUTTON_PRESS_EVENT, button, 0, 0)?;
        self.server.fake(BUTTON_RELEASE_EVENT, button, 0, 0)
    }
}

impl Mouse for X11Mouse {
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()> {
        let px = to_root(x, self.server.width);
        let py = to_root(y, self.server.height);
        self.server.fake(MOTION_NOTIFY_EVENT, 0, px, py)
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        let dx = dx.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        let dy = dy.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        self.server.fake(MOTION_NOTIFY_EVENT, 1, dx, dy)
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let kind = if pressed {
            BUTTON_PRESS_EVENT
        } else {
            BUTTON_RELEASE_EVENT
        };
        let number = button_number(button).ok_or(Error::Unsupported)?;
        self.server.fake(kind, number, 0, 0)
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        for button in wheel_taps(dx, dy) {
            self.tap(button)?;
        }
        Ok(())
    }
}
