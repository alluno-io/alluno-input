//! The key table and the pen transitions, no display needed.

#![cfg(target_os = "macos")]

use std::collections::HashSet;

use alluno_input_core::{Key, MouseButton, PenState};
use alluno_input_macos::cg::{Desktop, button_events, pen_event};
use alluno_input_macos::keymap;
use core_graphics::event::CGEventType;

#[test]
fn every_key_has_its_own_virtual_key_code() {
    let mut seen = HashSet::new();
    for key in Key::ALL {
        let code = keymap::to_keycode(*key);
        assert!(seen.insert(code), "{key:?} reuses key code {code:#x}");
    }
}

#[test]
fn every_mouse_button_has_an_event_triple() {
    for button in MouseButton::ALL {
        assert!(button_events(*button).is_some(), "{button:?}");
    }
    assert_eq!(button_events(MouseButton::Back).unwrap().4, 3);
}

#[test]
fn a_pen_sample_becomes_the_event_its_edge_calls_for() {
    let hover = PenState {
        in_range: true,
        ..PenState::default()
    };
    let down = PenState {
        down: true,
        ..PenState::default()
    };
    let barrel = PenState {
        down: true,
        barrel: true,
        ..PenState::default()
    };
    assert!(matches!(pen_event(None, &hover).0, CGEventType::MouseMoved));
    assert!(matches!(
        pen_event(Some(&hover), &down).0,
        CGEventType::LeftMouseDown
    ));
    assert!(matches!(
        pen_event(Some(&down), &down).0,
        CGEventType::LeftMouseDragged
    ));
    assert!(matches!(
        pen_event(Some(&down), &hover).0,
        CGEventType::LeftMouseUp
    ));
    assert!(matches!(
        pen_event(Some(&hover), &barrel).0,
        CGEventType::RightMouseDown
    ));
}

#[test]
fn the_desktop_maps_the_far_corner_inside_itself() {
    let desktop = Desktop {
        left: -1440.0,
        top: 0.0,
        width: 2880.0,
        height: 900.0,
    };
    let far = desktop.point(65535, 65535);
    assert_eq!((far.x, far.y), (1439.0, 899.0));
    let origin = desktop.point(0, 0);
    assert_eq!((origin.x, origin.y), (-1440.0, 0.0));
}
