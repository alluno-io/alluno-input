//! The AllunoVHID bus client: the IOCTL contract `driver/windows/vhid/vhid_ioctl.h`
//! declares, a device that plugs one virtual HID node with a descriptor and a
//! feature table, and the pad, pen and touch devices built on it.
//!
//! Output reports the HID stack writes back arrive on a thread the client
//! owns, which pends one `WAIT_OUTPUT` at a time and hands each report to the
//! profile's codec; the bus completes that wait with a removal status when the
//! slot is unplugged, which is how the thread ends.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub use alluno_input_core::hid::wire::{
    API_VERSION, MAX_DESCRIPTOR, MAX_DEVICES, MAX_FEATURES, MAX_REPORT, OUTPUT_KIND_FEATURE,
    OUTPUT_KIND_REPORT, OutputHeader, PlugHeader, ReportHeader, parse_output, plug_request,
    report_fits, report_request,
};
use alluno_input_core::hid::{self, FeatureReport, Identity, PadCodec};
use alluno_input_core::{
    Error, Gamepad, GamepadOutput, GamepadProfile, GamepadState, Key, Keyboard, Mouse, MouseButton,
    OutputSink, Pen, PenState, Result, Touch, TouchState,
};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW,
    CM_Get_Device_Interface_ListW, CR_SUCCESS,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::IO::{DeviceIoControl, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::CreateEventW;
use windows::core::{GUID, PCWSTR};

/// The bus device interface.
pub const INTERFACE: GUID = GUID::from_values(
    0x7F3A_6C2E,
    0x4B1D,
    0x4E8A,
    [0x9C, 0x0B, 0x5A, 0x2D, 0x3F, 0x6E, 0x1B, 0x90],
);

const DEVICE_TYPE: u32 = 0x8A11;

const fn ctl(function: u32) -> u32 {
    (DEVICE_TYPE << 16) | (function << 2)
}

pub const IOCTL_VERSION: u32 = ctl(0x800);
pub const IOCTL_PLUG: u32 = ctl(0x801);
pub const IOCTL_UNPLUG: u32 = ctl(0x802);
pub const IOCTL_INPUT: u32 = ctl(0x803);
pub const IOCTL_WAIT_OUTPUT: u32 = ctl(0x804);
pub const IOCTL_SET_FEATURE: u32 = ctl(0x805);

struct Handle(HANDLE);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
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

fn interface_paths() -> Vec<Vec<u16>> {
    unsafe {
        let mut len = 0u32;
        if CM_Get_Device_Interface_List_SizeW(
            &mut len,
            &INTERFACE,
            PCWSTR::null(),
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        ) != CR_SUCCESS
            || len == 0
        {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        if CM_Get_Device_Interface_ListW(
            &INTERFACE,
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

fn open_path(path: &[u16]) -> windows::core::Result<Handle> {
    unsafe {
        let handle = CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
            None,
        )?;
        Ok(Handle(handle))
    }
}

/// One buffered IOCTL over an overlapped handle, waited to completion.
fn ioctl(
    handle: &Handle,
    event: &Event,
    code: u32,
    input: &[u8],
    output: &mut [u8],
) -> windows::core::Result<usize> {
    unsafe {
        let mut overlapped = OVERLAPPED {
            hEvent: event.0,
            ..Default::default()
        };
        let mut transferred = 0u32;
        let in_ptr = (!input.is_empty()).then_some(input.as_ptr().cast::<c_void>());
        let out_ptr = (!output.is_empty()).then_some(output.as_mut_ptr().cast::<c_void>());
        if let Err(err) = DeviceIoControl(
            handle.0,
            code,
            in_ptr,
            input.len() as u32,
            out_ptr,
            output.len() as u32,
            Some(&mut transferred),
            Some(&mut overlapped),
        ) && err.code() != ERROR_IO_PENDING.to_hresult()
        {
            return Err(err);
        }
        GetOverlappedResult(handle.0, &overlapped, &mut transferred, true)?;
        Ok(transferred as usize)
    }
}

/// Whether the bus driver is present and speaks this client's version.
pub fn installed() -> bool {
    Bus::connect().is_ok()
}

/// The installed bus driver's file version, or `None` when it is absent.
pub fn driver_version() -> Option<String> {
    crate::kernel::driver_file_version("AllunoVHID.sys")
}

/// An open bus device. One IOCTL runs at a time on it; the output thread of
/// each node opens its own handle so a pending wait never blocks a submit.
pub struct Bus {
    handle: Handle,
    event: Mutex<Event>,
    path: Vec<u16>,
}

impl Bus {
    /// Opens the first bus interface whose version matches.
    pub fn connect() -> Result<Self> {
        let mut last = Error::unavailable("the AllunoVHID bus is not installed");
        for path in interface_paths() {
            let handle = match open_path(&path) {
                Ok(handle) => handle,
                Err(err) => {
                    last = Error::backend(err);
                    continue;
                }
            };
            let event = Event::new().map_err(Error::backend)?;
            let mut version = [0u8; 4];
            match ioctl(&handle, &event, IOCTL_VERSION, &[], &mut version) {
                Ok(4) if u32::from_le_bytes(version) == API_VERSION => {
                    return Ok(Self {
                        handle,
                        event: Mutex::new(event),
                        path,
                    });
                }
                Ok(_) => {
                    last = Error::unavailable("the AllunoVHID bus speaks another version");
                }
                Err(err) => last = Error::backend(err),
            }
        }
        Err(last)
    }

    fn call(&self, code: u32, input: &[u8], output: &mut [u8]) -> Result<usize> {
        let event = self
            .event
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ioctl(&self.handle, &event, code, input, output).map_err(Error::backend)
    }
}

/// One plugged virtual HID node.
pub struct Device {
    bus: Arc<Bus>,
    slot: u32,
    stop: Arc<AtomicBool>,
    _thread: PhantomData<*const ()>,
}

impl Device {
    /// Plugs a node with an identity, a descriptor and the feature reports it answers.
    pub fn plug(
        bus: Arc<Bus>,
        identity: Identity,
        descriptor: &[u8],
        features: &[FeatureReport],
    ) -> Result<Self> {
        if descriptor.is_empty() || descriptor.len() > MAX_DESCRIPTOR {
            return Err(Error::Unsupported);
        }
        let request = plug_request(identity, descriptor, features);
        let mut out = [0u8; 4];
        let written = bus.call(IOCTL_PLUG, &request, &mut out)?;
        if written != 4 {
            return Err(Error::backend("the bus answered a plug without a slot"));
        }
        Ok(Self {
            bus,
            slot: u32::from_le_bytes(out),
            stop: Arc::new(AtomicBool::new(false)),
            _thread: PhantomData,
        })
    }

    /// The bus slot the node occupies.
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// Submits one input report, id byte first.
    pub fn input(&mut self, report: &[u8]) -> Result<()> {
        if !report_fits(report) {
            return Err(Error::Unsupported);
        }
        let request = report_request(self.slot, report);
        self.bus.call(IOCTL_INPUT, &request, &mut [])?;
        Ok(())
    }

    /// Replaces or adds one feature report the node answers.
    pub fn set_feature(&mut self, report: &[u8]) -> Result<()> {
        if !report_fits(report) {
            return Err(Error::Unsupported);
        }
        let request = report_request(self.slot, report);
        self.bus.call(IOCTL_SET_FEATURE, &request, &mut [])?;
        Ok(())
    }

    /// Starts the thread that pends `WAIT_OUTPUT` and delivers each report to
    /// `handle`, which answers an optional input report to send back.
    pub fn spawn_output<F>(&self, mut handle: F) -> Result<()>
    where
        F: FnMut(u8, &[u8]) -> Option<Vec<u8>> + Send + 'static,
    {
        let handle_path = open_path(&self.bus.path).map_err(Error::backend)?;
        let slot = self.slot;
        let stop = self.stop.clone();
        std::thread::Builder::new()
            .name("vhid-output".into())
            .spawn(move || {
                let device = handle_path;
                let Ok(event) = Event::new() else {
                    return;
                };
                let slot_bytes = slot.to_le_bytes();
                let mut buffer = vec![0u8; mem::size_of::<OutputHeader>() + MAX_REPORT];
                while !stop.load(Ordering::Relaxed) {
                    let Ok(written) =
                        ioctl(&device, &event, IOCTL_WAIT_OUTPUT, &slot_bytes, &mut buffer)
                    else {
                        break;
                    };
                    let Some((kind, report)) = parse_output(&buffer[..written]) else {
                        continue;
                    };
                    if let Some(reply) = handle(kind, report) {
                        let request = report_request(slot, &reply);
                        let _ = ioctl(&device, &event, IOCTL_INPUT, &request, &mut []);
                    }
                }
            })
            .map_err(Error::Io)?;
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self
            .bus
            .call(IOCTL_UNPLUG, &self.slot.to_le_bytes(), &mut []);
    }
}

/// A pad on the bus, speaking one profile's HID protocol.
pub struct VhidPad {
    device: Device,
    codec: Arc<Mutex<PadCodec>>,
}

impl VhidPad {
    /// Plugs the profile's device; a profile without a HID codec is unsupported.
    pub fn plug(bus: Arc<Bus>, profile: GamepadProfile) -> Result<Self> {
        let codec = PadCodec::new(profile).ok_or(Error::Unsupported)?;
        let device = Device::plug(bus, codec.identity(), codec.descriptor(), codec.features())?;
        Ok(Self {
            device,
            codec: Arc::new(Mutex::new(codec)),
        })
    }
}

impl Gamepad for VhidPad {
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
        let codec = self.codec.clone();
        self.device.spawn_output(move |kind, report| {
            if kind != OUTPUT_KIND_REPORT {
                sink(GamepadOutput::Raw(report.to_vec()));
                return None;
            }
            let decoded = codec.lock().unwrap().decode(report);
            for output in decoded.outputs {
                sink(output);
            }
            decoded.reply
        })
    }

    fn slot(&mut self) -> Option<u8> {
        None
    }
}

/// A pen digitiser node on the bus.
pub struct VhidPen {
    device: Device,
}

impl VhidPen {
    /// Plugs the pen node.
    pub fn plug(bus: Arc<Bus>) -> Result<Self> {
        let device = Device::plug(bus, hid::pen::IDENTITY, hid::pen::DESCRIPTOR, &[])?;
        Ok(Self { device })
    }
}

impl Pen for VhidPen {
    fn report(&mut self, state: &PenState) -> Result<()> {
        self.device.input(&hid::pen::encode(state))
    }
}

/// A touch screen node on the bus.
pub struct VhidTouch {
    device: Device,
}

impl VhidTouch {
    /// Plugs the touch node.
    pub fn plug(bus: Arc<Bus>) -> Result<Self> {
        let device = Device::plug(
            bus,
            hid::touch::IDENTITY,
            hid::touch::DESCRIPTOR,
            hid::touch::FEATURES,
        )?;
        Ok(Self { device })
    }
}

impl Touch for VhidTouch {
    fn report(&mut self, state: &TouchState) -> Result<()> {
        if !state.fits() {
            return Err(Error::Unsupported);
        }
        self.device.input(&hid::touch::encode(state))
    }
}

/// A keyboard node on the bus: a second keyboard identity the OS cannot tell
/// from hardware, for hosts without the filter or with more than one seat.
pub struct VhidKeyboard {
    device: Device,
    codec: hid::keyboard::Codec,
}

impl VhidKeyboard {
    /// Plugs the keyboard node.
    pub fn plug(bus: Arc<Bus>) -> Result<Self> {
        let device = Device::plug(bus, hid::keyboard::IDENTITY, hid::keyboard::DESCRIPTOR, &[])?;
        Ok(Self {
            device,
            codec: hid::keyboard::Codec::default(),
        })
    }
}

impl Keyboard for VhidKeyboard {
    fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        let report = self.codec.key(key, pressed).ok_or(Error::Unsupported)?;
        self.device.input(&report)
    }
}

impl Drop for VhidKeyboard {
    fn drop(&mut self) {
        if self.codec.any_down() {
            let _ = self.device.input(&hid::keyboard::Codec::default().report());
        }
    }
}

/// A relative mouse node on the bus. Absolute placement answers `Unsupported`
/// so a layered set hands it to the filter or `SendInput`, which place the
/// cursor on the whole virtual desktop; buttons, motion and wheels go here.
pub struct VhidMouse {
    device: Device,
    codec: hid::mouse::Codec,
}

impl VhidMouse {
    /// Plugs the mouse node.
    pub fn plug(bus: Arc<Bus>) -> Result<Self> {
        let device = Device::plug(bus, hid::mouse::IDENTITY, hid::mouse::DESCRIPTOR, &[])?;
        Ok(Self {
            device,
            codec: hid::mouse::Codec::default(),
        })
    }
}

impl Mouse for VhidMouse {
    fn move_abs(&mut self, _x: u16, _y: u16) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<()> {
        self.device.input(&self.codec.move_rel(dx, dy))
    }

    fn button(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let report = self
            .codec
            .button(button, pressed)
            .ok_or(Error::Unsupported)?;
        self.device.input(&report)
    }

    fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
        self.device.input(&self.codec.wheel(dx, dy))
    }
}

/// An Xbox 360 device on the bus: Windows binds its own xusb22 driver to the
/// node, which is what makes the pad an XInput controller. The lamp and rumble
/// packets the game writes arrive on a thread the node owns from the moment it
/// is plugged, so the player slot is known before a consumer installs a sink.
pub struct BusXusbPad {
    device: Device,
    player: Arc<Mutex<Option<u8>>>,
    sink: Arc<Mutex<Option<OutputSink>>>,
}

impl BusXusbPad {
    /// Plugs the device and waits up to a second for xusb22 to assign a lamp,
    /// which is the moment XInput can see the pad.
    pub fn plug(bus: Arc<Bus>) -> Result<Self> {
        let request = hid::wire::xusb_plug_request(hid::xusb::IDENTITY);
        let mut out = [0u8; 4];
        let written = bus.call(IOCTL_PLUG, &request, &mut out)?;
        if written != 4 {
            return Err(Error::backend("the bus answered a plug without a slot"));
        }
        let device = Device {
            bus,
            slot: u32::from_le_bytes(out),
            stop: Arc::new(AtomicBool::new(false)),
            _thread: PhantomData,
        };
        let player: Arc<Mutex<Option<u8>>> = Arc::new(Mutex::new(None));
        let sink: Arc<Mutex<Option<OutputSink>>> = Arc::new(Mutex::new(None));
        let ready = Arc::new((Mutex::new(false), Condvar::new()));

        let player_thread = player.clone();
        let sink_thread = sink.clone();
        let ready_thread = ready.clone();
        device.spawn_output(move |kind, report| {
            if kind != OUTPUT_KIND_REPORT {
                return None;
            }
            let decoded = hid::xusb::decode(report);
            for output in decoded.outputs {
                if let GamepadOutput::PlayerLed(lamp) = output {
                    *player_thread.lock().unwrap() = lamp.checked_sub(1);
                    let (flag, signal) = &*ready_thread;
                    *flag.lock().unwrap() = true;
                    signal.notify_all();
                }
                if let Some(sink) = sink_thread.lock().unwrap().as_mut() {
                    sink(output);
                }
            }
            None
        })?;

        let (flag, signal) = &*ready;
        let guard = flag.lock().unwrap();
        let _ = signal.wait_timeout_while(guard, Duration::from_secs(1), |lit| !*lit);

        Ok(Self {
            device,
            player,
            sink,
        })
    }

    /// The bus slot the node occupies.
    pub fn bus_slot(&self) -> u32 {
        self.device.slot()
    }
}

impl Gamepad for BusXusbPad {
    fn submit(&mut self, state: &GamepadState) -> Result<()> {
        self.device.input(&hid::xusb::packet(state))
    }

    fn on_output(&mut self, sink: OutputSink) -> Result<()> {
        *self.sink.lock().unwrap() = Some(sink);
        Ok(())
    }

    fn slot(&mut self) -> Option<u8> {
        *self.player.lock().unwrap()
    }
}
