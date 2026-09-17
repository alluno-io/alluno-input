/*
 * AllunoVHID user-mode contract. The Rust client in alluno-input-windows/src/vhid.rs
 * mirrors every value here byte for byte; change both together.
 */

#pragma once

#ifndef VHID_CTL_CODE
#ifdef CTL_CODE
#define VHID_CTL_CODE(function) CTL_CODE(VHID_DEVICE_TYPE, function, METHOD_BUFFERED, FILE_ANY_ACCESS)
#else
#define VHID_CTL_CODE(function) ((VHID_DEVICE_TYPE << 16) | ((function) << 2))
#endif
#endif

#define VHID_DEVICE_TYPE            0x8A11
#define VHID_API_VERSION            1

/* {7F3A6C2E-4B1D-4E8A-9C0B-5A2D3F6E1B90} */
#define VHID_INTERFACE_GUID_STRING  L"{7F3A6C2E-4B1D-4E8A-9C0B-5A2D3F6E1B90}"

#define IOCTL_VHID_VERSION          VHID_CTL_CODE(0x800)
#define IOCTL_VHID_PLUG             VHID_CTL_CODE(0x801)
#define IOCTL_VHID_UNPLUG           VHID_CTL_CODE(0x802)
#define IOCTL_VHID_INPUT            VHID_CTL_CODE(0x803)
#define IOCTL_VHID_WAIT_OUTPUT      VHID_CTL_CODE(0x804)
#define IOCTL_VHID_SET_FEATURE      VHID_CTL_CODE(0x805)

#define VHID_MAX_DEVICES            16
#define VHID_MAX_DESCRIPTOR         4096
#define VHID_MAX_REPORT             512
#define VHID_MAX_FEATURES           32

/* A plug publishes a HID node from the descriptor that follows the header. */
#define VHID_KIND_HID               0
/* A plug publishes an Xbox 360 XUSB device; the descriptor and features are empty. */
#define VHID_KIND_XUSB              1

#define VHID_OUTPUT_KIND_REPORT     0
#define VHID_OUTPUT_KIND_FEATURE    1

/* The interrupt packet an XUSB node takes and the largest packet it sends back. */
#define VHID_XUSB_INPUT_LENGTH      20
#define VHID_XUSB_OUTPUT_LENGTH     8

#pragma pack(push, 1)

typedef struct _VHID_VERSION_OUT {
    unsigned long Version;
} VHID_VERSION_OUT;

/*
 * Followed by DescriptorLength descriptor bytes, then FeatureCount entries of
 * VHID_FEATURE_ENTRY each followed by its Length data bytes (id byte first).
 */
typedef struct _VHID_PLUG_IN {
    unsigned short VendorId;
    unsigned short ProductId;
    unsigned short VersionNumber;
    unsigned short DescriptorLength;
    unsigned short FeatureCount;
    unsigned short Kind;
} VHID_PLUG_IN;

typedef struct _VHID_FEATURE_ENTRY {
    unsigned short Length;
} VHID_FEATURE_ENTRY;

typedef struct _VHID_PLUG_OUT {
    unsigned long Slot;
} VHID_PLUG_OUT;

typedef struct _VHID_SLOT_IN {
    unsigned long Slot;
} VHID_SLOT_IN;

/* Followed by Length report bytes, id byte first. */
typedef struct _VHID_REPORT_IN {
    unsigned long Slot;
    unsigned short Length;
    unsigned short Reserved;
} VHID_REPORT_IN;

/* Followed by Length report bytes, id byte first. */
typedef struct _VHID_OUTPUT_OUT {
    unsigned char Kind;
    unsigned char Reserved;
    unsigned short Length;
} VHID_OUTPUT_OUT;

#pragma pack(pop)
