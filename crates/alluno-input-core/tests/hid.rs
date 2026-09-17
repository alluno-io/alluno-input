//! Every descriptor parses, balances its collections, and declares reports
//! exactly as long as the codecs produce and consume.

use std::collections::BTreeMap;

use alluno_input_core::hid::{self, PadCodec};
use alluno_input_core::{
    GamepadOutput, GamepadProfile, GamepadState, Key, MAX_CONTACTS, MouseButton, PenState,
    TouchContact, TouchState, buttons,
};

/// Report sizes in bytes by (kind, report id): kind 0 input, 1 output, 2 feature.
#[derive(Debug, Default, PartialEq, Eq)]
struct Layout {
    sizes: BTreeMap<(u8, u8), usize>,
}

impl Layout {
    fn input(&self, id: u8) -> Option<usize> {
        self.sizes.get(&(0, id)).copied()
    }

    fn output(&self, id: u8) -> Option<usize> {
        self.sizes.get(&(1, id)).copied()
    }

    fn feature(&self, id: u8) -> Option<usize> {
        self.sizes.get(&(2, id)).copied()
    }

    fn largest_output(&self) -> usize {
        self.sizes
            .iter()
            .filter(|((kind, _), _)| *kind == 1)
            .map(|(_, size)| *size)
            .max()
            .unwrap_or(0)
    }
}

/// A short-item HID descriptor walk: bit counts per report, balanced
/// collections, no truncated item. Sizes include the id byte.
fn parse(descriptor: &[u8]) -> Layout {
    let mut bits: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut report_id = 0u8;
    let mut report_size = 0usize;
    let mut report_count = 0usize;
    let mut depth = 0i32;
    let mut at = 0usize;
    while at < descriptor.len() {
        let prefix = descriptor[at];
        assert_ne!(prefix, 0xFE, "long items are not used");
        let size = match prefix & 0x03 {
            3 => 4,
            n => n as usize,
        };
        let tag = prefix & 0xFC;
        assert!(at + 1 + size <= descriptor.len(), "truncated item at {at}");
        let data = &descriptor[at + 1..at + 1 + size];
        let value = data
            .iter()
            .rev()
            .fold(0u32, |acc, byte| (acc << 8) | u32::from(*byte));
        match tag {
            0xA0 => depth += 1,
            0xC0 => {
                depth -= 1;
                assert!(depth >= 0, "end collection without a start at {at}");
            }
            0x84 => report_id = value as u8,
            0x74 => report_size = value as usize,
            0x94 => report_count = value as usize,
            0x80 | 0x90 | 0xB0 => {
                let kind = match tag {
                    0x80 => 0,
                    0x90 => 1,
                    _ => 2,
                };
                *bits.entry((kind, report_id)).or_default() += report_size * report_count;
            }
            _ => {}
        }
        at += 1 + size;
    }
    assert_eq!(depth, 0, "collections are unbalanced");
    Layout {
        sizes: bits
            .into_iter()
            .map(|(key, bits)| {
                assert_eq!(bits % 8, 0, "report {key:?} is not byte aligned");
                (key, bits / 8 + 1)
            })
            .collect(),
    }
}

fn full_state() -> GamepadState {
    GamepadState {
        buttons: buttons::A | buttons::DPAD_UP | buttons::LEFT_SHOULDER | buttons::GUIDE,
        left_trigger: 255,
        right_trigger: 1,
        thumb_lx: i16::MAX,
        thumb_ly: i16::MIN,
        thumb_rx: 0,
        thumb_ry: -1,
    }
}

#[test]
fn every_pad_descriptor_matches_its_codec() {
    for profile in GamepadProfile::ALL {
        let Some(mut codec) = PadCodec::new(*profile) else {
            assert_eq!(*profile, GamepadProfile::Xbox360);
            continue;
        };
        let layout = parse(codec.descriptor());
        let report = codec.encode(&full_state());
        assert_eq!(report.len(), codec.input_len(), "{profile:?} input length");
        assert_eq!(
            layout.input(report[0]),
            Some(report.len()),
            "{profile:?} declares input {:#04x} at the encoded size",
            report[0]
        );
        assert_eq!(
            layout.largest_output(),
            codec.output_len(),
            "{profile:?} output length"
        );
        for feature in codec.features() {
            assert_eq!(feature.data[0], feature.id, "{profile:?} feature id byte");
            assert_eq!(
                layout.feature(feature.id),
                Some(feature.data.len()),
                "{profile:?} declares feature {:#04x} at the table's size",
                feature.id
            );
        }
        assert_ne!(codec.identity().vendor, 0);
        assert_ne!(codec.identity().product, 0);
        assert!(!codec.name().is_empty());
    }
}

#[test]
fn the_pad_codecs_are_only_for_hid_profiles() {
    assert!(!PadCodec::supports(GamepadProfile::Xbox360));
    assert!(PadCodec::supports(GamepadProfile::DualShock4));
    assert!(PadCodec::supports(GamepadProfile::GenericHid));
    assert!(PadCodec::supports(GamepadProfile::XboxOne));
    assert!(PadCodec::supports(GamepadProfile::XboxSeries));
}

#[test]
fn an_xbox_node_carries_the_bluetooth_layout_and_its_guide_button_apart() {
    let mut codec = PadCodec::new(GamepadProfile::XboxSeries).unwrap();
    assert_eq!(codec.identity().vendor, 0x045E);
    assert_eq!(codec.identity().product, 0x0B13);
    let report = codec.encode(&full_state());
    assert_eq!(report[0], 0x01);
    assert_eq!(&report[1..3], &u16::MAX.to_le_bytes(), "left x full right");
    assert_eq!(
        &report[3..5],
        &u16::MAX.to_le_bytes(),
        "left y full down reads high"
    );
    assert_eq!(&report[9..11], &1023u16.to_le_bytes(), "brake full");
    assert_eq!(&report[11..13], &4u16.to_le_bytes(), "accelerator one step");
    assert_eq!(report[13], 1, "hat north is 1, neutral is 0");
    assert_ne!(report[14] & 0x01, 0, "A");
    assert_ne!(report[14] & 0x40, 0, "LB");
    let guide = codec
        .follow_up(&full_state())
        .expect("the guide button changed");
    assert_eq!(guide, vec![0x02, 1]);
    assert!(
        codec.follow_up(&full_state()).is_none(),
        "unchanged, no repeat"
    );
    let released = codec.follow_up(&GamepadState::default()).unwrap();
    assert_eq!(released, vec![0x02, 0]);

    let decoded = codec.decode(&[0x03, 0x0F, 0, 0, 100, 50, 0xFF, 0, 1]);
    assert_eq!(
        decoded.outputs,
        vec![
            GamepadOutput::Rumble {
                large: 255,
                small: 127
            },
            GamepadOutput::TriggerRumble { left: 0, right: 0 },
        ]
    );
}

#[test]
fn the_hat_follows_the_dpad_clockwise_from_north() {
    let hat_of = |bits: u16| {
        hid::hat(&GamepadState {
            buttons: bits,
            ..GamepadState::default()
        })
    };
    assert_eq!(hat_of(buttons::DPAD_UP), 0);
    assert_eq!(hat_of(buttons::DPAD_UP | buttons::DPAD_RIGHT), 1);
    assert_eq!(hat_of(buttons::DPAD_RIGHT), 2);
    assert_eq!(hat_of(buttons::DPAD_DOWN | buttons::DPAD_RIGHT), 3);
    assert_eq!(hat_of(buttons::DPAD_DOWN), 4);
    assert_eq!(hat_of(buttons::DPAD_DOWN | buttons::DPAD_LEFT), 5);
    assert_eq!(hat_of(buttons::DPAD_LEFT), 6);
    assert_eq!(hat_of(buttons::DPAD_UP | buttons::DPAD_LEFT), 7);
    assert_eq!(hat_of(0), 8);
    assert_eq!(hat_of(buttons::DPAD_UP | buttons::DPAD_DOWN), 8);
}

#[test]
fn sony_sticks_are_bytes_with_up_as_zero() {
    assert_eq!(hid::stick_byte(0), 128);
    assert_eq!(hid::stick_byte(i16::MAX), 255);
    assert_eq!(hid::stick_byte(i16::MIN), 0);
    assert_eq!(hid::stick_byte_inverted(i16::MAX), 0);
    assert_eq!(hid::stick_byte_inverted(i16::MIN), 255);
}

#[test]
fn a_dualshock4_report_carries_the_buttons_where_a_game_reads_them() {
    let mut codec = PadCodec::new(GamepadProfile::DualShock4).unwrap();
    let report = codec.encode(&full_state());
    assert_eq!(report[0], 0x01);
    assert_eq!(report[1], 255);
    assert_eq!(report[2], 255);
    assert_eq!(report[5] & 0x0F, 0, "hat north");
    assert_ne!(report[5] & 0x20, 0, "cross");
    assert_ne!(report[6] & 0x01, 0, "L1");
    assert_ne!(report[6] & 0x04, 0, "L2 as a button");
    assert_ne!(report[7] & 0x01, 0, "PS");
    assert_eq!((report[8], report[9]), (255, 1));
    for byte in &report[35..=42] {
        assert_eq!(*byte, 0x80, "no touchpad contact");
    }

    let second = codec.encode(&full_state());
    assert_ne!(second[7] >> 2, report[7] >> 2, "the counter advances");
}

#[test]
fn a_dualshock4_output_report_decodes_motors_and_light_bar() {
    let mut codec = PadCodec::new(GamepadProfile::DualShock4).unwrap();
    let mut out = vec![0u8; 32];
    out[0] = 0x05;
    out[1] = 0x03;
    out[4] = 10;
    out[5] = 200;
    out[6] = 1;
    out[7] = 2;
    out[8] = 3;
    let decoded = codec.decode(&out);
    assert_eq!(
        decoded.outputs,
        vec![
            GamepadOutput::Rumble {
                large: 200,
                small: 10
            },
            GamepadOutput::Rgb { r: 1, g: 2, b: 3 }
        ]
    );
    assert!(decoded.reply.is_none());
}

#[test]
fn a_dualsense_output_report_decodes_lamps_and_triggers() {
    let mut codec = PadCodec::new(GamepadProfile::DualSense).unwrap();
    let mut out = vec![0u8; 48];
    out[0] = 0x02;
    out[1] = 0x0F;
    out[2] = 0x14;
    out[3] = 7;
    out[4] = 9;
    out[12] = 60;
    out[23] = 90;
    out[44] = 0x15;
    out[45] = 4;
    out[46] = 5;
    out[47] = 6;
    let decoded = codec.decode(&out);
    assert_eq!(
        decoded.outputs,
        vec![
            GamepadOutput::Rumble { large: 9, small: 7 },
            GamepadOutput::TriggerRumble {
                left: 90,
                right: 60
            },
            GamepadOutput::PlayerLed(3),
            GamepadOutput::Rgb { r: 4, g: 5, b: 6 },
        ]
    );
}

#[test]
fn the_switch_pro_handshake_answers_every_step_on_the_input_pipe() {
    let mut codec = PadCodec::new(GamepadProfile::SwitchPro).unwrap();

    let handshake = codec.decode(&[0x80, 0x02]);
    let reply = handshake.reply.expect("USB commands are acknowledged");
    assert_eq!((reply[0], reply[1]), (0x81, 0x02));

    let mut info = vec![0u8; 11];
    info[0] = 0x01;
    info[10] = 0x02;
    let reply = codec.decode(&info).reply.expect("device info replies");
    assert_eq!((reply[0], reply[13], reply[14]), (0x21, 0x82, 0x02));
    assert_eq!(reply[17], 0x03, "a Pro Controller");

    let mut spi = vec![0u8; 16];
    spi[0] = 0x01;
    spi[10] = 0x10;
    spi[11..15].copy_from_slice(&0x603Du32.to_le_bytes());
    spi[15] = 18;
    let reply = codec.decode(&spi).reply.expect("SPI reads reply");
    assert_eq!((reply[13], reply[14]), (0x90, 0x10));
    assert_eq!(&reply[15..19], &0x603Du32.to_le_bytes());
    assert_eq!(reply[19], 18);
    assert!(
        reply[20..38].iter().any(|byte| *byte != 0xFF),
        "the factory calibration is populated"
    );
    assert_eq!(reply[21], 0xF7, "two packed 12-bit ranges of 0x7FF");

    let mut lamps = vec![0u8; 12];
    lamps[0] = 0x01;
    lamps[10] = 0x30;
    lamps[11] = 0x03;
    let decoded = codec.decode(&lamps);
    assert_eq!(decoded.outputs, vec![GamepadOutput::PlayerLed(2)]);

    let mut enable = vec![0u8; 12];
    enable[0] = 0x01;
    enable[10] = 0x48;
    enable[11] = 0x01;
    codec.decode(&enable);

    let mut rumble = vec![0u8; 10];
    rumble[0] = 0x10;
    rumble[2..6].copy_from_slice(&[0x00, 0xC8, 0x40, 0x40]);
    rumble[6..10].copy_from_slice(&[0x00, 0x01, 0x40, 0x40]);
    let decoded = codec.decode(&rumble);
    assert_eq!(
        decoded.outputs,
        vec![GamepadOutput::Rumble {
            large: 255,
            small: 0
        }]
    );

    let report = codec.encode(&full_state());
    assert_eq!(report[0], 0x30);
    assert_ne!(report[3] & 0x08, 0, "A");
    assert_ne!(report[5] & 0x02, 0, "up");
    assert_ne!(report[5] & 0x40, 0, "L");
    assert_ne!(report[5] & 0x80, 0, "ZL");
    assert_ne!(report[4] & 0x10, 0, "home");
}

#[test]
fn the_generic_pad_has_no_feedback_path() {
    let mut codec = PadCodec::new(GamepadProfile::GenericHid).unwrap();
    assert_eq!(codec.output_len(), 0);
    assert!(codec.features().is_empty());
    let report = codec.encode(&full_state());
    assert_eq!(report[1] & 0x01, 1, "button 1 is south");
    assert_eq!(report[3], 0, "hat north");
    assert_eq!(&report[4..6], &i16::MAX.to_le_bytes());
    assert_eq!(
        &report[6..8],
        &i16::MAX.to_le_bytes(),
        "a stick pushed fully down reads positive, saturated"
    );
    let decoded = codec.decode(&[0x02, 1, 2]);
    assert_eq!(decoded.outputs, vec![GamepadOutput::Raw(vec![0x02, 1, 2])]);
}

#[test]
fn the_pen_descriptor_matches_its_report() {
    let layout = parse(hid::pen::DESCRIPTOR);
    let report = hid::pen::encode(&PenState {
        x: 65535,
        y: 1,
        pressure: 65535,
        tilt_x: -45,
        tilt_y: 90,
        twist: 359,
        down: true,
        barrel: true,
        eraser: false,
        in_range: true,
    });
    assert_eq!(layout.input(hid::pen::REPORT_ID), Some(report.len()));
    assert_eq!(report[1], 0b1_0011);
    assert_eq!(&report[2..4], &[0xFF, 0xFF]);
    assert_eq!(&report[6..8], &4095u16.to_le_bytes());
    assert_eq!(report[8] as i8, -45);
    assert_eq!(&report[10..12], &359u16.to_le_bytes());

    let hover = hid::pen::encode(&PenState {
        in_range: true,
        ..PenState::default()
    });
    assert_eq!(hover[1], 0b1_0000);
    assert_eq!(&hover[6..8], &[0, 0], "no pressure while hovering");

    let eraser = hid::pen::encode(&PenState {
        down: true,
        eraser: true,
        ..PenState::default()
    });
    assert_eq!(eraser[1], 0b1_1101, "eraser down sets invert and eraser");
    assert!(layout.output(hid::pen::REPORT_ID).is_none());
}

#[test]
fn the_touch_descriptor_matches_its_report_and_feature() {
    let layout = parse(hid::touch::DESCRIPTOR);
    let mut state = TouchState::default();
    state.contacts.push(TouchContact {
        id: 3,
        x: 100,
        y: 200,
        pressure: 32768,
        width: 10,
        height: 20,
        down: true,
    });
    state.contacts.push(TouchContact {
        id: 4,
        down: false,
        ..TouchContact::default()
    });
    let report = hid::touch::encode(&state);
    assert_eq!(layout.input(hid::touch::REPORT_ID), Some(report.len()));
    assert_eq!(report.len(), hid::touch::INPUT_LEN);
    assert_eq!(report[1], 1, "first slot down");
    assert_eq!(report[2], 3);
    assert_eq!(&report[3..5], &100u16.to_le_bytes());
    assert_eq!(&report[5..7], &200u16.to_le_bytes());
    assert_eq!(&report[7..9], &10u16.to_le_bytes());
    assert_eq!(&report[9..11], &20u16.to_le_bytes());
    assert_eq!(report[13], 0, "second slot lifted");
    assert_eq!(report[14], 4);
    assert_eq!(report[hid::touch::INPUT_LEN - 1], 2, "both count");
    for feature in hid::touch::FEATURES {
        assert_eq!(layout.feature(feature.id), Some(feature.data.len()));
        assert_eq!(feature.data[1] as usize, MAX_CONTACTS);
    }
}

#[test]
fn a_report_id_reads_from_the_first_byte() {
    assert_eq!(hid::report_id(&[]), 0);
    assert_eq!(hid::report_id(&[0x30, 1]), 0x30);
}

#[test]
fn every_key_has_one_keyboard_usage() {
    let mut seen = std::collections::HashSet::new();
    for key in Key::ALL {
        let usage = hid::keyboard::usage(*key).unwrap_or_else(|| panic!("{key:?} has no usage"));
        assert!(seen.insert(usage), "{key:?} reuses usage {usage:#04x}");
        assert!(usage < 0x80 || (0xE0..=0xE7).contains(&usage));
    }
}

#[test]
fn the_keyboard_descriptor_matches_its_bitmap_report() {
    let layout = parse(hid::keyboard::DESCRIPTOR);
    let mut codec = hid::keyboard::Codec::default();
    assert!(!codec.any_down());
    let report = codec.key(Key::ShiftLeft, true).unwrap();
    assert_eq!(layout.input(hid::keyboard::REPORT_ID), Some(report.len()));
    assert_eq!(
        layout.output(hid::keyboard::REPORT_ID),
        Some(hid::keyboard::OUTPUT_LEN)
    );
    assert_eq!(report[1], 0b0000_0010, "left shift is modifier bit 1");
    let report = codec.key(Key::A, true).unwrap();
    assert_eq!(
        report[2] & (1 << 4),
        1 << 4,
        "A is usage 0x04, bit 4 of byte 0"
    );
    assert_eq!(report[1], 0b0000_0010, "shift stays down");
    let report = codec.key(Key::ShiftLeft, false).unwrap();
    assert_eq!(report[1], 0);
    assert!(codec.any_down());
    codec.key(Key::A, false).unwrap();
    assert!(!codec.any_down());
    assert_eq!(
        codec.key(Key::Pause, true).unwrap()[2 + 9] & 1,
        1,
        "Pause is usage 0x48, bit 0 of byte 9"
    );
}

#[test]
fn the_mouse_descriptor_matches_its_relative_report() {
    let layout = parse(hid::mouse::DESCRIPTOR);
    let mut codec = hid::mouse::Codec::default();
    let report = codec.button(MouseButton::Right, true).unwrap();
    assert_eq!(layout.input(hid::mouse::REPORT_ID), Some(report.len()));
    assert_eq!(report.len(), hid::mouse::INPUT_LEN);
    assert_eq!(report[1], 0b10);
    let report = codec.move_rel(-3, 70_000);
    assert_eq!(report[1], 0b10, "buttons are restated");
    assert_eq!(&report[2..4], &(-3i16).to_le_bytes());
    assert_eq!(&report[4..6], &i16::MAX.to_le_bytes());
    let report = codec.wheel(2, -1);
    assert_eq!(report[6] as i8, -1, "vertical wheel");
    assert_eq!(report[7] as i8, 2, "horizontal pan");
    assert!(layout.output(hid::mouse::REPORT_ID).is_none());
    for button in MouseButton::ALL {
        assert!(hid::mouse::button_bit(*button).is_some(), "{button:?}");
    }
}

#[test]
fn the_xusb_packet_is_the_xinput_state_behind_a_two_byte_header() {
    let packet = hid::xusb::packet(&full_state());
    assert_eq!(packet.len(), hid::xusb::INPUT_LEN);
    assert_eq!(&packet[0..2], &[0x00, 0x14]);
    assert_eq!(&packet[2..4], &full_state().buttons.to_le_bytes());
    assert_eq!((packet[4], packet[5]), (255, 1));
    assert_eq!(&packet[6..8], &i16::MAX.to_le_bytes());
    assert_eq!(&packet[8..10], &i16::MIN.to_le_bytes());
    assert_eq!(&packet[14..20], &[0u8; 6]);

    assert_eq!(hid::xusb::player_of_lamp(0x02), Some(0));
    assert_eq!(hid::xusb::player_of_lamp(0x05), Some(3));
    assert_eq!(hid::xusb::player_of_lamp(0x0E), None);

    let lamp = hid::xusb::decode(&[0x01, 0x03, 0x03]);
    assert_eq!(lamp.outputs, vec![GamepadOutput::PlayerLed(2)]);
    let rumble = hid::xusb::decode(&[0x00, 0x08, 0x00, 200, 30, 0, 0, 0]);
    assert_eq!(
        rumble.outputs,
        vec![GamepadOutput::Rumble {
            large: 200,
            small: 30
        }]
    );
    let unknown = hid::xusb::decode(&[0x02, 0x03, 0x00]);
    assert_eq!(
        unknown.outputs,
        vec![GamepadOutput::Raw(vec![0x02, 0x03, 0x00])]
    );
}

#[test]
fn an_xusb_plug_carries_the_kind_and_no_descriptor() {
    let request = hid::wire::xusb_plug_request(hid::xusb::IDENTITY);
    assert_eq!(request.len(), 12);
    assert_eq!(&request[0..2], &0x045Eu16.to_le_bytes());
    assert_eq!(&request[2..4], &0x028Eu16.to_le_bytes());
    assert_eq!(&request[6..8], &[0, 0], "no descriptor");
    assert_eq!(&request[10..12], &hid::wire::KIND_XUSB.to_le_bytes());
    let hid_request = hid::wire::plug_request(hid::pen::IDENTITY, hid::pen::DESCRIPTOR, &[]);
    assert_eq!(&hid_request[10..12], &hid::wire::KIND_HID.to_le_bytes());
}
