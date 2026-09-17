//! The pure half of the X11 layer: the keysym table, the button numbers and the
//! root-window scaling. Nothing here opens a display.

#![cfg(target_os = "linux")]

use alluno_input_core::{Key, MouseButton};
use alluno_input_linux::x11::{button_number, keysym_of, keysym_of_char, to_root, wheel_taps};

#[test]
fn every_key_has_a_keysym() {
    for key in Key::ALL {
        assert_ne!(keysym_of(*key), 0, "{key:?}");
    }
}

#[test]
fn letters_and_digits_are_latin_1_keysyms() {
    assert_eq!(keysym_of(Key::A), u32::from(b'a'));
    assert_eq!(keysym_of(Key::Z), u32::from(b'z'));
    assert_eq!(keysym_of(Key::Num0), u32::from(b'0'));
    assert_eq!(keysym_of(Key::Space), u32::from(b' '));
    assert_eq!(keysym_of(Key::F12), 0xffc9);
    assert_eq!(keysym_of(Key::Numpad9), 0xffb9);
}

#[test]
fn characters_beyond_latin_1_go_by_code_point() {
    assert_eq!(keysym_of_char('a'), u32::from(b'a'));
    assert_eq!(keysym_of_char('\n'), 0xff0d);
    assert_eq!(keysym_of_char('é'), 0xe9);
    assert_eq!(keysym_of_char('日'), 0x0100_0000 | u32::from('日'));
}

#[test]
fn every_button_has_a_number() {
    let numbers: Vec<u8> = MouseButton::ALL
        .iter()
        .map(|b| button_number(*b).unwrap_or_else(|| panic!("{b:?} has no X button")))
        .collect();
    assert_eq!(numbers.len(), MouseButton::ALL.len());
    for (i, a) in numbers.iter().enumerate() {
        assert!(!numbers[i + 1..].contains(a), "button {a} used twice");
    }
    assert_eq!(button_number(MouseButton::Left), Some(1));
    assert_eq!(button_number(MouseButton::Back), Some(8));
}

#[test]
fn wheel_notches_become_button_taps() {
    assert_eq!(wheel_taps(0, 2), vec![4, 4]);
    assert_eq!(wheel_taps(0, -1), vec![5]);
    assert_eq!(wheel_taps(1, -2), vec![5, 5, 7]);
    assert_eq!(wheel_taps(-1, 0), vec![6]);
    assert!(wheel_taps(0, 0).is_empty());
}

#[test]
fn desktop_coordinates_land_on_the_root_extent() {
    assert_eq!(to_root(0, 1920), 0);
    assert_eq!(to_root(65535, 1920), 1919);
    assert_eq!(to_root(32768, 1080), 539);
    assert_eq!(to_root(65535, 0), 0);
}
