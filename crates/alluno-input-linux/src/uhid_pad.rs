//! A gamepad on `uhid`, speaking one profile's HID protocol through the
//! shared codec: the kernel binds `hid-playstation`, `hid-nintendo` or the
//! generic HID driver to the node and every consumer above sees the real pad.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use alluno_input_core::hid::PadCodec;
use alluno_input_core::{
    Error, Gamepad, GamepadOutput, GamepadProfile, GamepadState, OutputSink, Result,
};

use crate::uhid::{Device, Request, UHID_FEATURE_REPORT};

/// A pad on uhid.
pub struct UhidGamepad {
    device: Device,
    codec: Arc<Mutex<PadCodec>>,
    stop: Arc<AtomicBool>,
    _thread: PhantomData<*const ()>,
}

impl UhidGamepad {
    /// Creates the node for a profile; a profile without a HID codec is unsupported.
    pub fn create(name: &str, profile: GamepadProfile) -> Result<Self> {
        let codec = PadCodec::new(profile).ok_or(Error::Unsupported)?;
        let device = Device::create(name, codec.identity(), codec.descriptor())?;
        Ok(Self {
            device,
            codec: Arc::new(Mutex::new(codec)),
            stop: Arc::new(AtomicBool::new(false)),
            _thread: PhantomData,
        })
    }
}

impl Gamepad for UhidGamepad {
    fn submit(&mut self, state: &GamepadState) -> Result<()> {
        let (report, follow_up) = {
            let mut codec = self.codec.lock().unwrap();
            (codec.encode(state), codec.follow_up(state))
        };
        self.device.input(&report)?;
        match follow_up {
            Some(extra) => self.device.input(&extra),
            None => Ok(()),
        }
    }

    fn on_output(&mut self, mut sink: OutputSink) -> Result<()> {
        let mut reader = self.device.reader()?;
        let codec = self.codec.clone();
        let stop = self.stop.clone();
        std::thread::Builder::new()
            .name("uhid-output".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let request = match reader.next(500) {
                        Ok(Some(request)) => request,
                        Ok(None) => continue,
                        Err(_) => break,
                    };
                    match request {
                        Request::Output(report) => {
                            let decoded = codec.lock().unwrap().decode(&report);
                            for output in decoded.outputs {
                                sink(output);
                            }
                            if let Some(reply) = decoded.reply {
                                let _ = reader.input(&reply);
                            }
                        }
                        Request::SetReport { id, report } => {
                            sink(GamepadOutput::Raw(report));
                            let _ = reader.reply_set(id, true);
                        }
                        Request::GetReport { id, number, kind } => {
                            let answer = if kind == UHID_FEATURE_REPORT {
                                codec
                                    .lock()
                                    .unwrap()
                                    .features()
                                    .iter()
                                    .find(|feature| feature.id == number)
                                    .map(|feature| feature.data.to_vec())
                            } else {
                                None
                            };
                            let _ = reader.reply_get(id, answer.as_deref());
                        }
                        Request::Other(_) => {}
                    }
                }
            })
            .map_err(Error::Io)?;
        Ok(())
    }

    fn slot(&mut self) -> Option<u8> {
        None
    }
}

impl Drop for UhidGamepad {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
