//! A plain HID gamepad with no vendor identity: sixteen buttons, a hat, two
//! sticks and two triggers on the standard usages, which every OS input stack
//! and SDL's generic mapping take without a lookup table. It has no output
//! reports, so a game's feedback has nowhere to go and none is sourced.

use super::{Decoded, FeatureReport, Identity, hat};
use crate::{GamepadOutput, GamepadState, buttons};

pub const IDENTITY: Identity = Identity {
    vendor: 0x1234,
    product: 0x5690,
    version: 0x0100,
};

pub const INPUT_LEN: usize = 14;
pub const OUTPUT_LEN: usize = 0;

const INPUT_ID: u8 = 0x01;

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x05,        // Usage (Game Pad)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x01,        //   Report ID (1)
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x01,        //   Usage Minimum (1)
    0x29, 0x10,        //   Usage Maximum (16)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x10,        //   Report Count (16)
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
    0x75, 0x04,        //   Report Size (4)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x09, 0x30,        //   Usage (X)
    0x09, 0x31,        //   Usage (Y)
    0x09, 0x33,        //   Usage (Rx)
    0x09, 0x34,        //   Usage (Ry)
    0x16, 0x00, 0x80,  //   Logical Minimum (-32768)
    0x26, 0xFF, 0x7F,  //   Logical Maximum (32767)
    0x75, 0x10,        //   Report Size (16)
    0x95, 0x04,        //   Report Count (4)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x09, 0x32,        //   Usage (Z)
    0x09, 0x35,        //   Usage (Rz)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x02,        //   Report Count (2)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0xC0,              // End Collection
];

pub const FEATURES: &[FeatureReport] = &[];

/// Button order on the generic pad: the XInput face and shoulder buttons in
/// SDL's generic numbering, so `Button 1` is south and `Button 4` north.
const BUTTON_ORDER: [u16; 11] = [
    buttons::A,
    buttons::B,
    buttons::X,
    buttons::Y,
    buttons::LEFT_SHOULDER,
    buttons::RIGHT_SHOULDER,
    buttons::BACK,
    buttons::START,
    buttons::GUIDE,
    buttons::LEFT_THUMB,
    buttons::RIGHT_THUMB,
];

#[derive(Default)]
pub struct Codec;

impl Codec {
    pub fn encode(&mut self, state: &GamepadState) -> Vec<u8> {
        let mut report = vec![0u8; INPUT_LEN];
        report[0] = INPUT_ID;
        let mut bits = 0u16;
        for (index, button) in BUTTON_ORDER.iter().enumerate() {
            if state.pressed(*button) {
                bits |= 1 << index;
            }
        }
        report[1..3].copy_from_slice(&bits.to_le_bytes());
        report[3] = hat(state);
        report[4..6].copy_from_slice(&state.thumb_lx.to_le_bytes());
        report[6..8].copy_from_slice(&state.thumb_ly.saturating_neg().to_le_bytes());
        report[8..10].copy_from_slice(&state.thumb_rx.to_le_bytes());
        report[10..12].copy_from_slice(&state.thumb_ry.saturating_neg().to_le_bytes());
        report[12] = state.left_trigger;
        report[13] = state.right_trigger;
        report
    }

    pub fn decode(&mut self, report: &[u8]) -> Decoded {
        Decoded {
            outputs: vec![GamepadOutput::Raw(report.to_vec())],
            reply: None,
        }
    }
}
