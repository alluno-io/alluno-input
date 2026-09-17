/*
 * The user client one host process opens: six external methods that carry the
 * same bytes as the Windows IOCTLs, checked against the selector table so a
 * malformed call never reaches the bus.
 */

#include <os/log.h>
#include <string.h>
#include <DriverKit/IOLib.h>
#include <DriverKit/IOUserClient.h>
#include <DriverKit/OSData.h>

#include "AllunoVHIDBus.h"
#include "AllunoVHIDUserClient.h"
#include "vhid_wire.h"

struct AllunoVHIDUserClient_IVars {
    AllunoVHIDBus* bus;
};

bool AllunoVHIDUserClient::init()
{
    if (!super::init())
        return false;
    ivars = IONewZero(AllunoVHIDUserClient_IVars, 1);
    return ivars != nullptr;
}

void AllunoVHIDUserClient::free()
{
    IOSafeDeleteNULL(ivars, AllunoVHIDUserClient_IVars, 1);
    super::free();
}

kern_return_t IMPL(AllunoVHIDUserClient, Start)
{
    kern_return_t ret = Start(provider, SUPERDISPATCH);
    if (ret != kIOReturnSuccess)
        return ret;
    ivars->bus = OSDynamicCast(AllunoVHIDBus, provider);
    if (ivars->bus == nullptr) {
        Stop(provider, SUPERDISPATCH);
        return kIOReturnNoDevice;
    }
    return kIOReturnSuccess;
}

kern_return_t IMPL(AllunoVHIDUserClient, Stop)
{
    if (ivars->bus != nullptr) {
        ivars->bus->unplugOwned(this);
        ivars->bus = nullptr;
    }
    return Stop(provider, SUPERDISPATCH);
}

static const IOUserClientMethodDispatch kDispatch[kVhidSelectorCount] = {
    // version: no input, one scalar out
    { nullptr, 0, 0, 1, 0 },
    // plug: struct in (variable), one scalar out
    { nullptr, 0, kIOUserClientVariableStructureSize, 1, 0 },
    // unplug: one scalar in
    { nullptr, 1, 0, 0, 0 },
    // input: struct in (variable)
    { nullptr, 0, kIOUserClientVariableStructureSize, 0, 0 },
    // poll output: struct in (4 bytes), struct out (variable)
    { nullptr, 0, sizeof(uint32_t), 0, kIOUserClientVariableStructureSize },
    // set feature: struct in (variable)
    { nullptr, 0, kIOUserClientVariableStructureSize, 0, 0 },
};

static bool structInput(IOUserClientMethodArguments* arguments, const uint8_t** bytes, size_t* length)
{
    if (arguments->structureInput != nullptr) {
        *bytes = static_cast<const uint8_t*>(arguments->structureInput->getBytesNoCopy());
        *length = arguments->structureInput->getLength();
        return *bytes != nullptr;
    }
    return false;
}

kern_return_t AllunoVHIDUserClient::ExternalMethod(uint64_t selector,
                                                   IOUserClientMethodArguments* arguments,
                                                   const IOUserClientMethodDispatch* dispatch,
                                                   OSObject* target,
                                                   void* reference)
{
    if (selector >= kVhidSelectorCount || ivars->bus == nullptr)
        return kIOReturnBadArgument;

    dispatch = &kDispatch[selector];
    kern_return_t ret = super::ExternalMethod(selector, arguments, dispatch, target, reference);
    if (ret != kIOReturnSuccess)
        return ret;

    switch (selector) {
    case kVhidSelectorVersion:
        arguments->scalarOutput[0] = VHID_API_VERSION;
        arguments->scalarOutputCount = 1;
        return kIOReturnSuccess;

    case kVhidSelectorPlug: {
        const uint8_t* bytes = nullptr;
        size_t length = 0;
        if (!structInput(arguments, &bytes, &length))
            return kIOReturnBadArgument;
        uint32_t slot = 0;
        ret = ivars->bus->plug(bytes, length, &slot, this);
        if (ret == kIOReturnSuccess) {
            arguments->scalarOutput[0] = slot;
            arguments->scalarOutputCount = 1;
        }
        return ret;
    }

    case kVhidSelectorUnplug:
        return ivars->bus->unplug(static_cast<uint32_t>(arguments->scalarInput[0]), this);

    case kVhidSelectorInput:
    case kVhidSelectorSetFeature: {
        const uint8_t* bytes = nullptr;
        size_t length = 0;
        if (!structInput(arguments, &bytes, &length) || length < sizeof(VhidReportHeader))
            return kIOReturnBadArgument;
        VhidReportHeader header;
        memcpy(&header, bytes, sizeof(header));
        if (header.length == 0 || header.length > VHID_MAX_REPORT || length < sizeof(header) + header.length)
            return kIOReturnBadArgument;
        const uint8_t* report = bytes + sizeof(header);
        if (selector == kVhidSelectorInput)
            return ivars->bus->input(header.slot, report, header.length, this);
        return ivars->bus->setFeature(header.slot, report, header.length, this);
    }

    case kVhidSelectorPollOutput: {
        const uint8_t* bytes = nullptr;
        size_t length = 0;
        if (!structInput(arguments, &bytes, &length) || length < sizeof(uint32_t))
            return kIOReturnBadArgument;
        uint32_t slot = 0;
        memcpy(&slot, bytes, sizeof(slot));
        uint8_t out[sizeof(VhidOutputHeader) + VHID_MAX_REPORT];
        size_t written = 0;
        ret = ivars->bus->pollOutput(slot, out, sizeof(out), &written, this);
        if (ret != kIOReturnSuccess)
            return ret;
        OSData* data = OSData::withBytes(out, static_cast<uint32_t>(written));
        if (data == nullptr)
            return kIOReturnNoMemory;
        arguments->structureOutput = data;
        return kIOReturnSuccess;
    }

    default:
        return kIOReturnBadArgument;
    }
}
