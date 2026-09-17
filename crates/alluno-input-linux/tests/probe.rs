//! The probe answers without touching a device, installed or not.

#![cfg(target_os = "linux")]

use alluno_input_core::{GamepadProfile, Host};
use alluno_input_linux::Runtime;

#[test]
fn the_probe_answers_every_profile_and_the_xbox_pads_come_from_uhid() {
    let caps = Runtime::probe();
    assert_eq!(
        caps.gamepad(GamepadProfile::XboxSeries).available(),
        alluno_input_linux::uhid::available()
    );
    assert_eq!(caps.gamepads.len(), GamepadProfile::ALL.len());
    assert_eq!(caps.max_gamepads, None);
    assert!(!caps.midi.available());
}

#[test]
fn the_uinput_pads_and_the_uhid_pads_answer_from_their_own_node() {
    let caps = Runtime::probe();
    assert_eq!(
        caps.gamepad(GamepadProfile::Xbox360).available(),
        alluno_input_linux::uinput::available()
    );
    assert_eq!(
        caps.gamepad(GamepadProfile::DualSense).available(),
        alluno_input_linux::uhid::available()
    );
}

#[test]
fn probing_is_stable() {
    assert_eq!(Runtime::probe(), Runtime::probe());
}
