//! The gamepad vocabulary: which controller to be, the state it is fed, and
//! what it says back.

/// Which controller a virtual pad presents itself as.
///
/// Each host answers which of these it can be in [`crate::Capabilities`]; a
/// consumer asks for the one the game in front of it expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GamepadProfile {
    /// Xbox 360 over XInput: the widest compatibility, at most four on Windows.
    Xbox360,
    XboxOne,
    XboxSeries,
    /// Sony DualShock 4 over DirectInput / HID: no four-pad cap.
    DualShock4,
    DualSense,
    SwitchPro,
    /// A plain HID gamepad with no vendor identity.
    GenericHid,
}

impl GamepadProfile {
    /// Every profile, so a host can answer for each one.
    pub const ALL: &[GamepadProfile] = &[
        GamepadProfile::Xbox360,
        GamepadProfile::XboxOne,
        GamepadProfile::XboxSeries,
        GamepadProfile::DualShock4,
        GamepadProfile::DualSense,
        GamepadProfile::SwitchPro,
        GamepadProfile::GenericHid,
    ];
}

/// The digital buttons as XInput lays them out, so a report submits to XInput
/// and uinput without translation.
pub mod buttons {
    pub const DPAD_UP: u16 = 0x0001;
    pub const DPAD_DOWN: u16 = 0x0002;
    pub const DPAD_LEFT: u16 = 0x0004;
    pub const DPAD_RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const BACK: u16 = 0x0020;
    pub const LEFT_THUMB: u16 = 0x0040;
    pub const RIGHT_THUMB: u16 = 0x0080;
    pub const LEFT_SHOULDER: u16 = 0x0100;
    pub const RIGHT_SHOULDER: u16 = 0x0200;
    pub const GUIDE: u16 = 0x0400;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;

    /// Every button bit, so a test can prove they are distinct and complete.
    pub const ALL: &[u16] = &[
        DPAD_UP,
        DPAD_DOWN,
        DPAD_LEFT,
        DPAD_RIGHT,
        START,
        BACK,
        LEFT_THUMB,
        RIGHT_THUMB,
        LEFT_SHOULDER,
        RIGHT_SHOULDER,
        GUIDE,
        A,
        B,
        X,
        Y,
    ];
}

/// One full snapshot of a pad: a button bitset, two triggers and two sticks.
///
/// The layout is XInput's `XINPUT_GAMEPAD`, which is what the Xbox bus takes
/// verbatim and what every other backend translates from. Sticks are signed
/// full-range, triggers are 0..=255; [`GamepadState::from_normalized`] builds one
/// from the -1.0..=1.0 and 0.0..=1.0 floats a web client sends.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GamepadState {
    /// The pressed buttons, as [`buttons`] bits.
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

impl GamepadState {
    /// Builds a snapshot from normalized axes: sticks in -1.0..=1.0, triggers in 0.0..=1.0.
    /// Values outside those ranges are clamped rather than wrapped.
    pub fn from_normalized(
        buttons: u16,
        left_stick: (f32, f32),
        right_stick: (f32, f32),
        triggers: (f32, f32),
    ) -> Self {
        Self {
            buttons,
            left_trigger: trigger(triggers.0),
            right_trigger: trigger(triggers.1),
            thumb_lx: axis(left_stick.0),
            thumb_ly: axis(left_stick.1),
            thumb_rx: axis(right_stick.0),
            thumb_ry: axis(right_stick.1),
        }
    }

    /// Whether a [`buttons`] bit is set.
    pub fn pressed(&self, button: u16) -> bool {
        self.buttons & button != 0
    }
}

fn axis(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

fn trigger(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// What a game sends back to a pad.
///
/// A superset of what every backend can source; a host reports which of these a
/// profile delivers, and a variant it cannot source is simply never emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GamepadOutput {
    /// Motor strengths, 0..=255 each.
    Rumble { large: u8, small: u8 },
    /// Impulse trigger strengths, 0..=255 each.
    TriggerRumble { left: u8, right: u8 },
    /// The player slot lamp, 1-based; 0 means none lit.
    PlayerLed(u8),
    /// The light bar colour on pads that have one.
    Rgb { r: u8, g: u8, b: u8 },
    /// A raw HID output report the profile did not decode.
    Raw(Vec<u8>),
}

/// Where a pad delivers its output. Called on a thread the host owns, never the
/// injecting thread, so it must be `Send`.
pub type OutputSink = Box<dyn FnMut(GamepadOutput) + Send + 'static>;
