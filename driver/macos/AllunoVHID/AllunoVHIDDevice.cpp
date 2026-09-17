/*
 * One virtual HID node: the descriptor and identity the bus was handed, the
 * feature table it answers, the input reports it forwards to the HID stack
 * and the output reports it queues for the client to poll.
 */

#include <os/log.h>
#include <string.h>
#include <DriverKit/IOLib.h>
#include <DriverKit/IOBufferMemoryDescriptor.h>
#include <DriverKit/IOMemoryMap.h>
#include <DriverKit/OSData.h>
#include <DriverKit/OSDictionary.h>
#include <DriverKit/OSNumber.h>
#include <DriverKit/OSString.h>
#include <HIDDriverKit/HIDDriverKit.h>

#include "AllunoVHIDDevice.h"
#include "vhid_wire.h"

struct VhidFeature {
    uint8_t  id;
    uint16_t length;
    uint8_t  data[VHID_MAX_REPORT];
};

struct VhidOutput {
    uint8_t  kind;
    uint16_t length;
    uint8_t  data[VHID_MAX_REPORT];
};

struct AllunoVHIDDevice_IVars {
    uint16_t    vendorId;
    uint16_t    productId;
    uint16_t    versionNumber;
    uint16_t    descriptorLength;
    uint8_t     descriptor[VHID_MAX_DESCRIPTOR];
    uint32_t    featureCount;
    VhidFeature features[VHID_MAX_FEATURES];
    VhidOutput  outputs[VHID_OUTPUT_QUEUE];
    uint32_t    outputHead;
    uint32_t    outputCount;
    uint16_t    lastInputLength;
    uint8_t     lastInput[VHID_MAX_REPORT];
    IOLock*     lock;
};

bool AllunoVHIDDevice::init()
{
    if (!super::init())
        return false;
    ivars = IONewZero(AllunoVHIDDevice_IVars, 1);
    if (ivars == nullptr)
        return false;
    ivars->lock = IOLockAlloc();
    return ivars->lock != nullptr;
}

void AllunoVHIDDevice::free()
{
    if (ivars != nullptr) {
        if (ivars->lock != nullptr)
            IOLockFree(ivars->lock);
        IOSafeDeleteNULL(ivars, AllunoVHIDDevice_IVars, 1);
    }
    super::free();
}

kern_return_t IMPL(AllunoVHIDDevice, Start)
{
    kern_return_t ret = Start(provider, SUPERDISPATCH);
    if (ret != kIOReturnSuccess)
        return ret;
    RegisterService();
    return kIOReturnSuccess;
}

kern_return_t IMPL(AllunoVHIDDevice, Stop)
{
    return Stop(provider, SUPERDISPATCH);
}

bool AllunoVHIDDevice::handleStart(IOService* provider)
{
    return super::handleStart(provider);
}

bool AllunoVHIDDevice::configure(uint16_t vendorId, uint16_t productId, uint16_t versionNumber,
                                 const uint8_t* descriptor, uint16_t descriptorLength)
{
    if (descriptorLength == 0 || descriptorLength > VHID_MAX_DESCRIPTOR)
        return false;
    ivars->vendorId = vendorId;
    ivars->productId = productId;
    ivars->versionNumber = versionNumber;
    ivars->descriptorLength = descriptorLength;
    memcpy(ivars->descriptor, descriptor, descriptorLength);
    return true;
}

bool AllunoVHIDDevice::addFeature(const uint8_t* report, uint16_t length)
{
    if (length == 0 || length > VHID_MAX_REPORT)
        return false;
    IOLockLock(ivars->lock);
    VhidFeature* slot = nullptr;
    for (uint32_t i = 0; i < ivars->featureCount; i++) {
        if (ivars->features[i].id == report[0]) {
            slot = &ivars->features[i];
            break;
        }
    }
    if (slot == nullptr && ivars->featureCount < VHID_MAX_FEATURES)
        slot = &ivars->features[ivars->featureCount++];
    if (slot != nullptr) {
        slot->id = report[0];
        slot->length = length;
        memcpy(slot->data, report, length);
    }
    IOLockUnlock(ivars->lock);
    return slot != nullptr;
}

OSDictionaryPtr AllunoVHIDDevice::newDeviceDescription(void)
{
    OSDictionary* description = OSDictionary::withCapacity(8);
    if (description == nullptr)
        return nullptr;

    OSNumber* vendor = OSNumber::withNumber(ivars->vendorId, 16);
    OSNumber* product = OSNumber::withNumber(ivars->productId, 16);
    OSNumber* version = OSNumber::withNumber(ivars->versionNumber, 16);
    OSString* transport = OSString::withCString("USB");
    OSString* manufacturer = OSString::withCString("Alluno");
    OSString* productName = OSString::withCString("Alluno Virtual Device");
    OSNumber* requestTimeout = OSNumber::withNumber(5000000ULL, 32);

    if (vendor) { description->setObject(kIOHIDVendorIDKey, vendor); vendor->release(); }
    if (product) { description->setObject(kIOHIDProductIDKey, product); product->release(); }
    if (version) { description->setObject(kIOHIDVersionNumberKey, version); version->release(); }
    if (transport) { description->setObject(kIOHIDTransportKey, transport); transport->release(); }
    if (manufacturer) { description->setObject(kIOHIDManufacturerKey, manufacturer); manufacturer->release(); }
    if (productName) { description->setObject(kIOHIDProductKey, productName); productName->release(); }
    if (requestTimeout) { description->setObject(kIOHIDRequestTimeoutKey, requestTimeout); requestTimeout->release(); }

    return description;
}

OSData* AllunoVHIDDevice::newReportDescriptor(void)
{
    return OSData::withBytes(ivars->descriptor, ivars->descriptorLength);
}

kern_return_t AllunoVHIDDevice::submitInput(const uint8_t* report, uint16_t length)
{
    if (length == 0 || length > VHID_MAX_REPORT)
        return kIOReturnBadArgument;

    IOBufferMemoryDescriptor* buffer = nullptr;
    kern_return_t ret = IOBufferMemoryDescriptor::Create(kIOMemoryDirectionInOut, length, 0, &buffer);
    if (ret != kIOReturnSuccess || buffer == nullptr)
        return ret;

    uint64_t address = 0;
    uint64_t mapped = 0;
    ret = buffer->Map(0, 0, 0, 0, &address, &mapped);
    if (ret == kIOReturnSuccess && mapped >= length) {
        memcpy(reinterpret_cast<void*>(address), report, length);
        IOLockLock(ivars->lock);
        ivars->lastInputLength = length;
        memcpy(ivars->lastInput, report, length);
        IOLockUnlock(ivars->lock);
        ret = handleReport(mach_absolute_time(), buffer, length, kIOHIDReportTypeInput, 0);
    }
    buffer->release();
    return ret;
}

kern_return_t AllunoVHIDDevice::getReport(IOMemoryDescriptor* report,
                                          IOHIDReportType reportType,
                                          IOOptionBits options,
                                          uint32_t completionTimeout,
                                          OSAction* action)
{
    uint8_t reportId = options & 0xFF;
    uint64_t address = 0;
    uint64_t length = 0;
    IOMemoryMap* map = nullptr;
    kern_return_t ret = report->CreateMapping(0, 0, 0, 0, 0, &map);
    if (ret != kIOReturnSuccess || map == nullptr)
        return ret;
    address = map->GetAddress();
    length = map->GetLength();

    ret = kIOReturnUnsupported;
    IOLockLock(ivars->lock);
    if (reportType == kIOHIDReportTypeFeature) {
        for (uint32_t i = 0; i < ivars->featureCount; i++) {
            VhidFeature* feature = &ivars->features[i];
            if (feature->id == reportId) {
                uint64_t copy = feature->length < length ? feature->length : length;
                memcpy(reinterpret_cast<void*>(address), feature->data, copy);
                ret = kIOReturnSuccess;
                break;
            }
        }
    } else if (reportType == kIOHIDReportTypeInput && ivars->lastInputLength > 0
               && ivars->lastInput[0] == reportId) {
        uint64_t copy = ivars->lastInputLength < length ? ivars->lastInputLength : length;
        memcpy(reinterpret_cast<void*>(address), ivars->lastInput, copy);
        ret = kIOReturnSuccess;
    }
    IOLockUnlock(ivars->lock);

    map->release();
    if (action != nullptr)
        CompleteReport(action, ret, ret == kIOReturnSuccess ? static_cast<uint32_t>(length) : 0);
    return ret;
}

kern_return_t AllunoVHIDDevice::setReport(IOMemoryDescriptor* report,
                                          IOHIDReportType reportType,
                                          IOOptionBits options,
                                          uint32_t completionTimeout,
                                          OSAction* action)
{
    IOMemoryMap* map = nullptr;
    kern_return_t ret = report->CreateMapping(0, 0, 0, 0, 0, &map);
    if (ret != kIOReturnSuccess || map == nullptr)
        return ret;
    const uint8_t* bytes = reinterpret_cast<const uint8_t*>(map->GetAddress());
    uint64_t length = map->GetLength();
    if (length > VHID_MAX_REPORT)
        length = VHID_MAX_REPORT;

    IOLockLock(ivars->lock);
    if (reportType == kIOHIDReportTypeFeature) {
        for (uint32_t i = 0; i < ivars->featureCount; i++) {
            VhidFeature* feature = &ivars->features[i];
            if (feature->id == bytes[0]) {
                feature->length = static_cast<uint16_t>(length);
                memcpy(feature->data, bytes, length);
                break;
            }
        }
    }
    if (ivars->outputCount == VHID_OUTPUT_QUEUE) {
        ivars->outputHead = (ivars->outputHead + 1) % VHID_OUTPUT_QUEUE;
        ivars->outputCount--;
    }
    VhidOutput* out = &ivars->outputs[(ivars->outputHead + ivars->outputCount) % VHID_OUTPUT_QUEUE];
    out->kind = reportType == kIOHIDReportTypeFeature ? VHID_OUTPUT_KIND_FEATURE : VHID_OUTPUT_KIND_REPORT;
    out->length = static_cast<uint16_t>(length);
    memcpy(out->data, bytes, length);
    ivars->outputCount++;
    IOLockUnlock(ivars->lock);

    map->release();
    if (action != nullptr)
        CompleteReport(action, kIOReturnSuccess, static_cast<uint32_t>(length));
    return kIOReturnSuccess;
}

uint16_t AllunoVHIDDevice::takeOutput(uint8_t* kind, uint8_t* buffer, uint16_t capacity)
{
    uint16_t length = 0;
    IOLockLock(ivars->lock);
    if (ivars->outputCount > 0) {
        VhidOutput* out = &ivars->outputs[ivars->outputHead];
        length = out->length < capacity ? out->length : capacity;
        *kind = out->kind;
        memcpy(buffer, out->data, length);
        ivars->outputHead = (ivars->outputHead + 1) % VHID_OUTPUT_QUEUE;
        ivars->outputCount--;
    }
    IOLockUnlock(ivars->lock);
    return length;
}
