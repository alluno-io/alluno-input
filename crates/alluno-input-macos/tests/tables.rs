//! The key table and the pen transitions, no display needed.

#![cfg(target_os = "macos")]

use std::collections::HashSet;
use std::time::{Duration, Instant};

use alluno_input_core::{Key, MouseButton, PenState};
use alluno_input_macos::cg::{
    ClickRun, Desktop, accessibility_trusted, button_events, double_click_interval, pen_event,
};
use alluno_input_macos::keymap;
use core_graphics::event::CGEventType;

#[test]
fn clicks_within_the_interval_at_the_same_spot_count_up() {
    let mut run = ClickRun::new(Duration::from_millis(500));
    let t0 = Instant::now();
    assert_eq!(run.press(MouseButton::Left, (10.0, 10.0), t0), 1);
    assert_eq!(run.current(MouseButton::Left), 1);
    let t1 = t0 + Duration::from_millis(200);
    assert_eq!(run.press(MouseButton::Left, (12.0, 9.0), t1), 2);
    assert_eq!(run.current(MouseButton::Left), 2);
    let t2 = t1 + Duration::from_millis(200);
    assert_eq!(run.press(MouseButton::Left, (11.0, 11.0), t2), 3);
    assert_eq!(run.current(MouseButton::Left), 3);
}

#[test]
fn a_slow_far_or_other_button_click_starts_a_new_run() {
    let mut run = ClickRun::new(Duration::from_millis(500));
    let t0 = Instant::now();
    run.press(MouseButton::Left, (10.0, 10.0), t0);
    let slow = t0 + Duration::from_millis(600);
    assert_eq!(run.press(MouseButton::Left, (10.0, 10.0), slow), 1);
    let quick = slow + Duration::from_millis(100);
    assert_eq!(run.press(MouseButton::Left, (10.0, 10.0), quick), 2);
    let far = quick + Duration::from_millis(100);
    assert_eq!(run.press(MouseButton::Left, (40.0, 10.0), far), 1);
    let other = far + Duration::from_millis(100);
    assert_eq!(run.press(MouseButton::Right, (40.0, 10.0), other), 1);
    assert_eq!(run.current(MouseButton::Left), 1);
}

#[test]
fn the_trust_probe_answers_without_opening_a_dialog() {
    let trusted = accessibility_trusted();
    let caps = <alluno_input_macos::Input as alluno_input_core::Host>::probe();
    assert_eq!(caps.keyboard.available(), trusted);
    assert_eq!(caps.mouse.available(), trusted);
    assert_eq!(caps.pen.available(), trusted);
}

#[test]
fn the_double_click_interval_is_a_sane_number_of_seconds() {
    let interval = double_click_interval();
    assert!(interval >= Duration::from_millis(50), "{interval:?}");
    assert!(interval <= Duration::from_secs(5), "{interval:?}");
}

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
