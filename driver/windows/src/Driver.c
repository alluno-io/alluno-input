/*
 * AllunoInput - KMDF upper filter driver for keyboard/mouse.
 *
 * Hooks the class service callback via IOCTL_INTERNAL_*_CONNECT
 * for kernel-level input emulation via the native input stack.
 */

#include "Driver.h"

WDFCOLLECTION FilterDeviceCollection;
WDFWAITLOCK   FilterDeviceCollectionLock;
WDFDEVICE     ControlDevice = NULL;

#ifdef ALLOC_PRAGMA
#pragma alloc_text(INIT, DriverEntry)
#pragma alloc_text(PAGE, EvtDeviceAdd)
#pragma alloc_text(PAGE, EvtDeviceCleanup)
#pragma alloc_text(PAGE, CreateControlDevice)
#pragma alloc_text(PAGE, DeleteControlDevice)
#endif

// ============================================================================
// DriverEntry
// ============================================================================

NTSTATUS DriverEntry(
    IN PDRIVER_OBJECT  DriverObject,
    IN PUNICODE_STRING RegistryPath)
{
    WDF_DRIVER_CONFIG config;
    NTSTATUS status;

    WDF_DRIVER_CONFIG_INIT(&config, EvtDeviceAdd);

    status = WdfDriverCreate(DriverObject, RegistryPath,
        WDF_NO_OBJECT_ATTRIBUTES, &config, WDF_NO_HANDLE);
    if (!NT_SUCCESS(status))
        return status;

    status = WdfCollectionCreate(WDF_NO_OBJECT_ATTRIBUTES, &FilterDeviceCollection);
    if (!NT_SUCCESS(status))
        return status;

    status = WdfWaitLockCreate(WDF_NO_OBJECT_ATTRIBUTES, &FilterDeviceCollectionLock);
    return status;
}

// ============================================================================
// EvtDeviceAdd - called for each keyboard/mouse device
// ============================================================================

NTSTATUS EvtDeviceAdd(
    IN WDFDRIVER        Driver,
    IN PWDFDEVICE_INIT  DeviceInit)
{
    WDF_OBJECT_ATTRIBUTES   deviceAttributes;
    NTSTATUS                status;
    WDFDEVICE               hDevice;
    WDF_IO_QUEUE_CONFIG     ioQueueConfig;

    UNREFERENCED_PARAMETER(Driver);
    PAGED_CODE();

    // This is the key: WDF auto-passes all IRPs we don't handle
    WdfFdoInitSetFilter(DeviceInit);
    WdfDeviceInitSetDeviceType(DeviceInit, ALLUNO_DEVICE_TYPE);

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&deviceAttributes, FILTER_EXTENSION);
    deviceAttributes.EvtCleanupCallback = EvtDeviceCleanup;

    status = WdfDeviceCreate(&DeviceInit, &deviceAttributes, &hDevice);
    if (!NT_SUCCESS(status))
        return status;

    {
        PFILTER_EXTENSION filterExt = FilterGetData(hDevice);
        filterExt->UpperConnectData.ClassDeviceObject = NULL;
        filterExt->UpperConnectData.ClassService = NULL;
    }

    // Parallel queue for InternalDeviceControl
    // (PS/2 port driver requires parallel dispatch to avoid deadlock)
    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&ioQueueConfig, WdfIoQueueDispatchParallel);
    ioQueueConfig.EvtIoInternalDeviceControl = EvtIoInternalDeviceControl;

    status = WdfIoQueueCreate(hDevice, &ioQueueConfig,
        WDF_NO_OBJECT_ATTRIBUTES, WDF_NO_HANDLE);
    if (!NT_SUCCESS(status))
        return status;

    // Track filter devices
    WdfWaitLockAcquire(FilterDeviceCollectionLock, NULL);
    status = WdfCollectionAdd(FilterDeviceCollection, hDevice);
    WdfWaitLockRelease(FilterDeviceCollectionLock);
    if (!NT_SUCCESS(status))
        return status;

    // Create control device for user-mode IOCTLs (once, on first device)
    CreateControlDevice(hDevice);

    return STATUS_SUCCESS;
}

// ============================================================================
// EvtDeviceCleanup - remove from collection, delete control device if last
// ============================================================================

VOID EvtDeviceCleanup(WDFOBJECT Device)
{
    ULONG count;

    PAGED_CODE();

    WdfWaitLockAcquire(FilterDeviceCollectionLock, NULL);

    count = WdfCollectionGetCount(FilterDeviceCollection);
    if (count == 1)
        DeleteControlDevice((WDFDEVICE)Device);

    WdfCollectionRemove(FilterDeviceCollection, Device);

    WdfWaitLockRelease(FilterDeviceCollectionLock);
}

// ============================================================================
// Control device
// ============================================================================

NTSTATUS CreateControlDevice(WDFDEVICE Device)
{
    PWDFDEVICE_INIT         pInit = NULL;
    WDFDEVICE               controlDev = NULL;
    WDF_OBJECT_ATTRIBUTES   controlAttributes;
    WDF_IO_QUEUE_CONFIG     ioQueueConfig;
    NTSTATUS                status;
    WDFQUEUE                queue;
    DECLARE_CONST_UNICODE_STRING(ntDeviceName, ALLUNO_NTDEVICE_NAME);
    DECLARE_CONST_UNICODE_STRING(symbolicLinkName, ALLUNO_SYMBOLIC_NAME);

    PAGED_CODE();

    // Only create once
    if (ControlDevice != NULL)
        return STATUS_SUCCESS;

    pInit = WdfControlDeviceInitAllocate(
        WdfDeviceGetDriver(Device),
        &SDDL_DEVOBJ_SYS_ALL_ADM_RWX_WORLD_RW_RES_R);
    if (pInit == NULL)
        return STATUS_INSUFFICIENT_RESOURCES;

    WdfDeviceInitSetExclusive(pInit, FALSE);

    status = WdfDeviceInitAssignName(pInit, &ntDeviceName);
    if (!NT_SUCCESS(status)) {
        WdfDeviceInitFree(pInit);
        return status;
    }

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&controlAttributes, CONTROL_EXTENSION);

    status = WdfDeviceCreate(&pInit, &controlAttributes, &controlDev);
    if (!NT_SUCCESS(status))
        return status;

    status = WdfDeviceCreateSymbolicLink(controlDev, &symbolicLinkName);
    if (!NT_SUCCESS(status)) {
        WdfObjectDelete(controlDev);
        return status;
    }

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&ioQueueConfig, WdfIoQueueDispatchSequential);
    ioQueueConfig.EvtIoDeviceControl = EvtIoDeviceControl;

    status = WdfIoQueueCreate(controlDev, &ioQueueConfig,
        WDF_NO_OBJECT_ATTRIBUTES, &queue);
    if (!NT_SUCCESS(status)) {
        WdfObjectDelete(controlDev);
        return status;
    }

    WdfControlFinishInitializing(controlDev);
    ControlDevice = controlDev;

    return STATUS_SUCCESS;
}

VOID DeleteControlDevice(WDFDEVICE Device)
{
    UNREFERENCED_PARAMETER(Device);
    PAGED_CODE();

    if (ControlDevice) {
        WdfObjectDelete(ControlDevice);
        ControlDevice = NULL;
    }
}

// ============================================================================
// InternalDeviceControl - hook the CONNECT callback
// ============================================================================

VOID EvtIoInternalDeviceControl(
    IN WDFQUEUE     Queue,
    IN WDFREQUEST   Request,
    IN size_t       OutputBufferLength,
    IN size_t       InputBufferLength,
    IN ULONG        IoControlCode)
{
    PFILTER_EXTENSION       filterExt;
    NTSTATUS                status = STATUS_SUCCESS;
    WDFDEVICE               hDevice;
    BOOLEAN                 forwardWithFlush = FALSE;
    WDF_REQUEST_SEND_OPTIONS options;
    BOOLEAN                 ret;

    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);

    hDevice = WdfIoQueueGetDevice(Queue);
    filterExt = FilterGetData(hDevice);

    switch (IoControlCode) {
    case ALLUNO_CONNECT_IOCTL:
    {
        PCONNECT_DATA connectData = NULL;
        size_t length;

        // Only allow one connection
        if (filterExt->UpperConnectData.ClassService != NULL) {
            status = STATUS_SHARING_VIOLATION;
            break;
        }

        status = WdfRequestRetrieveInputBuffer(Request,
            sizeof(CONNECT_DATA),
            (PVOID *)&connectData,
            &length);
        if (!NT_SUCCESS(status)) {
            break;
        }

        // Save original class driver callback
        filterExt->UpperConnectData = *connectData;

        // Substitute our callback
        connectData->ClassDeviceObject = WdfDeviceWdmGetDeviceObject(hDevice);

        #pragma warning(disable:4152) // function/data pointer conversion
        connectData->ClassService = ServiceCallback;
        #pragma warning(default:4152)

        forwardWithFlush = TRUE;
        break;
    }

    case ALLUNO_DISCONNECT_IOCTL:
        status = STATUS_NOT_IMPLEMENTED;
        break;

    default:
        forwardWithFlush = TRUE;
        break;
    }

    if (!NT_SUCCESS(status)) {
        WdfRequestComplete(Request, status);
        return;
    }

    // Forward: fire and forget
    WDF_REQUEST_SEND_OPTIONS_INIT(&options, WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET);
    ret = WdfRequestSend(Request, WdfDeviceGetIoTarget(hDevice), &options);
    if (ret == FALSE) {
        status = WdfRequestGetStatus(Request);
        WdfRequestComplete(Request, status);
    }
}

// ============================================================================
// Service callback - called at DISPATCH_LEVEL when hardware input arrives
// ============================================================================

VOID ServiceCallback(
    IN PDEVICE_OBJECT  DeviceObject,
    IN PVOID           InputDataStart,
    IN PVOID           InputDataEnd,
    IN OUT PULONG      InputDataConsumed)
{
    WDFDEVICE           device;
    PFILTER_EXTENSION   filterExt;

    device = WdfWdmDeviceGetWdfDeviceHandle(DeviceObject);
    filterExt = FilterGetData(device);

    // Forward to original class driver callback (passthrough)
    if (filterExt->UpperConnectData.ClassService != NULL) {
        ((PSERVICE_CALLBACK_ROUTINE)(ULONG_PTR)filterExt->UpperConnectData.ClassService)(
            filterExt->UpperConnectData.ClassDeviceObject,
            InputDataStart,
            InputDataEnd,
            InputDataConsumed);
    }
}

// ============================================================================
// Control device IOCTLs - injection from user-mode
// ============================================================================

VOID EvtIoDeviceControl(
    IN WDFQUEUE     Queue,
    IN WDFREQUEST   Request,
    IN size_t       OutputBufferLength,
    IN size_t       InputBufferLength,
    IN ULONG        IoControlCode)
{
    NTSTATUS            status = STATUS_SUCCESS;
    size_t              bytesTransferred = 0;
    PVOID               inputData;
    size_t              bufferSize;

    UNREFERENCED_PARAMETER(Queue);
    UNREFERENCED_PARAMETER(OutputBufferLength);

    switch (IoControlCode) {
    case IOCTL_ALLUNO_SEND:
    {
        PFILTER_EXTENSION filterExt;
        WDFDEVICE         hFilterDevice;

        if (InputBufferLength < ALLUNO_INPUT_DATA_SIZE) {
            status = STATUS_BUFFER_TOO_SMALL;
            break;
        }

        status = WdfRequestRetrieveInputBuffer(Request, ALLUNO_INPUT_DATA_SIZE,
            &inputData, &bufferSize);
        if (!NT_SUCCESS(status))
            break;

        // Send to all connected filter devices
        {
            ULONG i, count;
            size_t inputCount = bufferSize / ALLUNO_INPUT_DATA_SIZE;
            PVOID end = (PUCHAR)inputData + inputCount * ALLUNO_INPUT_DATA_SIZE;
            KIRQL oldIrql;

            WdfWaitLockAcquire(FilterDeviceCollectionLock, NULL);
            count = WdfCollectionGetCount(FilterDeviceCollection);
            WdfWaitLockRelease(FilterDeviceCollectionLock);

            if (count == 0) {
                status = STATUS_DEVICE_NOT_CONNECTED;
                break;
            }

            KeRaiseIrql(DISPATCH_LEVEL, &oldIrql);

            for (i = 0; i < count; i++) {
                ULONG consumed = 0;

                WdfWaitLockAcquire(FilterDeviceCollectionLock, NULL);
                hFilterDevice = WdfCollectionGetItem(FilterDeviceCollection, i);
                WdfWaitLockRelease(FilterDeviceCollectionLock);

                filterExt = FilterGetData(hFilterDevice);
                if (filterExt->UpperConnectData.ClassService != NULL) {
                    ((PSERVICE_CALLBACK_ROUTINE)(ULONG_PTR)filterExt->UpperConnectData.ClassService)(
                        filterExt->UpperConnectData.ClassDeviceObject,
                        inputData,
                        end,
                        &consumed);
                    bytesTransferred += consumed * ALLUNO_INPUT_DATA_SIZE;
                    break;
                }
            }

            KeLowerIrql(oldIrql);
        }
        break;
    }

    default:
        status = STATUS_INVALID_DEVICE_REQUEST;
        break;
    }

    WdfRequestCompleteWithInformation(Request, status, bytesTransferred);
}
