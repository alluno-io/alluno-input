/*
 * AllunoVHID on macOS: the DriverKit service that owns the slots. It matches
 * IOUserResources, so it needs no hardware, and hands each connecting app a
 * user client. Every slot a user client plugged is unplugged when that client
 * goes away, so a crashed host leaves no ghost controllers behind.
 */

#include <os/log.h>
#include <string.h>
#include <DriverKit/IOLib.h>
#include <DriverKit/IOUserClient.h>
#include <DriverKit/OSDictionary.h>

#include "AllunoVHIDBus.h"
#include "AllunoVHIDDevice.h"
#include "AllunoVHIDUserClient.h"
#include "vhid_wire.h"

struct VhidSlot {
    bool               used;
    void*              owner;
    AllunoVHIDDevice*  device;
};

struct AllunoVHIDBus_IVars {
    IOLock*  lock;
    VhidSlot slots[VHID_MAX_DEVICES];
};

bool AllunoVHIDBus::init()
{
    if (!super::init())
        return false;
    ivars = IONewZero(AllunoVHIDBus_IVars, 1);
    if (ivars == nullptr)
        return false;
    ivars->lock = IOLockAlloc();
    return ivars->lock != nullptr;
}

void AllunoVHIDBus::free()
{
    if (ivars != nullptr) {
        if (ivars->lock != nullptr)
            IOLockFree(ivars->lock);
        IOSafeDeleteNULL(ivars, AllunoVHIDBus_IVars, 1);
    }
    super::free();
}

kern_return_t IMPL(AllunoVHIDBus, Start)
{
    kern_return_t ret = Start(provider, SUPERDISPATCH);
    if (ret != kIOReturnSuccess)
        return ret;
    ret = RegisterService();
    if (ret != kIOReturnSuccess)
        Stop(provider, SUPERDISPATCH);
    return ret;
}

kern_return_t IMPL(AllunoVHIDBus, Stop)
{
    for (uint32_t i = 0; i < VHID_MAX_DEVICES; i++)
        unplug(i, nullptr);
    return Stop(provider, SUPERDISPATCH);
}

kern_return_t IMPL(AllunoVHIDBus, NewUserClient)
{
    IOService* client = nullptr;
    kern_return_t ret = Create(this, "UserClientProperties", &client);
    if (ret != kIOReturnSuccess)
        return ret;
    *userClient = OSDynamicCast(IOUserClient, client);
    if (*userClient == nullptr) {
        client->release();
        return kIOReturnError;
    }
    return kIOReturnSuccess;
}

kern_return_t AllunoVHIDBus::plug(const uint8_t* request, size_t length, uint32_t* slotOut, void* owner)
{
    if (length < sizeof(VhidPlugHeader))
        return kIOReturnBadArgument;

    VhidPlugHeader header;
    memcpy(&header, request, sizeof(header));
    const uint8_t* cursor = request + sizeof(header);
    const uint8_t* end = request + length;

    if (header.descriptorLength == 0 || header.descriptorLength > VHID_MAX_DESCRIPTOR)
        return kIOReturnBadArgument;
    if (header.featureCount > VHID_MAX_FEATURES)
        return kIOReturnBadArgument;
    if (static_cast<size_t>(end - cursor) < header.descriptorLength)
        return kIOReturnBadArgument;

    VhidSlot* slot = nullptr;
    uint32_t index = 0;
    IOLockLock(ivars->lock);
    for (uint32_t i = 0; i < VHID_MAX_DEVICES; i++) {
        if (!ivars->slots[i].used) {
            slot = &ivars->slots[i];
            index = i;
            slot->used = true;
            slot->owner = owner;
            slot->device = nullptr;
            break;
        }
    }
    IOLockUnlock(ivars->lock);
    if (slot == nullptr)
        return kIOReturnNoResources;

    IOService* created = nullptr;
    OSDictionary* properties = OSDictionary::withCapacity(1);
    kern_return_t ret = properties ? Create(this, "DeviceProperties", &created) : kIOReturnNoMemory;
    if (properties)
        properties->release();
    AllunoVHIDDevice* device = created ? OSDynamicCast(AllunoVHIDDevice, created) : nullptr;
    if (ret != kIOReturnSuccess || device == nullptr) {
        if (created)
            created->release();
        unplug(index, owner);
        return ret == kIOReturnSuccess ? kIOReturnError : ret;
    }

    if (!device->configure(header.vendorId, header.productId, header.versionNumber, cursor, header.descriptorLength)) {
        device->release();
        unplug(index, owner);
        return kIOReturnBadArgument;
    }
    cursor += header.descriptorLength;

    for (uint16_t i = 0; i < header.featureCount; i++) {
        VhidFeatureEntry entry;
        if (static_cast<size_t>(end - cursor) < sizeof(entry)) {
            device->release();
            unplug(index, owner);
            return kIOReturnBadArgument;
        }
        memcpy(&entry, cursor, sizeof(entry));
        cursor += sizeof(entry);
        if (entry.length == 0 || entry.length > VHID_MAX_REPORT || static_cast<size_t>(end - cursor) < entry.length) {
            device->release();
            unplug(index, owner);
            return kIOReturnBadArgument;
        }
        device->addFeature(cursor, entry.length);
        cursor += entry.length;
    }

    IOLockLock(ivars->lock);
    slot->device = device;
    IOLockUnlock(ivars->lock);

    ret = device->Start(this);
    if (ret != kIOReturnSuccess) {
        unplug(index, owner);
        return ret;
    }

    *slotOut = index;
    return kIOReturnSuccess;
}

kern_return_t AllunoVHIDBus::unplug(uint32_t index, void* owner)
{
    if (index >= VHID_MAX_DEVICES)
        return kIOReturnBadArgument;

    AllunoVHIDDevice* device = nullptr;
    IOLockLock(ivars->lock);
    VhidSlot* slot = &ivars->slots[index];
    if (!slot->used || (owner != nullptr && slot->owner != owner)) {
        IOLockUnlock(ivars->lock);
        return kIOReturnBadArgument;
    }
    device = slot->device;
    slot->device = nullptr;
    slot->owner = nullptr;
    slot->used = false;
    IOLockUnlock(ivars->lock);

    if (device != nullptr) {
        device->Terminate(0);
        device->release();
    }
    return kIOReturnSuccess;
}

void AllunoVHIDBus::unplugOwned(void* owner)
{
    for (uint32_t i = 0; i < VHID_MAX_DEVICES; i++)
        unplug(i, owner);
}

static AllunoVHIDDevice* deviceFor(AllunoVHIDBus_IVars* ivars, uint32_t index, void* owner)
{
    if (index >= VHID_MAX_DEVICES)
        return nullptr;
    AllunoVHIDDevice* device = nullptr;
    IOLockLock(ivars->lock);
    VhidSlot* slot = &ivars->slots[index];
    if (slot->used && slot->owner == owner && slot->device != nullptr) {
        device = slot->device;
        device->retain();
    }
    IOLockUnlock(ivars->lock);
    return device;
}

kern_return_t AllunoVHIDBus::input(uint32_t index, const uint8_t* report, uint16_t length, void* owner)
{
    AllunoVHIDDevice* device = deviceFor(ivars, index, owner);
    if (device == nullptr)
        return kIOReturnNotAttached;
    kern_return_t ret = device->submitInput(report, length);
    device->release();
    return ret;
}

kern_return_t AllunoVHIDBus::setFeature(uint32_t index, const uint8_t* report, uint16_t length, void* owner)
{
    AllunoVHIDDevice* device = deviceFor(ivars, index, owner);
    if (device == nullptr)
        return kIOReturnNotAttached;
    bool ok = device->addFeature(report, length);
    device->release();
    return ok ? kIOReturnSuccess : kIOReturnNoResources;
}

kern_return_t AllunoVHIDBus::pollOutput(uint32_t index, uint8_t* out, size_t capacity, size_t* written, void* owner)
{
    if (capacity < sizeof(VhidOutputHeader))
        return kIOReturnBadArgument;
    AllunoVHIDDevice* device = deviceFor(ivars, index, owner);
    if (device == nullptr)
        return kIOReturnNotAttached;

    VhidOutputHeader header = { VHID_OUTPUT_KIND_REPORT, 0, 0 };
    uint16_t room = static_cast<uint16_t>(capacity - sizeof(header) > VHID_MAX_REPORT ? VHID_MAX_REPORT : capacity - sizeof(header));
    header.length = device->takeOutput(&header.kind, out + sizeof(header), room);
    memcpy(out, &header, sizeof(header));
    *written = sizeof(header) + header.length;
    device->release();
    return kIOReturnSuccess;
}
