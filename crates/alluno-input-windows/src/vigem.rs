use crate::report::{GamepadNotification, XGamepad};
use std::ffi::c_void;
use std::fmt;
use std::mem;
use std::sync::Arc;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW,
    CM_Get_Device_Interface_ListW, CR_SUCCESS,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_INVALID_PARAMETER, ERROR_IO_PENDING, ERROR_NO_MORE_ITEMS, GENERIC_READ,
    GENERIC_WRITE, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_NO_BUFFERING, FILE_FLAG_OVERLAPPED,
    FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::{DeviceIoControl, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::CreateEventW;
use windows::core::{GUID, PCWSTR};

const GUID_DEVINTERFACE_BUS: GUID = GUID::from_values(
    0x96E4_2B22,
    0xF5E9,
    0x42F8,
    [0xB0, 0x43, 0xED, 0x0F, 0x93, 0x2F, 0x01, 0x4F],
);

const IOCTL_PLUGIN_TARGET: u32 = 0x2AA004;
const IOCTL_UNPLUG_TARGET: u32 = 0x2AA008;
const IOCTL_CHECK_VERSION: u32 = 0x2AA00C;
const IOCTL_WAIT_DEVICE_READY: u32 = 0x2AA010;
const IOCTL_XUSB_REQUEST_NOTIFICATION: u32 = 0x2AE804;
const IOCTL_XUSB_SUBMIT_REPORT: u32 = 0x2AA808;
const IOCTL_DS4_SUBMIT_REPORT: u32 = 0x2AA80C;
const IOCTL_DS4_REQUEST_NOTIFICATION: u32 = 0x2AA810;
const IOCTL_XUSB_GET_USER_INDEX: u32 = 0x2AE81C;

const API_VERSION_COMMON: u32 = 0x0001;
const TARGET_TYPE_XBOX360_WIRED: i32 = 0;
const TARGET_TYPE_DUALSHOCK4_WIRED: i32 = 2;
const X360_VENDOR_ID: u16 = 0x045E;
const X360_PRODUCT_ID: u16 = 0x028E;
const DS4_VENDOR_ID: u16 = 0x054C;
const DS4_PRODUCT_ID: u16 = 0x05C4;

#[derive(Debug)]
pub enum Error {
    BusNotFound,
    VersionMismatch,
    NoFreeSlot,
    NotPluggedIn,
    Win32(windows::core::Error),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BusNotFound => write!(f, "gamepad bus device not found (driver not installed)"),
            Error::VersionMismatch => write!(f, "gamepad bus driver API version mismatch"),
            Error::NoFreeSlot => write!(f, "no free virtual gamepad slot"),
            Error::NotPluggedIn => write!(f, "virtual gamepad is not plugged in"),
            Error::Win32(err) => write!(f, "{err}"),
            Error::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Self {
        Error::Win32(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[repr(C)]
struct CheckVersion {
    size: u32,
    version: u32,
}

#[repr(C)]
struct PluginTarget {
    size: u32,
    serial_no: u32,
    target_type: i32,
    vendor_id: u16,
    product_id: u16,
}

#[repr(C)]
struct UnplugTarget {
    size: u32,
    serial_no: u32,
}

#[repr(C)]
struct WaitDeviceReady {
    size: u32,
    serial_no: u32,
}

#[repr(C)]
struct XusbSubmitReport {
    size: u32,
    serial_no: u32,
    report: XGamepad,
}

#[repr(C)]
struct XusbRequestNotificationBuf {
    size: u32,
    serial_no: u32,
    large_motor: u8,
    small_motor: u8,
    led_number: u8,
}

#[repr(C)]
struct XusbGetUserIndex {
    size: u32,
    serial_no: u32,
    user_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Ds4Report {
    thumb_lx: u8,
    thumb_ly: u8,
    thumb_rx: u8,
    thumb_ry: u8,
    buttons: u16,
    special: u8,
    trigger_l: u8,
    trigger_r: u8,
}

#[repr(C)]
struct Ds4SubmitReport {
    size: u32,
    serial_no: u32,
    report: Ds4Report,
}

#[repr(C)]
struct Ds4RequestNotificationBuf {
    size: u32,
    serial_no: u32,
    small_motor: u8,
    large_motor: u8,
    lightbar_r: u8,
    lightbar_g: u8,
    lightbar_b: u8,
}

fn xgamepad_to_ds4(g: &XGamepad) -> Ds4Report {
    use crate::report::xbuttons;
    let on = |flag: u16| g.buttons & flag != 0;

    // D-pad hat: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=neutral.
    let hat: u16 = match (
        on(xbuttons::UP),
        on(xbuttons::RIGHT),
        on(xbuttons::DOWN),
        on(xbuttons::LEFT),
    ) {
        (true, false, false, false) => 0,
        (true, true, false, false) => 1,
        (false, true, false, false) => 2,
        (false, true, true, false) => 3,
        (false, false, true, false) => 4,
        (false, false, true, true) => 5,
        (false, false, false, true) => 6,
        (true, false, false, true) => 7,
        _ => 8,
    };

    let mut buttons = hat;
    if on(xbuttons::X) {
        buttons |= 1 << 4; // Square
    }
    if on(xbuttons::A) {
        buttons |= 1 << 5; // Cross
    }
    if on(xbuttons::B) {
        buttons |= 1 << 6; // Circle
    }
    if on(xbuttons::Y) {
        buttons |= 1 << 7; // Triangle
    }
    if on(xbuttons::LB) {
        buttons |= 1 << 8; // L1
    }
    if on(xbuttons::RB) {
        buttons |= 1 << 9; // R1
    }
    if g.left_trigger > 0 {
        buttons |= 1 << 10; // L2
    }
    if g.right_trigger > 0 {
        buttons |= 1 << 11; // R2
    }
    if on(xbuttons::BACK) {
        buttons |= 1 << 12; // Share
    }
    if on(xbuttons::START) {
        buttons |= 1 << 13; // Options
    }
    if on(xbuttons::LTHUMB) {
        buttons |= 1 << 14; // L3
    }
    if on(xbuttons::RTHUMB) {
        buttons |= 1 << 15; // R3
    }

    let special = if on(xbuttons::GUIDE) { 1 } else { 0 }; // PS button

    // DS4 Y axes are inverted (0 = up).
    let axis = |v: i16| (((v as i32) + 32768) >> 8).clamp(0, 255) as u8;
    let axis_inv = |v: i16| ((32768 - (v as i32)) >> 8).clamp(0, 255) as u8;

    Ds4Report {
        thumb_lx: axis(g.thumb_lx),
        thumb_ly: axis_inv(g.thumb_ly),
        thumb_rx: axis(g.thumb_rx),
        thumb_ry: axis_inv(g.thumb_ry),
        buttons,
        special,
        trigger_l: g.left_trigger,
        trigger_r: g.right_trigger,
    }
}

struct Device(HANDLE);

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct Event(HANDLE);

unsafe impl Send for Event {}

impl Event {
    fn new() -> windows::core::Result<Self> {
        Ok(Self(unsafe {
            CreateEventW(None, false, false, PCWSTR::null())
        }?))
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

unsafe fn ioctl<T>(
    device: HANDLE,
    event: HANDLE,
    code: u32,
    data: &mut T,
    read_output: bool,
) -> windows::core::Result<()> {
    unsafe {
        let mut overlapped = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        let mut transferred = 0u32;
        let ptr = data as *mut T as *mut c_void;
        let size = mem::size_of::<T>() as u32;
        if let Err(err) = DeviceIoControl(
            device,
            code,
            Some(ptr as *const c_void),
            size,
            read_output.then_some(ptr),
            if read_output { size } else { 0 },
            Some(&mut transferred),
            Some(&mut overlapped),
        ) && err.code() != ERROR_IO_PENDING.to_hresult()
        {
            return Err(err);
        }
        GetOverlappedResult(device, &overlapped, &mut transferred, true)
    }
}

fn device_interface_paths() -> Vec<Vec<u16>> {
    unsafe {
        let mut len = 0u32;
        if CM_Get_Device_Interface_List_SizeW(
            &mut len,
            &GUID_DEVINTERFACE_BUS,
            PCWSTR::null(),
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        ) != CR_SUCCESS
            || len == 0
        {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        if CM_Get_Device_Interface_ListW(
            &GUID_DEVINTERFACE_BUS,
            PCWSTR::null(),
            &mut buf,
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        ) != CR_SUCCESS
        {
            return Vec::new();
        }
        buf.split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(|s| {
                let mut path = s.to_vec();
                path.push(0);
                path
            })
            .collect()
    }
}

fn open_device(path: &[u16]) -> windows::core::Result<Device> {
    unsafe {
        let handle = CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL
                | FILE_FLAG_NO_BUFFERING
                | FILE_FLAG_WRITE_THROUGH
                | FILE_FLAG_OVERLAPPED,
            None,
        )?;
        Ok(Device(handle))
    }
}

pub struct GamepadBus {
    device: Device,
    path: Vec<u16>,
}

impl GamepadBus {
    pub fn connect() -> Result<Self> {
        let paths = device_interface_paths();
        let mut last_err = Error::BusNotFound;
        for path in paths {
            let device = match open_device(&path) {
                Ok(device) => device,
                Err(err) => {
                    last_err = Error::Win32(err);
                    continue;
                }
            };
            let event = Event::new()?;
            let mut check = CheckVersion {
                size: mem::size_of::<CheckVersion>() as u32,
                version: API_VERSION_COMMON,
            };
            match unsafe { ioctl(device.0, event.0, IOCTL_CHECK_VERSION, &mut check, false) } {
                Ok(()) => return Ok(Self { device, path }),
                Err(_) => last_err = Error::VersionMismatch,
            }
        }
        Err(last_err)
    }
}

pub struct XusbTarget {
    bus: Arc<GamepadBus>,
    event: Event,
    serial_no: u32,
}

impl XusbTarget {
    pub fn plugin(bus: Arc<GamepadBus>) -> Result<Self> {
        let event = Event::new()?;
        let mut plugin = PluginTarget {
            size: mem::size_of::<PluginTarget>() as u32,
            serial_no: 1,
            target_type: TARGET_TYPE_XBOX360_WIRED,
            vendor_id: X360_VENDOR_ID,
            product_id: X360_PRODUCT_ID,
        };
        loop {
            match unsafe {
                ioctl(
                    bus.device.0,
                    event.0,
                    IOCTL_PLUGIN_TARGET,
                    &mut plugin,
                    false,
                )
            } {
                Ok(()) => break,
                Err(_) => {
                    plugin.serial_no += 1;
                    if plugin.serial_no >= u16::MAX as u32 {
                        return Err(Error::NoFreeSlot);
                    }
                }
            }
        }
        Ok(Self {
            bus,
            event,
            serial_no: plugin.serial_no,
        })
    }

    pub fn is_attached(&self) -> bool {
        self.serial_no != 0
    }

    pub fn wait_ready(&self) -> Result<()> {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let mut wait = WaitDeviceReady {
            size: mem::size_of::<WaitDeviceReady>() as u32,
            serial_no: self.serial_no,
        };
        if let Err(err) = unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_WAIT_DEVICE_READY,
                &mut wait,
                false,
            )
        } && err.code() != ERROR_INVALID_PARAMETER.to_hresult()
        {
            return Err(Error::Win32(err));
        }
        Ok(())
    }

    pub fn update(&mut self, report: &XGamepad) -> Result<()> {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let mut submit = XusbSubmitReport {
            size: mem::size_of::<XusbSubmitReport>() as u32,
            serial_no: self.serial_no,
            report: *report,
        };
        unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_XUSB_SUBMIT_REPORT,
                &mut submit,
                false,
            )
        }?;
        Ok(())
    }

    pub fn get_user_index(&mut self) -> Result<u32> {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let mut gui = XusbGetUserIndex {
            size: mem::size_of::<XusbGetUserIndex>() as u32,
            serial_no: self.serial_no,
            user_index: 0,
        };
        unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_XUSB_GET_USER_INDEX,
                &mut gui,
                true,
            )
        }?;
        Ok(gui.user_index)
    }

    pub fn spawn_notification<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(GamepadNotification) + Send + 'static,
    {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let device = open_device(&self.bus.path)?;
        let serial_no = self.serial_no;
        std::thread::Builder::new()
            .name("gamepad-rumble".into())
            .spawn(move || {
                let device = device;
                let Ok(event) = Event::new() else {
                    return;
                };
                loop {
                    let mut req = XusbRequestNotificationBuf {
                        size: mem::size_of::<XusbRequestNotificationBuf>() as u32,
                        serial_no,
                        large_motor: 0,
                        small_motor: 0,
                        led_number: 0,
                    };
                    match unsafe {
                        ioctl(
                            device.0,
                            event.0,
                            IOCTL_XUSB_REQUEST_NOTIFICATION,
                            &mut req,
                            true,
                        )
                    } {
                        Ok(()) => callback(GamepadNotification {
                            large_motor: req.large_motor,
                            small_motor: req.small_motor,
                            led_number: req.led_number,
                        }),
                        Err(_) => break,
                    }
                }
            })
            .map_err(Error::Io)?;
        Ok(())
    }

    pub fn unplug(&mut self) -> Result<()> {
        if !self.is_attached() {
            return Ok(());
        }
        let mut unplug = UnplugTarget {
            size: mem::size_of::<UnplugTarget>() as u32,
            serial_no: self.serial_no,
        };
        unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_UNPLUG_TARGET,
                &mut unplug,
                false,
            )
        }?;
        self.serial_no = 0;
        Ok(())
    }
}

impl Drop for XusbTarget {
    fn drop(&mut self) {
        let _ = self.unplug();
    }
}

pub struct Ds4Target {
    bus: Arc<GamepadBus>,
    event: Event,
    serial_no: u32,
}

impl Ds4Target {
    pub fn plugin(bus: Arc<GamepadBus>) -> Result<Self> {
        let event = Event::new()?;
        let mut plugin = PluginTarget {
            size: mem::size_of::<PluginTarget>() as u32,
            serial_no: 1,
            target_type: TARGET_TYPE_DUALSHOCK4_WIRED,
            vendor_id: DS4_VENDOR_ID,
            product_id: DS4_PRODUCT_ID,
        };
        loop {
            match unsafe {
                ioctl(
                    bus.device.0,
                    event.0,
                    IOCTL_PLUGIN_TARGET,
                    &mut plugin,
                    false,
                )
            } {
                Ok(()) => break,
                Err(_) => {
                    plugin.serial_no += 1;
                    if plugin.serial_no >= u16::MAX as u32 {
                        return Err(Error::NoFreeSlot);
                    }
                }
            }
        }
        Ok(Self {
            bus,
            event,
            serial_no: plugin.serial_no,
        })
    }

    pub fn is_attached(&self) -> bool {
        self.serial_no != 0
    }

    pub fn wait_ready(&self) -> Result<()> {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let mut wait = WaitDeviceReady {
            size: mem::size_of::<WaitDeviceReady>() as u32,
            serial_no: self.serial_no,
        };
        if let Err(err) = unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_WAIT_DEVICE_READY,
                &mut wait,
                false,
            )
        } && err.code() != ERROR_INVALID_PARAMETER.to_hresult()
        {
            return Err(Error::Win32(err));
        }
        Ok(())
    }

    pub fn update(&mut self, report: &XGamepad) -> Result<()> {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let mut submit = Ds4SubmitReport {
            size: mem::size_of::<Ds4SubmitReport>() as u32,
            serial_no: self.serial_no,
            report: xgamepad_to_ds4(report),
        };
        match unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_DS4_SUBMIT_REPORT,
                &mut submit,
                false,
            )
        } {
            Ok(()) => Ok(()),
            Err(err) if err.code() == ERROR_NO_MORE_ITEMS.to_hresult() => Ok(()),
            Err(err) => Err(Error::Win32(err)),
        }
    }

    pub fn spawn_notification<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(GamepadNotification) + Send + 'static,
    {
        if !self.is_attached() {
            return Err(Error::NotPluggedIn);
        }
        let device = open_device(&self.bus.path)?;
        let serial_no = self.serial_no;
        std::thread::Builder::new()
            .name("gamepad-rumble".into())
            .spawn(move || {
                let device = device;
                let Ok(event) = Event::new() else {
                    return;
                };
                loop {
                    let mut req = Ds4RequestNotificationBuf {
                        size: mem::size_of::<Ds4RequestNotificationBuf>() as u32,
                        serial_no,
                        small_motor: 0,
                        large_motor: 0,
                        lightbar_r: 0,
                        lightbar_g: 0,
                        lightbar_b: 0,
                    };
                    match unsafe {
                        ioctl(
                            device.0,
                            event.0,
                            IOCTL_DS4_REQUEST_NOTIFICATION,
                            &mut req,
                            true,
                        )
                    } {
                        Ok(()) => callback(GamepadNotification {
                            large_motor: req.large_motor,
                            small_motor: req.small_motor,
                            led_number: 0,
                        }),
                        Err(_) => break,
                    }
                }
            })
            .map_err(Error::Io)?;
        Ok(())
    }

    pub fn unplug(&mut self) -> Result<()> {
        if !self.is_attached() {
            return Ok(());
        }
        let mut unplug = UnplugTarget {
            size: mem::size_of::<UnplugTarget>() as u32,
            serial_no: self.serial_no,
        };
        unsafe {
            ioctl(
                self.bus.device.0,
                self.event.0,
                IOCTL_UNPLUG_TARGET,
                &mut unplug,
                false,
            )
        }?;
        self.serial_no = 0;
        Ok(())
    }
}

impl Drop for Ds4Target {
    fn drop(&mut self) {
        let _ = self.unplug();
    }
}
