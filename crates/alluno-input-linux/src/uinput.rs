//! The one shared uinput device builder.
//!
//! The `input_event`, `uinput_user_dev`, `uinput_setup` and `uinput_abs_setup`
//! layouts and the `UI_*` request values are kernel ABI and must stay byte-exact.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;

use alluno_input_core::{Error, Result};

/// The uinput character device.
pub const UINPUT_PATH: &str = "/dev/uinput";

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_REL: u16 = 0x02;
pub const EV_ABS: u16 = 0x03;

pub const REL_X: u16 = 0x00;
pub const REL_Y: u16 = 0x01;
pub const REL_HWHEEL: u16 = 0x06;
pub const REL_WHEEL: u16 = 0x08;

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_PRESSURE: u16 = 0x18;
pub const ABS_DISTANCE: u16 = 0x19;
pub const ABS_TILT_X: u16 = 0x1a;
pub const ABS_TILT_Y: u16 = 0x1b;
pub const ABS_MT_SLOT: u16 = 0x2f;
pub const ABS_MT_TOUCH_MAJOR: u16 = 0x30;
pub const ABS_MT_TOUCH_MINOR: u16 = 0x31;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
pub const ABS_MT_PRESSURE: u16 = 0x3a;

pub const BTN_LEFT: u16 = 0x110;
pub const BTN_RIGHT: u16 = 0x111;
pub const BTN_MIDDLE: u16 = 0x112;
pub const BTN_SIDE: u16 = 0x113;
pub const BTN_EXTRA: u16 = 0x114;

pub const BTN_TOOL_PEN: u16 = 0x140;
pub const BTN_TOOL_RUBBER: u16 = 0x141;
pub const BTN_TOUCH: u16 = 0x14a;
pub const BTN_STYLUS: u16 = 0x14b;
pub const BTN_STYLUS2: u16 = 0x14c;

pub const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
pub const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
pub const UI_SET_RELBIT: libc::c_ulong = 0x40045566;
pub const UI_SET_ABSBIT: libc::c_ulong = 0x40045567;
pub const UI_SET_PROPBIT: libc::c_ulong = 0x4004556e;
pub const UI_DEV_SETUP: libc::c_ulong = 0x405c5503;
pub const UI_ABS_SETUP: libc::c_ulong = 0x401c5504;
pub const UI_DEV_CREATE: libc::c_ulong = 0x5501;
pub const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

pub const INPUT_PROP_DIRECT: u16 = 0x01;

pub const BUS_USB: u16 = 0x03;
pub const BUS_VIRTUAL: u16 = 0x06;

/// The vendor id every Alluno virtual device carries.
pub const VENDOR: u16 = 0x1234;

/// The kernel `input_event`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputEvent {
    pub time: libc::timeval,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

/// Input device identity (`input_id`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

/// Modern uinput device setup (`uinput_setup`, kernel 4.5 and later).
#[repr(C)]
pub struct UinputSetup {
    pub id: InputId,
    pub name: [u8; 80],
    pub ff_effects_max: u32,
}

/// Per-axis configuration with resolution (`uinput_abs_setup`).
#[repr(C)]
pub struct UinputAbsSetup {
    pub code: u16,
    pub _pad: u16,
    pub info: InputAbsinfo,
}

/// Absolute-axis info (`input_absinfo`).
#[repr(C)]
pub struct InputAbsinfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

/// One absolute axis a device declares.
#[derive(Debug, Clone, Copy)]
pub struct Axis {
    pub code: u16,
    pub min: i32,
    pub max: i32,
    pub resolution: i32,
}

/// Whether `/dev/uinput` exists and this process may write it.
pub fn available() -> bool {
    OpenOptions::new().write(true).open(UINPUT_PATH).is_ok()
}

/// An open uinput device; `Drop` destroys it.
pub struct Device {
    file: File,
}

impl Device {
    /// Opens `/dev/uinput` for writing.
    pub fn open() -> Result<Self> {
        let file = OpenOptions::new().write(true).open(UINPUT_PATH)?;
        Ok(Self { file })
    }

    fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    /// Sets one capability bit (`UI_SET_*`).
    pub fn set_bit(&self, request: libc::c_ulong, value: u16) -> Result<()> {
        let rc = unsafe { libc::ioctl(self.fd(), request, libc::c_ulong::from(value)) };
        if rc < 0 {
            return Err(Error::backend(format!(
                "uinput ioctl {request:#x} refused value {value}"
            )));
        }
        Ok(())
    }

    /// Declares an absolute axis with its range and resolution (`UI_ABS_SETUP`).
    pub fn abs_setup(&self, axis: Axis) -> Result<()> {
        self.set_bit(UI_SET_ABSBIT, axis.code)?;
        let setup = UinputAbsSetup {
            code: axis.code,
            _pad: 0,
            info: InputAbsinfo {
                value: 0,
                minimum: axis.min,
                maximum: axis.max,
                fuzz: 0,
                flat: 0,
                resolution: axis.resolution,
            },
        };
        let rc = unsafe { libc::ioctl(self.fd(), UI_ABS_SETUP, &setup as *const UinputAbsSetup) };
        if rc < 0 {
            return Err(Error::backend(format!(
                "UI_ABS_SETUP refused axis {:#x}",
                axis.code
            )));
        }
        Ok(())
    }

    /// Names the device and creates it (`UI_DEV_SETUP` then `UI_DEV_CREATE`).
    pub fn create(&self, name: &str, id: InputId) -> Result<()> {
        let mut setup = UinputSetup {
            id,
            name: [0u8; 80],
            ff_effects_max: 0,
        };
        let bytes = name.as_bytes();
        let len = bytes.len().min(79);
        setup.name[..len].copy_from_slice(&bytes[..len]);
        let rc = unsafe { libc::ioctl(self.fd(), UI_DEV_SETUP, &setup as *const UinputSetup) };
        if rc < 0 {
            return Err(Error::backend("UI_DEV_SETUP refused"));
        }
        let rc = unsafe { libc::ioctl(self.fd(), UI_DEV_CREATE) };
        if rc < 0 {
            return Err(Error::backend("UI_DEV_CREATE refused"));
        }
        Ok(())
    }

    /// Writes one event.
    pub fn emit(&mut self, type_: u16, code: u16, value: i32) -> Result<()> {
        let event = InputEvent {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&event as *const InputEvent).cast::<u8>(),
                std::mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(bytes)?;
        Ok(())
    }

    /// Ends the frame with a `SYN_REPORT`.
    pub fn sync(&mut self) -> Result<()> {
        self.emit(EV_SYN, 0, 0)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::ioctl(self.fd(), UI_DEV_DESTROY);
        }
    }
}

/// Lets udev finish naming the node before the first event, which is what
/// keeps a compositor from missing the device it was just handed.
pub fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(200));
}
