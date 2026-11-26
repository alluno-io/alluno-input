//! What a posted key leaves in the system's modifier state, on a real Mac. Each test posts keys to
//! the frontmost app and needs the Accessibility permission, so it runs only when asked:
//! `cargo test -p alluno-input-macos --test hw_globe -- --ignored`, from a terminal that has it.

#![cfg(target_os = "macos")]

use std::time::Duration;

use alluno_input_core::{Key, Keyboard};
use alluno_input_macos::cg::CgKeyboard;
use core_graphics::event::CGEventFlags;

const HID_SYSTEM_STATE: i32 = 1;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceFlagsState(state: i32) -> u64;
    fn CGPreflightPostEventAccess() -> bool;
}

fn system_flags() -> CGEventFlags {
    std::thread::sleep(Duration::from_millis(150));
    // SAFETY: a read of the global modifier state, valid for any source state id.
    CGEventFlags::from_bits_retain(unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) })
}

/// A posted End key left the Globe key's flag in the system's modifier state, so the Mac acted as
/// if the Globe key were held: the next letters opened the emoji picker, started dictation or went
/// missing, until a key was pressed on the Mac itself.
#[test]
#[ignore = "posts keys to the frontmost app; needs Accessibility"]
fn regression_a_posted_navigation_key_leaves_the_globe_key_up() {
    // SAFETY: a permission probe with no arguments.
    assert!(
        unsafe { CGPreflightPostEventAccess() },
        "this terminal cannot post events: grant it Accessibility"
    );
    let mut keyboard = CgKeyboard::open().expect("event source");
    keyboard.key(Key::End, true).expect("end down");
    keyboard.key(Key::End, false).expect("end up");
    let flags = system_flags();
    assert!(
        !flags.contains(CGEventFlags::CGEventFlagSecondaryFn),
        "the Globe key stays held after End: {flags:?}"
    );
}
