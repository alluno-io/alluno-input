//! Sony DualShock 4 (second revision) over USB.
//!
//! Input report 0x01 is the 64-byte USB report: sticks, hat plus buttons,
//! triggers, then the sensor and touchpad block a game without motion
//! controls ignores. Output report 0x05 carries the motors and the light bar.
//! The feature reports are what SDL, Steam and `hid-playstation` read before
//! they trust the pad: calibration (0x02), the pairing address (0x81 and 0x12)
//! and the firmware block (0xA3).

use super::{Decoded, FeatureReport, Identity, hat, stick_byte, stick_byte_inverted};
use crate::{GamepadOutput, GamepadState, buttons};

pub const IDENTITY: Identity = Identity {
    vendor: 0x054C,
    product: 0x09CC,
    version: 0x0100,
};

pub const INPUT_LEN: usize = 64;
pub const OUTPUT_LEN: usize = 32;

const INPUT_ID: u8 = 0x01;
const OUTPUT_ID: u8 = 0x05;

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x05,        // Usage (Game Pad)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x01,        //   Report ID (1)
    0x09, 0x30,        //   Usage (X)
    0x09, 0x31,        //   Usage (Y)
    0x09, 0x32,        //   Usage (Z)
    0x09, 0x35,        //   Usage (Rz)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x04,        //   Report Count (4)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x09, 0x39,        //   Usage (Hat switch)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x07,        //   Logical Maximum (7)
    0x35, 0x00,        //   Physical Minimum (0)
    0x46, 0x3B, 0x01,  //   Physical Maximum (315)
    0x65, 0x14,        //   Unit (Degrees)
    0x75, 0x04,        //   Report Size (4)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x42,        //   Input (Data,Var,Abs,Null)
    0x65, 0x00,        //   Unit (None)
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x01,        //   Usage Minimum (1)
    0x29, 0x0E,        //   Usage Maximum (14)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x0E,        //   Report Count (14)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor 0xFF00)
    0x09, 0x20,        //   Usage (0x20)
    0x75, 0x06,        //   Report Size (6)
    0x95, 0x01,        //   Report Count (1)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x3F,        //   Logical Maximum (63)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x05, 0x01,        //   Usage Page (Generic Desktop)
    0x09, 0x33,        //   Usage (Rx)
    0x09, 0x34,        //   Usage (Ry)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x02,        //   Report Count (2)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor 0xFF00)
    0x09, 0x21,        //   Usage (0x21)
    0x95, 0x36,        //   Report Count (54)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x85, 0x05,        //   Report ID (5)
    0x09, 0x22,        //   Usage (0x22)
    0x95, 0x1F,        //   Report Count (31)
    0x91, 0x02,        //   Output (Data,Var,Abs)
    0x85, 0x04,        //   Report ID (4)
    0x09, 0x23,        //   Usage (0x23)
    0x95, 0x24,        //   Report Count (36)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x02,        //   Report ID (2)
    0x09, 0x24,        //   Usage (0x24)
    0x95, 0x24,        //   Report Count (36)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x08,        //   Report ID (8)
    0x09, 0x25,        //   Usage (0x25)
    0x95, 0x03,        //   Report Count (3)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x10,        //   Report ID (16)
    0x09, 0x26,        //   Usage (0x26)
    0x95, 0x04,        //   Report Count (4)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x11,        //   Report ID (17)
    0x09, 0x27,        //   Usage (0x27)
    0x95, 0x02,        //   Report Count (2)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x12,        //   Report ID (18)
    0x06, 0x02, 0xFF,  //   Usage Page (Vendor 0xFF02)
    0x09, 0x21,        //   Usage (0x21)
    0x95, 0x0F,        //   Report Count (15)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x13,        //   Report ID (19)
    0x09, 0x22,        //   Usage (0x22)
    0x95, 0x16,        //   Report Count (22)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x14,        //   Report ID (20)
    0x06, 0x05, 0xFF,  //   Usage Page (Vendor 0xFF05)
    0x09, 0x20,        //   Usage (0x20)
    0x95, 0x10,        //   Report Count (16)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x15,        //   Report ID (21)
    0x09, 0x21,        //   Usage (0x21)
    0x95, 0x2C,        //   Report Count (44)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x06, 0x80, 0xFF,  //   Usage Page (Vendor 0xFF80)
    0x85, 0x80,        //   Report ID (128)
    0x09, 0x20,        //   Usage (0x20)
    0x95, 0x06,        //   Report Count (6)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x81,        //   Report ID (129)
    0x09, 0x21,        //   Usage (0x21)
    0x95, 0x06,        //   Report Count (6)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x82,        //   Report ID (130)
    0x09, 0x22,        //   Usage (0x22)
    0x95, 0x05,        //   Report Count (5)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x83,        //   Report ID (131)
    0x09, 0x23,        //   Usage (0x23)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x84,        //   Report ID (132)
    0x09, 0x24,        //   Usage (0x24)
    0x95, 0x04,        //   Report Count (4)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x85,        //   Report ID (133)
    0x09, 0x25,        //   Usage (0x25)
    0x95, 0x06,        //   Report Count (6)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x86,        //   Report ID (134)
    0x09, 0x26,        //   Usage (0x26)
    0x95, 0x06,        //   Report Count (6)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x87,        //   Report ID (135)
    0x09, 0x27,        //   Usage (0x27)
    0x95, 0x23,        //   Report Count (35)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x88,        //   Report ID (136)
    0x09, 0x28,        //   Usage (0x28)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x89,        //   Report ID (137)
    0x09, 0x29,        //   Usage (0x29)
    0x95, 0x02,        //   Report Count (2)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x90,        //   Report ID (144)
    0x09, 0x30,        //   Usage (0x30)
    0x95, 0x05,        //   Report Count (5)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x91,        //   Report ID (145)
    0x09, 0x31,        //   Usage (0x31)
    0x95, 0x03,        //   Report Count (3)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x92,        //   Report ID (146)
    0x09, 0x32,        //   Usage (0x32)
    0x95, 0x03,        //   Report Count (3)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x93,        //   Report ID (147)
    0x09, 0x33,        //   Usage (0x33)
    0x95, 0x0C,        //   Report Count (12)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA0,        //   Report ID (160)
    0x09, 0x40,        //   Usage (0x40)
    0x95, 0x06,        //   Report Count (6)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA1,        //   Report ID (161)
    0x09, 0x41,        //   Usage (0x41)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA2,        //   Report ID (162)
    0x09, 0x42,        //   Usage (0x42)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA3,        //   Report ID (163)
    0x09, 0x43,        //   Usage (0x43)
    0x95, 0x30,        //   Report Count (48)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA4,        //   Report ID (164)
    0x09, 0x44,        //   Usage (0x44)
    0x95, 0x0D,        //   Report Count (13)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA5,        //   Report ID (165)
    0x09, 0x45,        //   Usage (0x45)
    0x95, 0x15,        //   Report Count (21)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA6,        //   Report ID (166)
    0x09, 0x46,        //   Usage (0x46)
    0x95, 0x15,        //   Report Count (21)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF0,        //   Report ID (240)
    0x09, 0x47,        //   Usage (0x47)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF1,        //   Report ID (241)
    0x09, 0x48,        //   Usage (0x48)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF2,        //   Report ID (242)
    0x09, 0x49,        //   Usage (0x49)
    0x95, 0x0F,        //   Report Count (15)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA7,        //   Report ID (167)
    0x09, 0x4A,        //   Usage (0x4A)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA8,        //   Report ID (168)
    0x09, 0x4B,        //   Usage (0x4B)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA9,        //   Report ID (169)
    0x09, 0x4C,        //   Usage (0x4C)
    0x95, 0x08,        //   Report Count (8)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xAA,        //   Report ID (170)
    0x09, 0x4E,        //   Usage (0x4E)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xAB,        //   Report ID (171)
    0x09, 0x4F,        //   Usage (0x4F)
    0x95, 0x39,        //   Report Count (57)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xAC,        //   Report ID (172)
    0x09, 0x50,        //   Usage (0x50)
    0x95, 0x39,        //   Report Count (57)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xAD,        //   Report ID (173)
    0x09, 0x51,        //   Usage (0x51)
    0x95, 0x0B,        //   Report Count (11)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xAE,        //   Report ID (174)
    0x09, 0x52,        //   Usage (0x52)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xAF,        //   Report ID (175)
    0x09, 0x53,        //   Usage (0x53)
    0x95, 0x02,        //   Report Count (2)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xB0,        //   Report ID (176)
    0x09, 0x54,        //   Usage (0x54)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xB1,        //   Report ID (177)
    0x09, 0x55,        //   Usage (0x55)
    0x95, 0x02,        //   Report Count (2)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xB2,        //   Report ID (178)
    0x09, 0x56,        //   Usage (0x56)
    0x95, 0x02,        //   Report Count (2)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0xC0,              // End Collection
];

/// The pairing address a pad reports on report 0x81 and inside 0x12; a
/// locally administered address so it collides with no real radio.
pub const ADDRESS: [u8; 6] = [0x02, 0xA1, 0x10, 0x00, 0x00, 0x01];

/// Calibration (0x02): gyro bias 0, gyro plus and minus ranges symmetric,
/// speed 540 degrees per second, accelerometer plus and minus 1g at 8192 units.
#[rustfmt::skip]
const CALIBRATION: [u8; 37] = [
    0x02,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x20, 0x00, 0x20, 0x00, 0x20,
    0x00, 0xE0, 0x00, 0xE0, 0x00, 0xE0,
    0x1C, 0x02, 0x1C, 0x02,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x00,
];

/// The pairing address block (0x81).
const PAIRING: [u8; 7] = [
    0x81, ADDRESS[5], ADDRESS[4], ADDRESS[3], ADDRESS[2], ADDRESS[1], ADDRESS[0],
];

/// The address block SDL reads on USB (0x12): the pad's address, then the host's.
#[rustfmt::skip]
const ADDRESS_BLOCK: [u8; 16] = [
    0x12,
    ADDRESS[5], ADDRESS[4], ADDRESS[3], ADDRESS[2], ADDRESS[1], ADDRESS[0],
    0x08, 0x25, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Firmware info (0xA3): a 16-byte build date, a 16-byte build time, then the
/// hardware version word at 35 and the firmware version word at 41, which is
/// where `hid-playstation` reads them.
#[rustfmt::skip]
const FIRMWARE: [u8; 49] = [
    0xA3,
    b'S', b'e', b'p', b' ', b'1', b'4', b' ', b'2', b'0', b'2', b'6', 0x00, 0x00, 0x00, 0x00, 0x00,
    b'1', b'2', b':', b'0', b'0', b':', b'0', b'0', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00,
    0x01, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x04, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub const FEATURES: &[FeatureReport] = &[
    FeatureReport {
        id: 0x02,
        data: &CALIBRATION,
    },
    FeatureReport {
        id: 0x12,
        data: &ADDRESS_BLOCK,
    },
    FeatureReport {
        id: 0x81,
        data: &PAIRING,
    },
    FeatureReport {
        id: 0xA3,
        data: &FIRMWARE,
    },
];

/// The USB report keeps a rolling packet counter in the low bits of byte 7.
#[derive(Default)]
pub struct Codec {
    counter: u8,
}

impl Codec {
    pub fn encode(&mut self, state: &GamepadState) -> Vec<u8> {
        let mut report = vec![0u8; INPUT_LEN];
        report[0] = INPUT_ID;
        report[1] = stick_byte(state.thumb_lx);
        report[2] = stick_byte_inverted(state.thumb_ly);
        report[3] = stick_byte(state.thumb_rx);
        report[4] = stick_byte_inverted(state.thumb_ry);

        let mut b5 = hat(state);
        if state.pressed(buttons::X) {
            b5 |= 1 << 4;
        }
        if state.pressed(buttons::A) {
            b5 |= 1 << 5;
        }
        if state.pressed(buttons::B) {
            b5 |= 1 << 6;
        }
        if state.pressed(buttons::Y) {
            b5 |= 1 << 7;
        }
        report[5] = b5;

        let mut b6 = 0u8;
        if state.pressed(buttons::LEFT_SHOULDER) {
            b6 |= 1 << 0;
        }
        if state.pressed(buttons::RIGHT_SHOULDER) {
            b6 |= 1 << 1;
        }
        if state.left_trigger > 0 {
            b6 |= 1 << 2;
        }
        if state.right_trigger > 0 {
            b6 |= 1 << 3;
        }
        if state.pressed(buttons::BACK) {
            b6 |= 1 << 4;
        }
        if state.pressed(buttons::START) {
            b6 |= 1 << 5;
        }
        if state.pressed(buttons::LEFT_THUMB) {
            b6 |= 1 << 6;
        }
        if state.pressed(buttons::RIGHT_THUMB) {
            b6 |= 1 << 7;
        }
        report[6] = b6;

        self.counter = (self.counter + 1) & 0x3F;
        let mut b7 = self.counter << 2;
        if state.pressed(buttons::GUIDE) {
            b7 |= 1 << 0;
        }
        report[7] = b7;

        report[8] = state.left_trigger;
        report[9] = state.right_trigger;
        report[30] = 0x1B;
        for byte in &mut report[35..=42] {
            *byte = 0x80;
        }
        report
    }

    pub fn decode(&mut self, report: &[u8]) -> Decoded {
        let mut decoded = Decoded::default();
        if report.first() != Some(&OUTPUT_ID) || report.len() < 9 {
            decoded.outputs.push(GamepadOutput::Raw(report.to_vec()));
            return decoded;
        }
        let flags = report[1];
        if flags & 0x01 != 0 {
            decoded.outputs.push(GamepadOutput::Rumble {
                large: report[5],
                small: report[4],
            });
        }
        if flags & 0x02 != 0 {
            decoded.outputs.push(GamepadOutput::Rgb {
                r: report[6],
                g: report[7],
                b: report[8],
            });
        }
        decoded
    }
}
