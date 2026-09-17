//! The wired Xbox 360 controller as XInput reads it: not HID but the XUSB
//! protocol Microsoft's own driver speaks over USB. A bus that presents this
//! identity gets that driver bound to it, and everything a game sends and
//! receives is two packet shapes: the 20-byte interrupt packet carrying the
//! XInput state, and the rumble and player-lamp packets coming back.

use super::{Decoded, Identity};
use crate::{GamepadOutput, GamepadState};

/// The wired Xbox 360 controller.
pub const IDENTITY: Identity = Identity {
    vendor: 0x045E,
    product: 0x028E,
    version: 0x0114,
};

/// The interrupt packet: type, size, then the XInput state and padding.
pub const INPUT_LEN: usize = 20;

/// The rumble packet, the largest one a game writes.
pub const OUTPUT_LEN: usize = 8;

const INPUT_TYPE: u8 = 0x00;
const INPUT_SIZE: u8 = 0x14;
const LAMP_TYPE: u8 = 0x01;
const LAMP_SIZE: u8 = 0x03;
const RUMBLE_TYPE: u8 = 0x00;
const RUMBLE_SIZE: u8 = 0x08;

/// The interrupt packet for a snapshot.
pub fn packet(state: &GamepadState) -> [u8; INPUT_LEN] {
    let mut packet = [0u8; INPUT_LEN];
    packet[0] = INPUT_TYPE;
    packet[1] = INPUT_SIZE;
    packet[2..4].copy_from_slice(&state.buttons.to_le_bytes());
    packet[4] = state.left_trigger;
    packet[5] = state.right_trigger;
    packet[6..8].copy_from_slice(&state.thumb_lx.to_le_bytes());
    packet[8..10].copy_from_slice(&state.thumb_ly.to_le_bytes());
    packet[10..12].copy_from_slice(&state.thumb_rx.to_le_bytes());
    packet[12..14].copy_from_slice(&state.thumb_ry.to_le_bytes());
    packet
}

/// The 0-based player slot a lamp pattern names, for the four steady patterns.
pub fn player_of_lamp(pattern: u8) -> Option<u8> {
    match pattern {
        0x02..=0x05 => Some(pattern - 0x02),
        _ => None,
    }
}

/// Decodes a packet the game wrote: a lamp change or a rumble command.
pub fn decode(packet: &[u8]) -> Decoded {
    let mut decoded = Decoded::default();
    match packet {
        [LAMP_TYPE, LAMP_SIZE, pattern, ..] => {
            let player = player_of_lamp(*pattern).map_or(0, |slot| slot + 1);
            decoded.outputs.push(GamepadOutput::PlayerLed(player));
        }
        [RUMBLE_TYPE, RUMBLE_SIZE, _, large, small, ..] => {
            decoded.outputs.push(GamepadOutput::Rumble {
                large: *large,
                small: *small,
            });
        }
        other => decoded.outputs.push(GamepadOutput::Raw(other.to_vec())),
    }
    decoded
}
