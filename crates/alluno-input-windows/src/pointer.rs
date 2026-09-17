//! Pen and touch through the Synthetic Pointer API.
//!
//! `CreateSyntheticPointerDevice` gives a process its own pointer device whose
//! frames go through the same pointer pipeline as a real digitiser, pressure,
//! tilt, rotation and contact rectangles included. Coordinates arrive in the
//! port's 0..=65535 virtual-desktop space and are mapped onto the virtual
//! screen's pixel rectangle here, which is the same mapping the kernel filter
//! and `SendInput` apply to their absolute moves.

use std::collections::HashMap;
use std::ffi::c_void;
use std::marker::PhantomData;

use alluno_input_core::{Error, MAX_CONTACTS, Pen, PenState, Result, Touch, TouchState};
use windows::Win32::Foundation::{HANDLE, HWND, POINT, RECT};
use windows::Win32::UI::Controls::{
    HSYNTHETICPOINTERDEVICE, POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
};
use windows::Win32::UI::Input::Pointer::{
    InjectSyntheticPointerInput, POINTER_BUTTON_CHANGE_TYPE, POINTER_FLAG_DOWN,
    POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_UP, POINTER_FLAG_UPDATE,
    POINTER_FLAGS, POINTER_INFO, POINTER_PEN_INFO, POINTER_TOUCH_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, POINTER_INPUT_TYPE, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateSyntheticPointerDevice(pointer_type: i32, max_count: u32, mode: i32) -> *mut c_void;
    fn DestroySyntheticPointerDevice(device: *mut c_void);
}

const PT_TOUCH: i32 = 2;
const PT_PEN: i32 = 3;
const POINTER_FEEDBACK_INDIRECT: i32 = 1;

const PEN_FLAG_NONE: u32 = 0;
const PEN_FLAG_BARREL: u32 = 1;
const PEN_FLAG_INVERTED: u32 = 2;
const PEN_FLAG_ERASER: u32 = 4;
const PEN_MASK_PRESSURE: u32 = 1;
const PEN_MASK_ROTATION: u32 = 2;
const PEN_MASK_TILT_X: u32 = 4;
const PEN_MASK_TILT_Y: u32 = 8;

const TOUCH_FLAG_NONE: u32 = 0;
const TOUCH_MASK_CONTACTAREA: u32 = 1;
const TOUCH_MASK_PRESSURE: u32 = 4;

/// The pointer flags one pen transition turns into.
///
/// `was` is the last reported state, `now` the new one; the flags say whether
/// this frame is a touch-down, a lift, a move, or a departure from range.
pub fn pen_flags(was: Option<&PenState>, now: &PenState) -> u32 {
    let was_down = was.is_some_and(|s| s.down);
    let in_range = now.in_range || now.down;
    if now.down && !was_down {
        (POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT).0
    } else if !now.down && was_down {
        (POINTER_FLAG_UP | POINTER_FLAG_INRANGE).0
    } else if now.down {
        (POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT).0
    } else if in_range {
        (POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE).0
    } else {
        POINTER_FLAG_UPDATE.0
    }
}

/// The pen-specific flag bits for a state.
pub fn pen_state_flags(state: &PenState) -> u32 {
    let mut flags = PEN_FLAG_NONE;
    if state.barrel {
        flags |= PEN_FLAG_BARREL;
    }
    if state.eraser {
        flags |= PEN_FLAG_INVERTED | PEN_FLAG_ERASER;
    }
    flags
}

/// Windows pen pressure is 0..=1024.
pub fn pen_pressure(pressure: u16) -> u32 {
    (u32::from(pressure) * 1024) / 65535
}

/// The virtual screen rectangle in pixels: origin and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualScreen {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl VirtualScreen {
    /// Reads the current virtual screen from the system metrics.
    pub fn current() -> Self {
        unsafe {
            Self {
                left: GetSystemMetrics(SM_XVIRTUALSCREEN),
                top: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
            }
        }
    }

    /// Maps a 0..=65535 coordinate pair onto this rectangle's pixels.
    pub fn pixel(&self, x: u16, y: u16) -> POINT {
        POINT {
            x: self.left + (i64::from(x) * i64::from(self.width - 1) / 65535) as i32,
            y: self.top + (i64::from(y) * i64::from(self.height - 1) / 65535) as i32,
        }
    }
}

fn pointer_info(kind: i32, id: u32, frame: u32, flags: u32, at: POINT) -> POINTER_INFO {
    POINTER_INFO {
        pointerType: POINTER_INPUT_TYPE(kind),
        pointerId: id,
        frameId: frame,
        pointerFlags: POINTER_FLAGS(flags),
        sourceDevice: HANDLE::default(),
        hwndTarget: HWND::default(),
        ptPixelLocation: at,
        ptHimetricLocation: POINT::default(),
        ptPixelLocationRaw: at,
        ptHimetricLocationRaw: POINT::default(),
        dwTime: 0,
        historyCount: 1,
        InputData: 0,
        dwKeyStates: 0,
        PerformanceCount: 0,
        ButtonChangeType: POINTER_BUTTON_CHANGE_TYPE(0),
    }
}

struct SyntheticDevice {
    handle: HSYNTHETICPOINTERDEVICE,
}

impl SyntheticDevice {
    fn create(kind: i32, max_count: u32) -> Result<Self> {
        let raw =
            unsafe { CreateSyntheticPointerDevice(kind, max_count, POINTER_FEEDBACK_INDIRECT) };
        if raw.is_null() {
            return Err(Error::unavailable(
                "CreateSyntheticPointerDevice refused; Windows 10 1809 or later is required",
            ));
        }
        Ok(Self {
            handle: HSYNTHETICPOINTERDEVICE(raw),
        })
    }

    fn inject(&self, frames: &[POINTER_TYPE_INFO]) -> Result<()> {
        unsafe { InjectSyntheticPointerInput(self.handle, frames) }
            .map_err(|e| Error::backend(format!("InjectSyntheticPointerInput: {e}")))
    }
}

impl Drop for SyntheticDevice {
    fn drop(&mut self) {
        unsafe { DestroySyntheticPointerDevice(self.handle.0) };
    }
}

/// A stylus over the Synthetic Pointer API.
pub struct SyntheticPen {
    device: SyntheticDevice,
    last: Option<PenState>,
    frame: u32,
    _thread: PhantomData<*const ()>,
}

impl SyntheticPen {
    /// Creates the pen device.
    pub fn open() -> Result<Self> {
        Ok(Self {
            device: SyntheticDevice::create(PT_PEN, 1)?,
            last: None,
            frame: 0,
            _thread: PhantomData,
        })
    }
}

impl Pen for SyntheticPen {
    fn report(&mut self, state: &PenState) -> Result<()> {
        self.frame = self.frame.wrapping_add(1);
        let flags = pen_flags(self.last.as_ref(), state);
        let at = VirtualScreen::current().pixel(state.x, state.y);
        let info = POINTER_PEN_INFO {
            pointerInfo: pointer_info(PT_PEN, 1, self.frame, flags, at),
            penFlags: pen_state_flags(state),
            penMask: PEN_MASK_PRESSURE | PEN_MASK_ROTATION | PEN_MASK_TILT_X | PEN_MASK_TILT_Y,
            pressure: pen_pressure(state.pressure),
            rotation: u32::from(state.twist % 360),
            tiltX: i32::from(state.tilt_x),
            tiltY: i32::from(state.tilt_y),
        };
        let frame = POINTER_TYPE_INFO {
            r#type: POINTER_INPUT_TYPE(PT_PEN),
            Anonymous: POINTER_TYPE_INFO_0 { penInfo: info },
        };
        self.device.inject(std::slice::from_ref(&frame))?;
        self.last = Some(*state);
        Ok(())
    }
}

/// One contact's transition, decided from what was known about its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactMove {
    Down,
    Update,
    Up,
}

/// Decides each contact's transition and which known contacts vanished.
///
/// `known` is the set of ids currently down; the answer lists every contact to
/// inject this frame, including lifts for ids that dropped out of the report.
pub fn touch_moves(known: &[u8], report: &TouchState) -> Vec<(u8, ContactMove)> {
    let mut moves = Vec::with_capacity(report.contacts.len() + known.len());
    for contact in &report.contacts {
        let was = known.contains(&contact.id);
        let step = match (was, contact.down) {
            (false, true) => ContactMove::Down,
            (true, true) => ContactMove::Update,
            (true, false) => ContactMove::Up,
            (false, false) => continue,
        };
        moves.push((contact.id, step));
    }
    for id in known {
        if !report.contacts.iter().any(|c| c.id == *id) {
            moves.push((*id, ContactMove::Up));
        }
    }
    moves
}

/// A touch digitiser over the Synthetic Pointer API.
pub struct SyntheticTouch {
    device: SyntheticDevice,
    down: Vec<u8>,
    last_at: HashMap<u8, (u16, u16)>,
    frame: u32,
    _thread: PhantomData<*const ()>,
}

impl SyntheticTouch {
    /// Creates the touch device with room for [`MAX_CONTACTS`] fingers.
    pub fn open() -> Result<Self> {
        Ok(Self {
            device: SyntheticDevice::create(PT_TOUCH, MAX_CONTACTS as u32)?,
            down: Vec::new(),
            last_at: HashMap::new(),
            frame: 0,
            _thread: PhantomData,
        })
    }
}

impl Touch for SyntheticTouch {
    fn report(&mut self, state: &TouchState) -> Result<()> {
        if !state.fits() {
            return Err(Error::Unsupported);
        }
        let moves = touch_moves(&self.down, state);
        if moves.is_empty() {
            return Ok(());
        }
        self.frame = self.frame.wrapping_add(1);
        let screen = VirtualScreen::current();
        let mut frames = Vec::with_capacity(moves.len());
        for (id, step) in &moves {
            let contact = state.contacts.iter().find(|c| c.id == *id).copied();
            let (x, y) = contact
                .map(|c| (c.x, c.y))
                .or_else(|| self.last_at.get(id).copied())
                .unwrap_or((0, 0));
            let at = screen.pixel(x, y);
            let (half_w, half_h) = contact
                .map(|c| {
                    (
                        i32::from(c.width / 2).max(2),
                        i32::from(c.height / 2).max(2),
                    )
                })
                .unwrap_or((2, 2));
            let flags = match step {
                ContactMove::Down => {
                    POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT
                }
                ContactMove::Update => {
                    POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT
                }
                ContactMove::Up => POINTER_FLAG_UP,
            };
            let rect = RECT {
                left: at.x - half_w,
                top: at.y - half_h,
                right: at.x + half_w,
                bottom: at.y + half_h,
            };
            let info = POINTER_TOUCH_INFO {
                pointerInfo: pointer_info(PT_TOUCH, u32::from(*id) + 1, self.frame, flags.0, at),
                touchFlags: TOUCH_FLAG_NONE,
                touchMask: TOUCH_MASK_CONTACTAREA | TOUCH_MASK_PRESSURE,
                rcContact: rect,
                rcContactRaw: rect,
                orientation: 0,
                pressure: contact.map(|c| pen_pressure(c.pressure)).unwrap_or(0),
            };
            frames.push(POINTER_TYPE_INFO {
                r#type: POINTER_INPUT_TYPE(PT_TOUCH),
                Anonymous: POINTER_TYPE_INFO_0 { touchInfo: info },
            });
        }
        self.device.inject(&frames)?;
        for (id, step) in moves {
            match step {
                ContactMove::Down => {
                    self.down.push(id);
                }
                ContactMove::Up => {
                    self.down.retain(|d| *d != id);
                    self.last_at.remove(&id);
                }
                ContactMove::Update => {}
            }
            if let Some(c) = state.contacts.iter().find(|c| c.id == id) {
                self.last_at.insert(id, (c.x, c.y));
            }
        }
        Ok(())
    }
}
