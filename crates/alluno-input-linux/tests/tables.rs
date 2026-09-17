//! The evdev tables and the protocol B slot bookkeeping, no device needed.

#![cfg(target_os = "linux")]

use std::collections::HashSet;

use alluno_input_core::{Key, MouseButton};
use alluno_input_linux::devices::{Slots, button_code, tablet_pressure};
use alluno_input_linux::keymap;

#[test]
fn every_key_has_its_own_evdev_code() {
    let mut seen = HashSet::new();
    for key in Key::ALL {
        let code = keymap::to_evdev(*key);
        assert!(seen.insert(code), "{key:?} reuses evdev code {code}");
    }
    assert_eq!(keymap::to_evdev(Key::Pause), keymap::KEY_PAUSE);
}

#[test]
fn every_mouse_button_has_an_evdev_button() {
    for button in MouseButton::ALL {
        assert!(button_code(*button).is_some(), "{button:?}");
    }
}

#[test]
fn slots_are_claimed_once_per_id_and_freed_on_release() {
    let mut slots = Slots::default();
    assert_eq!(slots.claim(7), Some(0));
    assert_eq!(slots.claim(9), Some(1));
    assert_eq!(slots.claim(7), Some(0));
    assert_eq!(slots.find(9), Some(1));
    slots.release(7);
    assert_eq!(slots.find(7), None);
    assert_eq!(slots.claim(11), Some(0));
    assert_eq!(slots.live(), vec![11, 9]);
}

#[test]
fn the_slot_table_is_exactly_as_wide_as_a_report_may_be() {
    let mut slots = Slots::default();
    for id in 0..alluno_input_core::MAX_CONTACTS as u8 {
        assert!(slots.claim(id).is_some());
    }
    assert_eq!(slots.claim(200), None);
}

#[test]
fn tablet_pressure_spans_the_declared_range() {
    assert_eq!(tablet_pressure(0), 0);
    assert_eq!(tablet_pressure(65535), 8191);
}
