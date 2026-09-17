//! The names the relocated uinput pad was written against.

/// The XInput-shaped report the pad translates onto evdev.
pub type XGamepad = alluno_input_core::GamepadState;

/// The button bits under the names the pad spells them.
pub mod xbuttons {
    use alluno_input_core::buttons;

    pub const UP: u16 = buttons::DPAD_UP;
    pub const DOWN: u16 = buttons::DPAD_DOWN;
    pub const LEFT: u16 = buttons::DPAD_LEFT;
    pub const RIGHT: u16 = buttons::DPAD_RIGHT;
    pub const START: u16 = buttons::START;
    pub const BACK: u16 = buttons::BACK;
    pub const LTHUMB: u16 = buttons::LEFT_THUMB;
    pub const RTHUMB: u16 = buttons::RIGHT_THUMB;
    pub const LB: u16 = buttons::LEFT_SHOULDER;
    pub const RB: u16 = buttons::RIGHT_SHOULDER;
    pub const GUIDE: u16 = buttons::GUIDE;
    pub const A: u16 = buttons::A;
    pub const B: u16 = buttons::B;
    pub const X: u16 = buttons::X;
    pub const Y: u16 = buttons::Y;
}

/// Which of the two uinput identities a pad takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GamepadKind {
    #[default]
    Xbox360,
    DualShock4,
}

/// What the kernel's force-feedback path delivers back from a game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadNotification {
    pub large_motor: u8,
    pub small_motor: u8,
    pub led_number: u8,
}
