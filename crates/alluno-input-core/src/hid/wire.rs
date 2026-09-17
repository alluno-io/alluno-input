//! The byte layouts a bus client and the AllunoVHID drivers exchange: the
//! plug request, the report submission and the output completion. Windows
//! carries them in IOCTL buffers and macOS in user-client struct arguments;
//! the bytes are the same, and `driver/windows/vhid/vhid_ioctl.h` mirrors
//! every value here.

use std::mem;

use super::{FeatureReport, Identity};

/// The contract version this vocabulary describes.
pub const API_VERSION: u32 = 1;

/// Slots one bus device offers.
pub const MAX_DEVICES: usize = 16;

/// The largest report the bus carries either way, id byte included.
pub const MAX_REPORT: usize = 512;

/// The largest descriptor a plug may carry.
pub const MAX_DESCRIPTOR: usize = 4096;

/// The most feature reports one plug may install.
pub const MAX_FEATURES: usize = 32;

/// A plug that publishes a HID node from the descriptor that follows.
pub const KIND_HID: u16 = 0;

/// A plug that publishes an Xbox 360 XUSB device; it carries no descriptor.
pub const KIND_XUSB: u16 = 1;

/// The interrupt packet an XUSB node takes.
pub const XUSB_INPUT_LENGTH: usize = 20;

/// The largest packet an XUSB node sends back.
pub const XUSB_OUTPUT_LENGTH: usize = 8;

/// An output completion is an output report.
pub const OUTPUT_KIND_REPORT: u8 = 0;

/// An output completion is a feature report the host set.
pub const OUTPUT_KIND_FEATURE: u8 = 1;

/// The plug header, packed, followed by the descriptor and the features.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct PlugHeader {
    pub vendor_id: u16,
    pub product_id: u16,
    pub version_number: u16,
    pub descriptor_length: u16,
    pub feature_count: u16,
    pub kind: u16,
}

/// The report header, packed, followed by the report bytes.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct ReportHeader {
    pub slot: u32,
    pub length: u16,
    pub reserved: u16,
}

/// The output header, packed, followed by the report bytes.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct OutputHeader {
    pub kind: u8,
    pub reserved: u8,
    pub length: u16,
}

/// The bytes of a plug request for a HID node: an identity, a descriptor and
/// a feature table.
pub fn plug_request(identity: Identity, descriptor: &[u8], features: &[FeatureReport]) -> Vec<u8> {
    plug_request_of(KIND_HID, identity, descriptor, features)
}

/// The bytes of a plug request for an XUSB device: an identity and nothing else.
pub fn xusb_plug_request(identity: Identity) -> Vec<u8> {
    plug_request_of(KIND_XUSB, identity, &[], &[])
}

fn plug_request_of(
    kind: u16,
    identity: Identity,
    descriptor: &[u8],
    features: &[FeatureReport],
) -> Vec<u8> {
    let header = PlugHeader {
        vendor_id: identity.vendor,
        product_id: identity.product,
        version_number: identity.version,
        descriptor_length: descriptor.len() as u16,
        feature_count: features.len() as u16,
        kind,
    };
    let mut bytes = Vec::with_capacity(mem::size_of::<PlugHeader>() + descriptor.len() + 64);
    bytes.extend_from_slice(bytes_of(&header));
    bytes.extend_from_slice(descriptor);
    for feature in features {
        bytes.extend_from_slice(&(feature.data.len() as u16).to_le_bytes());
        bytes.extend_from_slice(feature.data);
    }
    bytes
}

/// The bytes of an input or feature submission for a slot.
pub fn report_request(slot: u32, report: &[u8]) -> Vec<u8> {
    let header = ReportHeader {
        slot,
        length: report.len() as u16,
        reserved: 0,
    };
    let mut bytes = Vec::with_capacity(mem::size_of::<ReportHeader>() + report.len());
    bytes.extend_from_slice(bytes_of(&header));
    bytes.extend_from_slice(report);
    bytes
}

/// Splits an output completion into its kind and report bytes.
pub fn parse_output(buffer: &[u8]) -> Option<(u8, &[u8])> {
    if buffer.len() < mem::size_of::<OutputHeader>() {
        return None;
    }
    let kind = buffer[0];
    let length = usize::from(u16::from_le_bytes([buffer[2], buffer[3]]));
    let data = buffer.get(4..4 + length)?;
    Some((kind, data))
}

/// Whether a report is a size the bus carries.
pub fn report_fits(report: &[u8]) -> bool {
    !report.is_empty() && report.len() <= MAX_REPORT
}

fn bytes_of<T: Copy>(value: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), mem::size_of::<T>()) }
}
