//! Why a device could not be opened or fed.

/// The one error type every host speaks.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// This machine cannot emulate the device; the message names what is missing.
    #[error("unavailable: {0}")]
    Unavailable(String),
    /// The device exists but cannot do this particular operation.
    #[error("unsupported by this device")]
    Unsupported,
    /// The driver or OS API refused or failed the operation.
    #[error("backend: {0}")]
    Backend(String),
    /// A file or device node could not be opened or written.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Wraps whatever the backend said into [`Error::Backend`].
    pub fn backend(message: impl std::fmt::Display) -> Self {
        Self::Backend(message.to_string())
    }

    /// Names what this machine lacks.
    pub fn unavailable(what: impl std::fmt::Display) -> Self {
        Self::Unavailable(what.to_string())
    }
}

/// The result type every device method returns.
pub type Result<T> = std::result::Result<T, Error>;
