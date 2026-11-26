//! The modifier keys a keyboard holds and the flags its events carry, nothing posted.

#![cfg(target_os = "macos")]

use alluno_input_core::Key;
use alluno_input_macos::cg::{CgKeyboard, Modifiers, modifier_flag};
use core_graphics::event::CGEventFlags;

#[test]
fn a_held_modifier_sets_its_flag_until_it_is_released() {
    let mut modifiers = Modifiers::default();
    modifiers.update(Key::Meta, true);
    modifiers.update(Key::C, true);
    assert_eq!(modifiers.flags(), CGEventFlags::CGEventFlagCommand);
    modifiers.update(Key::Meta, false);
    assert_eq!(modifiers.flags(), CGEventFlags::CGEventFlagNull);
}

#[test]
fn releasing_one_side_keeps_the_flag_while_the_other_is_held() {
    let mut modifiers = Modifiers::default();
    modifiers.update(Key::ShiftLeft, true);
    modifiers.update(Key::ShiftRight, true);
    modifiers.update(Key::ShiftLeft, false);
    assert_eq!(modifiers.flags(), CGEventFlags::CGEventFlagShift);
    modifiers.update(Key::ShiftRight, false);
    assert_eq!(modifiers.flags(), CGEventFlags::CGEventFlagNull);
}

#[test]
fn only_the_modifier_keys_have_a_flag() {
    let flagged: Vec<Key> = Key::ALL
        .iter()
        .copied()
        .filter(|key| modifier_flag(*key).is_some())
        .collect();
    assert_eq!(
        flagged,
        [
            Key::ShiftLeft,
            Key::ShiftRight,
            Key::ControlLeft,
            Key::ControlRight,
            Key::AltLeft,
            Key::AltRight,
            Key::Meta,
        ]
    );
}

/// Each key event took the modifier flags the system had taken in by the time it was made, so a
/// key that arrived right behind its Control or Command press came out as the bare letter: Control+C
/// typed a c and stopped nothing.
#[test]
fn regression_a_key_right_after_a_modifier_carries_its_flag() {
    let mut keyboard = CgKeyboard::open().expect("event source");
    keyboard
        .event(Key::ControlLeft, true)
        .expect("control down");
    let held = keyboard.event(Key::C, true).expect("c down");
    assert!(
        held.get_flags().contains(CGEventFlags::CGEventFlagControl),
        "c pressed while Control is held lost the Control flag"
    );
    keyboard.event(Key::C, false).expect("c up");
    keyboard.event(Key::ControlLeft, false).expect("control up");
    let bare = keyboard.event(Key::C, true).expect("c down again");
    assert!(
        !bare.get_flags().contains(CGEventFlags::CGEventFlagControl),
        "c pressed after Control was released still carries its flag"
    );
}
