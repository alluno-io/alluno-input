//! The C ABI over the host, declared for C callers in `include/alluno_input.h`.
//!
//! Every handle is an opaque pointer the caller closes; every call answers a
//! `ALLUNO_INPUT_*` code and leaves a message behind `alluno_input_last_error` on the
//! calling thread. Keys, buttons and profiles are indexes into the port's
//! `ALL` lists, in the order the header spells them, which a test pins.
//!
//! `alluno_input_open_recording` opens the testkit's recording host through the same
//! ABI, so a binding's own tests need no driver either.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};

use alluno_input::{
    Backing, Capabilities, Error, Gamepad, GamepadOutput, GamepadProfile, GamepadState, Host,
    Input, Key, Keyboard, Mouse, MouseButton, Options, Pen, PenState, Touch, TouchContact,
    TouchState,
};
use alluno_input_testkit::{FakeHost, OutputFeed, Recorder, fake_gamepad};

pub const ALLUNO_INPUT_OK: i32 = 0;
pub const ALLUNO_INPUT_UNAVAILABLE: i32 = 1;
pub const ALLUNO_INPUT_UNSUPPORTED: i32 = 2;
pub const ALLUNO_INPUT_BACKEND: i32 = 3;
pub const ALLUNO_INPUT_IO: i32 = 4;
pub const ALLUNO_INPUT_INVALID: i32 = 5;

pub const ALLUNO_INPUT_BACKING_KERNEL: u8 = 0;
pub const ALLUNO_INPUT_BACKING_BUS: u8 = 1;
pub const ALLUNO_INPUT_BACKING_USER_API: u8 = 2;
pub const ALLUNO_INPUT_BACKING_UNAVAILABLE: u8 = 3;

pub const ALLUNO_INPUT_OUTPUT_RUMBLE: u8 = 0;
pub const ALLUNO_INPUT_OUTPUT_TRIGGER_RUMBLE: u8 = 1;
pub const ALLUNO_INPUT_OUTPUT_PLAYER_LED: u8 = 2;
pub const ALLUNO_INPUT_OUTPUT_RGB: u8 = 3;
pub const ALLUNO_INPUT_OUTPUT_RAW: u8 = 4;

/// The number of profiles the capabilities array holds room for.
pub const ALLUNO_INPUT_PROFILE_SLOTS: usize = 8;

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn remember(message: String) {
    let text = CString::new(message).unwrap_or_default();
    LAST_ERROR.with(|slot| *slot.borrow_mut() = text);
}

fn code(error: Error) -> i32 {
    let code = match &error {
        Error::Unavailable(_) => ALLUNO_INPUT_UNAVAILABLE,
        Error::Unsupported => ALLUNO_INPUT_UNSUPPORTED,
        Error::Backend(_) => ALLUNO_INPUT_BACKEND,
        Error::Io(_) => ALLUNO_INPUT_IO,
        _ => ALLUNO_INPUT_BACKEND,
    };
    remember(error.to_string());
    code
}

fn invalid(what: &str) -> i32 {
    remember(format!("invalid argument: {what}"));
    ALLUNO_INPUT_INVALID
}

fn ok<T>(result: alluno_input::Result<T>) -> i32 {
    match result {
        Ok(_) => ALLUNO_INPUT_OK,
        Err(error) => code(error),
    }
}

fn backing(value: &Backing) -> u8 {
    match value {
        Backing::Kernel => ALLUNO_INPUT_BACKING_KERNEL,
        Backing::Bus => ALLUNO_INPUT_BACKING_BUS,
        Backing::UserApi => ALLUNO_INPUT_BACKING_USER_API,
        _ => ALLUNO_INPUT_BACKING_UNAVAILABLE,
    }
}

/// The capability answer, one byte per device kind in `ALLUNO_INPUT_BACKING_*` terms.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllunoInputCapabilities {
    pub keyboard: u8,
    pub mouse: u8,
    pub pen: u8,
    pub touch: u8,
    /// Indexed by `ALLUNO_INPUT_PROFILE_*`; unused slots read unavailable.
    pub gamepads: [u8; ALLUNO_INPUT_PROFILE_SLOTS],
    /// The bus cap on pads plugged at once, or -1 for none.
    pub max_gamepads: i16,
    pub midi: u8,
    pub camera: u8,
    pub microphone: u8,
}

impl AllunoInputCapabilities {
    fn from(caps: &Capabilities) -> Self {
        let mut gamepads = [ALLUNO_INPUT_BACKING_UNAVAILABLE; ALLUNO_INPUT_PROFILE_SLOTS];
        for (index, profile) in GamepadProfile::ALL.iter().enumerate() {
            if let Some(slot) = gamepads.get_mut(index) {
                *slot = backing(caps.gamepad(*profile));
            }
        }
        Self {
            keyboard: backing(&caps.keyboard),
            mouse: backing(&caps.mouse),
            pen: backing(&caps.pen),
            touch: backing(&caps.touch),
            gamepads,
            max_gamepads: caps.max_gamepads.map_or(-1, i16::from),
            midi: backing(&caps.midi),
            camera: backing(&caps.camera),
            microphone: backing(&caps.microphone),
        }
    }
}

/// A stylus sample, the port's `PenState` with C booleans.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllunoInputPenState {
    pub x: u16,
    pub y: u16,
    pub pressure: u16,
    pub tilt_x: i8,
    pub tilt_y: i8,
    pub twist: u16,
    pub down: u8,
    pub barrel: u8,
    pub eraser: u8,
    pub in_range: u8,
}

/// One finger, the port's `TouchContact` with a C boolean.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllunoInputTouchContact {
    pub id: u8,
    pub down: u8,
    pub x: u16,
    pub y: u16,
    pub pressure: u16,
    pub width: u16,
    pub height: u16,
}

/// A pad snapshot, laid out exactly like the port's `GamepadState`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllunoInputGamepadState {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

/// One thing a game sent back; `kind` says which fields mean something.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AllunoInputGamepadOutput {
    pub kind: u8,
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub raw: *const u8,
    pub raw_len: usize,
}

/// Where a pad's output goes: called on a thread the library owns.
pub type AllunoInputOutputCallback =
    Option<unsafe extern "C" fn(user: *mut c_void, output: *const AllunoInputGamepadOutput)>;

enum Inner {
    Real(Input),
    Fake(FakeHost),
}

/// An opened host.
pub struct AllunoInputHost {
    inner: Inner,
}

impl AllunoInputHost {
    fn keyboard(&self) -> alluno_input::Result<Box<dyn Keyboard>> {
        match &self.inner {
            Inner::Real(host) => host.keyboard(),
            Inner::Fake(host) => host.keyboard(),
        }
    }

    fn mouse(&self) -> alluno_input::Result<Box<dyn Mouse>> {
        match &self.inner {
            Inner::Real(host) => host.mouse(),
            Inner::Fake(host) => host.mouse(),
        }
    }

    fn pen(&self) -> alluno_input::Result<Box<dyn Pen>> {
        match &self.inner {
            Inner::Real(host) => host.pen(),
            Inner::Fake(host) => host.pen(),
        }
    }

    fn touch(&self) -> alluno_input::Result<Box<dyn Touch>> {
        match &self.inner {
            Inner::Real(host) => host.touch(),
            Inner::Fake(host) => host.touch(),
        }
    }

    fn gamepad(&self, profile: GamepadProfile) -> alluno_input::Result<AllunoInputGamepad> {
        match &self.inner {
            Inner::Real(host) => Ok(AllunoInputGamepad {
                pad: host.gamepad(profile)?,
                feed: None,
            }),
            Inner::Fake(_) => {
                let (pad, _) = fake_gamepad(profile);
                let feed = pad.output();
                Ok(AllunoInputGamepad {
                    pad: Box::new(pad),
                    feed: Some(feed),
                })
            }
        }
    }

    fn recorder(&self) -> Option<Recorder> {
        match &self.inner {
            Inner::Real(_) => None,
            Inner::Fake(host) => Some(host.recorder()),
        }
    }
}

/// An opened keyboard.
pub struct AllunoInputKeyboard(Box<dyn Keyboard>);

/// An opened mouse.
pub struct AllunoInputMouse(Box<dyn Mouse>);

/// An opened pen.
pub struct AllunoInputPen(Box<dyn Pen>);

/// An opened touch digitiser.
pub struct AllunoInputTouch(Box<dyn Touch>);

/// A plugged pad.
pub struct AllunoInputGamepad {
    pad: Box<dyn Gamepad>,
    feed: Option<OutputFeed>,
}

struct Callback {
    call: unsafe extern "C" fn(*mut c_void, *const AllunoInputGamepadOutput),
    user: *mut c_void,
}

unsafe impl Send for Callback {}

impl Callback {
    fn deliver(&self, output: GamepadOutput) {
        let raw: Vec<u8>;
        let ffi = match output {
            GamepadOutput::Rumble { large, small } => AllunoInputGamepadOutput {
                kind: ALLUNO_INPUT_OUTPUT_RUMBLE,
                a: large,
                b: small,
                c: 0,
                raw: std::ptr::null(),
                raw_len: 0,
            },
            GamepadOutput::TriggerRumble { left, right } => AllunoInputGamepadOutput {
                kind: ALLUNO_INPUT_OUTPUT_TRIGGER_RUMBLE,
                a: left,
                b: right,
                c: 0,
                raw: std::ptr::null(),
                raw_len: 0,
            },
            GamepadOutput::PlayerLed(player) => AllunoInputGamepadOutput {
                kind: ALLUNO_INPUT_OUTPUT_PLAYER_LED,
                a: player,
                b: 0,
                c: 0,
                raw: std::ptr::null(),
                raw_len: 0,
            },
            GamepadOutput::Rgb { r, g, b } => AllunoInputGamepadOutput {
                kind: ALLUNO_INPUT_OUTPUT_RGB,
                a: r,
                b: g,
                c: b,
                raw: std::ptr::null(),
                raw_len: 0,
            },
            GamepadOutput::Raw(bytes) => {
                raw = bytes;
                AllunoInputGamepadOutput {
                    kind: ALLUNO_INPUT_OUTPUT_RAW,
                    a: 0,
                    b: 0,
                    c: 0,
                    raw: raw.as_ptr(),
                    raw_len: raw.len(),
                }
            }
            _ => return,
        };
        unsafe { (self.call)(self.user, &ffi) };
    }
}

/// The library version, as a static C string.
#[unsafe(no_mangle)]
pub extern "C" fn alluno_input_version() -> *const c_char {
    c"2.0.0".as_ptr()
}

/// The message behind the last failing call on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn alluno_input_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

/// Answers what this machine can emulate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_probe(out: *mut AllunoInputCapabilities) -> i32 {
    if out.is_null() {
        return invalid("null capabilities");
    }
    unsafe { *out = AllunoInputCapabilities::from(&Input::probe()) };
    ALLUNO_INPUT_OK
}

/// Opens the host on the calling thread; `device_name` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_open(device_name: *const c_char) -> *mut AllunoInputHost {
    let mut options = Options::default();
    if !device_name.is_null() {
        let name = unsafe { CStr::from_ptr(device_name) };
        options.device_name = name.to_str().ok().map(str::to_string);
    }
    match Input::open(options) {
        Ok(host) => Box::into_raw(Box::new(AllunoInputHost {
            inner: Inner::Real(host),
        })),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

/// Opens the recording host: every device records instead of injecting.
#[unsafe(no_mangle)]
pub extern "C" fn alluno_input_open_recording() -> *mut AllunoInputHost {
    match FakeHost::open(Options::default()) {
        Ok(host) => Box::into_raw(Box::new(AllunoInputHost {
            inner: Inner::Fake(host),
        })),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

/// How many calls a recording host has logged; 0 for a real host.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_recording_len(host: *const AllunoInputHost) -> usize {
    if host.is_null() {
        return 0;
    }
    unsafe { &*host }
        .recorder()
        .map_or(0, |recorder| recorder.len())
}

/// Closes a host; devices opened from it stay valid until closed themselves.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_close(host: *mut AllunoInputHost) {
    if !host.is_null() {
        drop(unsafe { Box::from_raw(host) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_keyboard_open(
    host: *const AllunoInputHost,
) -> *mut AllunoInputKeyboard {
    if host.is_null() {
        invalid("null host");
        return std::ptr::null_mut();
    }
    match unsafe { &*host }.keyboard() {
        Ok(device) => Box::into_raw(Box::new(AllunoInputKeyboard(device))),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_keyboard_close(keyboard: *mut AllunoInputKeyboard) {
    if !keyboard.is_null() {
        drop(unsafe { Box::from_raw(keyboard) });
    }
}

/// Presses or releases a `ALLUNO_INPUT_KEY_*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_keyboard_key(
    keyboard: *mut AllunoInputKeyboard,
    key: u8,
    pressed: bool,
) -> i32 {
    if keyboard.is_null() {
        return invalid("null keyboard");
    }
    let Some(key) = Key::ALL.get(usize::from(key)) else {
        return invalid("key index");
    };
    ok(unsafe { &mut *keyboard }.0.key(*key, pressed))
}

/// Types UTF-8 text where the device has a text path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_keyboard_text(
    keyboard: *mut AllunoInputKeyboard,
    text: *const c_char,
) -> i32 {
    if keyboard.is_null() || text.is_null() {
        return invalid("null keyboard or text");
    }
    let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
        return invalid("text is not UTF-8");
    };
    ok(unsafe { &mut *keyboard }.0.text(text))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_mouse_open(
    host: *const AllunoInputHost,
) -> *mut AllunoInputMouse {
    if host.is_null() {
        invalid("null host");
        return std::ptr::null_mut();
    }
    match unsafe { &*host }.mouse() {
        Ok(device) => Box::into_raw(Box::new(AllunoInputMouse(device))),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_mouse_close(mouse: *mut AllunoInputMouse) {
    if !mouse.is_null() {
        drop(unsafe { Box::from_raw(mouse) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_mouse_move_abs(
    mouse: *mut AllunoInputMouse,
    x: u16,
    y: u16,
) -> i32 {
    if mouse.is_null() {
        return invalid("null mouse");
    }
    ok(unsafe { &mut *mouse }.0.move_abs(x, y))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_mouse_move_rel(
    mouse: *mut AllunoInputMouse,
    dx: i32,
    dy: i32,
) -> i32 {
    if mouse.is_null() {
        return invalid("null mouse");
    }
    ok(unsafe { &mut *mouse }.0.move_rel(dx, dy))
}

/// Presses or releases a `ALLUNO_INPUT_BUTTON_*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_mouse_button(
    mouse: *mut AllunoInputMouse,
    button: u8,
    pressed: bool,
) -> i32 {
    if mouse.is_null() {
        return invalid("null mouse");
    }
    let Some(button) = MouseButton::ALL.get(usize::from(button)) else {
        return invalid("button index");
    };
    ok(unsafe { &mut *mouse }.0.button(*button, pressed))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_mouse_wheel(
    mouse: *mut AllunoInputMouse,
    dx: i32,
    dy: i32,
) -> i32 {
    if mouse.is_null() {
        return invalid("null mouse");
    }
    ok(unsafe { &mut *mouse }.0.wheel(dx, dy))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_pen_open(
    host: *const AllunoInputHost,
) -> *mut AllunoInputPen {
    if host.is_null() {
        invalid("null host");
        return std::ptr::null_mut();
    }
    match unsafe { &*host }.pen() {
        Ok(device) => Box::into_raw(Box::new(AllunoInputPen(device))),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_pen_close(pen: *mut AllunoInputPen) {
    if !pen.is_null() {
        drop(unsafe { Box::from_raw(pen) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_pen_report(
    pen: *mut AllunoInputPen,
    state: *const AllunoInputPenState,
) -> i32 {
    if pen.is_null() || state.is_null() {
        return invalid("null pen or state");
    }
    let state = unsafe { *state };
    let sample = PenState {
        x: state.x,
        y: state.y,
        pressure: state.pressure,
        tilt_x: state.tilt_x,
        tilt_y: state.tilt_y,
        twist: state.twist,
        down: state.down != 0,
        barrel: state.barrel != 0,
        eraser: state.eraser != 0,
        in_range: state.in_range != 0,
    };
    ok(unsafe { &mut *pen }.0.report(&sample))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_touch_open(
    host: *const AllunoInputHost,
) -> *mut AllunoInputTouch {
    if host.is_null() {
        invalid("null host");
        return std::ptr::null_mut();
    }
    match unsafe { &*host }.touch() {
        Ok(device) => Box::into_raw(Box::new(AllunoInputTouch(device))),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_touch_close(touch: *mut AllunoInputTouch) {
    if !touch.is_null() {
        drop(unsafe { Box::from_raw(touch) });
    }
}

/// Reports every current finger; `count` may be 0 to lift them all.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_touch_report(
    touch: *mut AllunoInputTouch,
    contacts: *const AllunoInputTouchContact,
    count: usize,
) -> i32 {
    if touch.is_null() || (count > 0 && contacts.is_null()) {
        return invalid("null touch or contacts");
    }
    let contacts = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(contacts, count) }
    };
    let state = TouchState {
        contacts: contacts
            .iter()
            .map(|c| TouchContact {
                id: c.id,
                x: c.x,
                y: c.y,
                pressure: c.pressure,
                width: c.width,
                height: c.height,
                down: c.down != 0,
            })
            .collect(),
    };
    ok(unsafe { &mut *touch }.0.report(&state))
}

/// Plugs a pad of a `ALLUNO_INPUT_PROFILE_*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_gamepad_open(
    host: *const AllunoInputHost,
    profile: u8,
) -> *mut AllunoInputGamepad {
    if host.is_null() {
        invalid("null host");
        return std::ptr::null_mut();
    }
    let Some(profile) = GamepadProfile::ALL.get(usize::from(profile)) else {
        invalid("profile index");
        return std::ptr::null_mut();
    };
    match unsafe { &*host }.gamepad(*profile) {
        Ok(pad) => Box::into_raw(Box::new(pad)),
        Err(error) => {
            code(error);
            std::ptr::null_mut()
        }
    }
}

/// Unplugs a pad.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_gamepad_close(pad: *mut AllunoInputGamepad) {
    if !pad.is_null() {
        drop(unsafe { Box::from_raw(pad) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_gamepad_submit(
    pad: *mut AllunoInputGamepad,
    state: *const AllunoInputGamepadState,
) -> i32 {
    if pad.is_null() || state.is_null() {
        return invalid("null pad or state");
    }
    let state = unsafe { *state };
    let snapshot = GamepadState {
        buttons: state.buttons,
        left_trigger: state.left_trigger,
        right_trigger: state.right_trigger,
        thumb_lx: state.thumb_lx,
        thumb_ly: state.thumb_ly,
        thumb_rx: state.thumb_rx,
        thumb_ry: state.thumb_ry,
    };
    ok(unsafe { &mut *pad }.pad.submit(&snapshot))
}

/// Installs where the game's output goes; `callback` runs on a library thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_gamepad_on_output(
    pad: *mut AllunoInputGamepad,
    callback: AllunoInputOutputCallback,
    user: *mut c_void,
) -> i32 {
    if pad.is_null() {
        return invalid("null pad");
    }
    let Some(call) = callback else {
        return invalid("null callback");
    };
    let sink = Callback { call, user };
    ok(unsafe { &mut *pad }
        .pad
        .on_output(Box::new(move |output| sink.deliver(output))))
}

/// The player slot the OS assigned, or -1 where the profile has none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_gamepad_slot(pad: *mut AllunoInputGamepad) -> i32 {
    if pad.is_null() {
        return -1;
    }
    unsafe { &mut *pad }.pad.slot().map_or(-1, i32::from)
}

/// On a recording host's pad, plays the game's side: feeds a rumble into the
/// installed callback and answers whether one was installed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alluno_input_recording_rumble(
    pad: *mut AllunoInputGamepad,
    large: u8,
    small: u8,
) -> bool {
    if pad.is_null() {
        return false;
    }
    unsafe { &*pad }
        .feed
        .as_ref()
        .is_some_and(|feed| feed.feed(GamepadOutput::Rumble { large, small }))
}
