//! Nintendo Switch Pro Controller over USB.
//!
//! The Pro Controller is not a plain HID gamepad: a host drives it with
//! vendor output reports (0x01 subcommands, 0x10 rumble, 0x80 USB commands)
//! and the pad acknowledges each subcommand on the input pipe (0x21) before it
//! streams 0x30 full reports at 60 Hz. `hid-nintendo`, SDL and Steam all run
//! that handshake, so the codec answers it: device info, the input mode, the
//! IMU and vibration switches, the player lamps, and SPI flash reads of the
//! factory calibration a host uses to centre the sticks.

use super::{Decoded, FeatureReport, Identity};
use crate::{GamepadOutput, GamepadState, buttons};

pub const IDENTITY: Identity = Identity {
    vendor: 0x057E,
    product: 0x2009,
    version: 0x0200,
};

pub const INPUT_LEN: usize = 64;
pub const OUTPUT_LEN: usize = 64;

const FULL_REPORT: u8 = 0x30;
const SUBCOMMAND_REPLY: u8 = 0x21;
const USB_REPLY: u8 = 0x81;

const OUTPUT_SUBCOMMAND: u8 = 0x01;
const OUTPUT_RUMBLE: u8 = 0x10;
const OUTPUT_USB: u8 = 0x80;

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x15, 0x00,        // Logical Minimum (0)
    0x09, 0x04,        // Usage (Joystick)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x30,        //   Report ID (48)
    0x05, 0x01,        //   Usage Page (Generic Desktop)
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x01,        //   Usage Minimum (1)
    0x29, 0x0A,        //   Usage Maximum (10)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x0A,        //   Report Count (10)
    0x55, 0x00,        //   Unit Exponent (0)
    0x65, 0x00,        //   Unit (None)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x0B,        //   Usage Minimum (11)
    0x29, 0x0E,        //   Usage Maximum (14)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x04,        //   Report Count (4)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x02,        //   Report Count (2)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x0B, 0x01, 0x00, 0x01, 0x00,  //   Usage (Generic Desktop: Pointer)
    0xA1, 0x00,        //   Collection (Physical)
    0x0B, 0x30, 0x00, 0x01, 0x00,  //     Usage (Generic Desktop: X)
    0x0B, 0x31, 0x00, 0x01, 0x00,  //     Usage (Generic Desktop: Y)
    0x0B, 0x32, 0x00, 0x01, 0x00,  //     Usage (Generic Desktop: Z)
    0x0B, 0x35, 0x00, 0x01, 0x00,  //     Usage (Generic Desktop: Rz)
    0x15, 0x00,        //     Logical Minimum (0)
    0x27, 0xFF, 0xFF, 0x00, 0x00,  //     Logical Maximum (65535)
    0x75, 0x10,        //     Report Size (16)
    0x95, 0x04,        //     Report Count (4)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0xC0,              //   End Collection
    0x0B, 0x39, 0x00, 0x01, 0x00,  //   Usage (Generic Desktop: Hat switch)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x07,        //   Logical Maximum (7)
    0x35, 0x00,        //   Physical Minimum (0)
    0x46, 0x3B, 0x01,  //   Physical Maximum (315)
    0x65, 0x14,        //   Unit (Degrees)
    0x75, 0x04,        //   Report Size (4)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x0F,        //   Usage Minimum (15)
    0x29, 0x12,        //   Usage Maximum (18)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x04,        //   Report Count (4)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x34,        //   Report Count (52)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor 0xFF00)
    0x85, 0x21,        //   Report ID (33)
    0x09, 0x01,        //   Usage (0x01)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x3F,        //   Report Count (63)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x85, 0x81,        //   Report ID (129)
    0x09, 0x02,        //   Usage (0x02)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x3F,        //   Report Count (63)
    0x81, 0x03,        //   Input (Const,Var,Abs)
    0x85, 0x01,        //   Report ID (1)
    0x09, 0x03,        //   Usage (0x03)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x3F,        //   Report Count (63)
    0x91, 0x83,        //   Output (Const,Var,Abs,Vol)
    0x85, 0x10,        //   Report ID (16)
    0x09, 0x04,        //   Usage (0x04)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x3F,        //   Report Count (63)
    0x91, 0x83,        //   Output (Const,Var,Abs,Vol)
    0x85, 0x80,        //   Report ID (128)
    0x09, 0x05,        //   Usage (0x05)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x3F,        //   Report Count (63)
    0x91, 0x83,        //   Output (Const,Var,Abs,Vol)
    0x85, 0x82,        //   Report ID (130)
    0x09, 0x06,        //   Usage (0x06)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x3F,        //   Report Count (63)
    0x91, 0x83,        //   Output (Const,Var,Abs,Vol)
    0xC0,              // End Collection
];

pub const FEATURES: &[FeatureReport] = &[];

/// The pad's Bluetooth address, locally administered.
pub const ADDRESS: [u8; 6] = [0x02, 0xA1, 0x10, 0x00, 0x00, 0x03];

/// Stick centre and range in the 12-bit units the SPI calibration uses.
const STICK_CENTRE: u16 = 0x800;
const STICK_RANGE: u16 = 0x7FF;

/// The flash image the host reads calibration and colours from: only the
/// regions a host asks for are populated, everything else reads as erased.
fn spi_read(address: u32, length: u8) -> Vec<u8> {
    let mut data = vec![0xFFu8; usize::from(length)];
    for (offset, byte) in data.iter_mut().enumerate() {
        let at = address + offset as u32;
        *byte = match at {
            0x6012 => 0x03,
            0x6013 => 0x02,
            0x6020..=0x603A => imu_calibration(at - 0x6020),
            0x603D..=0x604E => factory_stick(at - 0x603D),
            0x6050..=0x605B => colour(at - 0x6050),
            0x6080..=0x6097 => stick_parameters(at - 0x6080),
            0x6098..=0x60A9 => stick_parameters(at - 0x6098 + 6),
            _ => 0xFF,
        };
    }
    data
}

fn factory_stick(offset: u32) -> u8 {
    let left = pack_stick([
        STICK_RANGE,
        STICK_RANGE,
        STICK_CENTRE,
        STICK_CENTRE,
        STICK_RANGE,
        STICK_RANGE,
    ]);
    let right = pack_stick([
        STICK_CENTRE,
        STICK_CENTRE,
        STICK_RANGE,
        STICK_RANGE,
        STICK_RANGE,
        STICK_RANGE,
    ]);
    match offset {
        0..=8 => left[offset as usize],
        9..=17 => right[(offset - 9) as usize],
        _ => 0xFF,
    }
}

/// Packs six 12-bit values into nine bytes, little-endian nibble order.
fn pack_stick(values: [u16; 6]) -> [u8; 9] {
    let mut out = [0u8; 9];
    for pair in 0..3 {
        let a = values[pair * 2];
        let b = values[pair * 2 + 1];
        out[pair * 3] = (a & 0xFF) as u8;
        out[pair * 3 + 1] = (((a >> 8) & 0x0F) as u8) | (((b & 0x0F) as u8) << 4);
        out[pair * 3 + 2] = ((b >> 4) & 0xFF) as u8;
    }
    out
}

fn imu_calibration(offset: u32) -> u8 {
    const TABLE: [u8; 24] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x40, 0x00, 0x40, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x3B, 0x34, 0x3B, 0x34, 0x3B, 0x34,
    ];
    TABLE.get(offset as usize).copied().unwrap_or(0xFF)
}

fn colour(offset: u32) -> u8 {
    const TABLE: [u8; 12] = [
        0x32, 0x32, 0x32, 0xFF, 0xFF, 0xFF, 0x46, 0x46, 0x46, 0x28, 0x28, 0x28,
    ];
    TABLE.get(offset as usize).copied().unwrap_or(0xFF)
}

fn stick_parameters(offset: u32) -> u8 {
    const TABLE: [u8; 24] = [
        0x50, 0xFD, 0x00, 0x00, 0xC6, 0x0F, 0x0F, 0x30, 0x61, 0xAE, 0x90, 0xD9, 0xD4, 0x14, 0x54,
        0x41, 0x15, 0x54, 0xC7, 0x79, 0x9C, 0x33, 0x36, 0x63,
    ];
    TABLE.get(offset as usize).copied().unwrap_or(0xFF)
}

/// The handshake state the pad keeps between reports.
#[derive(Default)]
pub struct Codec {
    timer: u8,
    player: u8,
    rumble_on: bool,
}

impl Codec {
    fn tick(&mut self) -> u8 {
        self.timer = self.timer.wrapping_add(1);
        self.timer
    }

    /// The 12-byte button and stick block every input report starts with.
    fn state_block(&self, state: &GamepadState, into: &mut [u8]) {
        let mut right = 0u8;
        if state.pressed(buttons::Y) {
            right |= 1 << 0;
        }
        if state.pressed(buttons::X) {
            right |= 1 << 1;
        }
        if state.pressed(buttons::B) {
            right |= 1 << 2;
        }
        if state.pressed(buttons::A) {
            right |= 1 << 3;
        }
        if state.pressed(buttons::RIGHT_SHOULDER) {
            right |= 1 << 6;
        }
        if state.right_trigger > 0 {
            right |= 1 << 7;
        }

        let mut shared = 0u8;
        if state.pressed(buttons::BACK) {
            shared |= 1 << 0;
        }
        if state.pressed(buttons::START) {
            shared |= 1 << 1;
        }
        if state.pressed(buttons::RIGHT_THUMB) {
            shared |= 1 << 2;
        }
        if state.pressed(buttons::LEFT_THUMB) {
            shared |= 1 << 3;
        }
        if state.pressed(buttons::GUIDE) {
            shared |= 1 << 4;
        }

        let mut left = 0u8;
        if state.pressed(buttons::DPAD_DOWN) {
            left |= 1 << 0;
        }
        if state.pressed(buttons::DPAD_UP) {
            left |= 1 << 1;
        }
        if state.pressed(buttons::DPAD_RIGHT) {
            left |= 1 << 2;
        }
        if state.pressed(buttons::DPAD_LEFT) {
            left |= 1 << 3;
        }
        if state.pressed(buttons::LEFT_SHOULDER) {
            left |= 1 << 6;
        }
        if state.left_trigger > 0 {
            left |= 1 << 7;
        }

        into[0] = 0x90;
        into[1] = right;
        into[2] = shared;
        into[3] = left;

        let lx = stick_12(state.thumb_lx);
        let ly = stick_12(state.thumb_ly);
        let rx = stick_12(state.thumb_rx);
        let ry = stick_12(state.thumb_ry);
        into[4] = (lx & 0xFF) as u8;
        into[5] = (((lx >> 8) & 0x0F) as u8) | (((ly & 0x0F) as u8) << 4);
        into[6] = ((ly >> 4) & 0xFF) as u8;
        into[7] = (rx & 0xFF) as u8;
        into[8] = (((rx >> 8) & 0x0F) as u8) | (((ry & 0x0F) as u8) << 4);
        into[9] = ((ry >> 4) & 0xFF) as u8;
        into[10] = 0x0C;
    }

    /// The full 0x30 report, which is the only input layout the descriptor
    /// declares: a host that has not yet asked for it reads the same bytes.
    pub fn encode(&mut self, state: &GamepadState) -> Vec<u8> {
        let mut report = vec![0u8; INPUT_LEN];
        report[0] = FULL_REPORT;
        report[1] = self.tick();
        self.state_block(state, &mut report[2..13]);
        report
    }

    fn ack(&mut self, subcommand: u8, ack: u8, payload: &[u8]) -> Vec<u8> {
        let mut reply = vec![0u8; INPUT_LEN];
        reply[0] = SUBCOMMAND_REPLY;
        reply[1] = self.tick();
        self.state_block(&GamepadState::default(), &mut reply[2..13]);
        reply[13] = ack;
        reply[14] = subcommand;
        let end = (15 + payload.len()).min(INPUT_LEN);
        reply[15..end].copy_from_slice(&payload[..end - 15]);
        reply
    }

    fn subcommand(&mut self, report: &[u8], decoded: &mut Decoded) {
        let Some(&subcommand) = report.get(10) else {
            return;
        };
        let args = &report[11..];
        self.rumble(&report[2..10.min(report.len())], decoded);
        let reply = match subcommand {
            0x02 => {
                let mut info = vec![0x04, 0x21, 0x03, 0x02];
                info.extend_from_slice(&ADDRESS);
                info.extend_from_slice(&[0x01, 0x02]);
                self.ack(subcommand, 0x82, &info)
            }
            0x03 | 0x04 | 0x08 | 0x22 | 0x38 | 0x40 | 0x41 | 0x42 | 0x43 => {
                self.ack(subcommand, 0x80, &[])
            }
            0x10 => {
                if args.len() < 5 {
                    self.ack(subcommand, 0x80, &[])
                } else {
                    let address = u32::from_le_bytes([args[0], args[1], args[2], args[3]]);
                    let length = args[4].min(0x1D);
                    let mut payload = Vec::with_capacity(5 + usize::from(length));
                    payload.extend_from_slice(&args[..5]);
                    payload.extend_from_slice(&spi_read(address, length));
                    self.ack(subcommand, 0x90, &payload)
                }
            }
            0x30 => {
                let lamps = args.first().copied().unwrap_or(0) & 0x0F;
                self.player = match lamps {
                    0x01 => 1,
                    0x03 => 2,
                    0x07 => 3,
                    0x0F => 4,
                    0x00 => 0,
                    other => other.count_ones() as u8,
                };
                decoded.outputs.push(GamepadOutput::PlayerLed(self.player));
                self.ack(subcommand, 0x80, &[])
            }
            0x48 => {
                self.rumble_on = args.first() == Some(&0x01);
                self.ack(subcommand, 0x80, &[])
            }
            0x50 => self.ack(subcommand, 0xD0, &[0x83, 0x06, 0x00]),
            _ => self.ack(subcommand, 0x80, &[]),
        };
        decoded.reply = Some(reply);
    }

    fn rumble(&mut self, data: &[u8], decoded: &mut Decoded) {
        if !self.rumble_on || data.len() < 8 {
            return;
        }
        let left = amplitude(&data[0..4]);
        let right = amplitude(&data[4..8]);
        decoded.outputs.push(GamepadOutput::Rumble {
            large: left,
            small: right,
        });
    }

    fn usb(&mut self, report: &[u8], decoded: &mut Decoded) {
        let command = report.get(1).copied().unwrap_or(0);
        let mut reply = vec![0u8; INPUT_LEN];
        reply[0] = USB_REPLY;
        reply[1] = command;
        match command {
            0x01 => {
                reply[2] = 0x00;
                reply[3] = 0x03;
                for (index, byte) in ADDRESS.iter().rev().enumerate() {
                    reply[4 + index] = *byte;
                }
            }
            0x02 | 0x03 | 0x04 | 0x05 | 0x06 | 0x91 | 0x92 => {}
            _ => {}
        }
        decoded.reply = Some(reply);
    }

    pub fn decode(&mut self, report: &[u8]) -> Decoded {
        let mut decoded = Decoded::default();
        match report.first() {
            Some(&OUTPUT_SUBCOMMAND) if report.len() >= 11 => self.subcommand(report, &mut decoded),
            Some(&OUTPUT_RUMBLE) if report.len() >= 10 => self.rumble(&report[2..10], &mut decoded),
            Some(&OUTPUT_USB) => self.usb(report, &mut decoded),
            _ => decoded.outputs.push(GamepadOutput::Raw(report.to_vec())),
        }
        decoded
    }
}

/// A signed axis as the 12-bit stick value, centred on 0x800.
fn stick_12(value: i16) -> u16 {
    ((i32::from(value) + 32768) >> 4).clamp(0, 0xFFF) as u16
}

/// The rumble amplitude a four-byte motor block encodes, as 0..=255.
///
/// The high band's amplitude sits in byte 1 (0x00..=0xC8), the low band's in
/// byte 3 with its ninth bit in byte 2 (0x40 silent, 0x72 full); the stronger
/// of the two is the strength the game meant.
fn amplitude(block: &[u8]) -> u8 {
    let high = u16::from(block[1] & 0xFE);
    let low = (u16::from(block[2] & 0x80) << 1) | u16::from(block[3]);
    let high_strength = (high * 255 / 0xC8).min(255);
    let low_strength = (low.saturating_sub(0x40) * 255 / 0x32).min(255);
    high_strength.max(low_strength) as u8
}
