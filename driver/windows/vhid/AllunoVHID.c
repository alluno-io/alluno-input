/*
 * AllunoVHID: virtual HID devices over the Virtual HID Framework.
 *
 * One root-enumerated software device owns up to VHID_MAX_DEVICES slots. A
 * user-mode client opens the device interface, plugs a slot with a report
 * descriptor and a feature table, streams input reports into it and pends a
 * WAIT_OUTPUT request to receive what the HID stack writes back. Everything a
 * file handle plugged is unplugged when that handle goes away, so a client
 * that crashes leaves no ghost controllers behind.
 */

#include "AllunoVHID.h"

#ifdef ALLOC_PRAGMA
#pragma alloc_text(INIT, DriverEntry)
#pragma alloc_text(PAGE, VhidEvtDeviceAdd)
#pragma alloc_text(PAGE, VhidEvtDeviceCleanup)
#pragma alloc_text(PAGE, VhidEvtFileCleanup)
#endif

static NTSTATUS VhidPlug(PVHID_DEVICE_CONTEXT Context, WDFFILEOBJECT Owner, PVOID Buffer, size_t Length, PULONG SlotOut);
static NTSTATUS VhidUnplug(PVHID_DEVICE_CONTEXT Context, ULONG Index, WDFFILEOBJECT Owner);
static VOID VhidUnplugOwned(PVHID_DEVICE_CONTEXT Context, WDFFILEOBJECT Owner);
static PVHID_SLOT VhidSlotForRequest(PVHID_DEVICE_CONTEXT Context, ULONG Index, WDFREQUEST Request);
static BOOLEAN VhidCompleteWaiter(PVHID_SLOT Slot);

static USHORT VhidClampReport(ULONG Length)
{
    return (USHORT)(Length > VHID_MAX_REPORT ? VHID_MAX_REPORT : Length);
}

// ============================================================================
// DriverEntry and device creation
// ============================================================================

NTSTATUS DriverEntry(
    IN PDRIVER_OBJECT  DriverObject,
    IN PUNICODE_STRING RegistryPath)
{
    WDF_DRIVER_CONFIG config;

    WDF_DRIVER_CONFIG_INIT(&config, VhidEvtDeviceAdd);
    return WdfDriverCreate(DriverObject, RegistryPath,
        WDF_NO_OBJECT_ATTRIBUTES, &config, WDF_NO_HANDLE);
}

NTSTATUS VhidEvtDeviceAdd(
    IN WDFDRIVER        Driver,
    IN PWDFDEVICE_INIT  DeviceInit)
{
    WDF_OBJECT_ATTRIBUTES   attributes;
    WDF_FILEOBJECT_CONFIG   fileConfig;
    WDF_IO_QUEUE_CONFIG     queueConfig;
    PVHID_DEVICE_CONTEXT    context;
    WDFDEVICE               device;
    NTSTATUS                status;
    ULONG                   i;

    UNREFERENCED_PARAMETER(Driver);
    PAGED_CODE();

    WdfDeviceInitSetDeviceType(DeviceInit, VHID_DEVICE_TYPE);
    WdfDeviceInitSetIoType(DeviceInit, WdfDeviceIoBuffered);

    WDF_FILEOBJECT_CONFIG_INIT(&fileConfig, WDF_NO_EVENT_CALLBACK, WDF_NO_EVENT_CALLBACK, VhidEvtFileCleanup);
    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, VHID_FILE_CONTEXT);
    WdfDeviceInitSetFileObjectConfig(DeviceInit, &fileConfig, &attributes);

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, VHID_DEVICE_CONTEXT);
    attributes.EvtCleanupCallback = VhidEvtDeviceCleanup;

    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status))
        return status;

    context = VhidGetDeviceContext(device);
    RtlZeroMemory(context, sizeof(VHID_DEVICE_CONTEXT));
    context->Device = device;

    status = WdfWaitLockCreate(WDF_NO_OBJECT_ATTRIBUTES, &context->SlotsLock);
    if (!NT_SUCCESS(status))
        return status;

    for (i = 0; i < VHID_MAX_DEVICES; i++) {
        WDF_IO_QUEUE_CONFIG waitersConfig;
        PVHID_SLOT slot = &context->Slots[i];

        slot->Index = i;

        status = WdfSpinLockCreate(WDF_NO_OBJECT_ATTRIBUTES, &slot->Lock);
        if (!NT_SUCCESS(status))
            return status;

        WDF_IO_QUEUE_CONFIG_INIT(&waitersConfig, WdfIoQueueDispatchManual);
        waitersConfig.PowerManaged = WdfFalse;
        status = WdfIoQueueCreate(device, &waitersConfig, WDF_NO_OBJECT_ATTRIBUTES, &slot->Waiters);
        if (!NT_SUCCESS(status))
            return status;
    }

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchParallel);
    queueConfig.EvtIoDeviceControl = VhidEvtIoDeviceControl;
    status = WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, WDF_NO_HANDLE);
    if (!NT_SUCCESS(status))
        return status;

    return WdfDeviceCreateDeviceInterface(device, &GUID_DEVINTERFACE_ALLUNO_VHID, NULL);
}

VOID VhidEvtDeviceCleanup(WDFOBJECT Device)
{
    PVHID_DEVICE_CONTEXT context = VhidGetDeviceContext((WDFDEVICE)Device);
    ULONG i;

    PAGED_CODE();

    for (i = 0; i < VHID_MAX_DEVICES; i++)
        VhidUnplug(context, i, NULL);
}

VOID VhidEvtFileCleanup(WDFFILEOBJECT FileObject)
{
    WDFDEVICE device = WdfFileObjectGetDevice(FileObject);

    PAGED_CODE();

    VhidUnplugOwned(VhidGetDeviceContext(device), FileObject);
}

// ============================================================================
// Slots
// ============================================================================

static NTSTATUS VhidPlug(
    PVHID_DEVICE_CONTEXT Context,
    WDFFILEOBJECT        Owner,
    PVOID                Buffer,
    size_t               Length,
    PULONG               SlotOut)
{
    VHID_PLUG_IN    header;
    PUCHAR          cursor = (PUCHAR)Buffer;
    PUCHAR          end = cursor + Length;
    PVHID_SLOT      slot = NULL;
    VHF_CONFIG      config;
    UNICODE_STRING  instanceId;
    NTSTATUS        status;
    ULONG           i;

    if (Length < sizeof(VHID_PLUG_IN))
        return STATUS_BUFFER_TOO_SMALL;

    RtlCopyMemory(&header, cursor, sizeof(header));
    cursor += sizeof(header);

    if (header.Kind != VHID_KIND_HID && header.Kind != VHID_KIND_XUSB)
        return STATUS_INVALID_PARAMETER;
    if (header.Kind == VHID_KIND_XUSB && (header.DescriptorLength != 0 || header.FeatureCount != 0))
        return STATUS_INVALID_PARAMETER;
    if (header.Kind == VHID_KIND_HID && (header.DescriptorLength == 0 || header.DescriptorLength > VHID_MAX_DESCRIPTOR))
        return STATUS_INVALID_PARAMETER;
    if (header.FeatureCount > VHID_MAX_FEATURES)
        return STATUS_INVALID_PARAMETER;
    if ((size_t)(end - cursor) < header.DescriptorLength)
        return STATUS_BUFFER_TOO_SMALL;

    WdfWaitLockAcquire(Context->SlotsLock, NULL);

    for (i = 0; i < VHID_MAX_DEVICES; i++) {
        if (!Context->Slots[i].Used) {
            slot = &Context->Slots[i];
            break;
        }
    }
    if (slot == NULL) {
        WdfWaitLockRelease(Context->SlotsLock);
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    slot->Used = TRUE;
    slot->Started = FALSE;
    slot->Kind = (UCHAR)header.Kind;
    slot->Owner = Owner;
    slot->Vhf = NULL;
    slot->Pdo = NULL;
    slot->FeatureCount = 0;
    slot->LastInputLength = 0;
    slot->OutputPending = FALSE;
    slot->OutputLength = 0;

    WdfWaitLockRelease(Context->SlotsLock);

    if (header.Kind == VHID_KIND_XUSB) {
        status = XusbPlug(Context, slot, header.VendorId, header.ProductId, header.VersionNumber);
        if (!NT_SUCCESS(status))
            goto fail;
        slot->Started = TRUE;
        *SlotOut = slot->Index;
        return STATUS_SUCCESS;
    }

    slot->Descriptor = (PUCHAR)ExAllocatePoolWithTag(NonPagedPoolNx, header.DescriptorLength, VHID_POOL_TAG);
    if (slot->Descriptor == NULL) {
        status = STATUS_INSUFFICIENT_RESOURCES;
        goto fail;
    }
    RtlCopyMemory(slot->Descriptor, cursor, header.DescriptorLength);
    slot->DescriptorLength = header.DescriptorLength;
    cursor += header.DescriptorLength;

    for (i = 0; i < header.FeatureCount; i++) {
        VHID_FEATURE_ENTRY entry;
        PVHID_FEATURE feature = &slot->Features[i];

        if ((size_t)(end - cursor) < sizeof(entry)) {
            status = STATUS_BUFFER_TOO_SMALL;
            goto fail;
        }
        RtlCopyMemory(&entry, cursor, sizeof(entry));
        cursor += sizeof(entry);
        if (entry.Length == 0 || entry.Length > VHID_MAX_REPORT || (size_t)(end - cursor) < entry.Length) {
            status = STATUS_INVALID_PARAMETER;
            goto fail;
        }
        feature->Id = cursor[0];
        feature->Length = entry.Length;
        RtlCopyMemory(feature->Data, cursor, entry.Length);
        cursor += entry.Length;
        slot->FeatureCount = i + 1;
    }

    RtlZeroMemory(slot->InstanceId, sizeof(slot->InstanceId));
    status = RtlStringCchPrintfW(slot->InstanceId, RTL_NUMBER_OF(slot->InstanceId), L"AllunoVHID%02u", slot->Index);
    if (!NT_SUCCESS(status))
        goto fail;
    RtlInitUnicodeString(&instanceId, slot->InstanceId);

    VHF_CONFIG_INIT(&config, WdfDeviceWdmGetDeviceObject(Context->Device), slot->DescriptorLength, slot->Descriptor);
    config.VhfClientContext = slot;
    config.VendorID = header.VendorId;
    config.ProductID = header.ProductId;
    config.VersionNumber = header.VersionNumber;
    config.InstanceID = slot->InstanceId;
    config.InstanceIDLength = instanceId.Length;
    config.EvtVhfAsyncOperationGetFeature = VhidEvtGetFeature;
    config.EvtVhfAsyncOperationSetFeature = VhidEvtSetFeature;
    config.EvtVhfAsyncOperationWriteReport = VhidEvtWriteReport;
    config.EvtVhfAsyncOperationGetInputReport = VhidEvtGetInputReport;
    config.EvtVhfCleanup = VhidEvtVhfCleanup;

    status = VhfCreate(&config, &slot->Vhf);
    if (!NT_SUCCESS(status))
        goto fail;

    status = VhfStart(slot->Vhf);
    if (!NT_SUCCESS(status))
        goto fail;

    slot->Started = TRUE;
    *SlotOut = slot->Index;
    return STATUS_SUCCESS;

fail:
    VhidUnplug(Context, slot->Index, NULL);
    return status;
}

static NTSTATUS VhidUnplug(PVHID_DEVICE_CONTEXT Context, ULONG Index, WDFFILEOBJECT Owner)
{
    PVHID_SLOT  slot;
    VHFHANDLE   vhf;
    WDFDEVICE   pdo;
    PUCHAR      descriptor;

    if (Index >= VHID_MAX_DEVICES)
        return STATUS_INVALID_PARAMETER;

    slot = &Context->Slots[Index];

    WdfWaitLockAcquire(Context->SlotsLock, NULL);
    if (!slot->Used || (Owner != NULL && slot->Owner != Owner)) {
        WdfWaitLockRelease(Context->SlotsLock);
        return STATUS_INVALID_PARAMETER;
    }
    slot->Used = FALSE;
    slot->Owner = NULL;
    WdfSpinLockAcquire(slot->Lock);
    slot->Started = FALSE;
    vhf = slot->Vhf;
    slot->Vhf = NULL;
    pdo = slot->Pdo;
    slot->Pdo = NULL;
    slot->FeatureCount = 0;
    slot->OutputPending = FALSE;
    WdfSpinLockRelease(slot->Lock);
    descriptor = slot->Descriptor;
    slot->Descriptor = NULL;
    slot->DescriptorLength = 0;
    WdfWaitLockRelease(Context->SlotsLock);

    WdfIoQueuePurgeSynchronously(slot->Waiters);
    WdfIoQueueStart(slot->Waiters);

    if (vhf != NULL)
        VhfDelete(vhf, TRUE);
    if (pdo != NULL)
        XusbUnplug(pdo);
    if (descriptor != NULL)
        ExFreePoolWithTag(descriptor, VHID_POOL_TAG);

    return STATUS_SUCCESS;
}

static VOID VhidUnplugOwned(PVHID_DEVICE_CONTEXT Context, WDFFILEOBJECT Owner)
{
    ULONG i;

    for (i = 0; i < VHID_MAX_DEVICES; i++)
        VhidUnplug(Context, i, Owner);
}

static PVHID_SLOT VhidSlotForRequest(PVHID_DEVICE_CONTEXT Context, ULONG Index, WDFREQUEST Request)
{
    PVHID_SLOT slot;

    if (Index >= VHID_MAX_DEVICES)
        return NULL;
    slot = &Context->Slots[Index];
    if (!slot->Used || !slot->Started || slot->Owner != WdfRequestGetFileObject(Request))
        return NULL;
    return slot;
}

// ============================================================================
// Output delivery: the latest report waits for the next WAIT_OUTPUT request
// ============================================================================

static BOOLEAN VhidCompleteWaiter(PVHID_SLOT Slot)
{
    WDFREQUEST      request;
    NTSTATUS        status;
    PVOID           buffer;
    size_t          length;
    VHID_OUTPUT_OUT header;

    status = WdfIoQueueRetrieveNextRequest(Slot->Waiters, &request);
    if (!NT_SUCCESS(status))
        return FALSE;

    header.Kind = Slot->OutputKind;
    header.Reserved = 0;
    header.Length = Slot->OutputLength;

    status = WdfRequestRetrieveOutputBuffer(request, sizeof(header), &buffer, &length);
    if (!NT_SUCCESS(status)) {
        WdfRequestComplete(request, status);
        return TRUE;
    }
    if (length < sizeof(header) + header.Length) {
        WdfRequestComplete(request, STATUS_BUFFER_TOO_SMALL);
        return TRUE;
    }

    RtlCopyMemory(buffer, &header, sizeof(header));
    RtlCopyMemory((PUCHAR)buffer + sizeof(header), Slot->Output, header.Length);
    Slot->OutputPending = FALSE;
    WdfRequestCompleteWithInformation(request, STATUS_SUCCESS, sizeof(header) + header.Length);
    return TRUE;
}

VOID VhidDeliverOutput(PVHID_SLOT Slot, UCHAR Kind, PUCHAR Data, USHORT Length)
{
    if (Length > VHID_MAX_REPORT)
        Length = VHID_MAX_REPORT;

    WdfSpinLockAcquire(Slot->Lock);
    Slot->OutputKind = Kind;
    Slot->OutputLength = Length;
    RtlCopyMemory(Slot->Output, Data, Length);
    Slot->OutputPending = TRUE;
    VhidCompleteWaiter(Slot);
    WdfSpinLockRelease(Slot->Lock);
}

// ============================================================================
// VHF callbacks
// ============================================================================

VOID VhidEvtGetFeature(
    PVOID               VhfClientContext,
    VHFOPERATIONHANDLE  VhfOperationHandle,
    PVOID               VhfOperationContext,
    PHID_XFER_PACKET    HidTransferPacket)
{
    PVHID_SLOT  slot = (PVHID_SLOT)VhfClientContext;
    NTSTATUS    status = STATUS_NOT_SUPPORTED;
    ULONG       i;

    UNREFERENCED_PARAMETER(VhfOperationContext);

    WdfSpinLockAcquire(slot->Lock);
    for (i = 0; i < slot->FeatureCount; i++) {
        PVHID_FEATURE feature = &slot->Features[i];
        if (feature->Id == HidTransferPacket->reportId) {
            ULONG copy = feature->Length;
            if (copy > HidTransferPacket->reportBufferLen)
                copy = HidTransferPacket->reportBufferLen;
            RtlCopyMemory(HidTransferPacket->reportBuffer, feature->Data, copy);
            status = STATUS_SUCCESS;
            break;
        }
    }
    WdfSpinLockRelease(slot->Lock);

    VhfAsyncOperationComplete(VhfOperationHandle, status);
}

VOID VhidEvtSetFeature(
    PVOID               VhfClientContext,
    VHFOPERATIONHANDLE  VhfOperationHandle,
    PVOID               VhfOperationContext,
    PHID_XFER_PACKET    HidTransferPacket)
{
    PVHID_SLOT  slot = (PVHID_SLOT)VhfClientContext;
    ULONG       i;

    UNREFERENCED_PARAMETER(VhfOperationContext);

    WdfSpinLockAcquire(slot->Lock);
    for (i = 0; i < slot->FeatureCount; i++) {
        PVHID_FEATURE feature = &slot->Features[i];
        if (feature->Id == HidTransferPacket->reportId) {
            ULONG copy = HidTransferPacket->reportBufferLen;
            if (copy > VHID_MAX_REPORT)
                copy = VHID_MAX_REPORT;
            RtlCopyMemory(feature->Data, HidTransferPacket->reportBuffer, copy);
            feature->Length = (USHORT)copy;
            break;
        }
    }
    WdfSpinLockRelease(slot->Lock);

    VhidDeliverOutput(slot, VHID_OUTPUT_KIND_FEATURE, HidTransferPacket->reportBuffer,
        VhidClampReport(HidTransferPacket->reportBufferLen));
    VhfAsyncOperationComplete(VhfOperationHandle, STATUS_SUCCESS);
}

VOID VhidEvtWriteReport(
    PVOID               VhfClientContext,
    VHFOPERATIONHANDLE  VhfOperationHandle,
    PVOID               VhfOperationContext,
    PHID_XFER_PACKET    HidTransferPacket)
{
    PVHID_SLOT slot = (PVHID_SLOT)VhfClientContext;

    UNREFERENCED_PARAMETER(VhfOperationContext);

    VhidDeliverOutput(slot, VHID_OUTPUT_KIND_REPORT, HidTransferPacket->reportBuffer,
        VhidClampReport(HidTransferPacket->reportBufferLen));
    VhfAsyncOperationComplete(VhfOperationHandle, STATUS_SUCCESS);
}

VOID VhidEvtGetInputReport(
    PVOID               VhfClientContext,
    VHFOPERATIONHANDLE  VhfOperationHandle,
    PVOID               VhfOperationContext,
    PHID_XFER_PACKET    HidTransferPacket)
{
    PVHID_SLOT  slot = (PVHID_SLOT)VhfClientContext;
    NTSTATUS    status = STATUS_NOT_SUPPORTED;

    UNREFERENCED_PARAMETER(VhfOperationContext);

    WdfSpinLockAcquire(slot->Lock);
    if (slot->LastInputLength > 0 && slot->LastInput[0] == HidTransferPacket->reportId) {
        ULONG copy = slot->LastInputLength;
        if (copy > HidTransferPacket->reportBufferLen)
            copy = HidTransferPacket->reportBufferLen;
        RtlCopyMemory(HidTransferPacket->reportBuffer, slot->LastInput, copy);
        status = STATUS_SUCCESS;
    }
    WdfSpinLockRelease(slot->Lock);

    VhfAsyncOperationComplete(VhfOperationHandle, status);
}

VOID VhidEvtVhfCleanup(PVOID VhfClientContext)
{
    UNREFERENCED_PARAMETER(VhfClientContext);
}

// ============================================================================
// IOCTLs
// ============================================================================

VOID VhidEvtIoDeviceControl(
    IN WDFQUEUE     Queue,
    IN WDFREQUEST   Request,
    IN size_t       OutputBufferLength,
    IN size_t       InputBufferLength,
    IN ULONG        IoControlCode)
{
    PVHID_DEVICE_CONTEXT    context = VhidGetDeviceContext(WdfIoQueueGetDevice(Queue));
    NTSTATUS                status = STATUS_SUCCESS;
    size_t                  information = 0;
    PVOID                   input = NULL;
    PVOID                   output = NULL;
    size_t                  inputLength = 0;
    size_t                  outputLength = 0;

    if (InputBufferLength > 0) {
        status = WdfRequestRetrieveInputBuffer(Request, InputBufferLength, &input, &inputLength);
        if (!NT_SUCCESS(status)) {
            WdfRequestComplete(Request, status);
            return;
        }
    }
    if (OutputBufferLength > 0) {
        status = WdfRequestRetrieveOutputBuffer(Request, OutputBufferLength, &output, &outputLength);
        if (!NT_SUCCESS(status)) {
            WdfRequestComplete(Request, status);
            return;
        }
    }

    switch (IoControlCode) {
    case IOCTL_VHID_VERSION:
    {
        VHID_VERSION_OUT version;
        if (outputLength < sizeof(version)) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }
        version.Version = VHID_API_VERSION;
        RtlCopyMemory(output, &version, sizeof(version));
        information = sizeof(version);
        break;
    }

    case IOCTL_VHID_PLUG:
    {
        VHID_PLUG_OUT   result;
        ULONG           slotIndex = 0;

        if (outputLength < sizeof(result)) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }
        status = VhidPlug(context, WdfRequestGetFileObject(Request), input, inputLength, &slotIndex);
        if (!NT_SUCCESS(status))
            break;
        result.Slot = slotIndex;
        RtlCopyMemory(output, &result, sizeof(result));
        information = sizeof(result);
        break;
    }

    case IOCTL_VHID_UNPLUG:
    {
        VHID_SLOT_IN in;
        if (inputLength < sizeof(in)) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }
        RtlCopyMemory(&in, input, sizeof(in));
        status = VhidUnplug(context, in.Slot, WdfRequestGetFileObject(Request));
        break;
    }

    case IOCTL_VHID_INPUT:
    {
        VHID_REPORT_IN  in;
        PVHID_SLOT      slot;
        HID_XFER_PACKET packet;
        PUCHAR          data;

        if (inputLength < sizeof(in)) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }
        RtlCopyMemory(&in, input, sizeof(in));
        if (in.Length == 0 || in.Length > VHID_MAX_REPORT || inputLength < sizeof(in) + in.Length) {
            status = STATUS_INVALID_PARAMETER;
            break;
        }
        slot = VhidSlotForRequest(context, in.Slot, Request);
        if (slot == NULL) {
            status = STATUS_DEVICE_NOT_CONNECTED;
            break;
        }
        data = (PUCHAR)input + sizeof(in);

        if (slot->Kind == VHID_KIND_XUSB) {
            status = XusbSubmitInput(slot, data, in.Length);
            information = NT_SUCCESS(status) ? in.Length : 0;
            break;
        }

        WdfSpinLockAcquire(slot->Lock);
        if (slot->Vhf == NULL) {
            status = STATUS_DEVICE_NOT_CONNECTED;
        } else {
            RtlCopyMemory(slot->LastInput, data, in.Length);
            slot->LastInputLength = in.Length;
            packet.reportBuffer = slot->LastInput;
            packet.reportBufferLen = in.Length;
            packet.reportId = slot->LastInput[0];
            status = VhfReadReportSubmit(slot->Vhf, &packet);
        }
        WdfSpinLockRelease(slot->Lock);

        information = NT_SUCCESS(status) ? in.Length : 0;
        break;
    }

    case IOCTL_VHID_WAIT_OUTPUT:
    {
        VHID_SLOT_IN    in;
        PVHID_SLOT      slot;

        if (inputLength < sizeof(in) || outputLength < sizeof(VHID_OUTPUT_OUT)) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }
        RtlCopyMemory(&in, input, sizeof(in));
        slot = VhidSlotForRequest(context, in.Slot, Request);
        if (slot == NULL) {
            status = STATUS_DEVICE_NOT_CONNECTED;
            break;
        }

        WdfSpinLockAcquire(slot->Lock);
        status = WdfRequestForwardToIoQueue(Request, slot->Waiters);
        if (NT_SUCCESS(status) && slot->OutputPending)
            VhidCompleteWaiter(slot);
        WdfSpinLockRelease(slot->Lock);

        if (NT_SUCCESS(status))
            return;
        break;
    }

    case IOCTL_VHID_SET_FEATURE:
    {
        VHID_REPORT_IN  in;
        PVHID_SLOT      slot;
        PUCHAR          data;
        ULONG           i;
        BOOLEAN         found = FALSE;

        if (inputLength < sizeof(in)) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }
        RtlCopyMemory(&in, input, sizeof(in));
        if (in.Length == 0 || in.Length > VHID_MAX_REPORT || inputLength < sizeof(in) + in.Length) {
            status = STATUS_INVALID_PARAMETER;
            break;
        }
        slot = VhidSlotForRequest(context, in.Slot, Request);
        if (slot == NULL) {
            status = STATUS_DEVICE_NOT_CONNECTED;
            break;
        }
        if (slot->Kind == VHID_KIND_XUSB) {
            status = STATUS_NOT_SUPPORTED;
            break;
        }
        data = (PUCHAR)input + sizeof(in);

        WdfSpinLockAcquire(slot->Lock);
        for (i = 0; i < slot->FeatureCount; i++) {
            if (slot->Features[i].Id == data[0]) {
                RtlCopyMemory(slot->Features[i].Data, data, in.Length);
                slot->Features[i].Length = in.Length;
                found = TRUE;
                break;
            }
        }
        if (!found && slot->FeatureCount < VHID_MAX_FEATURES) {
            PVHID_FEATURE feature = &slot->Features[slot->FeatureCount];
            feature->Id = data[0];
            feature->Length = in.Length;
            RtlCopyMemory(feature->Data, data, in.Length);
            slot->FeatureCount++;
            found = TRUE;
        }
        WdfSpinLockRelease(slot->Lock);

        status = found ? STATUS_SUCCESS : STATUS_INSUFFICIENT_RESOURCES;
        break;
    }

    default:
        status = STATUS_INVALID_DEVICE_REQUEST;
        break;
    }

    WdfRequestCompleteWithInformation(Request, status, information);
}
