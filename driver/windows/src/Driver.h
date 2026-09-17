#pragma once

#include <ntddk.h>
#include <wdf.h>
#include <kbdmou.h>
#include <ntddkbd.h>
#include <ntddmou.h>
#include <ntstrsafe.h>

#pragma warning(disable:4996) // ExAllocatePoolWithTag deprecation

// Build-time: define ALLUNO_INPUT_KEYBOARD or ALLUNO_INPUT_MOUSE

#if defined(ALLUNO_INPUT_KEYBOARD)
    #define ALLUNO_DEVICE_TYPE        FILE_DEVICE_KEYBOARD
    #define ALLUNO_CONNECT_IOCTL      IOCTL_INTERNAL_KEYBOARD_CONNECT
    #define ALLUNO_DISCONNECT_IOCTL   IOCTL_INTERNAL_KEYBOARD_DISCONNECT
    #define ALLUNO_NTDEVICE_NAME      L"\\Device\\KeyboardAllunoInput"
    #define ALLUNO_SYMBOLIC_NAME      L"\\DosDevices\\KeyboardAllunoInput"
    #define ALLUNO_INPUT_DATA_SIZE    sizeof(KEYBOARD_INPUT_DATA)
#elif defined(ALLUNO_INPUT_MOUSE)
    #define ALLUNO_DEVICE_TYPE        FILE_DEVICE_MOUSE
    #define ALLUNO_CONNECT_IOCTL      IOCTL_INTERNAL_MOUSE_CONNECT
    #define ALLUNO_DISCONNECT_IOCTL   IOCTL_INTERNAL_MOUSE_DISCONNECT
    #define ALLUNO_NTDEVICE_NAME      L"\\Device\\MouseAllunoInput"
    #define ALLUNO_SYMBOLIC_NAME      L"\\DosDevices\\MouseAllunoInput"
    #define ALLUNO_INPUT_DATA_SIZE    sizeof(MOUSE_INPUT_DATA)
#else
    #error "Define ALLUNO_INPUT_KEYBOARD or ALLUNO_INPUT_MOUSE"
#endif

// User-mode IOCTL for sending input
#define IOCTL_ALLUNO_SEND  CTL_CODE(ALLUNO_DEVICE_TYPE, 0x820, METHOD_BUFFERED, FILE_ANY_ACCESS)

// Per-filter-device context
typedef struct _FILTER_EXTENSION {
    CONNECT_DATA UpperConnectData;
} FILTER_EXTENSION, *PFILTER_EXTENSION;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(FILTER_EXTENSION, FilterGetData)

// Control device context
typedef struct _CONTROL_EXTENSION {
    USHORT Placeholder;
} CONTROL_EXTENSION, *PCONTROL_EXTENSION;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(CONTROL_EXTENSION, ControlGetData)

// Globals
extern WDFCOLLECTION FilterDeviceCollection;
extern WDFWAITLOCK   FilterDeviceCollectionLock;
extern WDFDEVICE     ControlDevice;

// Functions
DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD EvtDeviceAdd;
EVT_WDF_DEVICE_CONTEXT_CLEANUP EvtDeviceCleanup;
EVT_WDF_IO_QUEUE_IO_INTERNAL_DEVICE_CONTROL EvtIoInternalDeviceControl;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL EvtIoDeviceControl;

VOID ServiceCallback(
    IN PDEVICE_OBJECT DeviceObject,
    IN PVOID InputDataStart,
    IN PVOID InputDataEnd,
    IN OUT PULONG InputDataConsumed);

NTSTATUS CreateControlDevice(WDFDEVICE Device);
VOID DeleteControlDevice(WDFDEVICE Device);
