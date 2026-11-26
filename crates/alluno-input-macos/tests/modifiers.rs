//! The modifier keys a keyboard holds and the flags its events carry, nothing posted.

#![cfg(target_os = "macos")]

use alluno_input_core::Key;
use alluno_input_macos::cg::{CgKeyboard, Modifiers, event_flags, key_flags, modifier_flag};
use core_graphics::event::CGEventFlags;

const GLOBE: CGEventFlags = CGEventFlags::CGEventFlagSecondaryFn;
const KEYPAD: CGEventFlags = CGEventFlags::CGEventFlagNumericPad;
const NONE: CGEventFlags = CGEventFlags::CGEventFlagNull;

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

/// An arrow, navigation or F key left the Globe key's flag in the state the system takes key
/// events from, and every key made after it went out with that flag: E opened the emoji picker, D
/// started dictation and other letters went missing, until a key was pressed on the Mac itself.
#[test]
fn regression_a_key_never_carries_the_globe_flag_an_earlier_key_left() {
    let left = GLOBE | KEYPAD;
    for key in [Key::E, Key::D, Key::A, Key::Enter] {
        for pressed in [true, false] {
            let flags = event_flags(left, key, pressed, &Modifiers::default());
            assert!(
                !flags.intersects(left),
                "{key:?} went out with the flags an arrow key left: {flags:?}"
            );
        }
    }
}

#[test]
fn an_arrow_key_carries_its_own_flags_only_while_it_is_pressed() {
    let none = Modifiers::default();
    assert_eq!(event_flags(NONE, Key::Up, true, &none), GLOBE | KEYPAD);
    assert_eq!(event_flags(GLOBE | KEYPAD, Key::Up, false, &none), NONE);
    assert_eq!(event_flags(NONE, Key::F5, true, &none), GLOBE);
    assert_eq!(event_flags(NONE, Key::Numpad5, true, &none), KEYPAD);
}

#[test]
fn held_modifiers_ride_on_an_arrow_key() {
    let mut modifiers = Modifiers::default();
    modifiers.update(Key::ShiftLeft, true);
    assert_eq!(
        event_flags(NONE, Key::Left, true, &modifiers),
        CGEventFlags::CGEventFlagShift | GLOBE | KEYPAD
    );
}

#[test]
fn flags_no_key_decides_pass_through() {
    let caps = CGEventFlags::CGEventFlagAlphaShift | CGEventFlags::CGEventFlagNonCoalesced;
    assert_eq!(
        event_flags(caps | GLOBE, Key::E, true, &Modifiers::default()),
        caps
    );
}

#[test]
fn only_the_arrow_navigation_f_and_keypad_keys_have_flags_of_their_own() {
    let own: Vec<Key> = Key::ALL
        .iter()
        .copied()
        .filter(|key| key_flags(*key) != NONE)
        .collect();
    assert_eq!(own.len(), 4 + 12 + 7 + 16, "{own:?}");
    for key in [Key::Left, Key::Right, Key::Up, Key::Down] {
        assert_eq!(key_flags(key), GLOBE | KEYPAD, "{key:?}");
    }
}

#[test]
fn an_event_made_from_the_system_state_gets_the_flags_its_key_decides() {
    let mut keyboard = CgKeyboard::open().expect("event source");
    let down = keyboard.event(Key::Up, true).expect("up down");
    assert!(down.get_flags().contains(GLOBE | KEYPAD), "up pressed");
    let up = keyboard.event(Key::Up, false).expect("up up");
    assert!(!up.get_flags().intersects(GLOBE | KEYPAD), "up released");
    let letter = keyboard.event(Key::E, true).expect("e down");
    assert!(!letter.get_flags().intersects(GLOBE | KEYPAD), "e pressed");
}
