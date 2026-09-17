//! The facade answers on every platform it builds for.

use alluno_input::{GamepadProfile, Host, Options, Runtime};

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
fn opening_the_host_never_fails_on_a_machine_without_drivers() {
    let host = Runtime::open(Options::default()).expect("opening the host costs nothing");
    let caps = Runtime::probe();
    if !caps.keyboard.available() {
        assert!(host.keyboard().is_err());
    }
    if !caps.gamepad(GamepadProfile::DualSense).available() {
        assert!(host.gamepad(GamepadProfile::DualSense).is_err());
    }
}
