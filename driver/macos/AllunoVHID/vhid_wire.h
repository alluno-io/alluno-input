/*
 * The byte layouts the AllunoVHID user client exchanges with alluno-input-macos.
 * alluno_input_core::hid::wire and driver/windows/vhid/vhid_ioctl.h carry the same
 * values; change all three together.
 */

#pragma once

#include <stdint.h>

#define VHID_API_VERSION            1
#define VHID_MAX_DEVICES            16
#define VHID_MAX_DESCRIPTOR         4096
#define VHID_MAX_REPORT             512
#define VHID_MAX_FEATURES           32
#define VHID_OUTPUT_QUEUE           8

#define VHID_OUTPUT_KIND_REPORT     0
#define VHID_OUTPUT_KIND_FEATURE    1

enum VhidSelector {
    kVhidSelectorVersion    = 0,
    kVhidSelectorPlug       = 1,
    kVhidSelectorUnplug     = 2,
    kVhidSelectorInput      = 3,
    kVhidSelectorPollOutput = 4,
    kVhidSelectorSetFeature = 5,
    kVhidSelectorCount      = 6,
};

#pragma pack(push, 1)

typedef struct {
    uint16_t vendorId;
    uint16_t productId;
    uint16_t versionNumber;
    uint16_t descriptorLength;
    uint16_t featureCount;
    uint16_t reserved;
} VhidPlugHeader;

typedef struct {
    uint16_t length;
} VhidFeatureEntry;

typedef struct {
    uint32_t slot;
    uint16_t length;
    uint16_t reserved;
} VhidReportHeader;

typedef struct {
    uint8_t  kind;
    uint8_t  reserved;
    uint16_t length;
} VhidOutputHeader;

#pragma pack(pop)
