/*
 * The Xbox 360 device an XUSB slot publishes.
 *
 * XInput never reads HID: it reads Microsoft's xusb22 driver, which binds to
 * a USB device with the Xbox 360 identity and speaks USB to it through URBs.
 * So this file is a very small USB device: the wired controller's descriptors,
 * the six packets a real pad sends on its interrupt pipe before it streams,
 * the vendor request answers xusb22 checks during start, and the two output
 * packets a game writes (rumble and the player lamp). Everything else a USB
 * device can do is declined politely.
 */

#include "AllunoVHID.h"

#ifdef ALLOC_PRAGMA
#pragma alloc_text(PAGE, XusbPlug)
#pragma alloc_text(PAGE, XusbEvtPdoCleanup)
#endif

#define XUSB_INTERFACE_HANDLE       ((USBD_INTERFACE_HANDLE)(ULONG_PTR)0xFFFF0000)
#define XUSB_CONFIGURATION_HANDLE   ((USBD_CONFIGURATION_HANDLE)(ULONG_PTR)0xFFFF0100)
#define XUSB_PIPE_HANDLE(endpoint)  ((USBD_PIPE_HANDLE)(ULONG_PTR)(0xFFFF0000 | (endpoint)))
#define XUSB_IN_ENDPOINT            0x81
#define XUSB_OUT_ENDPOINT           0x01
#define XUSB_MAX_TRANSFER           0x00400000
#define XUSB_BOOT_SHORT             3

static const UCHAR XusbDeviceDescriptorTemplate[XUSB_DEVICE_DESCRIPTOR_LENGTH] = {
    0x12, 0x01, 0x00, 0x02, 0xFF, 0xFF, 0xFF, 0x08,
    0x5E, 0x04, 0x8E, 0x02, 0x14, 0x01, 0x01, 0x02, 0x03, 0x01
};

static const UCHAR XusbConfiguration[XUSB_CONFIGURATION_LENGTH] = {
    0x09, 0x02, 0x99, 0x00, 0x04, 0x01, 0x00, 0xA0, 0xFA,
    0x09, 0x04, 0x00, 0x00, 0x02, 0xFF, 0x5D, 0x01, 0x00,
    0x11, 0x21, 0x00, 0x01, 0x01, 0x25, 0x81, 0x14, 0x00, 0x00, 0x00, 0x00, 0x13, 0x01, 0x08, 0x00, 0x00,
    0x07, 0x05, 0x81, 0x03, 0x20, 0x00, 0x04,
    0x07, 0x05, 0x01, 0x03, 0x20, 0x00, 0x08,
    0x09, 0x04, 0x01, 0x00, 0x04, 0xFF, 0x5D, 0x03, 0x00,
    0x1B, 0x21, 0x00, 0x01, 0x01, 0x01, 0x82, 0x40, 0x01,
    0x02, 0x20, 0x16, 0x83, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x16, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 0x05, 0x82, 0x03, 0x20, 0x00, 0x02,
    0x07, 0x05, 0x02, 0x03, 0x20, 0x00, 0x04,
    0x07, 0x05, 0x83, 0x03, 0x20, 0x00, 0x40,
    0x07, 0x05, 0x03, 0x03, 0x20, 0x00, 0x10,
    0x09, 0x04, 0x02, 0x00, 0x01, 0xFF, 0x5D, 0x02, 0x00,
    0x09, 0x21, 0x00, 0x01, 0x01, 0x22, 0x84, 0x07, 0x00,
    0x07, 0x05, 0x84, 0x03, 0x20, 0x00, 0x10,
    0x09, 0x04, 0x03, 0x00, 0x00, 0xFF, 0xFD, 0x13, 0x04,
    0x06, 0x41, 0x00, 0x01, 0x01, 0x03
};

static const UCHAR XusbBootPackets[XUSB_BOOT_STAGES][VHID_XUSB_INPUT_LENGTH] = {
    { 0x01, 0x03, 0x0E },
    { 0x02, 0x03, 0x00 },
    { 0x03, 0x03, 0x03 },
    { 0x08, 0x03, 0x00 },
    { 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0xE4, 0xF2, 0xB3, 0xF8, 0x49, 0xF3, 0xB0, 0xFC, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 },
    { 0x01, 0x03, 0x03 }
};

static const UCHAR XusbBootLengths[XUSB_BOOT_STAGES] = { 3, 3, 3, 3, VHID_XUSB_INPUT_LENGTH, 3 };

static const UCHAR XusbVendorAnswer[4] = { 0x31, 0x3F, 0xCF, 0xDC };

typedef struct _XUSB_INTERFACE {
    UCHAR Number;
    UCHAR Class;
    UCHAR SubClass;
    UCHAR Protocol;
    UCHAR PipeCount;
    UCHAR Endpoints[4];
    UCHAR Intervals[4];
} XUSB_INTERFACE;

static const XUSB_INTERFACE XusbInterfaces[4] = {
    { 0, 0xFF, 0x5D, 0x01, 2, { 0x81, 0x01, 0x00, 0x00 }, { 4, 8, 0, 0 } },
    { 1, 0xFF, 0x5D, 0x03, 4, { 0x82, 0x02, 0x83, 0x03 }, { 4, 8, 8, 8 } },
    { 2, 0xFF, 0x5D, 0x02, 1, { 0x84, 0x00, 0x00, 0x00 }, { 4, 0, 0, 0 } },
    { 3, 0xFF, 0xFD, 0x13, 0, { 0x00, 0x00, 0x00, 0x00 }, { 0, 0, 0, 0 } }
};

// ============================================================================
// The USB bus interface xusb22 queries
// ============================================================================

static VOID USB_BUSIFFN XusbUsbdiGetVersion(
    PVOID                       BusContext,
    PUSBD_VERSION_INFORMATION   VersionInformation,
    PULONG                      HcdCapabilities)
{
    UNREFERENCED_PARAMETER(BusContext);

    if (VersionInformation != NULL) {
        VersionInformation->USBDI_Version = 0x500;
        VersionInformation->Supported_USB_Version = 0x200;
    }
    if (HcdCapabilities != NULL)
        *HcdCapabilities = 0;
}

static NTSTATUS USB_BUSIFFN XusbUsbdiQueryBusTime(PVOID BusContext, PULONG CurrentUsbFrame)
{
    UNREFERENCED_PARAMETER(BusContext);
    UNREFERENCED_PARAMETER(CurrentUsbFrame);
    return STATUS_UNSUCCESSFUL;
}

static NTSTATUS USB_BUSIFFN XusbUsbdiSubmitIsoOutUrb(PVOID BusContext, PURB Urb)
{
    UNREFERENCED_PARAMETER(BusContext);
    UNREFERENCED_PARAMETER(Urb);
    return STATUS_UNSUCCESSFUL;
}

static NTSTATUS USB_BUSIFFN XusbUsbdiQueryBusInformation(
    PVOID   BusContext,
    ULONG   Level,
    PVOID   BusInformationBuffer,
    PULONG  BusInformationBufferLength,
    PULONG  BusInformationActualLength)
{
    UNREFERENCED_PARAMETER(BusContext);
    UNREFERENCED_PARAMETER(Level);
    UNREFERENCED_PARAMETER(BusInformationBuffer);
    UNREFERENCED_PARAMETER(BusInformationBufferLength);
    UNREFERENCED_PARAMETER(BusInformationActualLength);
    return STATUS_UNSUCCESSFUL;
}

static BOOLEAN USB_BUSIFFN XusbUsbdiIsDeviceHighSpeed(PVOID BusContext)
{
    UNREFERENCED_PARAMETER(BusContext);
    return TRUE;
}

// ============================================================================
// Plug and unplug
// ============================================================================

static NTSTATUS XusbAddIds(PWDFDEVICE_INIT Init, USHORT VendorId, USHORT ProductId, ULONG Index)
{
    DECLARE_UNICODE_STRING_SIZE(id, 64);
    DECLARE_CONST_UNICODE_STRING(compat0, L"USB\\MS_COMP_XUSB10");
    DECLARE_CONST_UNICODE_STRING(compat1, L"USB\\Class_FF&SubClass_5D&Prot_01");
    DECLARE_CONST_UNICODE_STRING(compat2, L"USB\\Class_FF&SubClass_5D");
    DECLARE_CONST_UNICODE_STRING(compat3, L"USB\\Class_FF");
    DECLARE_CONST_UNICODE_STRING(description, L"Alluno Xbox 360 Controller");
    DECLARE_CONST_UNICODE_STRING(location, L"Alluno Virtual HID Bus");
    NTSTATUS status;

    status = RtlUnicodeStringPrintf(&id, L"USB\\VID_%04X&PID_%04X", VendorId, ProductId);
    if (!NT_SUCCESS(status))
        return status;
    status = WdfPdoInitAssignDeviceID(Init, &id);
    if (!NT_SUCCESS(status))
        return status;
    status = WdfPdoInitAddHardwareID(Init, &id);
    if (!NT_SUCCESS(status))
        return status;

    status = WdfPdoInitAddCompatibleID(Init, &compat0);
    if (!NT_SUCCESS(status))
        return status;
    status = WdfPdoInitAddCompatibleID(Init, &compat1);
    if (!NT_SUCCESS(status))
        return status;
    status = WdfPdoInitAddCompatibleID(Init, &compat2);
    if (!NT_SUCCESS(status))
        return status;
    status = WdfPdoInitAddCompatibleID(Init, &compat3);
    if (!NT_SUCCESS(status))
        return status;

    status = RtlUnicodeStringPrintf(&id, L"%02u", Index);
    if (!NT_SUCCESS(status))
        return status;
    status = WdfPdoInitAssignInstanceID(Init, &id);
    if (!NT_SUCCESS(status))
        return status;

    status = WdfPdoInitAddDeviceText(Init, &description, &location, 0x409);
    if (!NT_SUCCESS(status))
        return status;
    WdfPdoInitSetDefaultLocale(Init, 0x409);
    return STATUS_SUCCESS;
}

NTSTATUS XusbPlug(
    PVHID_DEVICE_CONTEXT Context,
    PVHID_SLOT           Slot,
    USHORT               VendorId,
    USHORT               ProductId,
    USHORT               VersionNumber)
{
    PWDFDEVICE_INIT                 init;
    WDF_OBJECT_ATTRIBUTES           attributes;
    WDF_DEVICE_PNP_CAPABILITIES     pnpCaps;
    WDF_DEVICE_POWER_CAPABILITIES   powerCaps;
    WDF_IO_QUEUE_CONFIG             queueConfig;
    WDF_QUERY_INTERFACE_CONFIG      interfaceConfig;
    PXUSB_PDO_CONTEXT               pdoContext;
    WDFDEVICE                       pdo = NULL;
    NTSTATUS                        status;

    PAGED_CODE();

    init = WdfPdoInitAllocate(Context->Device);
    if (init == NULL)
        return STATUS_INSUFFICIENT_RESOURCES;

    WdfDeviceInitSetDeviceType(init, FILE_DEVICE_BUS_EXTENDER);

    status = XusbAddIds(init, VendorId, ProductId, Slot->Index);
    if (!NT_SUCCESS(status)) {
        WdfDeviceInitFree(init);
        return status;
    }

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, XUSB_PDO_CONTEXT);
    attributes.EvtCleanupCallback = XusbEvtPdoCleanup;

    status = WdfDeviceCreate(&init, &attributes, &pdo);
    if (!NT_SUCCESS(status)) {
        WdfDeviceInitFree(init);
        return status;
    }

    pdoContext = XusbGetPdoContext(pdo);
    RtlZeroMemory(pdoContext, sizeof(XUSB_PDO_CONTEXT));
    pdoContext->Slot = Slot;
    RtlCopyMemory(pdoContext->DeviceDescriptor, XusbDeviceDescriptorTemplate, XUSB_DEVICE_DESCRIPTOR_LENGTH);
    pdoContext->DeviceDescriptor[8] = (UCHAR)(VendorId & 0xFF);
    pdoContext->DeviceDescriptor[9] = (UCHAR)(VendorId >> 8);
    pdoContext->DeviceDescriptor[10] = (UCHAR)(ProductId & 0xFF);
    pdoContext->DeviceDescriptor[11] = (UCHAR)(ProductId >> 8);
    pdoContext->DeviceDescriptor[12] = (UCHAR)(VersionNumber & 0xFF);
    pdoContext->DeviceDescriptor[13] = (UCHAR)(VersionNumber >> 8);

    WDF_DEVICE_PNP_CAPABILITIES_INIT(&pnpCaps);
    pnpCaps.Removable = WdfTrue;
    pnpCaps.SurpriseRemovalOK = WdfTrue;
    pnpCaps.UniqueID = WdfTrue;
    pnpCaps.Address = Slot->Index + 1;
    pnpCaps.UINumber = Slot->Index + 1;
    WdfDeviceSetPnpCapabilities(pdo, &pnpCaps);

    WDF_DEVICE_POWER_CAPABILITIES_INIT(&powerCaps);
    powerCaps.DeviceD1 = WdfTrue;
    powerCaps.DeviceD2 = WdfTrue;
    powerCaps.WakeFromD0 = WdfTrue;
    powerCaps.WakeFromD1 = WdfTrue;
    powerCaps.WakeFromD2 = WdfTrue;
    powerCaps.DeviceState[PowerSystemWorking] = PowerDeviceD0;
    powerCaps.DeviceState[PowerSystemSleeping1] = PowerDeviceD2;
    powerCaps.DeviceState[PowerSystemSleeping2] = PowerDeviceD2;
    powerCaps.DeviceState[PowerSystemSleeping3] = PowerDeviceD2;
    powerCaps.DeviceState[PowerSystemHibernate] = PowerDeviceD2;
    powerCaps.DeviceState[PowerSystemShutdown] = PowerDeviceD3;
    WdfDeviceSetPowerCapabilities(pdo, &powerCaps);

    pdoContext->Usbdi.Size = sizeof(USB_BUS_INTERFACE_USBDI_V1);
    pdoContext->Usbdi.Version = USB_BUSIF_USBDI_VERSION_1;
    pdoContext->Usbdi.BusContext = pdo;
    pdoContext->Usbdi.InterfaceReference = WdfDeviceInterfaceReferenceNoOp;
    pdoContext->Usbdi.InterfaceDereference = WdfDeviceInterfaceDereferenceNoOp;
    pdoContext->Usbdi.GetUSBDIVersion = XusbUsbdiGetVersion;
    pdoContext->Usbdi.QueryBusTime = XusbUsbdiQueryBusTime;
    pdoContext->Usbdi.SubmitIsoOutUrb = XusbUsbdiSubmitIsoOutUrb;
    pdoContext->Usbdi.QueryBusInformation = XusbUsbdiQueryBusInformation;
    pdoContext->Usbdi.IsDeviceHighSpeed = XusbUsbdiIsDeviceHighSpeed;

    WDF_QUERY_INTERFACE_CONFIG_INIT(&interfaceConfig, (PINTERFACE)&pdoContext->Usbdi,
        &USB_BUS_INTERFACE_USBDI_GUID, NULL);
    status = WdfDeviceAddQueryInterface(pdo, &interfaceConfig);
    if (!NT_SUCCESS(status))
        goto fail;

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchParallel);
    queueConfig.EvtIoInternalDeviceControl = XusbEvtInternalDeviceControl;
    status = WdfIoQueueCreate(pdo, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, WDF_NO_HANDLE);
    if (!NT_SUCCESS(status))
        goto fail;

    WDF_IO_QUEUE_CONFIG_INIT(&queueConfig, WdfIoQueueDispatchManual);
    queueConfig.PowerManaged = WdfFalse;
    status = WdfIoQueueCreate(pdo, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, &pdoContext->PendingIn);
    if (!NT_SUCCESS(status))
        goto fail;

    status = WdfFdoAddStaticChild(Context->Device, pdo);
    if (!NT_SUCCESS(status))
        goto fail;

    WdfSpinLockAcquire(Slot->Lock);
    Slot->Pdo = pdo;
    WdfSpinLockRelease(Slot->Lock);
    return STATUS_SUCCESS;

fail:
    WdfObjectDelete(pdo);
    return status;
}

VOID XusbUnplug(WDFDEVICE Pdo)
{
    PXUSB_PDO_CONTEXT context = XusbGetPdoContext(Pdo);

    WdfIoQueuePurge(context->PendingIn, NULL, NULL);
    WdfPdoMarkMissing(Pdo);
}

VOID XusbEvtPdoCleanup(WDFOBJECT Device)
{
    PXUSB_PDO_CONTEXT context = XusbGetPdoContext((WDFDEVICE)Device);
    PVHID_SLOT slot = context->Slot;

    PAGED_CODE();

    if (slot != NULL) {
        WdfSpinLockAcquire(slot->Lock);
        if (slot->Pdo == (WDFDEVICE)Device)
            slot->Pdo = NULL;
        WdfSpinLockRelease(slot->Lock);
    }
}

// ============================================================================
// URB helpers
// ============================================================================

static PVOID XusbTransferBuffer(PVOID Buffer, PMDL Mdl)
{
    if (Buffer != NULL)
        return Buffer;
    if (Mdl != NULL)
        return MmGetSystemAddressForMdlSafe(Mdl, NormalPagePriority | MdlMappingNoExecute);
    return NULL;
}

static ULONG XusbCopyOut(PVOID Buffer, PMDL Mdl, ULONG Capacity, const UCHAR* Source, ULONG Length)
{
    PVOID target = XusbTransferBuffer(Buffer, Mdl);
    ULONG copy = Length < Capacity ? Length : Capacity;

    if (target == NULL)
        return 0;
    RtlCopyMemory(target, Source, copy);
    return copy;
}

static VOID XusbFillPipe(PUSBD_PIPE_INFORMATION Pipe, UCHAR Endpoint, UCHAR Interval)
{
    Pipe->MaximumPacketSize = 0x20;
    Pipe->EndpointAddress = Endpoint;
    Pipe->Interval = Interval;
    Pipe->PipeType = UsbdPipeTypeInterrupt;
    Pipe->PipeHandle = XUSB_PIPE_HANDLE(Endpoint);
    Pipe->MaximumTransferSize = XUSB_MAX_TRANSFER;
    Pipe->PipeFlags = 0;
}

static ULONG XusbInterfaceSize(const XUSB_INTERFACE* Interface)
{
    ULONG pipes = Interface->PipeCount == 0 ? 1 : Interface->PipeCount;
    return sizeof(USBD_INTERFACE_INFORMATION) + (pipes - 1) * sizeof(USBD_PIPE_INFORMATION);
}

static VOID XusbFillInterface(PUSBD_INTERFACE_INFORMATION Info, const XUSB_INTERFACE* Interface)
{
    UCHAR i;

    Info->Length = (USHORT)XusbInterfaceSize(Interface);
    Info->InterfaceNumber = Interface->Number;
    Info->AlternateSetting = 0;
    Info->Class = Interface->Class;
    Info->SubClass = Interface->SubClass;
    Info->Protocol = Interface->Protocol;
    Info->Reserved = 0;
    Info->InterfaceHandle = XUSB_INTERFACE_HANDLE;
    Info->NumberOfPipes = Interface->PipeCount;
    for (i = 0; i < Interface->PipeCount; i++)
        XusbFillPipe(&Info->Pipes[i], Interface->Endpoints[i], Interface->Intervals[i]);
}

static NTSTATUS XusbSelectConfiguration(PURB Urb)
{
    PUSBD_INTERFACE_INFORMATION info;
    ULONG required = sizeof(struct _URB_SELECT_CONFIGURATION) - sizeof(USBD_INTERFACE_INFORMATION);
    ULONG i;

    if (Urb->UrbSelectConfiguration.ConfigurationDescriptor == NULL) {
        Urb->UrbSelectConfiguration.ConfigurationHandle = NULL;
        return STATUS_SUCCESS;
    }

    for (i = 0; i < RTL_NUMBER_OF(XusbInterfaces); i++)
        required += XusbInterfaceSize(&XusbInterfaces[i]);
    if (Urb->UrbHeader.Length < required)
        return STATUS_BUFFER_TOO_SMALL;

    Urb->UrbSelectConfiguration.ConfigurationHandle = XUSB_CONFIGURATION_HANDLE;
    info = &Urb->UrbSelectConfiguration.Interface;
    for (i = 0; i < RTL_NUMBER_OF(XusbInterfaces); i++) {
        XusbFillInterface(info, &XusbInterfaces[i]);
        info = (PUSBD_INTERFACE_INFORMATION)((PUCHAR)info + info->Length);
    }
    return STATUS_SUCCESS;
}

static NTSTATUS XusbSelectInterface(PURB Urb)
{
    UCHAR number = Urb->UrbSelectInterface.Interface.InterfaceNumber;
    ULONG i;

    for (i = 0; i < RTL_NUMBER_OF(XusbInterfaces); i++) {
        if (XusbInterfaces[i].Number != number)
            continue;
        if (Urb->UrbSelectInterface.Interface.Length < XusbInterfaceSize(&XusbInterfaces[i]))
            return STATUS_BUFFER_TOO_SMALL;
        XusbFillInterface(&Urb->UrbSelectInterface.Interface, &XusbInterfaces[i]);
        return STATUS_SUCCESS;
    }
    return STATUS_INVALID_PARAMETER;
}

static NTSTATUS XusbGetDescriptor(PXUSB_PDO_CONTEXT Context, PURB Urb)
{
    struct _URB_CONTROL_DESCRIPTOR_REQUEST* request = &Urb->UrbControlDescriptorRequest;
    const UCHAR* source;
    ULONG length;

    switch (request->DescriptorType) {
    case USB_DEVICE_DESCRIPTOR_TYPE:
        source = Context->DeviceDescriptor;
        length = XUSB_DEVICE_DESCRIPTOR_LENGTH;
        break;
    case USB_CONFIGURATION_DESCRIPTOR_TYPE:
        source = XusbConfiguration;
        length = XUSB_CONFIGURATION_LENGTH;
        break;
    default:
        return STATUS_NOT_IMPLEMENTED;
    }

    request->TransferBufferLength = XusbCopyOut(request->TransferBuffer, request->TransferBufferMDL,
        request->TransferBufferLength, source, length);
    return STATUS_SUCCESS;
}

static NTSTATUS XusbControlTransfer(PURB Urb)
{
    struct _URB_CONTROL_TRANSFER* transfer = &Urb->UrbControlTransfer;

    switch (transfer->SetupPacket[6]) {
    case 0x04:
        transfer->TransferBufferLength = XusbCopyOut(transfer->TransferBuffer, transfer->TransferBufferMDL,
            transfer->TransferBufferLength, XusbVendorAnswer, sizeof(XusbVendorAnswer));
        return STATUS_SUCCESS;
    case 0x14:
    case 0x08:
        Urb->UrbHeader.Status = USBD_STATUS_STALL_PID;
        return STATUS_UNSUCCESSFUL;
    default:
        return STATUS_SUCCESS;
    }
}

/* Delivers a completed interrupt-in transfer; the lock is held by the caller. */
static VOID XusbFillInterruptIn(PURB Urb, const UCHAR* Packet, ULONG Length)
{
    struct _URB_BULK_OR_INTERRUPT_TRANSFER* transfer = &Urb->UrbBulkOrInterruptTransfer;

    transfer->TransferBufferLength = XusbCopyOut(transfer->TransferBuffer, transfer->TransferBufferMDL,
        transfer->TransferBufferLength, Packet, Length);
    Urb->UrbHeader.Status = USBD_STATUS_SUCCESS;
}

static PURB XusbUrbOf(WDFREQUEST Request)
{
    PIRP irp = WdfRequestWdmGetIrp(Request);
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(irp);
    return (PURB)stack->Parameters.Others.Argument1;
}

static NTSTATUS XusbInterruptTransfer(PXUSB_PDO_CONTEXT Context, PVHID_SLOT Slot, WDFREQUEST Request, PURB Urb)
{
    struct _URB_BULK_OR_INTERRUPT_TRANSFER* transfer = &Urb->UrbBulkOrInterruptTransfer;
    NTSTATUS status;

    if (transfer->PipeHandle == XUSB_PIPE_HANDLE(XUSB_OUT_ENDPOINT)) {
        PUCHAR data = (PUCHAR)XusbTransferBuffer(transfer->TransferBuffer, transfer->TransferBufferMDL);
        ULONG length = transfer->TransferBufferLength;

        if (data != NULL && length >= 3 && data[0] == 0x01 && data[1] == 0x03)
            VhidDeliverOutput(Slot, VHID_OUTPUT_KIND_REPORT, data, 3);
        else if (data != NULL && length >= VHID_XUSB_OUTPUT_LENGTH && data[0] == 0x00 && data[1] == 0x08)
            VhidDeliverOutput(Slot, VHID_OUTPUT_KIND_REPORT, data, VHID_XUSB_OUTPUT_LENGTH);
        return STATUS_SUCCESS;
    }

    if (transfer->PipeHandle != XUSB_PIPE_HANDLE(XUSB_IN_ENDPOINT))
        return STATUS_SUCCESS;

    WdfSpinLockAcquire(Slot->Lock);
    if (Context->Stage < XUSB_BOOT_STAGES) {
        XusbFillInterruptIn(Urb, XusbBootPackets[Context->Stage], XusbBootLengths[Context->Stage]);
        Context->Stage++;
        status = STATUS_SUCCESS;
    } else if (Context->InputFresh) {
        XusbFillInterruptIn(Urb, Context->Input, VHID_XUSB_INPUT_LENGTH);
        Context->InputFresh = FALSE;
        status = STATUS_SUCCESS;
    } else {
        status = WdfRequestForwardToIoQueue(Request, Context->PendingIn);
        if (NT_SUCCESS(status))
            status = STATUS_PENDING;
    }
    WdfSpinLockRelease(Slot->Lock);
    return status;
}

static NTSTATUS XusbSubmitUrb(PXUSB_PDO_CONTEXT Context, PVHID_SLOT Slot, WDFREQUEST Request, PURB Urb)
{
    NTSTATUS status;

    switch (Urb->UrbHeader.Function) {
    case URB_FUNCTION_GET_DESCRIPTOR_FROM_DEVICE:
        status = XusbGetDescriptor(Context, Urb);
        break;
    case URB_FUNCTION_SELECT_CONFIGURATION:
        status = XusbSelectConfiguration(Urb);
        break;
    case URB_FUNCTION_SELECT_INTERFACE:
        status = XusbSelectInterface(Urb);
        break;
    case URB_FUNCTION_CONTROL_TRANSFER:
        status = XusbControlTransfer(Urb);
        break;
    case URB_FUNCTION_BULK_OR_INTERRUPT_TRANSFER:
        status = XusbInterruptTransfer(Context, Slot, Request, Urb);
        break;
    case URB_FUNCTION_ABORT_PIPE:
        WdfIoQueuePurge(Context->PendingIn, NULL, NULL);
        WdfIoQueueStart(Context->PendingIn);
        status = STATUS_SUCCESS;
        break;
    case URB_FUNCTION_GET_STATUS_FROM_DEVICE:
    case URB_FUNCTION_SYNC_RESET_PIPE_AND_CLEAR_STALL:
    case URB_FUNCTION_SYNC_RESET_PIPE:
    case URB_FUNCTION_SYNC_CLEAR_STALL:
        status = STATUS_SUCCESS;
        break;
    case URB_FUNCTION_CONTROL_TRANSFER_EX:
        status = STATUS_UNSUCCESSFUL;
        break;
    case URB_FUNCTION_CLASS_INTERFACE:
    case URB_FUNCTION_GET_DESCRIPTOR_FROM_INTERFACE:
        status = STATUS_NOT_IMPLEMENTED;
        break;
    default:
        status = STATUS_SUCCESS;
        break;
    }

    if (status != STATUS_PENDING && NT_SUCCESS(status))
        Urb->UrbHeader.Status = USBD_STATUS_SUCCESS;
    else if (status != STATUS_PENDING && Urb->UrbHeader.Status == USBD_STATUS_SUCCESS)
        Urb->UrbHeader.Status = USBD_STATUS_INVALID_URB_FUNCTION;
    return status;
}

// ============================================================================
// Internal device control from xusb22
// ============================================================================

VOID XusbEvtInternalDeviceControl(
    IN WDFQUEUE     Queue,
    IN WDFREQUEST   Request,
    IN size_t       OutputBufferLength,
    IN size_t       InputBufferLength,
    IN ULONG        IoControlCode)
{
    WDFDEVICE           pdo = WdfIoQueueGetDevice(Queue);
    PXUSB_PDO_CONTEXT   context = XusbGetPdoContext(pdo);
    PVHID_SLOT          slot = context->Slot;
    NTSTATUS            status;

    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);

    if (slot == NULL) {
        WdfRequestComplete(Request, STATUS_DEVICE_NOT_CONNECTED);
        return;
    }

    switch (IoControlCode) {
    case IOCTL_INTERNAL_USB_SUBMIT_URB:
    {
        PURB urb = XusbUrbOf(Request);
        if (urb == NULL) {
            status = STATUS_INVALID_PARAMETER;
            break;
        }
        status = XusbSubmitUrb(context, slot, Request, urb);
        if (status == STATUS_PENDING)
            return;
        break;
    }

    case IOCTL_INTERNAL_USB_GET_PORT_STATUS:
    {
        PIRP irp = WdfRequestWdmGetIrp(Request);
        PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(irp);
        PULONG portStatus = (PULONG)stack->Parameters.Others.Argument1;
        if (portStatus != NULL)
            *portStatus = USBD_PORT_ENABLED | USBD_PORT_CONNECTED;
        status = STATUS_SUCCESS;
        break;
    }

    case IOCTL_INTERNAL_USB_RESET_PORT:
    case IOCTL_INTERNAL_USB_SUBMIT_IDLE_NOTIFICATION:
        status = STATUS_SUCCESS;
        break;

    default:
        status = STATUS_INVALID_DEVICE_REQUEST;
        break;
    }

    WdfRequestComplete(Request, status);
}

// ============================================================================
// Input from user mode
// ============================================================================

NTSTATUS XusbSubmitInput(PVHID_SLOT Slot, PUCHAR Data, USHORT Length)
{
    PXUSB_PDO_CONTEXT   context;
    WDFDEVICE           pdo;
    WDFREQUEST          pending = NULL;
    NTSTATUS            status;

    if (Length != VHID_XUSB_INPUT_LENGTH)
        return STATUS_INVALID_PARAMETER;

    WdfSpinLockAcquire(Slot->Lock);
    pdo = Slot->Pdo;
    if (pdo == NULL) {
        WdfSpinLockRelease(Slot->Lock);
        return STATUS_DEVICE_NOT_CONNECTED;
    }
    context = XusbGetPdoContext(pdo);
    RtlCopyMemory(context->Input, Data, VHID_XUSB_INPUT_LENGTH);
    RtlCopyMemory(Slot->LastInput, Data, VHID_XUSB_INPUT_LENGTH);
    Slot->LastInputLength = VHID_XUSB_INPUT_LENGTH;

    status = WdfIoQueueRetrieveNextRequest(context->PendingIn, &pending);
    if (NT_SUCCESS(status)) {
        PURB urb = XusbUrbOf(pending);
        if (urb != NULL)
            XusbFillInterruptIn(urb, context->Input, VHID_XUSB_INPUT_LENGTH);
        context->InputFresh = FALSE;
    } else {
        context->InputFresh = TRUE;
        pending = NULL;
    }
    WdfSpinLockRelease(Slot->Lock);

    if (pending != NULL)
        WdfRequestComplete(pending, STATUS_SUCCESS);
    return STATUS_SUCCESS;
}
