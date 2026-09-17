//! Sony DualSense over USB.
//!
//! Input report 0x01 is the 64-byte USB report. Output report 0x02 is the
//! 48-byte USB report: motors, adaptive triggers, the player lamps and the
//! light bar, each gated by the flag bytes at the front. The feature reports
//! are what `hid-playstation` and SDL read before they trust the pad: the
//! calibration (0x05), the pairing address (0x09) and the firmware block (0x20).

use super::{Decoded, FeatureReport, Identity, hat, stick_byte, stick_byte_inverted};
use crate::{GamepadOutput, GamepadState, buttons};

pub const IDENTITY: Identity = Identity {
    vendor: 0x054C,
    product: 0x0CE6,
    version: 0x0100,
};

pub const INPUT_LEN: usize = 64;
pub const OUTPUT_LEN: usize = 48;

const INPUT_ID: u8 = 0x01;
const OUTPUT_ID: u8 = 0x02;

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
    0x09, 0x33,        //   Usage (Rx)
    0x09, 0x34,        //   Usage (Ry)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x06,        //   Report Count (6)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor 0xFF00)
    0x09, 0x20,        //   Usage (0x20)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x05, 0x01,        //   Usage Page (Generic Desktop)
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
    0x29, 0x0F,        //   Usage Maximum (15)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x0F,        //   Report Count (15)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor 0xFF00)
    0x09, 0x21,        //   Usage (0x21)
    0x95, 0x0D,        //   Report Count (13)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor 0xFF00)
    0x09, 0x22,        //   Usage (0x22)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x34,        //   Report Count (52)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x85, 0x02,        //   Report ID (2)
    0x09, 0x23,        //   Usage (0x23)
    0x95, 0x2F,        //   Report Count (47)
    0x91, 0x02,        //   Output (Data,Var,Abs)
    0x85, 0x05,        //   Report ID (5)
    0x09, 0x33,        //   Usage (0x33)
    0x95, 0x28,        //   Report Count (40)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x08,        //   Report ID (8)
    0x09, 0x34,        //   Usage (0x34)
    0x95, 0x2F,        //   Report Count (47)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x09,        //   Report ID (9)
    0x09, 0x24,        //   Usage (0x24)
    0x95, 0x13,        //   Report Count (19)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x0A,        //   Report ID (10)
    0x09, 0x25,        //   Usage (0x25)
    0x95, 0x1A,        //   Report Count (26)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x20,        //   Report ID (32)
    0x09, 0x26,        //   Usage (0x26)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x21,        //   Report ID (33)
    0x09, 0x27,        //   Usage (0x27)
    0x95, 0x04,        //   Report Count (4)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x22,        //   Report ID (34)
    0x09, 0x40,        //   Usage (0x40)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x80,        //   Report ID (128)
    0x09, 0x28,        //   Usage (0x28)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x81,        //   Report ID (129)
    0x09, 0x29,        //   Usage (0x29)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x82,        //   Report ID (130)
    0x09, 0x2A,        //   Usage (0x2A)
    0x95, 0x09,        //   Report Count (9)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x83,        //   Report ID (131)
    0x09, 0x2B,        //   Usage (0x2B)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x84,        //   Report ID (132)
    0x09, 0x2C,        //   Usage (0x2C)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0x85,        //   Report ID (133)
    0x09, 0x2D,        //   Usage (0x2D)
    0x95, 0x02,        //   Report Count (2)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xA0,        //   Report ID (160)
    0x09, 0x2E,        //   Usage (0x2E)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xE0,        //   Report ID (224)
    0x09, 0x2F,        //   Usage (0x2F)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF0,        //   Report ID (240)
    0x09, 0x30,        //   Usage (0x30)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF1,        //   Report ID (241)
    0x09, 0x31,        //   Usage (0x31)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF2,        //   Report ID (242)
    0x09, 0x32,        //   Usage (0x32)
    0x95, 0x0F,        //   Report Count (15)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF4,        //   Report ID (244)
    0x09, 0x35,        //   Usage (0x35)
    0x95, 0x3F,        //   Report Count (63)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0x85, 0xF5,        //   Report ID (245)
    0x09, 0x36,        //   Usage (0x36)
    0x95, 0x03,        //   Report Count (3)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0xC0,              // End Collection
];

/// The pairing address, locally administered so it collides with no radio.
pub const ADDRESS: [u8; 6] = [0x02, 0xA1, 0x10, 0x00, 0x00, 0x02];

/// Calibration (0x05): gyro bias 0, symmetric gyro ranges at plus and minus
/// 8192 with 540 degrees per second, accelerometer plus and minus 1g at 8192.
#[rustfmt::skip]
const CALIBRATION: [u8; 41] = [
    0x05,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x20, 0x00, 0xE0,
    0x1C, 0x02, 0x1C, 0x02,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x20, 0x00, 0xE0,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// The pairing block (0x09): the pad's address, a Bluetooth class, the host's address.
#[rustfmt::skip]
const PAIRING: [u8; 20] = [
    0x09,
    ADDRESS[5], ADDRESS[4], ADDRESS[3], ADDRESS[2], ADDRESS[1], ADDRESS[0],
    0x08, 0x25, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

/// Firmware info (0x20): an 11-byte build date, an 8-byte build time, the
/// firmware type and series words, then the hardware version at 24, the
/// firmware version at 28 and the update version at 44, which is where
/// `hid-playstation` reads them.
#[rustfmt::skip]
const FIRMWARE: [u8; 64] = [
    0x20,
    b'S', b'e', b'p', b' ', b'1', b'4', b' ', b'2', b'0', b'2', b'6',
    b'1', b'2', b':', b'0', b'0', b':', b'0', b'0',
    0x01, 0x00,
    0x01, 0x00,
    0x03, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x04, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x04, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub const FEATURES: &[FeatureReport] = &[
    FeatureReport {
        id: 0x05,
        data: &CALIBRATION,
    },
    FeatureReport {
        id: 0x09,
        data: &PAIRING,
    },
    FeatureReport {
        id: 0x20,
        data: &FIRMWARE,
    },
];

/// The USB report carries a rolling sequence number in byte 7.
#[derive(Default)]
pub struct Codec {
    sequence: u8,
}

impl Codec {
    pub fn encode(&mut self, state: &GamepadState) -> Vec<u8> {
        let mut report = vec![0u8; INPUT_LEN];
        report[0] = INPUT_ID;
        report[1] = stick_byte(state.thumb_lx);
        report[2] = stick_byte_inverted(state.thumb_ly);
        report[3] = stick_byte(state.thumb_rx);
        report[4] = stick_byte_inverted(state.thumb_ry);
        report[5] = state.left_trigger;
        report[6] = state.right_trigger;
        self.sequence = self.sequence.wrapping_add(1);
        report[7] = self.sequence;

        let mut b8 = hat(state);
        if state.pressed(buttons::X) {
            b8 |= 1 << 4;
        }
        if state.pressed(buttons::A) {
            b8 |= 1 << 5;
        }
        if state.pressed(buttons::B) {
            b8 |= 1 << 6;
        }
        if state.pressed(buttons::Y) {
            b8 |= 1 << 7;
        }
        report[8] = b8;

        let mut b9 = 0u8;
        if state.pressed(buttons::LEFT_SHOULDER) {
            b9 |= 1 << 0;
        }
        if state.pressed(buttons::RIGHT_SHOULDER) {
            b9 |= 1 << 1;
        }
        if state.left_trigger > 0 {
            b9 |= 1 << 2;
        }
        if state.right_trigger > 0 {
            b9 |= 1 << 3;
        }
        if state.pressed(buttons::BACK) {
            b9 |= 1 << 4;
        }
        if state.pressed(buttons::START) {
            b9 |= 1 << 5;
        }
        if state.pressed(buttons::LEFT_THUMB) {
            b9 |= 1 << 6;
        }
        if state.pressed(buttons::RIGHT_THUMB) {
            b9 |= 1 << 7;
        }
        report[9] = b9;

        let mut b10 = 0u8;
        if state.pressed(buttons::GUIDE) {
            b10 |= 1 << 0;
        }
        report[10] = b10;

        report[33] = 0x80;
        report[37] = 0x80;
        report[53] = 0x08;
        report
    }

    pub fn decode(&mut self, report: &[u8]) -> Decoded {
        let mut decoded = Decoded::default();
        if report.first() != Some(&OUTPUT_ID) || report.len() < 48 {
            decoded.outputs.push(GamepadOutput::Raw(report.to_vec()));
            return decoded;
        }
        let valid0 = report[1];
        let valid1 = report[2];
        if valid0 & 0x03 != 0 {
            decoded.outputs.push(GamepadOutput::Rumble {
                large: report[4],
                small: report[3],
            });
        }
        if valid0 & 0x0C != 0 {
            decoded.outputs.push(GamepadOutput::TriggerRumble {
                left: report[23],
                right: report[12],
            });
        }
        if valid1 & 0x10 != 0 {
            let lamps = report[44] & 0x1F;
            let player = match lamps {
                0x04 => 1,
                0x0A => 2,
                0x15 => 3,
                0x1B => 4,
                0x00 => 0,
                other => other.count_ones() as u8,
            };
            decoded.outputs.push(GamepadOutput::PlayerLed(player));
        }
        if valid1 & 0x04 != 0 {
            decoded.outputs.push(GamepadOutput::Rgb {
                r: report[45],
                g: report[46],
                b: report[47],
            });
        }
        decoded
    }
}
