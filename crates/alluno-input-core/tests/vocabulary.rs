//! The state vocabulary behaves the way every backend assumes.

use std::collections::HashSet;

use alluno_input_core::{
    Backing, Capabilities, Error, GamepadProfile, GamepadState, Key, MAX_CONTACTS, MouseButton,
    TouchContact, TouchState, buttons,
};

#[test]
fn a_gamepad_snapshot_is_laid_out_like_xinput() {
    assert_eq!(std::mem::size_of::<GamepadState>(), 12);
    assert_eq!(std::mem::align_of::<GamepadState>(), 2);
}

#[test]
fn every_button_bit_is_distinct_and_the_set_is_complete() {
    let bits: HashSet<u16> = buttons::ALL.iter().copied().collect();
    assert_eq!(bits.len(), buttons::ALL.len());
    for bit in buttons::ALL {
        assert_eq!(bit.count_ones(), 1, "{bit:#06x} is not a single bit");
    }
    assert_eq!(buttons::ALL.iter().fold(0u16, |acc, b| acc | b), 0xF7FF);
}

#[test]
fn normalized_axes_fill_the_signed_range_and_clamp_past_it() {
    let full = GamepadState::from_normalized(buttons::A, (1.0, -1.0), (0.5, 0.0), (1.0, 0.25));
    assert_eq!(full.thumb_lx, i16::MAX);
    assert_eq!(full.thumb_ly, -i16::MAX);
    assert_eq!(full.thumb_rx, 16384);
    assert_eq!(full.thumb_ry, 0);
    assert_eq!(full.left_trigger, 255);
    assert_eq!(full.right_trigger, 64);
    assert!(full.pressed(buttons::A));
    assert!(!full.pressed(buttons::B));

    let wild = GamepadState::from_normalized(0, (7.0, -9.0), (0.0, 0.0), (-1.0, 3.0));
    assert_eq!(wild.thumb_lx, i16::MAX);
    assert_eq!(wild.thumb_ly, -i16::MAX);
    assert_eq!(wild.left_trigger, 0);
    assert_eq!(wild.right_trigger, 255);
}

#[test]
fn the_key_and_button_lists_are_complete_and_free_of_repeats() {
    let keys: HashSet<Key> = Key::ALL.iter().copied().collect();
    assert_eq!(keys.len(), Key::ALL.len());
    assert_eq!(Key::ALL.len(), 99);

    let buttons: HashSet<MouseButton> = MouseButton::ALL.iter().copied().collect();
    assert_eq!(buttons.len(), MouseButton::ALL.len());
    assert_eq!(GamepadProfile::ALL.len(), 7);
}

#[test]
fn capabilities_answer_for_a_profile_nobody_listed() {
    let caps = Capabilities {
        keyboard: Backing::Kernel,
        mouse: Backing::UserApi,
        pen: Backing::Unavailable("no digitiser".into()),
        touch: Backing::Unavailable("no digitiser".into()),
        gamepads: vec![
            (GamepadProfile::Xbox360, Backing::Bus),
            (
                GamepadProfile::DualSense,
                Backing::Unavailable("no driver".into()),
            ),
        ],
        max_gamepads: Some(4),
        midi: Backing::Unavailable("no port".into()),
        camera: Backing::Unavailable("no source".into()),
        microphone: Backing::Unavailable("no endpoint".into()),
    };

    assert!(caps.keyboard.available());
    assert!(!caps.pen.available());
    assert_eq!(caps.gamepad(GamepadProfile::Xbox360), &Backing::Bus);
    assert!(!caps.gamepad(GamepadProfile::SwitchPro).available());
    assert_eq!(caps.available_gamepads(), vec![GamepadProfile::Xbox360]);
}

#[test]
fn a_touch_report_knows_when_it_is_too_wide() {
    let mut state = TouchState::default();
    assert!(state.fits());
    state.contacts = (0..=MAX_CONTACTS as u8)
        .map(|id| TouchContact {
            id,
            down: true,
            ..TouchContact::default()
        })
        .collect();
    assert!(!state.fits());
}

#[test]
fn errors_say_what_is_missing() {
    let error = Error::unavailable("AllunoInput keyboard filter not installed");
    assert_eq!(
        error.to_string(),
        "unavailable: AllunoInput keyboard filter not installed"
    );
    let io: Error = std::io::Error::other("closed").into();
    assert!(matches!(io, Error::Io(_)));
}
