//! The facade answers on every platform it builds for.

use alluno_input::{GamepadProfile, Host, Input, Options};

#[test]
fn the_probe_lists_every_profile_exactly_once() {
    let caps = alluno_input::probe();
    assert_eq!(caps.gamepads.len(), GamepadProfile::ALL.len());
    for profile in GamepadProfile::ALL {
        assert_eq!(
            caps.gamepads.iter().filter(|(p, _)| p == profile).count(),
            1
        );
    }
}

#[test]
fn check_and_new_are_probe_and_open() {
    assert_eq!(Input::check(), Input::probe());
    assert_eq!(alluno_input::check(), alluno_input::probe());
    let host = Input::new(Options::default()).expect("new opens like open");
    assert_eq!(
        host.keyboard().is_ok(),
        Input::open(Options::default())
            .expect("open")
            .keyboard()
            .is_ok()
    );
}

#[test]
fn opening_the_host_never_fails_on_a_machine_without_drivers() {
    let host = Input::open(Options::default()).expect("opening the host costs nothing");
    let caps = Input::probe();
    if !caps.keyboard.available() {
        assert!(host.keyboard().is_err());
    }
    if !caps.gamepad(GamepadProfile::DualSense).available() {
        assert!(host.gamepad(GamepadProfile::DualSense).is_err());
    }
}
