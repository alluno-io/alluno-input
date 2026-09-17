//! The names the relocated bus client was written against.

/// The XInput report the bus takes verbatim.
pub type XGamepad = alluno_input_core::GamepadState;

/// The button bits under the names the bus client spells them.
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

/// What the bus delivers back from a game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadNotification {
    pub large_motor: u8,
    pub small_motor: u8,
    pub led_number: u8,
}
