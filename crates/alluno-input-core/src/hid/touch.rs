//! The HID touch screen: up to [`MAX_CONTACTS`] fingers in one parallel-mode
//! report over a 0..=65535 surface, with the contact-count-maximum feature
//! report Windows and the kernel's `hid-multitouch` read at bind time.

use crate::{MAX_CONTACTS, TouchState};

/// The device's vendor identity.
pub const IDENTITY: super::Identity = super::Identity {
    vendor: 0x1234,
    product: 0x5681,
    version: 0x0100,
};

/// The bus-visible name.
pub const NAME: &str = "Alluno Touch";

/// The input report id.
pub const REPORT_ID: u8 = 0x08;

/// The feature report id carrying the contact count maximum.
pub const FEATURE_ID: u8 = 0x09;

/// Bytes per contact in the input report.
pub const CONTACT_LEN: usize = 12;

/// The input report size, id byte included: every slot is always present.
pub const INPUT_LEN: usize = 1 + CONTACT_LEN * MAX_CONTACTS + 1;

/// Contact pressure is reported on 0..=4095.
pub const PRESSURE_MAX: u16 = 4095;

/// The contact count maximum feature report.
pub const FEATURES: &[super::FeatureReport] = &[super::FeatureReport {
    id: FEATURE_ID,
    data: &[FEATURE_ID, MAX_CONTACTS as u8],
}];

#[rustfmt::skip]
const CONTACT: &[u8] = &[
    0x05, 0x0D,        //   Usage Page (Digitizer)
    0x09, 0x22,        //   Usage (Finger)
    0xA1, 0x02,        //   Collection (Logical)
    0x09, 0x42,        //     Usage (Tip Switch)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x01,        //     Logical Maximum (1)
    0x75, 0x01,        //     Report Size (1)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x95, 0x07,        //     Report Count (7)
    0x81, 0x03,        //     Input (Const,Var,Abs)
    0x09, 0x51,        //     Usage (Contact Identifier)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0xFF, 0x00,  //     Logical Maximum (255)
    0x75, 0x08,        //     Report Size (8)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x05, 0x01,        //     Usage Page (Generic Desktop)
    0x09, 0x30,        //     Usage (X)
    0x09, 0x31,        //     Usage (Y)
    0x15, 0x00,        //     Logical Minimum (0)
    0x27, 0xFF, 0xFF, 0x00, 0x00,  //     Logical Maximum (65535)
    0x35, 0x00,        //     Physical Minimum (0)
    0x47, 0xFF, 0xFF, 0x00, 0x00,  //     Physical Maximum (65535)
    0x65, 0x11,        //     Unit (SI Linear: Centimeter)
    0x55, 0x0E,        //     Unit Exponent (-2)
    0x75, 0x10,        //     Report Size (16)
    0x95, 0x02,        //     Report Count (2)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x05, 0x0D,        //     Usage Page (Digitizer)
    0x09, 0x48,        //     Usage (Width)
    0x09, 0x49,        //     Usage (Height)
    0x95, 0x02,        //     Report Count (2)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x09, 0x30,        //     Usage (Tip Pressure)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0xFF, 0x0F,  //     Logical Maximum (4095)
    0x35, 0x00,        //     Physical Minimum (0)
    0x46, 0xFF, 0x0F,  //     Physical Maximum (4095)
    0x65, 0x00,        //     Unit (None)
    0x55, 0x00,        //     Unit Exponent (0)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0xC0,              //   End Collection
];

#[rustfmt::skip]
const HEAD: &[u8] = &[
    0x05, 0x0D,        // Usage Page (Digitizer)
    0x09, 0x04,        // Usage (Touch Screen)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x08,        //   Report ID (8)
];

#[rustfmt::skip]
const TAIL: &[u8] = &[
    0x05, 0x0D,        //   Usage Page (Digitizer)
    0x09, 0x54,        //   Usage (Contact Count)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x0A,        //   Logical Maximum (10)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x02,        //   Input (Data,Var,Abs)
    0x85, 0x09,        //   Report ID (9)
    0x09, 0x55,        //   Usage (Contact Count Maximum)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x0A,        //   Logical Maximum (10)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x01,        //   Report Count (1)
    0xB1, 0x02,        //   Feature (Data,Var,Abs)
    0xC0,              // End Collection
];

const DESCRIPTOR_LEN: usize = HEAD.len() + CONTACT.len() * MAX_CONTACTS + TAIL.len();

const fn build_descriptor() -> [u8; DESCRIPTOR_LEN] {
    let mut out = [0u8; DESCRIPTOR_LEN];
    let mut at = 0;
    let mut i = 0;
    while i < HEAD.len() {
        out[at] = HEAD[i];
        at += 1;
        i += 1;
    }
    let mut slot = 0;
    while slot < MAX_CONTACTS {
        let mut j = 0;
        while j < CONTACT.len() {
            out[at] = CONTACT[j];
            at += 1;
            j += 1;
        }
        slot += 1;
    }
    let mut k = 0;
    while k < TAIL.len() {
        out[at] = TAIL[k];
        at += 1;
        k += 1;
    }
    out
}

static DESCRIPTOR_BYTES: [u8; DESCRIPTOR_LEN] = build_descriptor();

/// The report descriptor: the same finger collection repeated [`MAX_CONTACTS`] times.
pub static DESCRIPTOR: &[u8] = &DESCRIPTOR_BYTES;

/// The input report for a state. A finger absent from the state is reported
/// lifted in a free slot, so the host sees the same fixed layout every frame.
pub fn encode(state: &TouchState) -> Vec<u8> {
    let mut report = vec![0u8; INPUT_LEN];
    report[0] = REPORT_ID;
    let mut count = 0u8;
    for (slot, contact) in state.contacts.iter().take(MAX_CONTACTS).enumerate() {
        let at = 1 + slot * CONTACT_LEN;
        report[at] = u8::from(contact.down);
        report[at + 1] = contact.id;
        report[at + 2..at + 4].copy_from_slice(&contact.x.to_le_bytes());
        report[at + 4..at + 6].copy_from_slice(&contact.y.to_le_bytes());
        report[at + 6..at + 8].copy_from_slice(&contact.width.to_le_bytes());
        report[at + 8..at + 10].copy_from_slice(&contact.height.to_le_bytes());
        let pressure = if contact.down {
            (u32::from(contact.pressure) * u32::from(PRESSURE_MAX) / 65535).max(1) as u16
        } else {
            0
        };
        report[at + 10..at + 12].copy_from_slice(&pressure.to_le_bytes());
        count += 1;
    }
    report[INPUT_LEN - 1] = count;
    report
}
