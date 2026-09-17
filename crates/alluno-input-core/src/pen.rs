//! The stylus vocabulary.

/// One stylus sample in the driver's own space.
///
/// Coordinates are 0..=65535 across the target surface; scaling from any other
/// space is the consumer's job, because only the consumer knows which display
/// the sample is meant for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PenState {
    pub x: u16,
    pub y: u16,
    /// Contact pressure, 0 when hovering.
    pub pressure: u16,
    /// Tilt away from vertical along x, in degrees.
    pub tilt_x: i8,
    /// Tilt away from vertical along y, in degrees.
    pub tilt_y: i8,
    /// Barrel rotation, 0..=359 degrees.
    pub twist: u16,
    /// The tip is touching the surface.
    pub down: bool,
    /// The barrel button is held.
    pub barrel: bool,
    /// The eraser end is presented rather than the tip.
    pub eraser: bool,
    /// The pen is within the surface's hover range at all.
    pub in_range: bool,
}
