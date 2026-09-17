//! The AllunoVHID DriverKit client: the user-client contract
//! `driver/macos/AllunoVHID` declares, spoken over IOKit. The extension is the
//! one backend gated on something other than engineering time (Apple's HID
//! DriverKit entitlement), so everything here answers `Unavailable` until the
//! extension is activated on the machine.
//!
//! The byte layouts are the ones every AllunoVHID speaks
//! (`alluno_input_core::hid::wire`); the selectors below index the extension's
//! external methods.

use std::ffi::{CString, c_char, c_void};
use std::marker::PhantomData;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use alluno_input_core::hid::wire::{
    API_VERSION, MAX_DESCRIPTOR, MAX_REPORT, OUTPUT_KIND_REPORT, OutputHeader, parse_output,
    plug_request, report_fits, report_request,
};
use alluno_input_core::hid::{self, FeatureReport, Identity, PadCodec};
use alluno_input_core::{
    Error, Gamepad, GamepadOutput, GamepadProfile, GamepadState, Key, Keyboard, Mouse, MouseButton,
    OutputSink, Pen, PenState, Result, Touch, TouchState,
};

/// The IOKit class the extension registers.
pub const SERVICE_CLASS: &str = "AllunoVHIDBus";

pub const SELECTOR_VERSION: u32 = 0;
pub const SELECTOR_PLUG: u32 = 1;
pub const SELECTOR_UNPLUG: u32 = 2;
pub const SELECTOR_INPUT: u32 = 3;
pub const SELECTOR_POLL_OUTPUT: u32 = 4;
pub const SELECTOR_SET_FEATURE: u32 = 5;

/// How often the output thread asks the extension for reports.
pub const POLL_INTERVAL: Duration = Duration::from_millis(4);

const KERN_SUCCESS: i32 = 0;
const MAIN_PORT_DEFAULT: u32 = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, owning_task: u32, kind: u32, connect: *mut u32) -> i32;
    fn IOServiceClose(connect: u32) -> i32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOConnectCallScalarMethod(
        connect: u32,
        selector: u32,
        input: *const u64,
        input_count: u32,
        output: *mut u64,
        output_count: *mut u32,
    ) -> i32;
    fn IOConnectCallStructMethod(
        connect: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
    fn IOConnectCallMethod(
        connect: u32,
        selector: u32,
        scalar_input: *const u64,
        scalar_input_count: u32,
        struct_input: *const c_void,
        struct_input_size: usize,
        scalar_output: *mut u64,
        scalar_output_count: *mut u32,
        struct_output: *mut c_void,
        struct_output_size: *mut usize,
    ) -> i32;
}

unsafe extern "C" {
    static mach_task_self_: u32;
}

/// Whether the extension is loaded and reachable.
pub fn installed() -> bool {
    Bus::connect().is_ok()
}

fn matching_service() -> Option<u32> {
    let name = CString::new(SERVICE_CLASS).ok()?;
    let service =
        unsafe { IOServiceGetMatchingService(MAIN_PORT_DEFAULT, IOServiceMatching(name.as_ptr())) };
    (service != 0).then_some(service)
}

/// An open connection to the extension.
pub struct Bus {
    connect: u32,
    lock: Mutex<()>,
}

unsafe impl Send for Bus {}
unsafe impl Sync for Bus {}

impl Bus {
    /// Opens the extension's user client and checks its contract version.
    pub fn connect() -> Result<Self> {
        let service = matching_service()
            .ok_or_else(|| Error::unavailable("the AllunoVHID extension is not activated"))?;
        let mut connect = 0u32;
        let opened = unsafe { IOServiceOpen(service, mach_task_self_, 0, &mut connect) };
        unsafe { IOObjectRelease(service) };
        if opened != KERN_SUCCESS {
            return Err(Error::backend(format!(
                "IOServiceOpen refused the AllunoVHID client: {opened:#x}"
            )));
        }
        let bus = Self {
            connect,
            lock: Mutex::new(()),
        };
        let mut version = [0u64; 1];
        bus.scalar(SELECTOR_VERSION, &[], &mut version)?;
        if version[0] != u64::from(API_VERSION) {
            return Err(Error::unavailable(
                "the AllunoVHID extension speaks another version",
            ));
        }
        Ok(bus)
    }

    fn scalar(&self, selector: u32, input: &[u64], output: &mut [u64]) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut count = output.len() as u32;
        let status = unsafe {
            IOConnectCallScalarMethod(
                self.connect,
                selector,
                input.as_ptr(),
                input.len() as u32,
                output.as_mut_ptr(),
                &mut count,
            )
        };
        check(status)
    }

    fn structured(&self, selector: u32, input: &[u8], output: &mut [u8]) -> Result<usize> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut size = output.len();
        let status = unsafe {
            IOConnectCallStructMethod(
                self.connect,
                selector,
                input.as_ptr().cast(),
                input.len(),
                output.as_mut_ptr().cast(),
                &mut size,
            )
        };
        check(status)?;
        Ok(size)
    }

    fn plug(&self, request: &[u8]) -> Result<u32> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut slot = [0u64; 1];
        let mut slot_count = 1u32;
        let mut none = 0usize;
        let status = unsafe {
            IOConnectCallMethod(
                self.connect,
                SELECTOR_PLUG,
                std::ptr::null(),
                0,
                request.as_ptr().cast(),
                request.len(),
                slot.as_mut_ptr(),
                &mut slot_count,
                std::ptr::null_mut(),
                &mut none,
            )
        };
        check(status)?;
        Ok(slot[0] as u32)
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        unsafe {
            let _ = IOServiceClose(self.connect);
        }
    }
}

fn check(status: i32) -> Result<()> {
    if status == KERN_SUCCESS {
        Ok(())
    } else {
        Err(Error::backend(format!("AllunoVHID answered {status:#x}")))
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
        let slot = bus.plug(&plug_request(identity, descriptor, features))?;
        Ok(Self {
            bus,
            slot,
            stop: Arc::new(AtomicBool::new(false)),
            _thread: PhantomData,
        })
    }

    /// Submits one input report, id byte first.
    pub fn input(&mut self, report: &[u8]) -> Result<()> {
        if !report_fits(report) {
            return Err(Error::Unsupported);
        }
        self.bus
            .structured(SELECTOR_INPUT, &report_request(self.slot, report), &mut [])?;
        Ok(())
    }

    /// Replaces or adds one feature report the node answers.
    pub fn set_feature(&mut self, report: &[u8]) -> Result<()> {
        if !report_fits(report) {
            return Err(Error::Unsupported);
        }
        self.bus.structured(
            SELECTOR_SET_FEATURE,
            &report_request(self.slot, report),
            &mut [],
        )?;
        Ok(())
    }

    /// Starts the thread that polls the extension for output reports and
    /// delivers each to `handle`, which answers an optional input report.
    pub fn spawn_output<F>(&self, mut handle: F) -> Result<()>
    where
        F: FnMut(u8, &[u8]) -> Option<Vec<u8>> + Send + 'static,
    {
        let bus = self.bus.clone();
        let slot = self.slot;
        let stop = self.stop.clone();
        std::thread::Builder::new()
            .name("vhid-output".into())
            .spawn(move || {
                let slot_bytes = slot.to_le_bytes();
                let mut buffer = vec![0u8; mem::size_of::<OutputHeader>() + MAX_REPORT];
                while !stop.load(Ordering::Relaxed) {
                    let Ok(written) =
                        bus.structured(SELECTOR_POLL_OUTPUT, &slot_bytes, &mut buffer)
                    else {
                        break;
                    };
                    match parse_output(&buffer[..written]) {
                        Some((kind, report)) if !report.is_empty() => {
                            if let Some(reply) = handle(kind, report) {
                                let _ = bus.structured(
                                    SELECTOR_INPUT,
                                    &report_request(slot, &reply),
                                    &mut [],
                                );
                            }
                        }
                        _ => std::thread::sleep(POLL_INTERVAL),
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
            .scalar(SELECTOR_UNPLUG, &[u64::from(self.slot)], &mut []);
    }
}

/// A pad on the extension, speaking one profile's HID protocol.
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

/// A touch screen node on the extension.
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

/// A pen digitiser node on the extension.
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

/// A keyboard node on the extension.
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

/// A relative mouse node on the extension; absolute placement answers
/// `Unsupported` and belongs to the Core Graphics mouse.
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
