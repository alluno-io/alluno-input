//! The driver contract: struct layouts, IOCTL codes, device names and the key
//! map, none of which needs the driver installed to check.

#![cfg(target_os = "windows")]

use std::collections::HashSet;

use alluno_input_core::Key;
use alluno_input_windows::kernel::{
    IOCTL_KEYBOARD_SEND, IOCTL_MOUSE_SEND, KEYBOARD_DEVICE, KeyboardInputData, MOUSE_DEVICE,
    MouseInputData,
};
use alluno_input_windows::scan;

#[test]
fn the_wire_structs_match_the_kernel_layouts() {
    assert_eq!(std::mem::size_of::<KeyboardInputData>(), 12);
    assert_eq!(std::mem::size_of::<MouseInputData>(), 24);
}

#[test]
fn the_ioctl_codes_are_the_ones_driver_h_declares() {
    assert_eq!(IOCTL_KEYBOARD_SEND, 0x000B_2080);
    assert_eq!(IOCTL_MOUSE_SEND, 0x000F_2080);
}

#[test]
fn the_control_devices_carry_the_filter_service_names() {
    assert!(KEYBOARD_DEVICE.starts_with("\\\\.\\Keyboard"));
    assert!(MOUSE_DEVICE.starts_with("\\\\.\\Mouse"));
    assert!(KEYBOARD_DEVICE.ends_with("AllunoInput\0"));
    assert!(MOUSE_DEVICE.ends_with("AllunoInput\0"));
}

#[test]
fn every_key_but_pause_has_a_single_stroke() {
    for key in Key::ALL {
        let mapped = scan::of(*key);
        if *key == Key::Pause {
            assert!(mapped.is_none(), "Pause is an E1 sequence, not one stroke");
        } else {
            assert!(mapped.is_some(), "{key:?} has no scan code");
        }
    }
}

#[test]
fn the_extended_keys_are_told_apart_from_the_numpad_by_the_prefix_alone() {
    let (insert, insert_e0) = scan::of(Key::Insert).unwrap();
    let (numpad0, numpad0_e0) = scan::of(Key::Numpad0).unwrap();
    assert_eq!(insert, numpad0);
    assert!(insert_e0 && !numpad0_e0);

    let (right_ctrl, e0) = scan::of(Key::ControlRight).unwrap();
    assert_eq!((right_ctrl, e0), (scan::code::LEFT_CTRL, true));
    assert_eq!(scan::of(Key::Meta), Some((scan::code::LEFT_WIN, true)));
}

#[test]
fn no_two_non_extended_keys_share_a_code() {
    let mut seen = HashSet::new();
    for key in Key::ALL {
        if let Some((code, false)) = scan::of(*key) {
            assert!(seen.insert(code), "{key:?} reuses {code:#04x}");
        }
    }
}
