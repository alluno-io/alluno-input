//! Xbox One and Xbox Series controllers as they appear over Bluetooth.
//!
//! Over USB an Xbox controller is not HID and only an XUSB bus can be one,
//! which is why the Xbox 360 identity stays on ViGEm. Over Bluetooth the same
//! controllers are plain HID gamepads with Microsoft's vendor id: Linux binds
//! `hid-microsoft` to that identity and macOS its controller framework, so a
//! node carrying the Xbox One S report layout (firmware 5.x) is a real Xbox
//! pad to every game there. On Windows the same node is a HID gamepad that
//! Steam Input, SDL and Windows.Gaming.Input's raw controllers see by its
//! identity, but not XInput: the inbox XInput HID filter binds only to real
//! Bluetooth devices and GIP children, so a game that reads XInput alone
//! wants the Xbox 360 profile instead. Rumble arrives as the PID output
//! report 0x03: four actuator magnitudes, a duration, a delay and a repeat.

use super::{Decoded, FeatureReport, Identity, hat};
use crate::{GamepadOutput, GamepadState, buttons};

/// Xbox One S, Bluetooth, firmware 5.x.
pub const ONE_IDENTITY: Identity = Identity {
    vendor: 0x045E,
    product: 0x02FD,
    version: 0x0903,
};

/// Xbox Series X|S, Bluetooth.
pub const SERIES_IDENTITY: Identity = Identity {
    vendor: 0x045E,
    product: 0x0B13,
    version: 0x0509,
};

pub const INPUT_LEN: usize = 17;
pub const OUTPUT_LEN: usize = 9;

const INPUT_ID: u8 = 0x01;
const HOME_ID: u8 = 0x02;
const RUMBLE_ID: u8 = 0x03;

/// The bus-visible name, the same for both models.
pub const NAME: &str = "Xbox Wireless Controller";

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x05,        // Usage (Game Pad)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x01,        //   Report ID (1)
    0x09, 0x01,        //   Usage (Pointer)
    0xA1, 0x00,        //   Collection (Physical)
    0x09, 0x30,        //     Usage (X)
    0x09, 0x31,        //     Usage (Y)
    0x15, 0x00,        //     Logical Minimum (0)
    0x27, 0xFF, 0xFF, 0x00, 0x00,  //     Logical Maximum (65535)
    0x95, 0x02,        //     Report Count (2)
    0x75, 0x10,        //     Report Size (16)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0xC0,              //   End Collection
    0x09, 0x01,        //   Usage (Pointer)
    0xA1, 0x00,        //   Collection (Physical)
    0x09, 0x32,        //     Usage (Z)
    0x09, 0x35,        //     Usage (Rz)
    0x15, 0x00,        //     Logical Minimum (0)
    0x27, 0xFF, 0xFF, 0x00, 0x00,  //     Logical Maximum (65535)
    0x95, 0x02,        //     Report Count (2)
    0x75, 0x10,        //     Report Size (16)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0xC0,              //   End Collection
    0x05, 0x02,        //   Usage Page (Simulation Controls)
    0x09, 0xC5,        //   Usage (Brake)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x03,  //   Logical Maximum (1023)
    0x95, 0x01,        //   Report Count (1)
    0x75, 0x0A,        //   Report Size (10)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x00,        //   Logical Maximum (0)
    0x75, 0x06,        //   Report Size (6)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x05, 0x02,        //   Usage Page (Simulation Controls)
    0x09, 0xC4,        //   Usage (Accelerator)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x03,  //   Logical Maximum (1023)
    0x95, 0x01,        //   Report Count (1)
    0x75, 0x0A,        //   Report Size (10)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x00,        //   Logical Maximum (0)
    0x75, 0x06,        //   Report Size (6)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x05, 0x01,        //   Usage Page (Generic Desktop)
    0x09, 0x39,        //   Usage (Hat switch)
    0x15, 0x01,        //   Logical Minimum (1)
    0x25, 0x08,        //   Logical Maximum (8)
    0x35, 0x00,        //   Physical Minimum (0)
    0x46, 0x3B, 0x01,  //   Physical Maximum (315)
    0x66, 0x14, 0x00,  //   Unit (Degrees)
    0x75, 0x04,        //   Report Size (4)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x42,        //   Input (Data,Var,Abs,Null)
    0x75, 0x04,        //   Report Size (4)
    0x95, 0x01,        //   Report Count (1)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x00,        //   Logical Maximum (0)
    0x35, 0x00,        //   Physical Minimum (0)
    0x45, 0x00,        //   Physical Maximum (0)
    0x65, 0x00,        //   Unit (None)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x01,        //   Usage Minimum (1)
    0x29, 0x0F,        //   Usage Maximum (15)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x0F,        //   Report Count (15)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x00,        //   Logical Maximum (0)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x05, 0x0C,        //   Usage Page (Consumer)
    0x0A, 0xB2, 0x00,  //   Usage (Record)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x95, 0x01,        //   Report Count (1)
    0x75, 0x01,        //   Report Size (1)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x00,        //   Logical Maximum (0)
    0x75, 0x07,        //   Report Size (7)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x05, 0x0C,        //   Usage Page (Consumer)
    0x09, 0x01,        //   Usage (Consumer Control)
    0x85, 0x02,        //   Report ID (2)
    0xA1, 0x01,        //   Collection (Application)
    0x05, 0x0C,        //     Usage Page (Consumer)
    0x0A, 0x23, 0x02,  //     Usage (AC Home)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x01,        //     Logical Maximum (1)
    0x95, 0x01,        //     Report Count (1)
    0x75, 0x01,        //     Report Size (1)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x00,        //     Logical Maximum (0)
    0x75, 0x07,        //     Report Size (7)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x03,        //     Input (Const,Var,Abs)
    0xC0,              //   End Collection
    0x05, 0x0F,        //   Usage Page (Physical Interface)
    0x09, 0x21,        //   Usage (Set Effect Report)
    0x85, 0x03,        //   Report ID (3)
    0xA1, 0x02,        //   Collection (Logical)
    0x09, 0x97,        //     Usage (DC Enable Actuators)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x01,        //     Logical Maximum (1)
    0x75, 0x04,        //     Report Size (4)
    0x95, 0x01,        //     Report Count (1)
    0x91, 0x02,        //     Output (Data,Var,Abs)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x00,        //     Logical Maximum (0)
    0x75, 0x04,        //     Report Size (4)
    0x95, 0x01,        //     Report Count (1)
    0x91, 0x03,        //     Output (Const,Var,Abs)
    0x09, 0x70,        //     Usage (Magnitude)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x64,        //     Logical Maximum (100)
    0x75, 0x08,        //     Report Size (8)
    0x95, 0x04,        //     Report Count (4)
    0x91, 0x02,        //     Output (Data,Var,Abs)
    0x09, 0x50,        //     Usage (Duration)
    0x66, 0x01, 0x10,  //     Unit (Seconds)
    0x55, 0x0E,        //     Unit Exponent (-2)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0xFF, 0x00,  //     Logical Maximum (255)
    0x95, 0x01,        //     Report Count (1)
    0x75, 0x08,        //     Report Size (8)
    0x91, 0x02,        //     Output (Data,Var,Abs)
    0x09, 0xA7,        //     Usage (Start Delay)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0xFF, 0x00,  //     Logical Maximum (255)
    0x95, 0x01,        //     Report Count (1)
    0x75, 0x08,        //     Report Size (8)
    0x91, 0x02,        //     Output (Data,Var,Abs)
    0x65, 0x00,        //     Unit (None)
    0x55, 0x00,        //     Unit Exponent (0)
    0x09, 0x7C,        //     Usage (Loop Count)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0xFF, 0x00,  //     Logical Maximum (255)
    0x95, 0x01,        //     Report Count (1)
    0x75, 0x08,        //     Report Size (8)
    0x91, 0x02,        //     Output (Data,Var,Abs)
    0xC0,              //   End Collection
    0x05, 0x06,        //   Usage Page (Generic Device Controls)
    0x09, 0x20,        //   Usage (Battery Strength)
    0x85, 0x04,        //   Report ID (4)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0xC0,              // End Collection
];

pub const FEATURES: &[FeatureReport] = &[];

/// Which of the two identities the node carries; the reports are the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    One,
    Series,
}

/// The guide button travels on its own report, so the codec remembers it to
/// send that report only when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codec {
    model: Model,
    guide: Option<bool>,
}

impl Codec {
    pub fn new(model: Model) -> Self {
        Self { model, guide: None }
    }

    pub fn identity(&self) -> Identity {
        match self.model {
            Model::One => ONE_IDENTITY,
            Model::Series => SERIES_IDENTITY,
        }
    }

    /// The 0x01 report for a snapshot.
    pub fn encode(&mut self, state: &GamepadState) -> Vec<u8> {
        let mut report = vec![0u8; INPUT_LEN];
        report[0] = INPUT_ID;
        report[1..3].copy_from_slice(&stick_u16(state.thumb_lx).to_le_bytes());
        report[3..5].copy_from_slice(&stick_u16_inverted(state.thumb_ly).to_le_bytes());
        report[5..7].copy_from_slice(&stick_u16(state.thumb_rx).to_le_bytes());
        report[7..9].copy_from_slice(&stick_u16_inverted(state.thumb_ry).to_le_bytes());
        report[9..11].copy_from_slice(&trigger_10(state.left_trigger).to_le_bytes());
        report[11..13].copy_from_slice(&trigger_10(state.right_trigger).to_le_bytes());
        report[13] = match hat(state) {
            8 => 0,
            direction => direction + 1,
        };

        let mut b14 = 0u8;
        if state.pressed(buttons::A) {
            b14 |= 1 << 0;
        }
        if state.pressed(buttons::B) {
            b14 |= 1 << 1;
        }
        if state.pressed(buttons::X) {
            b14 |= 1 << 3;
        }
        if state.pressed(buttons::Y) {
            b14 |= 1 << 4;
        }
        if state.pressed(buttons::LEFT_SHOULDER) {
            b14 |= 1 << 6;
        }
        if state.pressed(buttons::RIGHT_SHOULDER) {
            b14 |= 1 << 7;
        }
        report[14] = b14;

        let mut b15 = 0u8;
        if state.pressed(buttons::BACK) {
            b15 |= 1 << 2;
        }
        if state.pressed(buttons::START) {
            b15 |= 1 << 3;
        }
        if state.pressed(buttons::LEFT_THUMB) {
            b15 |= 1 << 5;
        }
        if state.pressed(buttons::RIGHT_THUMB) {
            b15 |= 1 << 6;
        }
        report[15] = b15;
        report
    }

    /// The 0x02 report when the guide button changed since the last call.
    pub fn guide_report(&mut self, state: &GamepadState) -> Option<[u8; 2]> {
        let pressed = state.pressed(buttons::GUIDE);
        if self.guide == Some(pressed) {
            return None;
        }
        self.guide = Some(pressed);
        Some([HOME_ID, u8::from(pressed)])
    }

    pub fn decode(&mut self, report: &[u8]) -> Decoded {
        let mut decoded = Decoded::default();
        if report.first() != Some(&RUMBLE_ID) || report.len() < 6 {
            decoded.outputs.push(GamepadOutput::Raw(report.to_vec()));
            return decoded;
        }
        let enable = report[1] & 0x0F;
        let percent = |value: u8| (u16::from(value.min(100)) * 255 / 100) as u8;
        let left_trigger = if enable & 0x01 != 0 {
            percent(report[2])
        } else {
            0
        };
        let right_trigger = if enable & 0x02 != 0 {
            percent(report[3])
        } else {
            0
        };
        let large = if enable & 0x04 != 0 {
            percent(report[4])
        } else {
            0
        };
        let small = if enable & 0x08 != 0 {
            percent(report[5])
        } else {
            0
        };
        decoded.outputs.push(GamepadOutput::Rumble { large, small });
        if enable & 0x03 != 0 {
            decoded.outputs.push(GamepadOutput::TriggerRumble {
                left: left_trigger,
                right: right_trigger,
            });
        }
        decoded
    }
}

fn stick_u16(value: i16) -> u16 {
    (i32::from(value) + 32768) as u16
}

fn stick_u16_inverted(value: i16) -> u16 {
    (32767 - i32::from(value)) as u16
}

fn trigger_10(value: u8) -> u16 {
    (u32::from(value) * 1023 / 255) as u16
}
