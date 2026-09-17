//! The AllunoVHID contract: IOCTL codes, packed headers and request layouts
//! exactly as `driver/windows/vhid/vhid_ioctl.h` declares them, checked
//! without the bus installed.

#![cfg(target_os = "windows")]

use alluno_input_core::GamepadProfile;
use alluno_input_core::hid::{FeatureReport, Identity, PadCodec};
use alluno_input_windows::vhid::{
    API_VERSION, INTERFACE, IOCTL_INPUT, IOCTL_PLUG, IOCTL_SET_FEATURE, IOCTL_UNPLUG,
    IOCTL_VERSION, IOCTL_WAIT_OUTPUT, MAX_DESCRIPTOR, MAX_DEVICES, MAX_REPORT, OutputHeader,
    PlugHeader, ReportHeader, parse_output, plug_request, report_request,
};

#[test]
fn the_ioctl_codes_are_the_ones_the_header_declares() {
    assert_eq!(IOCTL_VERSION, 0x8A11_2000);
    assert_eq!(IOCTL_PLUG, 0x8A11_2004);
    assert_eq!(IOCTL_UNPLUG, 0x8A11_2008);
    assert_eq!(IOCTL_INPUT, 0x8A11_200C);
    assert_eq!(IOCTL_WAIT_OUTPUT, 0x8A11_2010);
    assert_eq!(IOCTL_SET_FEATURE, 0x8A11_2014);
    assert_eq!(API_VERSION, 1);
    assert_eq!((MAX_DEVICES, MAX_REPORT, MAX_DESCRIPTOR), (16, 512, 4096));
}

#[test]
fn the_headers_are_packed_like_the_c_structs() {
    assert_eq!(std::mem::size_of::<PlugHeader>(), 12);
    assert_eq!(std::mem::size_of::<ReportHeader>(), 8);
    assert_eq!(std::mem::size_of::<OutputHeader>(), 4);
    assert_eq!(std::mem::align_of::<PlugHeader>(), 1);
}

#[test]
fn the_interface_guid_is_the_one_the_inf_and_driver_share() {
    assert_eq!(
        format!("{INTERFACE:?}").to_uppercase(),
        "7F3A6C2E-4B1D-4E8A-9C0B-5A2D3F6E1B90"
    );
}

#[test]
fn a_plug_request_lays_out_the_descriptor_then_each_feature_with_its_length() {
    let request = plug_request(
        Identity {
            vendor: 0x1234,
            product: 0x5678,
            version: 0x0100,
        },
        &[0x05, 0x01, 0xC0],
        &[FeatureReport {
            id: 0x09,
            data: &[0x09, 10],
        }],
    );
    assert_eq!(&request[0..2], &0x1234u16.to_le_bytes());
    assert_eq!(&request[2..4], &0x5678u16.to_le_bytes());
    assert_eq!(&request[4..6], &0x0100u16.to_le_bytes());
    assert_eq!(&request[6..8], &3u16.to_le_bytes());
    assert_eq!(&request[8..10], &1u16.to_le_bytes());
    assert_eq!(&request[12..15], &[0x05, 0x01, 0xC0]);
    assert_eq!(&request[15..17], &2u16.to_le_bytes());
    assert_eq!(&request[17..19], &[0x09, 10]);
    assert_eq!(request.len(), 19);
}

#[test]
fn a_report_request_prefixes_the_slot_and_length() {
    let request = report_request(3, &[0x01, 0xAA]);
    assert_eq!(&request[0..4], &3u32.to_le_bytes());
    assert_eq!(&request[4..6], &2u16.to_le_bytes());
    assert_eq!(&request[8..10], &[0x01, 0xAA]);
}

#[test]
fn an_output_completion_parses_to_its_kind_and_bytes() {
    assert_eq!(
        parse_output(&[1, 0, 2, 0, 0x05, 0xFF]),
        Some((1, &[0x05, 0xFF][..]))
    );
    assert_eq!(parse_output(&[0, 0, 3, 0, 0x05]), None);
    assert_eq!(parse_output(&[0, 0]), None);
}

#[test]
fn every_hid_profile_fits_the_bus_limits() {
    for profile in GamepadProfile::ALL {
        let Some(codec) = PadCodec::new(*profile) else {
            continue;
        };
        assert!(codec.descriptor().len() <= MAX_DESCRIPTOR);
        assert!(codec.input_len() <= MAX_REPORT);
        assert!(codec.output_len() <= MAX_REPORT);
        for feature in codec.features() {
            assert!(feature.data.len() <= MAX_REPORT);
        }
    }
    for descriptor in [
        alluno_input_core::hid::touch::DESCRIPTOR,
        alluno_input_core::hid::pen::DESCRIPTOR,
        alluno_input_core::hid::keyboard::DESCRIPTOR,
        alluno_input_core::hid::mouse::DESCRIPTOR,
    ] {
        assert!(descriptor.len() <= MAX_DESCRIPTOR);
    }
    for report_len in [
        alluno_input_core::hid::touch::INPUT_LEN,
        alluno_input_core::hid::pen::INPUT_LEN,
        alluno_input_core::hid::keyboard::INPUT_LEN,
        alluno_input_core::hid::mouse::INPUT_LEN,
    ] {
        assert!(report_len <= MAX_REPORT);
    }
}
