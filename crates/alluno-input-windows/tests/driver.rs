//! Against the installed driver, when there is one. Every test here opens a
//! device and sends nothing a person would notice.

#![cfg(target_os = "windows")]

use alluno_input_core::{Backing, Host};
use alluno_input_windows::Input;
use alluno_input_windows::kernel::{KernelKeyboard, KernelMouse};

#[test]
fn probing_never_opens_anything_it_cannot_close() {
    let first = Input::probe();
    let second = Input::probe();
    assert_eq!(first, second);
}

#[test]
fn the_probe_names_the_kernel_layer_exactly_when_the_filter_is_installed() {
    let caps = Input::probe();
    let expected_keyboard = if KernelKeyboard::available() {
        Backing::Kernel
    } else {
        Backing::UserApi
    };
    let expected_mouse = if KernelMouse::available() {
        Backing::Kernel
    } else {
        Backing::UserApi
    };
    assert_eq!(caps.keyboard, expected_keyboard);
    assert_eq!(caps.mouse, expected_mouse);
    assert!(caps.keyboard.available() && caps.mouse.available());
}

#[test]
fn a_zero_move_reaches_the_driver_when_it_is_installed() {
    let Some(mouse) = KernelMouse::open() else {
        eprintln!("skipped: mouse filter not installed");
        return;
    };
    let consumed = mouse.move_rel(0, 0).expect("the driver takes a packet");
    assert_eq!(
        consumed as usize,
        std::mem::size_of::<alluno_input_windows::kernel::MouseInputData>()
    );
}
