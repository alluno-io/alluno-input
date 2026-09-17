//! The HID keyboard: eight modifier bits and a 128-key bitmap, so any number
//! of keys can be down at once, plus the lamp output report every keyboard
//! declares. Each port [`Key`] maps to one usage on the Keyboard/Keypad page.

use super::Identity;
use crate::Key;

/// The device's vendor identity.
pub const IDENTITY: Identity = Identity {
    vendor: 0x1234,
    product: 0x5682,
    version: 0x0100,
};

/// The bus-visible name.
pub const NAME: &str = "Alluno Keyboard";

/// The input and output report id.
pub const REPORT_ID: u8 = 0x0A;

/// The input report size, id byte included: id, modifiers, 16 bitmap bytes.
pub const INPUT_LEN: usize = 18;

/// The lamp output report size, id byte included.
pub const OUTPUT_LEN: usize = 2;

const BITMAP_LEN: usize = 16;
const MODIFIER_BASE: u8 = 0xE0;

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x06,        // Usage (Keyboard)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x0A,        //   Report ID (10)
    0x05, 0x07,        //   Usage Page (Keyboard/Keypad)
    0x19, 0xE0,        //   Usage Minimum (Left Control)
    0x29, 0xE7,        //   Usage Maximum (Right GUI)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x08,        //   Report Count (8)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x19, 0x00,        //   Usage Minimum (0)
    0x29, 0x7F,        //   Usage Maximum (127)
    0x95, 0x80,        //   Report Count (128)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x05, 0x08,        //   Usage Page (LEDs)
    0x19, 0x01,        //   Usage Minimum (Num Lock)
    0x29, 0x05,        //   Usage Maximum (Kana)
    0x95, 0x05,        //   Report Count (5)
    0x91, 0x02,        //   Output (Data,Var,Abs)
    0x95, 0x03,        //   Report Count (3)
    0x91, 0x03,        //   Output (Const,Var,Abs)
    0xC0,              // End Collection
];

/// The Keyboard/Keypad page usage for a key.
pub fn usage(key: Key) -> Option<u8> {
    Some(match key {
        Key::A => 0x04,
        Key::B => 0x05,
        Key::C => 0x06,
        Key::D => 0x07,
        Key::E => 0x08,
        Key::F => 0x09,
        Key::G => 0x0A,
        Key::H => 0x0B,
        Key::I => 0x0C,
        Key::J => 0x0D,
        Key::K => 0x0E,
        Key::L => 0x0F,
        Key::M => 0x10,
        Key::N => 0x11,
        Key::O => 0x12,
        Key::P => 0x13,
        Key::Q => 0x14,
        Key::R => 0x15,
        Key::S => 0x16,
        Key::T => 0x17,
        Key::U => 0x18,
        Key::V => 0x19,
        Key::W => 0x1A,
        Key::X => 0x1B,
        Key::Y => 0x1C,
        Key::Z => 0x1D,
        Key::Num1 => 0x1E,
        Key::Num2 => 0x1F,
        Key::Num3 => 0x20,
        Key::Num4 => 0x21,
        Key::Num5 => 0x22,
        Key::Num6 => 0x23,
        Key::Num7 => 0x24,
        Key::Num8 => 0x25,
        Key::Num9 => 0x26,
        Key::Num0 => 0x27,
        Key::Enter => 0x28,
        Key::Escape => 0x29,
        Key::Backspace => 0x2A,
        Key::Tab => 0x2B,
        Key::Space => 0x2C,
        Key::Minus => 0x2D,
        Key::Equal => 0x2E,
        Key::BracketLeft => 0x2F,
        Key::BracketRight => 0x30,
        Key::Backslash => 0x31,
        Key::Semicolon => 0x33,
        Key::Quote => 0x34,
        Key::Backquote => 0x35,
        Key::Comma => 0x36,
        Key::Period => 0x37,
        Key::Slash => 0x38,
        Key::CapsLock => 0x39,
        Key::F1 => 0x3A,
        Key::F2 => 0x3B,
        Key::F3 => 0x3C,
        Key::F4 => 0x3D,
        Key::F5 => 0x3E,
        Key::F6 => 0x3F,
        Key::F7 => 0x40,
        Key::F8 => 0x41,
        Key::F9 => 0x42,
        Key::F10 => 0x43,
        Key::F11 => 0x44,
        Key::F12 => 0x45,
        Key::Pause => 0x48,
        Key::Insert => 0x49,
        Key::Home => 0x4A,
        Key::PageUp => 0x4B,
        Key::Delete => 0x4C,
        Key::End => 0x4D,
        Key::PageDown => 0x4E,
        Key::Right => 0x4F,
        Key::Left => 0x50,
        Key::Down => 0x51,
        Key::Up => 0x52,
        Key::NumpadDivide => 0x54,
        Key::NumpadMultiply => 0x55,
        Key::NumpadSubtract => 0x56,
        Key::NumpadAdd => 0x57,
        Key::NumpadEnter => 0x58,
        Key::Numpad1 => 0x59,
        Key::Numpad2 => 0x5A,
        Key::Numpad3 => 0x5B,
        Key::Numpad4 => 0x5C,
        Key::Numpad5 => 0x5D,
        Key::Numpad6 => 0x5E,
        Key::Numpad7 => 0x5F,
        Key::Numpad8 => 0x60,
        Key::Numpad9 => 0x61,
        Key::Numpad0 => 0x62,
        Key::NumpadDecimal => 0x63,
        Key::ControlLeft => 0xE0,
        Key::ShiftLeft => 0xE1,
        Key::AltLeft => 0xE2,
        Key::Meta => 0xE3,
        Key::ControlRight => 0xE4,
        Key::ShiftRight => 0xE5,
        Key::AltRight => 0xE6,
    })
}

/// The keys currently down, which is what each report restates in full.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Codec {
    modifiers: u8,
    bitmap: [u8; BITMAP_LEN],
}

impl Codec {
    /// Applies one transition and answers the report to send, or `None` for
    /// a key the page has no usage for.
    pub fn key(&mut self, key: Key, pressed: bool) -> Option<[u8; INPUT_LEN]> {
        let usage = usage(key)?;
        if usage >= MODIFIER_BASE {
            let bit = 1u8 << (usage - MODIFIER_BASE);
            if pressed {
                self.modifiers |= bit;
            } else {
                self.modifiers &= !bit;
            }
        } else {
            let byte = usize::from(usage / 8);
            let bit = 1u8 << (usage % 8);
            if pressed {
                self.bitmap[byte] |= bit;
            } else {
                self.bitmap[byte] &= !bit;
            }
        }
        Some(self.report())
    }

    /// The report for the current state.
    pub fn report(&self) -> [u8; INPUT_LEN] {
        let mut report = [0u8; INPUT_LEN];
        report[0] = REPORT_ID;
        report[1] = self.modifiers;
        report[2..].copy_from_slice(&self.bitmap);
        report
    }

    /// Whether any key is down.
    pub fn any_down(&self) -> bool {
        self.modifiers != 0 || self.bitmap.iter().any(|byte| *byte != 0)
    }
}
