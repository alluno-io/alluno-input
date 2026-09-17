//! A [`Host`] with no devices behind it: every call is recorded, every output
//! a test feeds is delivered, so a consumer can assert on what it injected
//! without a driver anywhere near the test.

use std::sync::{Arc, Mutex};

use alluno_input_core::{
    Backing, Capabilities, Gamepad, GamepadOutput, GamepadProfile, GamepadState, Host, Key,
    Keyboard, Mouse, MouseButton, Options, OutputSink, Pen, PenState, Result, Touch, TouchState,
};

/// One thing a consumer fed to a fake device.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Recorded {
    Key {
        key: Key,
        pressed: bool,
    },
    Text(String),
    MoveAbs {
        x: u16,
        y: u16,
    },
    MoveRel {
        dx: i32,
        dy: i32,
    },
    Button {
        button: MouseButton,
        pressed: bool,
    },
    Wheel {
        dx: i32,
        dy: i32,
    },
    Pen(PenState),
    Touch(TouchState),
    Gamepad {
        profile: GamepadProfile,
        state: GamepadState,
    },
}

/// The shared log every fake device appends to.
#[derive(Debug, Default, Clone)]
pub struct Recorder {
    log: Arc<Mutex<Vec<Recorded>>>,
}

impl Recorder {
    /// Everything recorded so far, in order.
    pub fn all(&self) -> Vec<Recorded> {
        self.log.lock().unwrap().clone()
    }

    /// How many calls were recorded.
    pub fn len(&self) -> usize {
        self.log.lock().unwrap().len()
    }

    /// Whether nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Forgets everything recorded so far.
    pub fn clear(&self) {
        self.log.lock().unwrap().clear();
    }

    fn push(&self, item: Recorded) {
        self.log.lock().unwrap().push(item);
    }
}

/// The recording host. Everything is available; every device writes to [`FakeHost::recorder`].
pub struct FakeHost {
    recorder: Recorder,
    options: Options,
}

impl FakeHost {
    /// The log its devices write to.
    pub fn recorder(&self) -> Recorder {
        self.recorder.clone()
    }

    /// The options it was opened with.
    pub fn options(&self) -> &Options {
        &self.options
    }
}

impl Host for FakeHost {
    fn probe() -> Capabilities {
        Capabilities {
            keyboard: Backing::UserApi,
            mouse: Backing::UserApi,
            pen: Backing::UserApi,
            touch: Backing::UserApi,
            gamepads: GamepadProfile::ALL
                .iter()
                .map(|p| (*p, Backing::UserApi))
                .collect(),
            max_gamepads: None,
            midi: Backing::UserApi,
            camera: Backing::UserApi,
            microphone: Backing::UserApi,
        }
    }

    fn open(options: Options) -> Result<Self> {
        Ok(Self {
            recorder: Recorder::default(),
            options,
        })
    }

    fn keyboard(&self) -> Result<Box<dyn Keyboard>> {
        Ok(Box::new(FakeKeyboard {
            recorder: self.recorder(),
        }))
    }

    fn mouse(&self) -> Result<Box<dyn Mouse>> {
        Ok(Box::new(FakeMouse {
            recorder: self.recorder(),
        }))
    }

    fn pen(&self) -> Result<Box<dyn Pen>> {
        Ok(Box::new(FakePen {
            recorder: self.recorder(),
        }))
    }

    fn touch(&self) -> Result<Box<dyn Touch>> {
        Ok(Box::new(FakeTouch {
            recorder: self.recorder(),
        }))
    }

    fn gamepad(&self, profile: GamepadProfile) -> Result<Box<dyn Gamepad>> {
        Ok(Box::new(FakeGamepad::new(profile, self.recorder())))
    }
}

/// A keyboard that records.
pub struct FakeKeyboard {
    recorder: Recorder,
}

impl Keyboard for FakeKeyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        self.recorder.push(Recorded::Key { key, pressed });
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<()> {
        self.recorder.push(Recorded::Text(text.to_string()));
        Ok(())
    }
}

/// A mouse that records.
pub struct FakeMouse {
    recorder: Recorder,
}

impl Mouse for FakeMouse {
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()> {
        self.recorder.push(Recorded::MoveAbs { x, y });
        Ok(())
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        self.recorder.push(Recorded::MoveRel { dx, dy });
        Ok(())
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        self.recorder.push(Recorded::Button { button, pressed });
        Ok(())
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        self.recorder.push(Recorded::Wheel { dx, dy });
        Ok(())
    }
}

/// A pen that records.
pub struct FakePen {
    recorder: Recorder,
}

impl Pen for FakePen {
    fn report(&mut self, state: &PenState) -> Result<()> {
        self.recorder.push(Recorded::Pen(*state));
        Ok(())
    }
}

/// A digitiser that records.
pub struct FakeTouch {
    recorder: Recorder,
}

impl Touch for FakeTouch {
    fn report(&mut self, state: &TouchState) -> Result<()> {
        self.recorder.push(Recorded::Touch(state.clone()));
        Ok(())
    }
}

/// A pad that records, and lets a test play the game's side.
pub struct FakeGamepad {
    profile: GamepadProfile,
    recorder: Recorder,
    sink: Arc<Mutex<Option<OutputSink>>>,
}

impl FakeGamepad {
    fn new(profile: GamepadProfile, recorder: Recorder) -> Self {
        Self {
            profile,
            recorder,
            sink: Arc::new(Mutex::new(None)),
        }
    }

    /// A handle a test keeps to feed output after the pad is boxed away.
    pub fn output(&self) -> OutputFeed {
        OutputFeed {
            sink: self.sink.clone(),
        }
    }
}

/// Feeds game output into a fake pad's installed sink.
#[derive(Clone)]
pub struct OutputFeed {
    sink: Arc<Mutex<Option<OutputSink>>>,
}

impl OutputFeed {
    /// Delivers one output; answers whether a sink was installed to take it.
    pub fn feed(&self, output: GamepadOutput) -> bool {
        match self.sink.lock().unwrap().as_mut() {
            Some(sink) => {
                sink(output);
                true
            }
            None => false,
        }
    }
}

impl Gamepad for FakeGamepad {
    fn submit(&mut self, state: &GamepadState) -> Result<()> {
        self.recorder.push(Recorded::Gamepad {
            profile: self.profile,
            state: *state,
        });
        Ok(())
    }

    fn on_output(&mut self, sink: OutputSink) -> Result<()> {
        *self.sink.lock().unwrap() = Some(sink);
        Ok(())
    }

    fn slot(&mut self) -> Option<u8> {
        Some(0)
    }
}

/// Opens a fake host and hands back the pad and its feed together, since the
/// boxed trait object hides the feed otherwise.
pub fn fake_gamepad(profile: GamepadProfile) -> (FakeGamepad, Recorder) {
    let recorder = Recorder::default();
    (FakeGamepad::new(profile, recorder.clone()), recorder)
}
