//! The fake host records what it is fed and delivers what a test feeds back.

use std::sync::{Arc, Mutex};

use alluno_input_core::{
    Gamepad, GamepadOutput, GamepadProfile, GamepadState, Host, Key, MouseButton, Options, buttons,
};
use alluno_input_testkit::{FakeHost, Recorded, fake_gamepad};

#[test]
fn every_device_writes_to_the_one_log_in_order() {
    let host = FakeHost::open(Options::default()).unwrap();
    let log = host.recorder();

    host.keyboard().unwrap().key(Key::A, true).unwrap();
    host.mouse().unwrap().move_abs(1, 2).unwrap();
    host.mouse()
        .unwrap()
        .button(MouseButton::Left, true)
        .unwrap();
    host.gamepad(GamepadProfile::Xbox360)
        .unwrap()
        .submit(&GamepadState {
            buttons: buttons::A,
            ..GamepadState::default()
        })
        .unwrap();

    assert_eq!(
        log.all(),
        vec![
            Recorded::Key {
                key: Key::A,
                pressed: true
            },
            Recorded::MoveAbs { x: 1, y: 2 },
            Recorded::Button {
                button: MouseButton::Left,
                pressed: true
            },
            Recorded::Gamepad {
                profile: GamepadProfile::Xbox360,
                state: GamepadState {
                    buttons: buttons::A,
                    ..GamepadState::default()
                }
            },
        ]
    );
    log.clear();
    assert!(log.is_empty());
}

#[test]
fn game_output_reaches_the_installed_sink_and_nowhere_before_it() {
    let (mut pad, _log) = fake_gamepad(GamepadProfile::DualSense);
    let feed = pad.output();
    let heard = Arc::new(Mutex::new(Vec::new()));

    assert!(!feed.feed(GamepadOutput::PlayerLed(1)), "no sink yet");

    let into = heard.clone();
    pad.on_output(Box::new(move |o| into.lock().unwrap().push(o)))
        .unwrap();
    assert!(feed.feed(GamepadOutput::Rumble { large: 9, small: 3 }));

    assert_eq!(
        heard.lock().unwrap().as_slice(),
        &[GamepadOutput::Rumble { large: 9, small: 3 }]
    );
    assert_eq!(pad.slot(), Some(0));
}

#[test]
fn the_fake_answers_available_for_everything() {
    let caps = FakeHost::probe();
    assert!(caps.keyboard.available() && caps.touch.available());
    assert_eq!(caps.available_gamepads().len(), GamepadProfile::ALL.len());
}
