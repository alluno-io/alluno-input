//! Passthrough of a client's own devices into the host: MIDI, camera and
//! microphone. None of these is a HID, but a host asks the same questions of
//! each (can this machine emulate it, open one, feed it), so they live behind
//! the same runtime and the same capability answer.

use crate::Result;

/// A MIDI input port other software can open.
pub trait Midi {
    /// Delivers one complete MIDI message (status byte first, running status
    /// already expanded) with the port's clock as its timestamp.
    fn message(&mut self, message: &[u8]) -> Result<()>;
}

/// How the pixels of a [`VideoFrame`] are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PixelFormat {
    /// Planar Y then interleaved UV, 4:2:0.
    Nv12,
    /// Interleaved 8-bit red, green, blue, alpha.
    Rgba8,
}

/// One frame for a virtual camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// Every plane, tightly packed in `format`'s order.
    pub data: &'a [u8],
    /// Presentation time in microseconds on the consumer's clock.
    pub timestamp_us: u64,
}

/// A camera other software can capture from.
pub trait Camera {
    /// Delivers one frame; a backend that cannot convert `format` answers
    /// [`crate::Error::Unsupported`] and the consumer converts before retrying.
    fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<()>;
}

/// A block of interleaved signed 16-bit PCM for a virtual microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFrame<'a> {
    pub sample_rate: u32,
    pub channels: u8,
    /// `channels` interleaved samples per frame.
    pub samples: &'a [i16],
}

/// A microphone other software can record from.
pub trait Microphone {
    /// Delivers one block of samples; the backend resamples where its
    /// endpoint runs at a different rate.
    fn samples(&mut self, frame: &AudioFrame<'_>) -> Result<()>;
}
