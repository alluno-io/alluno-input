//! The HID profiles every bus-backed host feeds: report descriptors, the
//! identity a device announces, the feature reports it answers, and the codec
//! that turns port state into input reports and output reports back into
//! [`GamepadOutput`].
//!
//! AllunoVHID on Windows, `uhid` on Linux and the DriverKit extension on macOS
//! publish these bytes verbatim, so a game, SDL, Steam Input or the kernel's
//! own controller driver sees the same device on every platform.

mod dualsense;
mod dualshock4;
mod generic;
pub mod keyboard;
pub mod mouse;
pub mod pen;
mod switch_pro;
pub mod touch;
pub mod wire;
pub mod xbox;
pub mod xusb;

use crate::{GamepadOutput, GamepadProfile, GamepadState};

/// The vendor, product and version a HID device announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

/// One feature report a device answers a `GET_FEATURE` for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureReport {
    /// The report id, which is also the first byte of `data`.
    pub id: u8,
    /// The whole report including its id byte.
    pub data: &'static [u8],
}

/// What decoding one output report yields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decoded {
    /// The feedback the report carried, already in port terms.
    pub outputs: Vec<GamepadOutput>,
    /// An input report the device must answer with, for protocols that
    /// acknowledge commands over the input pipe.
    pub reply: Option<Vec<u8>>,
}

/// The per-profile codec: one instance per plugged device, since some
/// protocols keep state across reports.
pub struct PadCodec {
    inner: Inner,
}

enum Inner {
    DualShock4(dualshock4::Codec),
    DualSense(dualsense::Codec),
    SwitchPro(switch_pro::Codec),
    Generic(generic::Codec),
    Xbox(xbox::Codec),
}

impl PadCodec {
    /// The codec for a profile, or `None` for the one identity that is not a
    /// HID device anywhere (the Xbox 360, which is XUSB on Windows).
    pub fn new(profile: GamepadProfile) -> Option<Self> {
        let inner = match profile {
            GamepadProfile::DualShock4 => Inner::DualShock4(dualshock4::Codec::default()),
            GamepadProfile::DualSense => Inner::DualSense(dualsense::Codec::default()),
            GamepadProfile::SwitchPro => Inner::SwitchPro(switch_pro::Codec::default()),
            GamepadProfile::GenericHid => Inner::Generic(generic::Codec),
            GamepadProfile::XboxOne => Inner::Xbox(xbox::Codec::new(xbox::Model::One)),
            GamepadProfile::XboxSeries => Inner::Xbox(xbox::Codec::new(xbox::Model::Series)),
            _ => return None,
        };
        Some(Self { inner })
    }

    /// Whether a profile has a HID codec.
    pub fn supports(profile: GamepadProfile) -> bool {
        Self::new(profile).is_some()
    }

    /// The vendor, product and version the device announces.
    pub fn identity(&self) -> Identity {
        match &self.inner {
            Inner::DualShock4(_) => dualshock4::IDENTITY,
            Inner::DualSense(_) => dualsense::IDENTITY,
            Inner::SwitchPro(_) => switch_pro::IDENTITY,
            Inner::Generic(_) => generic::IDENTITY,
            Inner::Xbox(codec) => codec.identity(),
        }
    }

    /// The device's name, as a bus shows it.
    pub fn name(&self) -> &'static str {
        match &self.inner {
            Inner::DualShock4(_) => "Wireless Controller",
            Inner::DualSense(_) => "Wireless Controller",
            Inner::SwitchPro(_) => "Pro Controller",
            Inner::Generic(_) => "Alluno Gamepad",
            Inner::Xbox(_) => xbox::NAME,
        }
    }

    /// The report descriptor.
    pub fn descriptor(&self) -> &'static [u8] {
        match &self.inner {
            Inner::DualShock4(_) => dualshock4::DESCRIPTOR,
            Inner::DualSense(_) => dualsense::DESCRIPTOR,
            Inner::SwitchPro(_) => switch_pro::DESCRIPTOR,
            Inner::Generic(_) => generic::DESCRIPTOR,
            Inner::Xbox(_) => xbox::DESCRIPTOR,
        }
    }

    /// The feature reports the device answers.
    pub fn features(&self) -> &'static [FeatureReport] {
        match &self.inner {
            Inner::DualShock4(_) => dualshock4::FEATURES,
            Inner::DualSense(_) => dualsense::FEATURES,
            Inner::SwitchPro(_) => switch_pro::FEATURES,
            Inner::Generic(_) => generic::FEATURES,
            Inner::Xbox(_) => xbox::FEATURES,
        }
    }

    /// The size of the input report [`PadCodec::encode`] produces, id byte included.
    pub fn input_len(&self) -> usize {
        match &self.inner {
            Inner::DualShock4(_) => dualshock4::INPUT_LEN,
            Inner::DualSense(_) => dualsense::INPUT_LEN,
            Inner::SwitchPro(_) => switch_pro::INPUT_LEN,
            Inner::Generic(_) => generic::INPUT_LEN,
            Inner::Xbox(_) => xbox::INPUT_LEN,
        }
    }

    /// The largest output report the device accepts, id byte included.
    pub fn output_len(&self) -> usize {
        match &self.inner {
            Inner::DualShock4(_) => dualshock4::OUTPUT_LEN,
            Inner::DualSense(_) => dualsense::OUTPUT_LEN,
            Inner::SwitchPro(_) => switch_pro::OUTPUT_LEN,
            Inner::Generic(_) => generic::OUTPUT_LEN,
            Inner::Xbox(_) => xbox::OUTPUT_LEN,
        }
    }

    /// The input report for a snapshot, id byte first.
    pub fn encode(&mut self, state: &GamepadState) -> Vec<u8> {
        match &mut self.inner {
            Inner::DualShock4(codec) => codec.encode(state),
            Inner::DualSense(codec) => codec.encode(state),
            Inner::SwitchPro(codec) => codec.encode(state),
            Inner::Generic(codec) => codec.encode(state),
            Inner::Xbox(codec) => codec.encode(state),
        }
    }

    /// A second input report some profiles owe after [`PadCodec::encode`],
    /// for a control that lives on its own report and just changed.
    pub fn follow_up(&mut self, state: &GamepadState) -> Option<Vec<u8>> {
        match &mut self.inner {
            Inner::Xbox(codec) => codec.guide_report(state).map(|report| report.to_vec()),
            _ => None,
        }
    }

    /// Decodes an output report the game wrote, id byte first.
    pub fn decode(&mut self, report: &[u8]) -> Decoded {
        match &mut self.inner {
            Inner::DualShock4(codec) => codec.decode(report),
            Inner::DualSense(codec) => codec.decode(report),
            Inner::SwitchPro(codec) => codec.decode(report),
            Inner::Generic(codec) => codec.decode(report),
            Inner::Xbox(codec) => codec.decode(report),
        }
    }
}

/// The hat-switch value for a D-pad bitset: 0 north, clockwise to 7, 8 neutral.
pub fn hat(state: &GamepadState) -> u8 {
    use crate::buttons::{DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT, DPAD_UP};
    let up = state.pressed(DPAD_UP);
    let right = state.pressed(DPAD_RIGHT);
    let down = state.pressed(DPAD_DOWN);
    let left = state.pressed(DPAD_LEFT);
    match (up, right, down, left) {
        (true, false, false, false) => 0,
        (true, true, false, false) => 1,
        (false, true, false, false) => 2,
        (false, true, true, false) => 3,
        (false, false, true, false) => 4,
        (false, false, true, true) => 5,
        (false, false, false, true) => 6,
        (true, false, false, true) => 7,
        _ => 8,
    }
}

/// A signed stick axis as the unsigned byte Sony pads carry, 128 centred.
pub fn stick_byte(value: i16) -> u8 {
    ((i32::from(value) + 32768) >> 8).clamp(0, 255) as u8
}

/// A signed stick axis as an unsigned byte with up as 0, which is how Sony
/// pads report their Y axes.
pub fn stick_byte_inverted(value: i16) -> u8 {
    ((32768 - i32::from(value)) >> 8).clamp(0, 255) as u8
}

/// The report id of a report, or 0 for an empty one.
pub fn report_id(report: &[u8]) -> u8 {
    report.first().copied().unwrap_or(0)
}
