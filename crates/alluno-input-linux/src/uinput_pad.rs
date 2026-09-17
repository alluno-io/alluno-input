use crate::report::{GamepadKind, GamepadNotification, XGamepad, xbuttons};
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::mem;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const UINPUT_PATH: &str = "/dev/uinput";

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const EV_FF: u16 = 0x15;
const EV_UINPUT: u16 = 0x0101;

const BTN_SOUTH: u16 = 0x130;
const BTN_EAST: u16 = 0x131;
const BTN_NORTH: u16 = 0x133;
const BTN_WEST: u16 = 0x134;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;
const BTN_MODE: u16 = 0x13c;
const BTN_THUMBL: u16 = 0x13d;
const BTN_THUMBR: u16 = 0x13e;

const DS4_BTN_SQUARE: u16 = 0x130;
const DS4_BTN_CROSS: u16 = 0x131;
const DS4_BTN_CIRCLE: u16 = 0x132;
const DS4_BTN_TRIANGLE: u16 = 0x133;
const DS4_BTN_L1: u16 = 0x134;
const DS4_BTN_R1: u16 = 0x135;
const DS4_BTN_L2: u16 = 0x136;
const DS4_BTN_R2: u16 = 0x137;
const DS4_BTN_SHARE: u16 = 0x138;
const DS4_BTN_OPTIONS: u16 = 0x139;
const DS4_BTN_L3: u16 = 0x13a;
const DS4_BTN_R3: u16 = 0x13b;
const DS4_BTN_PS: u16 = 0x13c;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

const FF_RUMBLE: u16 = 0x50;
const FF_GAIN: u16 = 0x60;
const FF_EFFECTS_MAX: u32 = 16;

// VID/PID/version chosen to match SDL's known controller GUIDs so games apply
// the right mapping and glyphs.
const XBOX_VENDOR: u16 = 0x045e;
const XBOX_PRODUCT: u16 = 0x028e;
const XBOX_VERSION: u16 = 0x0110;
const DS4_VENDOR: u16 = 0x054c;
const DS4_PRODUCT: u16 = 0x05c4;
const DS4_VERSION: u16 = 0x0111;

const UI_FF_UPLOAD: u16 = 1;
const UI_FF_ERASE: u16 = 2;

const UINPUT_IOCTL_BASE: u32 = b'U' as u32;

const fn ioc(dir: u32, nr: u32, size: usize) -> libc::c_ulong {
    ((dir << 30) | ((size as u32) << 16) | (UINPUT_IOCTL_BASE << 8) | nr) as libc::c_ulong
}

const UI_SET_EVBIT: libc::c_ulong = ioc(1, 100, mem::size_of::<libc::c_int>());
const UI_SET_KEYBIT: libc::c_ulong = ioc(1, 101, mem::size_of::<libc::c_int>());
const UI_SET_ABSBIT: libc::c_ulong = ioc(1, 103, mem::size_of::<libc::c_int>());
const UI_SET_FFBIT: libc::c_ulong = ioc(1, 107, mem::size_of::<libc::c_int>());
const UI_DEV_CREATE: libc::c_ulong = ioc(0, 1, 0);
const UI_DEV_DESTROY: libc::c_ulong = ioc(0, 2, 0);
const UI_BEGIN_FF_UPLOAD: libc::c_ulong = ioc(3, 200, mem::size_of::<UinputFfUpload>());
const UI_END_FF_UPLOAD: libc::c_ulong = ioc(1, 201, mem::size_of::<UinputFfUpload>());
const UI_BEGIN_FF_ERASE: libc::c_ulong = ioc(3, 202, mem::size_of::<UinputFfErase>());
const UI_END_FF_ERASE: libc::c_ulong = ioc(1, 203, mem::size_of::<UinputFfErase>());

#[derive(Debug)]
pub enum Error {
    UinputNotAvailable(std::io::Error),
    Ioctl(&'static str, std::io::Error),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UinputNotAvailable(err) => write!(
                f,
                "failed to open {UINPUT_PATH}: {err} (try adding user to input group or loading the uinput module)"
            ),
            Error::Ioctl(name, err) => write!(f, "ioctl {name} failed: {err}"),
            Error::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[repr(C)]
#[derive(Clone, Copy)]
struct InputEvent {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
#[derive(Default)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputUserDev {
    name: [u8; 80],
    id: InputId,
    ff_effects_max: u32,
    absmax: [i32; 64],
    absmin: [i32; 64],
    absfuzz: [i32; 64],
    absflat: [i32; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfTrigger {
    button: u16,
    interval: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfReplay {
    length: u16,
    delay: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfEnvelope {
    attack_length: u16,
    attack_level: u16,
    fade_length: u16,
    fade_level: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfRumbleEffect {
    strong_magnitude: u16,
    weak_magnitude: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfConstantEffect {
    level: i16,
    envelope: FfEnvelope,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfRampEffect {
    start_level: i16,
    end_level: i16,
    envelope: FfEnvelope,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfConditionEffect {
    right_saturation: u16,
    left_saturation: u16,
    right_coeff: i16,
    left_coeff: i16,
    deadband: u16,
    center: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfPeriodicEffect {
    waveform: u16,
    period: u16,
    magnitude: i16,
    offset: i16,
    phase: u16,
    envelope: FfEnvelope,
    custom_len: u32,
    custom_data: *mut i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
union FfEffectUnion {
    constant: FfConstantEffect,
    ramp: FfRampEffect,
    periodic: FfPeriodicEffect,
    condition: [FfConditionEffect; 2],
    rumble: FfRumbleEffect,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FfEffect {
    type_: u16,
    id: i16,
    direction: u16,
    trigger: FfTrigger,
    replay: FfReplay,
    u: FfEffectUnion,
}

#[repr(C)]
struct UinputFfUpload {
    request_id: u32,
    retval: i32,
    effect: FfEffect,
    old: FfEffect,
}

#[repr(C)]
struct UinputFfErase {
    request_id: u32,
    retval: i32,
    effect_id: u32,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(mem::size_of::<InputEvent>() == 24);
    assert!(mem::size_of::<FfEffect>() == 48);
    assert!(mem::size_of::<UinputFfUpload>() == 104);
    assert!(mem::size_of::<UinputFfErase>() == 12);
};

unsafe fn ioctl_val(
    fd: i32,
    code: libc::c_ulong,
    name: &'static str,
    val: libc::c_ulong,
) -> Result<()> {
    if unsafe { libc::ioctl(fd, code, val) } < 0 {
        return Err(Error::Ioctl(name, std::io::Error::last_os_error()));
    }
    Ok(())
}

unsafe fn ioctl_none(fd: i32, code: libc::c_ulong, name: &'static str) -> Result<()> {
    if unsafe { libc::ioctl(fd, code) } < 0 {
        return Err(Error::Ioctl(name, std::io::Error::last_os_error()));
    }
    Ok(())
}

unsafe fn ioctl_ptr<T>(
    fd: i32,
    code: libc::c_ulong,
    name: &'static str,
    data: *mut T,
) -> Result<()> {
    if unsafe { libc::ioctl(fd, code, data) } < 0 {
        return Err(Error::Ioctl(name, std::io::Error::last_os_error()));
    }
    Ok(())
}

pub struct UinputGamepad {
    file: File,
    stop: Arc<AtomicBool>,
    kind: GamepadKind,
}

impl UinputGamepad {
    pub fn is_available() -> bool {
        std::path::Path::new(UINPUT_PATH).exists()
    }

    pub fn kind(&self) -> GamepadKind {
        self.kind
    }

    pub fn create(name: &str, kind: GamepadKind) -> Result<Self> {
        let (vendor, product, version) = match kind {
            GamepadKind::Xbox360 => (XBOX_VENDOR, XBOX_PRODUCT, XBOX_VERSION),
            GamepadKind::DualShock4 => (DS4_VENDOR, DS4_PRODUCT, DS4_VERSION),
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(UINPUT_PATH)
            .map_err(Error::UinputNotAvailable)?;

        let fd = file.as_raw_fd();

        unsafe {
            ioctl_val(fd, UI_SET_EVBIT, "UI_SET_EVBIT", EV_KEY as libc::c_ulong)?;

            let buttons: &[u16] = match kind {
                GamepadKind::Xbox360 => &[
                    BTN_SOUTH, BTN_EAST, BTN_NORTH, BTN_WEST, BTN_TL, BTN_TR, BTN_SELECT,
                    BTN_START, BTN_MODE, BTN_THUMBL, BTN_THUMBR,
                ],
                GamepadKind::DualShock4 => &[
                    DS4_BTN_SQUARE,
                    DS4_BTN_CROSS,
                    DS4_BTN_CIRCLE,
                    DS4_BTN_TRIANGLE,
                    DS4_BTN_L1,
                    DS4_BTN_R1,
                    DS4_BTN_L2,
                    DS4_BTN_R2,
                    DS4_BTN_SHARE,
                    DS4_BTN_OPTIONS,
                    DS4_BTN_L3,
                    DS4_BTN_R3,
                    DS4_BTN_PS,
                ],
            };
            for &btn in buttons {
                ioctl_val(fd, UI_SET_KEYBIT, "UI_SET_KEYBIT", btn as libc::c_ulong)?;
            }

            ioctl_val(fd, UI_SET_EVBIT, "UI_SET_EVBIT", EV_ABS as libc::c_ulong)?;

            let axes = [
                ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ, ABS_HAT0X, ABS_HAT0Y,
            ];
            for axis in axes {
                ioctl_val(fd, UI_SET_ABSBIT, "UI_SET_ABSBIT", axis as libc::c_ulong)?;
            }

            ioctl_val(fd, UI_SET_EVBIT, "UI_SET_EVBIT", EV_FF as libc::c_ulong)?;
            ioctl_val(fd, UI_SET_FFBIT, "UI_SET_FFBIT", FF_RUMBLE as libc::c_ulong)?;
        }

        let mut dev = UinputUserDev {
            name: [0u8; 80],
            id: InputId {
                bustype: 0x03,
                vendor,
                product,
                version,
            },
            ff_effects_max: FF_EFFECTS_MAX,
            absmax: [0; 64],
            absmin: [0; 64],
            absfuzz: [0; 64],
            absflat: [0; 64],
        };

        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(79);
        dev.name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        match kind {
            GamepadKind::Xbox360 => {
                for axis in [ABS_X, ABS_Y, ABS_RX, ABS_RY] {
                    let i = axis as usize;
                    dev.absmin[i] = -32768;
                    dev.absmax[i] = 32767;
                    dev.absfuzz[i] = 16;
                    dev.absflat[i] = 128;
                }
                for axis in [ABS_Z, ABS_RZ] {
                    let i = axis as usize;
                    dev.absmin[i] = 0;
                    dev.absmax[i] = 1023;
                }
            }
            GamepadKind::DualShock4 => {
                for axis in [ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ] {
                    let i = axis as usize;
                    dev.absmin[i] = 0;
                    dev.absmax[i] = 255;
                    dev.absfuzz[i] = 0;
                    dev.absflat[i] = 0;
                }
            }
        }

        dev.absmin[ABS_HAT0X as usize] = -1;
        dev.absmax[ABS_HAT0X as usize] = 1;
        dev.absmin[ABS_HAT0Y as usize] = -1;
        dev.absmax[ABS_HAT0Y as usize] = 1;

        let dev_bytes = unsafe {
            std::slice::from_raw_parts(
                &dev as *const UinputUserDev as *const u8,
                mem::size_of::<UinputUserDev>(),
            )
        };
        file.write_all(dev_bytes)?;

        unsafe {
            ioctl_none(fd, UI_DEV_CREATE, "UI_DEV_CREATE")?;
        }

        Ok(Self {
            file,
            stop: Arc::new(AtomicBool::new(false)),
            kind,
        })
    }

    fn send_event(&mut self, type_: u16, code: u16, value: i32) -> Result<()> {
        let event = InputEvent {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        };
        let event_bytes = unsafe {
            std::slice::from_raw_parts(
                &event as *const InputEvent as *const u8,
                mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(event_bytes)?;
        Ok(())
    }

    pub fn update(&mut self, report: &XGamepad) -> Result<()> {
        let pressed = |flag: u16| (report.buttons & flag != 0) as i32;

        let hat_x = if report.buttons & xbuttons::RIGHT != 0 {
            1
        } else if report.buttons & xbuttons::LEFT != 0 {
            -1
        } else {
            0
        };
        let hat_y = if report.buttons & xbuttons::DOWN != 0 {
            1
        } else if report.buttons & xbuttons::UP != 0 {
            -1
        } else {
            0
        };

        match self.kind {
            GamepadKind::Xbox360 => {
                self.send_event(EV_KEY, BTN_SOUTH, pressed(xbuttons::A))?;
                self.send_event(EV_KEY, BTN_EAST, pressed(xbuttons::B))?;
                self.send_event(EV_KEY, BTN_WEST, pressed(xbuttons::X))?;
                self.send_event(EV_KEY, BTN_NORTH, pressed(xbuttons::Y))?;
                self.send_event(EV_KEY, BTN_TL, pressed(xbuttons::LB))?;
                self.send_event(EV_KEY, BTN_TR, pressed(xbuttons::RB))?;
                self.send_event(EV_KEY, BTN_SELECT, pressed(xbuttons::BACK))?;
                self.send_event(EV_KEY, BTN_START, pressed(xbuttons::START))?;
                self.send_event(EV_KEY, BTN_MODE, pressed(xbuttons::GUIDE))?;
                self.send_event(EV_KEY, BTN_THUMBL, pressed(xbuttons::LTHUMB))?;
                self.send_event(EV_KEY, BTN_THUMBR, pressed(xbuttons::RTHUMB))?;

                self.send_event(EV_ABS, ABS_X, report.thumb_lx as i32)?;
                self.send_event(EV_ABS, ABS_Y, -(report.thumb_ly as i32))?;
                self.send_event(EV_ABS, ABS_RX, report.thumb_rx as i32)?;
                self.send_event(EV_ABS, ABS_RY, -(report.thumb_ry as i32))?;

                self.send_event(EV_ABS, ABS_Z, (report.left_trigger as i32) * 1023 / 255)?;
                self.send_event(EV_ABS, ABS_RZ, (report.right_trigger as i32) * 1023 / 255)?;
            }
            GamepadKind::DualShock4 => {
                self.send_event(EV_KEY, DS4_BTN_SQUARE, pressed(xbuttons::X))?;
                self.send_event(EV_KEY, DS4_BTN_CROSS, pressed(xbuttons::A))?;
                self.send_event(EV_KEY, DS4_BTN_CIRCLE, pressed(xbuttons::B))?;
                self.send_event(EV_KEY, DS4_BTN_TRIANGLE, pressed(xbuttons::Y))?;
                self.send_event(EV_KEY, DS4_BTN_L1, pressed(xbuttons::LB))?;
                self.send_event(EV_KEY, DS4_BTN_R1, pressed(xbuttons::RB))?;
                self.send_event(
                    EV_KEY,
                    DS4_BTN_L2,
                    if report.left_trigger > 0 { 1 } else { 0 },
                )?;
                self.send_event(
                    EV_KEY,
                    DS4_BTN_R2,
                    if report.right_trigger > 0 { 1 } else { 0 },
                )?;
                self.send_event(EV_KEY, DS4_BTN_SHARE, pressed(xbuttons::BACK))?;
                self.send_event(EV_KEY, DS4_BTN_OPTIONS, pressed(xbuttons::START))?;
                self.send_event(EV_KEY, DS4_BTN_L3, pressed(xbuttons::LTHUMB))?;
                self.send_event(EV_KEY, DS4_BTN_R3, pressed(xbuttons::RTHUMB))?;
                self.send_event(EV_KEY, DS4_BTN_PS, pressed(xbuttons::GUIDE))?;

                let to_u8 = |v: i16| -> i32 { (((v as i32) + 32768) >> 8).clamp(0, 255) };
                let inv_to_u8 = |v: i16| -> i32 { 255 - to_u8(v) };
                self.send_event(EV_ABS, ABS_X, to_u8(report.thumb_lx))?;
                self.send_event(EV_ABS, ABS_Y, inv_to_u8(report.thumb_ly))?;
                self.send_event(EV_ABS, ABS_Z, to_u8(report.thumb_rx))?;
                self.send_event(EV_ABS, ABS_RZ, inv_to_u8(report.thumb_ry))?;

                self.send_event(EV_ABS, ABS_RX, report.left_trigger as i32)?;
                self.send_event(EV_ABS, ABS_RY, report.right_trigger as i32)?;
            }
        }

        self.send_event(EV_ABS, ABS_HAT0X, hat_x)?;
        self.send_event(EV_ABS, ABS_HAT0Y, hat_y)?;

        self.send_event(EV_SYN, 0, 0)
    }

    pub fn spawn_notification<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(GamepadNotification) + Send + 'static,
    {
        let mut file = self.file.try_clone()?;
        let stop = Arc::clone(&self.stop);
        std::thread::Builder::new()
            .name("gamepad-rumble".into())
            .spawn(move || {
                let fd = file.as_raw_fd();
                let mut effects: HashMap<i16, FfRumbleEffect> = HashMap::new();
                let mut buf = [0u8; mem::size_of::<InputEvent>()];
                while !stop.load(Ordering::Relaxed) {
                    let mut pfd = libc::pollfd {
                        fd,
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let ready = unsafe { libc::poll(&mut pfd, 1, 500) };
                    if ready < 0 {
                        break;
                    }
                    if ready == 0 {
                        continue;
                    }
                    if file.read_exact(&mut buf).is_err() {
                        break;
                    }
                    let event: InputEvent = unsafe { mem::transmute_copy(&buf) };
                    match (event.type_, event.code) {
                        (EV_UINPUT, UI_FF_UPLOAD) => {
                            let mut upload: UinputFfUpload = unsafe { mem::zeroed() };
                            upload.request_id = event.value as u32;
                            unsafe {
                                if ioctl_ptr(
                                    fd,
                                    UI_BEGIN_FF_UPLOAD,
                                    "UI_BEGIN_FF_UPLOAD",
                                    &mut upload,
                                )
                                .is_ok()
                                {
                                    if upload.effect.type_ == FF_RUMBLE {
                                        effects.insert(upload.effect.id, upload.effect.u.rumble);
                                    }
                                    upload.retval = 0;
                                    let _ = ioctl_ptr(
                                        fd,
                                        UI_END_FF_UPLOAD,
                                        "UI_END_FF_UPLOAD",
                                        &mut upload,
                                    );
                                }
                            }
                        }
                        (EV_UINPUT, UI_FF_ERASE) => {
                            let mut erase: UinputFfErase = unsafe { mem::zeroed() };
                            erase.request_id = event.value as u32;
                            unsafe {
                                if ioctl_ptr(fd, UI_BEGIN_FF_ERASE, "UI_BEGIN_FF_ERASE", &mut erase)
                                    .is_ok()
                                {
                                    effects.remove(&(erase.effect_id as i16));
                                    erase.retval = 0;
                                    let _ = ioctl_ptr(
                                        fd,
                                        UI_END_FF_ERASE,
                                        "UI_END_FF_ERASE",
                                        &mut erase,
                                    );
                                }
                            }
                        }
                        (EV_FF, FF_GAIN) => {}
                        (EV_FF, effect_id) => {
                            if event.value > 0 {
                                if let Some(rumble) = effects.get(&(effect_id as i16)) {
                                    callback(GamepadNotification {
                                        large_motor: (rumble.strong_magnitude >> 8) as u8,
                                        small_motor: (rumble.weak_magnitude >> 8) as u8,
                                        led_number: 0,
                                    });
                                }
                            } else {
                                callback(GamepadNotification::default());
                            }
                        }
                        _ => {}
                    }
                }
            })
            .map_err(Error::Io)?;
        Ok(())
    }
}

impl Drop for UinputGamepad {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        unsafe {
            let _ = ioctl_none(self.file.as_raw_fd(), UI_DEV_DESTROY, "UI_DEV_DESTROY");
        }
    }
}
