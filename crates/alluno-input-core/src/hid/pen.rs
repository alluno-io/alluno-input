//! The HID pen digitiser: one stylus with pressure, tilt and twist over a
//! 0..=65535 surface, which is the port's coordinate space unchanged.

use crate::PenState;

/// The device's vendor identity.
pub const IDENTITY: super::Identity = super::Identity {
    vendor: 0x1234,
    product: 0x5680,
    version: 0x0100,
};

/// The bus-visible name.
pub const NAME: &str = "Alluno Pen";

/// The input report id.
pub const REPORT_ID: u8 = 0x07;

/// The input report size, id byte included.
pub const INPUT_LEN: usize = 12;

/// Tip pressure is reported on 0..=4095.
pub const PRESSURE_MAX: u16 = 4095;

#[rustfmt::skip]
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x0D,        // Usage Page (Digitizer)
    0x09, 0x02,        // Usage (Pen)
    0xA1, 0x01,        // Collection (Application)
    0x85, 0x07,        //   Report ID (7)
    0x09, 0x20,        //   Usage (Stylus)
    0xA1, 0x00,        //   Collection (Physical)
    0x09, 0x42,        //     Usage (Tip Switch)
    0x09, 0x44,        //     Usage (Barrel Switch)
    0x09, 0x3C,        //     Usage (Invert)
    0x09, 0x45,        //     Usage (Eraser)
    0x09, 0x32,        //     Usage (In Range)
    0x15, 0x00,        //     Logical Minimum (0)
    0x25, 0x01,        //     Logical Maximum (1)
    0x75, 0x01,        //     Report Size (1)
    0x95, 0x05,        //     Report Count (5)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x95, 0x03,        //     Report Count (3)
    0x81, 0x03,        //     Input (Const,Var,Abs)
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
    0x09, 0x30,        //     Usage (Tip Pressure)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0xFF, 0x0F,  //     Logical Maximum (4095)
    0x35, 0x00,        //     Physical Minimum (0)
    0x46, 0xFF, 0x0F,  //     Physical Maximum (4095)
    0x65, 0x00,        //     Unit (None)
    0x55, 0x00,        //     Unit Exponent (0)
    0x75, 0x10,        //     Report Size (16)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x09, 0x3D,        //     Usage (X Tilt)
    0x09, 0x3E,        //     Usage (Y Tilt)
    0x15, 0xA6,        //     Logical Minimum (-90)
    0x25, 0x5A,        //     Logical Maximum (90)
    0x35, 0xA6,        //     Physical Minimum (-90)
    0x45, 0x5A,        //     Physical Maximum (90)
    0x65, 0x14,        //     Unit (Degrees)
    0x75, 0x08,        //     Report Size (8)
    0x95, 0x02,        //     Report Count (2)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x09, 0x41,        //     Usage (Twist)
    0x15, 0x00,        //     Logical Minimum (0)
    0x26, 0x67, 0x01,  //     Logical Maximum (359)
    0x35, 0x00,        //     Physical Minimum (0)
    0x46, 0x67, 0x01,  //     Physical Maximum (359)
    0x75, 0x10,        //     Report Size (16)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x02,        //     Input (Data,Var,Abs)
    0x65, 0x00,        //     Unit (None)
    0xC0,              //   End Collection
    0xC0,              // End Collection
];

/// The input report for a sample.
pub fn encode(state: &PenState) -> [u8; INPUT_LEN] {
    let mut report = [0u8; INPUT_LEN];
    report[0] = REPORT_ID;
    let mut switches = 0u8;
    if state.down {
        switches |= 1 << 0;
    }
    if state.barrel {
        switches |= 1 << 1;
    }
    if state.eraser {
        switches |= 1 << 2;
        if state.down {
            switches |= 1 << 3;
        }
    }
    if state.in_range || state.down {
        switches |= 1 << 4;
    }
    report[1] = switches;
    report[2..4].copy_from_slice(&state.x.to_le_bytes());
    report[4..6].copy_from_slice(&state.y.to_le_bytes());
    let pressure = if state.down {
        (u32::from(state.pressure) * u32::from(PRESSURE_MAX) / 65535).max(1) as u16
    } else {
        0
    };
    report[6..8].copy_from_slice(&pressure.to_le_bytes());
    report[8] = state.tilt_x.clamp(-90, 90) as u8;
    report[9] = state.tilt_y.clamp(-90, 90) as u8;
    report[10..12].copy_from_slice(&(state.twist % 360).to_le_bytes());
    report
}
