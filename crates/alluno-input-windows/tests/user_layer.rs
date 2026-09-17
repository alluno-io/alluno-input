//! The user-mode layer's packets and the pointer transitions, decided without
//! sending anything.

#![cfg(target_os = "windows")]

use alluno_input_core::{Key, MouseButton, PenState, TouchContact, TouchState};
use alluno_input_windows::pointer::{
    ContactMove, VirtualScreen, pen_flags, pen_pressure, touch_moves,
};
use alluno_input_windows::sendinput::{
    packet_button, packet_move_abs, packet_move_rel, packet_wheel, stroke_for,
};

#[test]
fn pause_goes_by_virtual_key_and_everything_else_by_scan_code() {
    let pause = stroke_for(Key::Pause, true).unwrap();
    assert_eq!((pause.scan, pause.vk, pause.up), (0, 0x13, false));

    let a = stroke_for(Key::A, false).unwrap();
    assert_eq!((a.scan, a.vk, a.extended, a.up), (0x1E, 0, false, true));

    let right_ctrl = stroke_for(Key::ControlRight, true).unwrap();
    assert!(right_ctrl.extended);
}

#[test]
fn every_key_reaches_the_user_layer() {
    for key in Key::ALL {
        assert!(
            stroke_for(*key, true).is_some(),
            "{key:?} has no user stroke"
        );
    }
}

#[test]
fn mouse_packets_carry_the_virtual_desktop_and_wheel_conventions() {
    let abs = packet_move_abs(65535, 0);
    assert_eq!((abs.dx, abs.dy), (65535, 0));
    assert_ne!(abs.flags & 0x8000, 0, "absolute");
    assert_ne!(abs.flags & 0x4000, 0, "virtual desktop");

    let rel = packet_move_rel(-3, 7);
    assert_eq!((rel.dx, rel.dy, rel.flags), (-3, 7, 0x0001));

    let back = packet_button(MouseButton::Back, true).unwrap();
    assert_eq!(back.data, 1);
    let forward = packet_button(MouseButton::Forward, false).unwrap();
    assert_eq!(forward.data, 2);

    assert_eq!(packet_wheel(2, true).data, 240);
    assert_eq!(packet_wheel(-1, false).data, -120);
    assert_eq!(packet_wheel(1_000_000, true).data, i32::from(i16::MAX));
}

#[test]
fn pen_transitions_follow_the_contact_edge() {
    let hover = PenState {
        in_range: true,
        ..PenState::default()
    };
    let down = PenState {
        down: true,
        in_range: true,
        pressure: 65535,
        ..PenState::default()
    };
    let gone = PenState::default();

    let flag_down = 0x0001_0000;
    let flag_up = 0x0004_0000;
    let flag_update = 0x0002_0000;
    let in_range = 0x0002;
    let in_contact = 0x0004;

    assert_eq!(pen_flags(None, &hover), flag_update | in_range);
    assert_eq!(
        pen_flags(Some(&hover), &down),
        flag_down | in_range | in_contact
    );
    assert_eq!(
        pen_flags(Some(&down), &down),
        flag_update | in_range | in_contact
    );
    assert_eq!(pen_flags(Some(&down), &hover), flag_up | in_range);
    assert_eq!(pen_flags(Some(&hover), &gone), flag_update);
    assert_eq!(pen_pressure(65535), 1024);
    assert_eq!(pen_pressure(0), 0);
}

#[test]
fn touch_frames_lift_whatever_dropped_out_of_the_report() {
    let finger = |id: u8, down: bool| TouchContact {
        id,
        down,
        x: 100,
        y: 100,
        ..TouchContact::default()
    };
    let first = TouchState {
        contacts: vec![finger(1, true), finger(2, true)],
    };
    assert_eq!(
        touch_moves(&[], &first),
        vec![(1, ContactMove::Down), (2, ContactMove::Down)]
    );

    let second = TouchState {
        contacts: vec![finger(1, true), finger(2, false)],
    };
    assert_eq!(
        touch_moves(&[1, 2], &second),
        vec![(1, ContactMove::Update), (2, ContactMove::Up)]
    );

    let third = TouchState { contacts: vec![] };
    assert_eq!(touch_moves(&[1], &third), vec![(1, ContactMove::Up)]);

    let never_down = TouchState {
        contacts: vec![finger(9, false)],
    };
    assert!(touch_moves(&[], &never_down).is_empty());
}

#[test]
fn the_virtual_screen_maps_the_corners_onto_its_own_rectangle() {
    let screen = VirtualScreen {
        left: -1920,
        top: 0,
        width: 3840,
        height: 1080,
    };
    let origin = screen.pixel(0, 0);
    assert_eq!((origin.x, origin.y), (-1920, 0));
    let far = screen.pixel(65535, 65535);
    assert_eq!((far.x, far.y), (1919, 1079));
    let live = VirtualScreen::current();
    assert!(live.width >= 1 && live.height >= 1);
}
