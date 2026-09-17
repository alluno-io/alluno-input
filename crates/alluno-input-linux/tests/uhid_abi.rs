//! The uhid event layouts are the kernel's, byte for byte.

#![cfg(target_os = "linux")]

use alluno_input_linux::uhid::{
    Create2Req, EVENT_SIZE, Event, GetReportReplyReq, GetReportReq, Input2Req, OutputReq,
    SetReportReplyReq, SetReportReq, UHID_CREATE2, UHID_DATA_MAX, UHID_GET_REPORT_REPLY,
    UHID_INPUT2, UHID_SET_REPORT_REPLY,
};

#[test]
fn the_event_and_every_request_are_packed_to_the_kernel_sizes() {
    assert_eq!(std::mem::size_of::<Create2Req>(), 4372);
    assert_eq!(std::mem::size_of::<Input2Req>(), 4098);
    assert_eq!(std::mem::size_of::<OutputReq>(), 4099);
    assert_eq!(std::mem::size_of::<GetReportReq>(), 6);
    assert_eq!(std::mem::size_of::<GetReportReplyReq>(), 4104);
    assert_eq!(std::mem::size_of::<SetReportReq>(), 4104);
    assert_eq!(std::mem::size_of::<SetReportReplyReq>(), 6);
    assert_eq!(std::mem::size_of::<Event>(), EVENT_SIZE);
    assert_eq!(EVENT_SIZE, 4 + 4372);
    assert_eq!(std::mem::align_of::<Event>(), 1);
    assert_eq!(UHID_DATA_MAX, 4096);
}

#[test]
fn the_request_numbers_are_the_kernel_enum() {
    assert_eq!(UHID_CREATE2, 11);
    assert_eq!(UHID_INPUT2, 12);
    assert_eq!(UHID_GET_REPORT_REPLY, 10);
    assert_eq!(UHID_SET_REPORT_REPLY, 14);
}
