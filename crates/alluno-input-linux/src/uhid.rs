//! A virtual HID device over `/dev/uhid`: the kernel parses the report
//! descriptor and binds its own HID driver to the node, so a pad shows up as
//! the exact controller `hid-playstation`, `hid-nintendo` and SDL expect.
//!
//! The `uhid_event` layouts are kernel ABI and packed; every size below is
//! pinned by a test.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::mem;
use std::os::unix::io::AsRawFd;

use alluno_input_core::hid::Identity;
use alluno_input_core::{Error, Result};

/// The uhid character device.
pub const UHID_PATH: &str = "/dev/uhid";

pub const UHID_DESTROY: u32 = 1;
pub const UHID_START: u32 = 2;
pub const UHID_STOP: u32 = 3;
pub const UHID_OPEN: u32 = 4;
pub const UHID_CLOSE: u32 = 5;
pub const UHID_OUTPUT: u32 = 6;
pub const UHID_GET_REPORT: u32 = 9;
pub const UHID_GET_REPORT_REPLY: u32 = 10;
pub const UHID_CREATE2: u32 = 11;
pub const UHID_INPUT2: u32 = 12;
pub const UHID_SET_REPORT: u32 = 13;
pub const UHID_SET_REPORT_REPLY: u32 = 14;

pub const UHID_FEATURE_REPORT: u8 = 0;
pub const UHID_OUTPUT_REPORT: u8 = 1;
pub const UHID_INPUT_REPORT: u8 = 2;

pub const BUS_USB: u16 = 0x03;

/// The largest descriptor or report the kernel ABI carries.
pub const UHID_DATA_MAX: usize = 4096;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Create2Req {
    pub name: [u8; 128],
    pub phys: [u8; 64],
    pub uniq: [u8; 64],
    pub rd_size: u16,
    pub bus: u16,
    pub vendor: u32,
    pub product: u32,
    pub version: u32,
    pub country: u32,
    pub rd_data: [u8; UHID_DATA_MAX],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Input2Req {
    pub size: u16,
    pub data: [u8; UHID_DATA_MAX],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OutputReq {
    pub data: [u8; UHID_DATA_MAX],
    pub size: u16,
    pub rtype: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GetReportReq {
    pub id: u32,
    pub rnum: u8,
    pub rtype: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GetReportReplyReq {
    pub id: u32,
    pub err: u16,
    pub size: u16,
    pub data: [u8; UHID_DATA_MAX],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SetReportReq {
    pub id: u32,
    pub rnum: u8,
    pub rtype: u8,
    pub size: u16,
    pub data: [u8; UHID_DATA_MAX],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SetReportReplyReq {
    pub id: u32,
    pub err: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub union EventBody {
    pub create2: Create2Req,
    pub input2: Input2Req,
    pub output: OutputReq,
    pub get_report: GetReportReq,
    pub get_report_reply: GetReportReplyReq,
    pub set_report: SetReportReq,
    pub set_report_reply: SetReportReplyReq,
}

/// One `uhid_event`, written to or read from the node whole.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Event {
    pub type_: u32,
    pub u: EventBody,
}

/// The exact size the kernel reads and writes.
pub const EVENT_SIZE: usize = mem::size_of::<Event>();

const _: () = {
    assert!(mem::size_of::<Create2Req>() == 4372);
    assert!(mem::size_of::<Input2Req>() == 4098);
    assert!(mem::size_of::<OutputReq>() == 4099);
    assert!(mem::size_of::<GetReportReq>() == 6);
    assert!(mem::size_of::<GetReportReplyReq>() == 4104);
    assert!(mem::size_of::<SetReportReq>() == 4104);
    assert!(mem::size_of::<SetReportReplyReq>() == 6);
    assert!(EVENT_SIZE == 4376);
};

impl Event {
    fn zeroed(type_: u32) -> Self {
        let mut event: Self = unsafe { mem::zeroed() };
        event.type_ = type_;
        event
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts((self as *const Self).cast::<u8>(), EVENT_SIZE) }
    }
}

/// What the kernel asked of the device, read off the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// An output report, id byte first.
    Output(Vec<u8>),
    /// A feature or output report the host set; the device answers with `set_report_reply`.
    SetReport { id: u32, report: Vec<u8> },
    /// A feature request for a report number; the device answers with `get_report_reply`.
    GetReport { id: u32, number: u8, kind: u8 },
    /// Lifecycle traffic a device only acknowledges by carrying on.
    Other(u32),
}

/// Whether `/dev/uhid` exists and this process may open it.
pub fn available() -> bool {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(UHID_PATH)
        .is_ok()
}

/// An open uhid node; `Drop` destroys it.
pub struct Device {
    file: File,
}

impl Device {
    /// Creates the node with a name, an identity and a report descriptor.
    pub fn create(name: &str, identity: Identity, descriptor: &[u8]) -> Result<Self> {
        if descriptor.is_empty() || descriptor.len() > UHID_DATA_MAX {
            return Err(Error::Unsupported);
        }
        let mut file = OpenOptions::new().read(true).write(true).open(UHID_PATH)?;
        let mut event = Event::zeroed(UHID_CREATE2);
        let create = unsafe { &mut event.u.create2 };
        let bytes = name.as_bytes();
        let len = bytes.len().min(127);
        create.name[..len].copy_from_slice(&bytes[..len]);
        let phys = b"alluno-input";
        create.phys[..phys.len()].copy_from_slice(phys);
        create.rd_size = descriptor.len() as u16;
        create.bus = BUS_USB;
        create.vendor = u32::from(identity.vendor);
        create.product = u32::from(identity.product);
        create.version = u32::from(identity.version);
        create.rd_data[..descriptor.len()].copy_from_slice(descriptor);
        file.write_all(event.as_bytes())?;
        Ok(Self { file })
    }

    /// A second handle on the same node, for the thread that reads requests.
    pub fn reader(&self) -> Result<Reader> {
        Ok(Reader {
            file: self.file.try_clone()?,
        })
    }

    /// Submits one input report, id byte first.
    pub fn input(&mut self, report: &[u8]) -> Result<()> {
        write_input(&mut self.file, report)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let event = Event::zeroed(UHID_DESTROY);
        let _ = self.file.write_all(event.as_bytes());
    }
}

fn write_input(file: &mut File, report: &[u8]) -> Result<()> {
    if report.is_empty() || report.len() > UHID_DATA_MAX {
        return Err(Error::Unsupported);
    }
    let mut event = Event::zeroed(UHID_INPUT2);
    let input = unsafe { &mut event.u.input2 };
    input.size = report.len() as u16;
    input.data[..report.len()].copy_from_slice(report);
    file.write_all(event.as_bytes())?;
    Ok(())
}

/// The request side of a node: reads what the kernel asks and answers it.
pub struct Reader {
    file: File,
}

impl Reader {
    /// Waits up to `timeout_ms` for a request; `None` when nothing arrived.
    pub fn next(&mut self, timeout_ms: i32) -> Result<Option<Request>> {
        let mut pfd = libc::pollfd {
            fd: self.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if ready < 0 {
            return Err(Error::Io(std::io::Error::last_os_error()));
        }
        if ready == 0 {
            return Ok(None);
        }
        let mut event = Event::zeroed(0);
        let buffer = unsafe {
            std::slice::from_raw_parts_mut((&mut event as *mut Event).cast::<u8>(), EVENT_SIZE)
        };
        let read = self.file.read(buffer)?;
        if read < mem::size_of::<u32>() {
            return Ok(None);
        }
        Ok(Some(parse(&event)))
    }

    /// Answers a `GetReport` with a report, id byte first, or an error code.
    pub fn reply_get(&mut self, id: u32, report: Option<&[u8]>) -> Result<()> {
        let mut event = Event::zeroed(UHID_GET_REPORT_REPLY);
        let reply = unsafe { &mut event.u.get_report_reply };
        reply.id = id;
        match report {
            Some(data) if data.len() <= UHID_DATA_MAX => {
                reply.err = 0;
                reply.size = data.len() as u16;
                reply.data[..data.len()].copy_from_slice(data);
            }
            _ => {
                reply.err = libc::EIO as u16;
                reply.size = 0;
            }
        }
        self.file.write_all(event.as_bytes())?;
        Ok(())
    }

    /// Acknowledges a `SetReport`.
    pub fn reply_set(&mut self, id: u32, ok: bool) -> Result<()> {
        let mut event = Event::zeroed(UHID_SET_REPORT_REPLY);
        let reply = unsafe { &mut event.u.set_report_reply };
        reply.id = id;
        reply.err = if ok { 0 } else { libc::EIO as u16 };
        self.file.write_all(event.as_bytes())?;
        Ok(())
    }

    /// Submits an input report from the request thread, for protocols that
    /// acknowledge commands on the input pipe.
    pub fn input(&mut self, report: &[u8]) -> Result<()> {
        write_input(&mut self.file, report)
    }
}

fn parse(event: &Event) -> Request {
    match event.type_ {
        UHID_OUTPUT => {
            let output = unsafe { event.u.output };
            let size = usize::from(output.size).min(UHID_DATA_MAX);
            Request::Output(output.data[..size].to_vec())
        }
        UHID_SET_REPORT => {
            let set = unsafe { event.u.set_report };
            let size = usize::from(set.size).min(UHID_DATA_MAX);
            Request::SetReport {
                id: set.id,
                report: set.data[..size].to_vec(),
            }
        }
        UHID_GET_REPORT => {
            let get = unsafe { event.u.get_report };
            Request::GetReport {
                id: get.id,
                number: get.rnum,
                kind: get.rtype,
            }
        }
        other => Request::Other(other),
    }
}
