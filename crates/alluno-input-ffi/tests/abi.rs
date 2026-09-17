//! The C ABI, exercised through the recording host, and the header pinned to
//! the port's own lists.

use std::ffi::{CStr, c_void};
use std::sync::{Arc, Mutex};

use alluno_input::{GamepadProfile, Key, MouseButton, buttons};
use alluno_input_ffi::*;

const HEADER: &str = include_str!("../include/alluno_input.h");

fn screaming(name: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() && previous_lower {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
        previous_lower = ch.is_ascii_lowercase();
    }
    out
}

fn defined(prefix: &str) -> Vec<(String, u64)> {
    HEADER
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("#define ")?;
            let mut parts = rest.split_whitespace();
            let name = parts.next()?;
            if !name.starts_with(prefix) {
                return None;
            }
            let value = parts.next()?;
            let value = value
                .strip_prefix("0x")
                .map(|hex| u64::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| value.parse().ok())?;
            Some((name.to_string(), value))
        })
        .collect()
}

#[test]
fn the_header_lists_every_key_in_the_ports_order() {
    let keys: Vec<(String, u64)> = HEADER
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_end_matches(',');
            let (name, value) = line.split_once(" = ")?;
            let name = name.strip_prefix("ALLUNO_INPUT_KEY_")?;
            Some((name.to_string(), value.parse().ok()?))
        })
        .collect();
    assert_eq!(keys.len(), Key::ALL.len() + 1);
    for (index, key) in Key::ALL.iter().enumerate() {
        let expected = screaming(&format!("{key:?}"));
        assert_eq!(keys[index], (expected, index as u64), "{key:?}");
    }
    assert_eq!(
        keys.last().unwrap(),
        &("COUNT".to_string(), Key::ALL.len() as u64)
    );
}

#[test]
fn the_header_pins_the_button_bits_profiles_and_codes() {
    let bits = defined("ALLUNO_INPUT_PAD_");
    assert_eq!(bits.len(), buttons::ALL.len());
    for (name, value) in &bits {
        let index = bits.iter().position(|(n, _)| n == name).unwrap();
        assert_eq!(*value, u64::from(buttons::ALL[index]), "{name}");
    }
    let profiles = defined("ALLUNO_INPUT_PROFILE_");
    assert_eq!(profiles.len(), GamepadProfile::ALL.len() + 1);
    let spelled = [
        "XBOX360",
        "XBOX_ONE",
        "XBOX_SERIES",
        "DUALSHOCK4",
        "DUALSENSE",
        "SWITCH_PRO",
        "GENERIC_HID",
    ];
    assert_eq!(spelled.len(), GamepadProfile::ALL.len());
    for (index, name) in spelled.iter().enumerate() {
        assert_eq!(profiles[index].0, format!("ALLUNO_INPUT_PROFILE_{name}"));
        assert_eq!(profiles[index].1, index as u64);
    }
    let mouse = defined("ALLUNO_INPUT_BUTTON_");
    assert_eq!(mouse.len(), MouseButton::ALL.len());
    for (index, button) in MouseButton::ALL.iter().enumerate() {
        assert_eq!(
            mouse[index].0,
            format!("ALLUNO_INPUT_BUTTON_{}", screaming(&format!("{button:?}")))
        );
    }
    assert_eq!(
        defined("ALLUNO_INPUT_OK")[0].1 as i32 + defined("ALLUNO_INPUT_INVALID")[0].1 as i32,
        ALLUNO_INPUT_OK + ALLUNO_INPUT_INVALID
    );
}

#[test]
fn the_capabilities_struct_matches_the_header_layout() {
    assert_eq!(
        std::mem::size_of::<AllunoInputCapabilities>(),
        4 + 8 + 2 + 3 + 1
    );
    assert_eq!(std::mem::size_of::<AllunoInputGamepadState>(), 12);
    assert_eq!(std::mem::size_of::<AllunoInputPenState>(), 14);
    assert_eq!(std::mem::size_of::<AllunoInputTouchContact>(), 12);
}

#[test]
fn a_probe_answers_every_profile_slot() {
    let mut caps = AllunoInputCapabilities {
        keyboard: 9,
        mouse: 9,
        pen: 9,
        touch: 9,
        gamepads: [9; ALLUNO_INPUT_PROFILE_SLOTS],
        max_gamepads: 9,
        midi: 9,
        camera: 9,
        microphone: 9,
    };
    assert_eq!(unsafe { alluno_input_probe(&mut caps) }, ALLUNO_INPUT_OK);
    assert!(caps.keyboard <= ALLUNO_INPUT_BACKING_UNAVAILABLE);
    assert_eq!(caps.gamepads[7], ALLUNO_INPUT_BACKING_UNAVAILABLE);
    assert_eq!(caps.midi, ALLUNO_INPUT_BACKING_UNAVAILABLE);
    assert_eq!(
        unsafe { alluno_input_probe(std::ptr::null_mut()) },
        ALLUNO_INPUT_INVALID
    );
}

#[test]
fn the_recording_host_takes_every_device_through_the_abi() {
    unsafe {
        let host = alluno_input_open_recording();
        assert!(!host.is_null());

        let keyboard = alluno_input_keyboard_open(host);
        assert_eq!(
            alluno_input_keyboard_key(keyboard, ALLUNO_INPUT_KEY_A, true),
            ALLUNO_INPUT_OK
        );
        assert_eq!(
            alluno_input_keyboard_key(keyboard, 200, true),
            ALLUNO_INPUT_INVALID
        );
        assert_eq!(
            alluno_input_keyboard_text(keyboard, c"hi".as_ptr()),
            ALLUNO_INPUT_OK
        );
        alluno_input_keyboard_close(keyboard);

        let mouse = alluno_input_mouse_open(host);
        assert_eq!(alluno_input_mouse_move_abs(mouse, 1, 2), ALLUNO_INPUT_OK);
        assert_eq!(alluno_input_mouse_button(mouse, 1, true), ALLUNO_INPUT_OK);
        assert_eq!(alluno_input_mouse_wheel(mouse, 0, -1), ALLUNO_INPUT_OK);
        alluno_input_mouse_close(mouse);

        let pen = alluno_input_pen_open(host);
        let sample = AllunoInputPenState {
            x: 5,
            y: 6,
            pressure: 7,
            tilt_x: 0,
            tilt_y: 0,
            twist: 0,
            down: 1,
            barrel: 0,
            eraser: 0,
            in_range: 1,
        };
        assert_eq!(alluno_input_pen_report(pen, &sample), ALLUNO_INPUT_OK);
        alluno_input_pen_close(pen);

        let touch = alluno_input_touch_open(host);
        let contact = AllunoInputTouchContact {
            id: 1,
            down: 1,
            x: 10,
            y: 20,
            pressure: 30,
            width: 0,
            height: 0,
        };
        assert_eq!(
            alluno_input_touch_report(touch, &contact, 1),
            ALLUNO_INPUT_OK
        );
        assert_eq!(
            alluno_input_touch_report(touch, std::ptr::null(), 0),
            ALLUNO_INPUT_OK
        );
        alluno_input_touch_close(touch);

        assert_eq!(alluno_input_recording_len(host), 8);
        alluno_input_close(host);
    }
}

const ALLUNO_INPUT_KEY_A: u8 = 0;

struct Seen(Arc<Mutex<Vec<(u8, u8, u8)>>>);

unsafe extern "C" fn on_output(user: *mut c_void, output: *const AllunoInputGamepadOutput) {
    let seen = unsafe { &*(user as *const Seen) };
    let output = unsafe { *output };
    seen.0
        .lock()
        .unwrap()
        .push((output.kind, output.a, output.b));
}

#[test]
fn a_recording_pad_delivers_the_games_output_to_the_callback() {
    unsafe {
        let host = alluno_input_open_recording();
        let pad = alluno_input_gamepad_open(host, ALLUNO_INPUT_PROFILE_DUALSHOCK4);
        assert!(!pad.is_null());
        assert!(alluno_input_gamepad_open(host, 7).is_null());
        assert_eq!(
            CStr::from_ptr(alluno_input_last_error()).to_str().unwrap(),
            "invalid argument: profile index"
        );

        let state = AllunoInputGamepadState {
            buttons: buttons::A,
            ..AllunoInputGamepadState::default()
        };
        assert_eq!(alluno_input_gamepad_submit(pad, &state), ALLUNO_INPUT_OK);
        assert_eq!(alluno_input_gamepad_slot(pad), 0);

        let seen = Seen(Arc::new(Mutex::new(Vec::new())));
        assert!(!alluno_input_recording_rumble(pad, 1, 2), "no callback yet");
        assert_eq!(
            alluno_input_gamepad_on_output(
                pad,
                Some(on_output),
                &seen as *const Seen as *mut c_void
            ),
            ALLUNO_INPUT_OK
        );
        assert!(alluno_input_recording_rumble(pad, 200, 100));
        assert_eq!(
            seen.0.lock().unwrap().as_slice(),
            &[(ALLUNO_INPUT_OUTPUT_RUMBLE, 200, 100)]
        );

        alluno_input_gamepad_close(pad);
        alluno_input_close(host);
    }
}

const ALLUNO_INPUT_PROFILE_DUALSHOCK4: u8 = 3;

#[test]
fn the_version_is_the_workspace_version() {
    let version = unsafe { CStr::from_ptr(alluno_input_version()) };
    assert_eq!(version.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
}
