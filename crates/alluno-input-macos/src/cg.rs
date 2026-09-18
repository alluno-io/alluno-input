//! Keyboard, mouse and pen over Core Graphics events, from one event source.
//!
//! Posting to the HID tap needs the Accessibility permission; without it every
//! post is silently dropped by the system, which is why a consumer should probe
//! `AXIsProcessTrusted` itself before promising input.

use std::marker::PhantomData;
use std::time::{Duration, Instant};

use alluno_input_core::{Error, Key, Keyboard, Mouse, MouseButton, Pen, PenState, Result};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::preferences::{CFPreferencesCopyAppValue, kCFPreferencesAnyApplication};
use core_foundation_sys::string::CFStringRef;
use core_graphics::display::CGDisplay;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;

use crate::keymap;

const MOUSE_SUBTYPE_TABLET_POINT: i64 = 1;
const SCROLL_UNIT_LINE: u32 = 1;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateScrollWheelEvent2(
        source: *const std::ffi::c_void,
        units: u32,
        wheel_count: u32,
        wheel1: i32,
        wheel2: i32,
        wheel3: i32,
    ) -> *mut std::ffi::c_void;
}

fn source() -> Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| Error::backend("CGEventSource could not be created"))
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

/// Whether the system lets this process post keyboard and mouse events.
pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// Asks the system for that permission: the dialog that points at the
/// Accessibility pane opens, and this app is listed there for the user to
/// switch on. Answers whether the permission is granted now.
pub fn request_accessibility() -> bool {
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value().as_CFType())]);
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

const DOUBLE_CLICK_DEFAULT: Duration = Duration::from_millis(500);
const DOUBLE_CLICK_KEY: &str = "com.apple.mouse.doubleClickThreshold";
const DOUBLE_CLICK_LONGEST: f64 = 5.0;
const CLICK_SLOP: f64 = 5.0;

/// The interval within which a second click continues a run, as the user set it.
pub fn double_click_interval() -> Duration {
    let key = CFString::from_static_string(DOUBLE_CLICK_KEY);
    let value = unsafe {
        CFPreferencesCopyAppValue(key.as_concrete_TypeRef(), kCFPreferencesAnyApplication)
    };
    if value.is_null() {
        return DOUBLE_CLICK_DEFAULT;
    }
    let value = unsafe { CFType::wrap_under_create_rule(value) };
    value
        .downcast::<CFNumber>()
        .and_then(|number| number.to_f64())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| Duration::from_secs_f64(seconds.min(DOUBLE_CLICK_LONGEST)))
        .unwrap_or(DOUBLE_CLICK_DEFAULT)
}

/// The click count the system stamps on hardware clicks, kept for posted ones.
///
/// A press of the same button within the double-click interval and a few points
/// of the last press continues the run; anything else starts one.
#[derive(Debug, Clone)]
pub struct ClickRun {
    interval: Duration,
    button: Option<MouseButton>,
    at: (f64, f64),
    last: Option<Instant>,
    count: i64,
}

impl ClickRun {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            button: None,
            at: (0.0, 0.0),
            last: None,
            count: 0,
        }
    }

    /// Counts a press and answers the click state it carries.
    pub fn press(&mut self, button: MouseButton, at: (f64, f64), now: Instant) -> i64 {
        let continues = self.button == Some(button)
            && self
                .last
                .is_some_and(|last| now.duration_since(last) <= self.interval)
            && (at.0 - self.at.0).abs() <= CLICK_SLOP
            && (at.1 - self.at.1).abs() <= CLICK_SLOP;
        self.count = if continues { self.count + 1 } else { 1 };
        self.button = Some(button);
        self.at = at;
        self.last = Some(now);
        self.count
    }

    /// The click state a release or a drag of `button` carries: that of the press it follows.
    pub fn current(&self, button: MouseButton) -> i64 {
        if self.button == Some(button) {
            self.count.max(1)
        } else {
            1
        }
    }
}

/// The union of every active display's bounds, in global points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Desktop {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl Desktop {
    /// Reads the current display arrangement.
    pub fn current() -> Self {
        let ids = CGDisplay::active_displays().unwrap_or_default();
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        for id in ids {
            let rect = CGDisplay::new(id).bounds();
            let (l, t) = (rect.origin.x, rect.origin.y);
            let (r, b) = (l + rect.size.width, t + rect.size.height);
            bounds = Some(match bounds {
                None => (l, t, r, b),
                Some((bl, bt, br, bb)) => (bl.min(l), bt.min(t), br.max(r), bb.max(b)),
            });
        }
        let (l, t, r, b) = bounds.unwrap_or_else(|| {
            let main = CGDisplay::main().bounds();
            (0.0, 0.0, main.size.width, main.size.height)
        });
        Self {
            left: l,
            top: t,
            width: (r - l).max(1.0),
            height: (b - t).max(1.0),
        }
    }

    /// Maps a 0..=65535 coordinate pair onto this desktop's points.
    pub fn point(&self, x: u16, y: u16) -> CGPoint {
        CGPoint {
            x: self.left + f64::from(x) * (self.width - 1.0) / 65535.0,
            y: self.top + f64::from(y) * (self.height - 1.0) / 65535.0,
        }
    }
}

/// A keyboard over `CGEvent`.
pub struct CgKeyboard {
    source: CGEventSource,
    _thread: PhantomData<*const ()>,
}

impl CgKeyboard {
    /// Creates the event source.
    pub fn open() -> Result<Self> {
        Ok(Self {
            source: source()?,
            _thread: PhantomData,
        })
    }
}

impl Keyboard for CgKeyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        let event =
            CGEvent::new_keyboard_event(self.source.clone(), keymap::to_keycode(key), pressed)
                .map_err(|_| Error::backend("keyboard event could not be created"))?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<()> {
        for pressed in [true, false] {
            let event = CGEvent::new_keyboard_event(self.source.clone(), 0, pressed)
                .map_err(|_| Error::backend("keyboard event could not be created"))?;
            event.set_string(text);
            event.post(CGEventTapLocation::HID);
        }
        Ok(())
    }
}

/// Which `CGEvent` a button transition or a drag becomes.
pub fn button_events(
    button: MouseButton,
) -> Option<(CGEventType, CGEventType, CGEventType, CGMouseButton, i64)> {
    Some(match button {
        MouseButton::Left => (
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::LeftMouseDragged,
            CGMouseButton::Left,
            0,
        ),
        MouseButton::Right => (
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGEventType::RightMouseDragged,
            CGMouseButton::Right,
            1,
        ),
        MouseButton::Middle => (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGEventType::OtherMouseDragged,
            CGMouseButton::Center,
            2,
        ),
        MouseButton::Back => (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGEventType::OtherMouseDragged,
            CGMouseButton::Center,
            3,
        ),
        MouseButton::Forward => (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGEventType::OtherMouseDragged,
            CGMouseButton::Center,
            4,
        ),
        _ => return None,
    })
}

/// A mouse over `CGEvent`.
pub struct CgMouse {
    source: CGEventSource,
    held: Option<MouseButton>,
    clicks: ClickRun,
    _thread: PhantomData<*const ()>,
}

impl CgMouse {
    /// Creates the event source.
    pub fn open() -> Result<Self> {
        Ok(Self {
            source: source()?,
            held: None,
            clicks: ClickRun::new(double_click_interval()),
            _thread: PhantomData,
        })
    }

    fn here(&self) -> Result<CGPoint> {
        CGEvent::new(self.source.clone())
            .map(|e| e.location())
            .map_err(|_| Error::backend("cursor position could not be read"))
    }

    fn move_to(&mut self, point: CGPoint) -> Result<()> {
        let drag = self.held.and_then(|held| {
            button_events(held).map(|(_, _, dragged, button, number)| {
                (dragged, button, number, self.clicks.current(held))
            })
        });
        let (kind, button, number, clicks) =
            drag.unwrap_or((CGEventType::MouseMoved, CGMouseButton::Left, 0, 0));
        let event = CGEvent::new_mouse_event(self.source.clone(), kind, point, button)
            .map_err(|_| Error::backend("mouse event could not be created"))?;
        if number > 0 {
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number);
        }
        if clicks > 0 {
            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, clicks);
        }
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
}

impl Mouse for CgMouse {
    fn move_abs(&mut self, x: u16, y: u16) -> Result<()> {
        let point = Desktop::current().point(x, y);
        self.move_to(point)
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        let here = self.here()?;
        let point = CGPoint {
            x: here.x + f64::from(dx),
            y: here.y + f64::from(dy),
        };
        self.move_to(point)?;
        Ok(())
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let (down, up, _, cg_button, number) = button_events(button).ok_or(Error::Unsupported)?;
        let point = self.here()?;
        let clicks = if pressed {
            self.clicks
                .press(button, (point.x, point.y), Instant::now())
        } else {
            self.clicks.current(button)
        };
        let event = CGEvent::new_mouse_event(
            self.source.clone(),
            if pressed { down } else { up },
            point,
            cg_button,
        )
        .map_err(|_| Error::backend("mouse event could not be created"))?;
        if number > 0 {
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number);
        }
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, clicks);
        event.post(CGEventTapLocation::HID);
        self.held = if pressed {
            Some(button)
        } else if self.held == Some(button) {
            None
        } else {
            self.held
        };
        Ok(())
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        let raw = unsafe {
            CGEventCreateScrollWheelEvent2(
                self.source.as_ptr().cast(),
                SCROLL_UNIT_LINE,
                2,
                dy,
                dx,
                0,
            )
        };
        if raw.is_null() {
            return Err(Error::backend("scroll event could not be created"));
        }
        let event = unsafe { CGEvent::from_ptr(raw.cast()) };
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
}

/// A stylus over `CGEvent` tablet-point mouse events.
pub struct CgPen {
    source: CGEventSource,
    last: Option<PenState>,
    _thread: PhantomData<*const ()>,
}

impl CgPen {
    /// Creates the event source.
    pub fn open() -> Result<Self> {
        Ok(Self {
            source: source()?,
            last: None,
            _thread: PhantomData,
        })
    }
}

/// Which event a pen sample becomes, given what came before it.
pub fn pen_event(was: Option<&PenState>, now: &PenState) -> (CGEventType, CGMouseButton) {
    let was_down = was.is_some_and(|s| s.down);
    let barrel = now.barrel;
    match (was_down, now.down, barrel) {
        (false, true, false) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
        (false, true, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
        (true, false, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
        (true, false, true) => (CGEventType::RightMouseUp, CGMouseButton::Right),
        (true, true, false) => (CGEventType::LeftMouseDragged, CGMouseButton::Left),
        (true, true, true) => (CGEventType::RightMouseDragged, CGMouseButton::Right),
        (false, false, _) => (CGEventType::MouseMoved, CGMouseButton::Left),
    }
}

impl Pen for CgPen {
    fn report(&mut self, state: &PenState) -> Result<()> {
        let (kind, button) = pen_event(self.last.as_ref(), state);
        let point = Desktop::current().point(state.x, state.y);
        let event = CGEvent::new_mouse_event(self.source.clone(), kind, point, button)
            .map_err(|_| Error::backend("pen event could not be created"))?;
        let pressure = if state.down {
            f64::from(state.pressure) / 65535.0
        } else {
            0.0
        };
        event.set_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE, MOUSE_SUBTYPE_TABLET_POINT);
        event.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, pressure);
        event.set_double_value_field(EventField::TABLET_EVENT_POINT_PRESSURE, pressure);
        event.set_double_value_field(
            EventField::TABLET_EVENT_TILT_X,
            f64::from(state.tilt_x) / 90.0,
        );
        event.set_double_value_field(
            EventField::TABLET_EVENT_TILT_Y,
            f64::from(state.tilt_y) / 90.0,
        );
        event.set_double_value_field(EventField::TABLET_EVENT_ROTATION, f64::from(state.twist));
        event.post(CGEventTapLocation::HID);
        self.last = Some(*state);
        Ok(())
    }
}
