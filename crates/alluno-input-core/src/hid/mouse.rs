//! The HID mouse: five buttons, relative X and Y, a wheel and a horizontal
//! pan. It is deliberately a relative mouse, the kind every OS maps across
//! every monitor; an absolute HID mouse lands on the primary monitor only, so
//! absolute placement belongs to another layer and this device answers
//! `Unsupported` for it.

use super::Identity;
use crate::MouseButton;

/// The device's vendor identity.
pub const IDENTITY: Identity = Identity {
    vendor: 0x1234,
    product: 0x5683,
    version: 0x0100,
};

/// The bus-visible name.
pub const NAME: &str = "Alluno Mouse";

/// The input report id.
pub const REPORT_ID: u8 = 0x0B;

/// The input report size, id byte included: id, buttons, x, y, wheel, pan.
pub const INPUT_LEN: usize = 8;

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x02,        // Usage (Mouse)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x0B,        //   Report ID (11)
    0x09, 0x01,        //   Usage (Pointer)
    0xA1, 0x00,        //   Collection (Physical)
    0x05, 0x09,        //     Usage Page (Button)
    0x19, 0x01,        //     Usage Minimum (1)
    0x29, 0x05,        //     Usage Maximum (5)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x01,        //     Logical Maximum (1)
    0x75, 0x01,        //     Report Size (1)
    0x95, 0x05,        //     Report Count (5)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x75, 0x03,        //     Report Size (3)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x03,        //     Input (Const,Var,Abs)
    0x05, 0x01,        //     Usage Page (Generic Desktop)
    0x09, 0x30,        //     Usage (X)
    0x09, 0x31,        //     Usage (Y)
    0x16, 0x00, 0x80,  //     Logical Minimum (-32768)
    0x26, 0xFF, 0x7F,  //     Logical Maximum (32767)
    0x75, 0x10,        //     Report Size (16)
    0x95, 0x02,        //     Report Count (2)
    0x81, 0x06,        //     Input (Data,Var,Rel)
    0x09, 0x38,        //     Usage (Wheel)
    0x15, 0x81,        //     Logical Minimum (-127)
    0x25, 0x7F,        //     Logical Maximum (127)
    0x75, 0x08,        //     Report Size (8)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x06,        //     Input (Data,Var,Rel)
    0x05, 0x0C,        //     Usage Page (Consumer)
    0x0A, 0x38, 0x02,  //     Usage (AC Pan)
    0x15, 0x81,        //     Logical Minimum (-127)
    0x25, 0x7F,        //     Logical Maximum (127)
    0x75, 0x08,        //     Report Size (8)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x06,        //     Input (Data,Var,Rel)
    0xC0,              //   End Collection
    0xC0,              // End Collection
];

/// The button bit for a port button, in HID button order.
pub fn button_bit(button: MouseButton) -> Option<u8> {
    Some(match button {
        MouseButton::Left => 1 << 0,
        MouseButton::Right => 1 << 1,
        MouseButton::Middle => 1 << 2,
        MouseButton::Back => 1 << 3,
        MouseButton::Forward => 1 << 4,
    })
}

/// The buttons currently held, restated in every report.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Codec {
    buttons: u8,
}

impl Codec {
    /// Applies one button transition and answers the report, or `None` for
    /// a button the descriptor does not declare.
    pub fn button(&mut self, button: MouseButton, pressed: bool) -> Option<[u8; INPUT_LEN]> {
        let bit = button_bit(button)?;
        if pressed {
            self.buttons |= bit;
        } else {
            self.buttons &= !bit;
        }
        Some(self.report(0, 0, 0, 0))
    }

    /// A relative move; deltas beyond the 16-bit range are clamped.
    pub fn move_rel(&self, dx: i32, dy: i32) -> [u8; INPUT_LEN] {
        self.report(clamp16(dx), clamp16(dy), 0, 0)
    }

    /// Whole notches, horizontal then vertical; positive is right and up.
    pub fn wheel(&self, dx: i32, dy: i32) -> [u8; INPUT_LEN] {
        self.report(0, 0, clamp8(dy), clamp8(dx))
    }

    fn report(&self, dx: i16, dy: i16, wheel: i8, pan: i8) -> [u8; INPUT_LEN] {
        let mut report = [0u8; INPUT_LEN];
        report[0] = REPORT_ID;
        report[1] = self.buttons;
        report[2..4].copy_from_slice(&dx.to_le_bytes());
        report[4..6].copy_from_slice(&dy.to_le_bytes());
        report[6] = wheel as u8;
        report[7] = pan as u8;
        report
    }
}

fn clamp16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn clamp8(value: i32) -> i8 {
    value.clamp(-127, 127) as i8
}
