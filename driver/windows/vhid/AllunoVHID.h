/*
 * AllunoVHID: a KMDF software device that publishes virtual input devices.
 * A HID slot is a Virtual HID Framework node built from a report descriptor;
 * an XUSB slot is a child device that looks like a wired Xbox 360 controller
 * to Windows' own xusb22 driver, which is what puts a pad into XInput.
 * User mode plugs a slot, submits input, and waits for what the OS writes
 * back: HID output and feature reports, or the XUSB rumble and lamp packets.
 */

#pragma once

#include <ntddk.h>
#include <wdf.h>
#include <initguid.h>
#include <vhf.h>
#include <usb.h>
#include <usbioctl.h>
#include <usbbusif.h>
#include <ntstrsafe.h>

#include "vhid_ioctl.h"

#pragma warning(disable:4996)

DEFINE_GUID(GUID_DEVINTERFACE_ALLUNO_VHID,
    0x7F3A6C2E, 0x4B1D, 0x4E8A, 0x9C, 0x0B, 0x5A, 0x2D, 0x3F, 0x6E, 0x1B, 0x90);

#define VHID_POOL_TAG 'dhVA'

#define XUSB_DEVICE_DESCRIPTOR_LENGTH   18
#define XUSB_CONFIGURATION_LENGTH       153
#define XUSB_BOOT_STAGES                6

typedef struct _VHID_FEATURE {
    UCHAR   Id;
    USHORT  Length;
    UCHAR   Data[VHID_MAX_REPORT];
} VHID_FEATURE, *PVHID_FEATURE;

typedef struct _VHID_SLOT {
    BOOLEAN         Used;
    BOOLEAN         Started;
    UCHAR           Kind;
    ULONG           Index;
    VHFHANDLE       Vhf;
    WDFDEVICE       Pdo;
    WDFFILEOBJECT   Owner;
    WDFQUEUE        Waiters;
    WDFSPINLOCK     Lock;
    PUCHAR          Descriptor;
    USHORT          DescriptorLength;
    ULONG           FeatureCount;
    VHID_FEATURE    Features[VHID_MAX_FEATURES];
    UCHAR           LastInput[VHID_MAX_REPORT];
    USHORT          LastInputLength;
    BOOLEAN         OutputPending;
    UCHAR           OutputKind;
    USHORT          OutputLength;
    UCHAR           Output[VHID_MAX_REPORT];
    WCHAR           InstanceId[16];
} VHID_SLOT, *PVHID_SLOT;

typedef struct _VHID_DEVICE_CONTEXT {
    WDFDEVICE   Device;
    WDFWAITLOCK SlotsLock;
    VHID_SLOT   Slots[VHID_MAX_DEVICES];
} VHID_DEVICE_CONTEXT, *PVHID_DEVICE_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(VHID_DEVICE_CONTEXT, VhidGetDeviceContext)

typedef struct _VHID_FILE_CONTEXT {
    ULONG OwnedSlots;
} VHID_FILE_CONTEXT, *PVHID_FILE_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(VHID_FILE_CONTEXT, VhidGetFileContext)

/* The child device an XUSB slot publishes. */
typedef struct _XUSB_PDO_CONTEXT {
    PVHID_SLOT                  Slot;
    WDFQUEUE                    PendingIn;
    UCHAR                       Stage;
    BOOLEAN                     InputFresh;
    UCHAR                       Input[VHID_XUSB_INPUT_LENGTH];
    UCHAR                       DeviceDescriptor[XUSB_DEVICE_DESCRIPTOR_LENGTH];
    USB_BUS_INTERFACE_USBDI_V1  Usbdi;
} XUSB_PDO_CONTEXT, *PXUSB_PDO_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(XUSB_PDO_CONTEXT, XusbGetPdoContext)

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD VhidEvtDeviceAdd;
EVT_WDF_DEVICE_CONTEXT_CLEANUP VhidEvtDeviceCleanup;
EVT_WDF_FILE_CLEANUP VhidEvtFileCleanup;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL VhidEvtIoDeviceControl;

EVT_VHF_ASYNC_OPERATION VhidEvtGetFeature;
EVT_VHF_ASYNC_OPERATION VhidEvtSetFeature;
EVT_VHF_ASYNC_OPERATION VhidEvtWriteReport;
EVT_VHF_ASYNC_OPERATION VhidEvtGetInputReport;
EVT_VHF_CLEANUP VhidEvtVhfCleanup;

EVT_WDF_IO_QUEUE_IO_INTERNAL_DEVICE_CONTROL XusbEvtInternalDeviceControl;
EVT_WDF_DEVICE_CONTEXT_CLEANUP XusbEvtPdoCleanup;

VOID VhidDeliverOutput(PVHID_SLOT Slot, UCHAR Kind, PUCHAR Data, USHORT Length);

NTSTATUS XusbPlug(PVHID_DEVICE_CONTEXT Context, PVHID_SLOT Slot, USHORT VendorId, USHORT ProductId, USHORT VersionNumber);
VOID XusbUnplug(WDFDEVICE Pdo);
NTSTATUS XusbSubmitInput(PVHID_SLOT Slot, PUCHAR Data, USHORT Length);
