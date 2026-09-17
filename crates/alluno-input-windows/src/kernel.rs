//! The AllunoInput control devices.
//!
//! The driver is a KMDF upper filter on `kbdclass` and `mouclass`. Each half
//! publishes one control device; an `IOCTL_ALLUNO_SEND` on it hands one
//! `KEYBOARD_INPUT_DATA` or `MOUSE_INPUT_DATA` to exactly one filter instance,
//! never a broadcast, which is what keeps a second physical keyboard from
//! doubling every stroke. The wire structs and the IOCTL codes here are the
//! driver's contract and must stay byte-exact with `Driver.h`.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::size_of;

use windows::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileA, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::core::PCSTR;

/// The keyboard control device the installer publishes. The service is named
/// `KeyboardAllunoInput`; a KMDF keyboard filter's service name must start with
/// `Keyboard`.
pub const KEYBOARD_DEVICE: &str = "\\\\.\\KeyboardAllunoInput\0";

/// The mouse control device; the service is `MouseAllunoInput` for the same reason.
pub const MOUSE_DEVICE: &str = "\\\\.\\MouseAllunoInput\0";

/// `CTL_CODE(FILE_DEVICE_KEYBOARD, 0x820, METHOD_BUFFERED, FILE_ANY_ACCESS)`.
pub const IOCTL_KEYBOARD_SEND: u32 = (0x000B << 16) | (0x820 << 2);

/// `CTL_CODE(FILE_DEVICE_MOUSE, 0x820, METHOD_BUFFERED, FILE_ANY_ACCESS)`.
pub const IOCTL_MOUSE_SEND: u32 = (0x000F << 16) | (0x820 << 2);

/// One keyboard stroke, laid out as the kernel's `KEYBOARD_INPUT_DATA`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardInputData {
    pub unit_id: u16,
    /// A PS/2 Set 1 scan code.
    pub make_code: u16,
    /// [`key_flags`] bits.
    pub flags: u16,
    pub reserved: u16,
    pub extra_information: u32,
}

/// One mouse packet, laid out as the kernel's `MOUSE_INPUT_DATA`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseInputData {
    pub unit_id: u16,
    /// [`mouse_move_flags`] bits.
    pub flags: u16,
    /// [`mouse_button_flags`] bits.
    pub button_flags: u16,
    /// The wheel delta when a wheel flag is set; 120 is one notch.
    pub button_data: i16,
    pub raw_buttons: u32,
    /// A delta, or 0..=65535 across the virtual desktop when absolute.
    pub last_x: i32,
    pub last_y: i32,
    pub extra_information: u32,
}

/// Stroke flags.
pub mod key_flags {
    pub const KEY_MAKE: u16 = 0x0000;
    pub const BREAK: u16 = 0x0001;
    pub const E0: u16 = 0x0002;
    pub const E1: u16 = 0x0004;
}

/// Button flags.
pub mod mouse_button_flags {
    pub const LEFT_BUTTON_DOWN: u16 = 0x0001;
    pub const LEFT_BUTTON_UP: u16 = 0x0002;
    pub const RIGHT_BUTTON_DOWN: u16 = 0x0004;
    pub const RIGHT_BUTTON_UP: u16 = 0x0008;
    pub const MIDDLE_BUTTON_DOWN: u16 = 0x0010;
    pub const MIDDLE_BUTTON_UP: u16 = 0x0020;
    pub const BUTTON_4_DOWN: u16 = 0x0040;
    pub const BUTTON_4_UP: u16 = 0x0080;
    pub const BUTTON_5_DOWN: u16 = 0x0100;
    pub const BUTTON_5_UP: u16 = 0x0200;
    pub const WHEEL: u16 = 0x0400;
    pub const HWHEEL: u16 = 0x0800;
}

/// Movement flags.
pub mod mouse_move_flags {
    pub const MOVE_RELATIVE: u16 = 0x0000;
    pub const MOVE_ABSOLUTE: u16 = 0x0001;
    pub const VIRTUAL_DESKTOP: u16 = 0x0002;
}

/// One wheel notch, as Windows counts it.
pub const WHEEL_DELTA: i16 = 120;

/// Whether either filter is installed, without keeping a handle.
pub fn installed() -> bool {
    KernelKeyboard::available() || KernelMouse::available()
}

/// The installed driver's file version, read from the `.sys` on disk the way
/// Device Manager reads it, since the driver has no version IOCTL.
pub fn driver_version() -> Option<String> {
    ["KeyboardAllunoInput.sys", "MouseAllunoInput.sys"]
        .iter()
        .find_map(|name| driver_file_version(name))
}

/// The file version of one installed driver binary under `System32\drivers`.
pub fn driver_file_version(name: &str) -> Option<String> {
    let system_root = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    let drivers = system_root.join("System32").join("drivers");
    file_version(&drivers.join(name))
}

fn file_version(path: &std::path::Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    };
    use windows::core::PCWSTR;

    if !path.exists() {
        return None;
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None) };
    if size == 0 {
        return None;
    }
    let mut buffer = vec![0u8; size as usize];
    unsafe {
        GetFileVersionInfoW(
            PCWSTR(wide.as_ptr()),
            None,
            size,
            buffer.as_mut_ptr().cast(),
        )
        .ok()?;
    }
    let mut info: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    let root: Vec<u16> = "\\\0".encode_utf16().collect();
    let found = unsafe {
        VerQueryValueW(
            buffer.as_ptr().cast(),
            PCWSTR(root.as_ptr()),
            &mut info,
            &mut len,
        )
    };
    if !found.as_bool() || info.is_null() || (len as usize) < size_of::<VS_FIXEDFILEINFO>() {
        return None;
    }
    let fixed = unsafe { &*(info as *const VS_FIXEDFILEINFO) };
    Some(format!(
        "{}.{}.{}.{}",
        fixed.dwFileVersionMS >> 16,
        fixed.dwFileVersionMS & 0xFFFF,
        fixed.dwFileVersionLS >> 16,
        fixed.dwFileVersionLS & 0xFFFF
    ))
}

fn open_device(device_path: &str) -> Option<HANDLE> {
    let handle = unsafe {
        CreateFileA(
            PCSTR(device_path.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };

    match handle {
        Ok(h) if h != INVALID_HANDLE_VALUE => Some(h),
        _ => None,
    }
}

fn send_ioctl(handle: HANDLE, ioctl: u32, data: &[u8]) -> windows::core::Result<u32> {
    let mut bytes_returned = 0u32;
    unsafe {
        DeviceIoControl(
            handle,
            ioctl,
            Some(data.as_ptr() as *const c_void),
            data.len() as u32,
            None,
            0,
            Some(&mut bytes_returned),
            None,
        )?;
    }
    Ok(bytes_returned)
}

fn bytes_of<T>(value: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

/// An open handle on the keyboard control device.
pub struct KernelKeyboard {
    handle: HANDLE,
    _thread: PhantomData<*const ()>,
}

impl KernelKeyboard {
    /// Opens the control device; `None` when the filter is not installed.
    pub fn open() -> Option<Self> {
        open_device(KEYBOARD_DEVICE).map(|handle| Self {
            handle,
            _thread: PhantomData,
        })
    }

    /// Whether the filter is installed, without keeping a handle.
    pub fn available() -> bool {
        Self::open().is_some()
    }

    /// Hands one packet to the driver; answers the bytes it consumed.
    pub fn send_raw(&self, data: &[u8]) -> windows::core::Result<u32> {
        send_ioctl(self.handle, IOCTL_KEYBOARD_SEND, data)
    }

    /// One make or break of a scan code with the given [`key_flags`].
    pub fn stroke(&self, scan_code: u16, flags: u16) -> windows::core::Result<u32> {
        let data = KeyboardInputData {
            make_code: scan_code,
            flags,
            ..KeyboardInputData::default()
        };
        self.send_raw(bytes_of(&data))
    }

    /// Key down.
    pub fn press(&self, scan_code: u16, extended: bool) -> windows::core::Result<u32> {
        self.stroke(scan_code, flag(key_flags::KEY_MAKE, extended))
    }

    /// Key up.
    pub fn release(&self, scan_code: u16, extended: bool) -> windows::core::Result<u32> {
        self.stroke(scan_code, flag(key_flags::BREAK, extended))
    }
}

fn flag(base: u16, extended: bool) -> u16 {
    if extended { base | key_flags::E0 } else { base }
}

impl Drop for KernelKeyboard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// An open handle on the mouse control device.
pub struct KernelMouse {
    handle: HANDLE,
    _thread: PhantomData<*const ()>,
}

impl KernelMouse {
    /// Opens the control device; `None` when the filter is not installed.
    pub fn open() -> Option<Self> {
        open_device(MOUSE_DEVICE).map(|handle| Self {
            handle,
            _thread: PhantomData,
        })
    }

    /// Whether the filter is installed, without keeping a handle.
    pub fn available() -> bool {
        Self::open().is_some()
    }

    /// Hands one packet to the driver; answers the bytes it consumed.
    pub fn send_raw(&self, data: &[u8]) -> windows::core::Result<u32> {
        send_ioctl(self.handle, IOCTL_MOUSE_SEND, data)
    }

    fn packet(&self, data: MouseInputData) -> windows::core::Result<u32> {
        self.send_raw(bytes_of(&data))
    }

    /// Moves to a point in 0..=65535 virtual-desktop space.
    pub fn move_abs(&self, x: u16, y: u16) -> windows::core::Result<u32> {
        self.packet(MouseInputData {
            flags: mouse_move_flags::MOVE_ABSOLUTE | mouse_move_flags::VIRTUAL_DESKTOP,
            last_x: i32::from(x),
            last_y: i32::from(y),
            ..MouseInputData::default()
        })
    }

    /// Moves by a delta.
    pub fn move_rel(&self, dx: i32, dy: i32) -> windows::core::Result<u32> {
        self.packet(MouseInputData {
            flags: mouse_move_flags::MOVE_RELATIVE,
            last_x: dx,
            last_y: dy,
            ..MouseInputData::default()
        })
    }

    /// One or more [`mouse_button_flags`] transitions.
    pub fn buttons(&self, button_flags: u16) -> windows::core::Result<u32> {
        self.packet(MouseInputData {
            button_flags,
            ..MouseInputData::default()
        })
    }

    /// Vertical wheel; positive is away from the user, 120 per notch.
    pub fn wheel(&self, delta: i16) -> windows::core::Result<u32> {
        self.packet(MouseInputData {
            button_flags: mouse_button_flags::WHEEL,
            button_data: delta,
            ..MouseInputData::default()
        })
    }

    /// Horizontal wheel; positive is right, 120 per notch.
    pub fn hwheel(&self, delta: i16) -> windows::core::Result<u32> {
        self.packet(MouseInputData {
            button_flags: mouse_button_flags::HWHEEL,
            button_data: delta,
            ..MouseInputData::default()
        })
    }
}

impl Drop for KernelMouse {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
