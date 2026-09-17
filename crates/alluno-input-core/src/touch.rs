//! The touch vocabulary.

/// The most contacts one report carries; every backend accepts at least this many.
pub const MAX_CONTACTS: usize = 10;

/// One finger, in the driver's 0..=65535 space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TouchContact {
    /// Stable per finger for the length of the touch.
    pub id: u8,
    pub x: u16,
    pub y: u16,
    pub pressure: u16,
    /// Contact ellipse size, 0 when the digitiser does not report it.
    pub width: u16,
    pub height: u16,
    /// The finger is on the surface; a contact reported with `down: false` lifts it.
    pub down: bool,
}

/// Every finger currently known, which is what every OS touch API takes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TouchState {
    pub contacts: Vec<TouchContact>,
}

impl TouchState {
    /// Whether the report fits every backend's contact limit.
    pub fn fits(&self) -> bool {
        self.contacts.len() <= MAX_CONTACTS
    }
}
