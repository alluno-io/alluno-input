/*
 * alluno-input: virtual input devices behind one host, the C ABI.
 *
 * Open a host on the thread that will inject and keep every device on it.
 * Every call answers a ALLUNO_INPUT_* code; alluno_input_last_error() carries the message
 * for the last failure on the calling thread. A null handle from an *_open
 * call is a failure; read alluno_input_last_error() for why.
 */

#ifndef ALLUNO_INPUT_H
#define ALLUNO_INPUT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Result codes. */
#define ALLUNO_INPUT_OK           0
#define ALLUNO_INPUT_UNAVAILABLE  1
#define ALLUNO_INPUT_UNSUPPORTED  2
#define ALLUNO_INPUT_BACKEND      3
#define ALLUNO_INPUT_IO           4
#define ALLUNO_INPUT_INVALID      5

/* How a device kind is backed on this machine. */
#define ALLUNO_INPUT_BACKING_KERNEL       0
#define ALLUNO_INPUT_BACKING_BUS          1
#define ALLUNO_INPUT_BACKING_USER_API     2
#define ALLUNO_INPUT_BACKING_UNAVAILABLE  3

/* Gamepad profiles, in the host's order. */
#define ALLUNO_INPUT_PROFILE_XBOX360      0
#define ALLUNO_INPUT_PROFILE_XBOX_ONE     1
#define ALLUNO_INPUT_PROFILE_XBOX_SERIES  2
#define ALLUNO_INPUT_PROFILE_DUALSHOCK4   3
#define ALLUNO_INPUT_PROFILE_DUALSENSE    4
#define ALLUNO_INPUT_PROFILE_SWITCH_PRO   5
#define ALLUNO_INPUT_PROFILE_GENERIC_HID  6
#define ALLUNO_INPUT_PROFILE_SLOTS        8

/* Mouse buttons. */
#define ALLUNO_INPUT_BUTTON_LEFT      0
#define ALLUNO_INPUT_BUTTON_RIGHT     1
#define ALLUNO_INPUT_BUTTON_MIDDLE    2
#define ALLUNO_INPUT_BUTTON_BACK      3
#define ALLUNO_INPUT_BUTTON_FORWARD   4

/* Gamepad button bits, XInput layout. */
#define ALLUNO_INPUT_PAD_DPAD_UP          0x0001
#define ALLUNO_INPUT_PAD_DPAD_DOWN        0x0002
#define ALLUNO_INPUT_PAD_DPAD_LEFT        0x0004
#define ALLUNO_INPUT_PAD_DPAD_RIGHT       0x0008
#define ALLUNO_INPUT_PAD_START            0x0010
#define ALLUNO_INPUT_PAD_BACK             0x0020
#define ALLUNO_INPUT_PAD_LEFT_THUMB       0x0040
#define ALLUNO_INPUT_PAD_RIGHT_THUMB      0x0080
#define ALLUNO_INPUT_PAD_LEFT_SHOULDER    0x0100
#define ALLUNO_INPUT_PAD_RIGHT_SHOULDER   0x0200
#define ALLUNO_INPUT_PAD_GUIDE            0x0400
#define ALLUNO_INPUT_PAD_A                0x1000
#define ALLUNO_INPUT_PAD_B                0x2000
#define ALLUNO_INPUT_PAD_X                0x4000
#define ALLUNO_INPUT_PAD_Y                0x8000

/* What a game sent back to a pad. */
#define ALLUNO_INPUT_OUTPUT_RUMBLE          0
#define ALLUNO_INPUT_OUTPUT_TRIGGER_RUMBLE  1
#define ALLUNO_INPUT_OUTPUT_PLAYER_LED      2
#define ALLUNO_INPUT_OUTPUT_RGB             3
#define ALLUNO_INPUT_OUTPUT_RAW             4

/* Keys by position, independent of layout. */
enum alluno_input_key {
    ALLUNO_INPUT_KEY_A = 0,
    ALLUNO_INPUT_KEY_B = 1,
    ALLUNO_INPUT_KEY_C = 2,
    ALLUNO_INPUT_KEY_D = 3,
    ALLUNO_INPUT_KEY_E = 4,
    ALLUNO_INPUT_KEY_F = 5,
    ALLUNO_INPUT_KEY_G = 6,
    ALLUNO_INPUT_KEY_H = 7,
    ALLUNO_INPUT_KEY_I = 8,
    ALLUNO_INPUT_KEY_J = 9,
    ALLUNO_INPUT_KEY_K = 10,
    ALLUNO_INPUT_KEY_L = 11,
    ALLUNO_INPUT_KEY_M = 12,
    ALLUNO_INPUT_KEY_N = 13,
    ALLUNO_INPUT_KEY_O = 14,
    ALLUNO_INPUT_KEY_P = 15,
    ALLUNO_INPUT_KEY_Q = 16,
    ALLUNO_INPUT_KEY_R = 17,
    ALLUNO_INPUT_KEY_S = 18,
    ALLUNO_INPUT_KEY_T = 19,
    ALLUNO_INPUT_KEY_U = 20,
    ALLUNO_INPUT_KEY_V = 21,
    ALLUNO_INPUT_KEY_W = 22,
    ALLUNO_INPUT_KEY_X = 23,
    ALLUNO_INPUT_KEY_Y = 24,
    ALLUNO_INPUT_KEY_Z = 25,
    ALLUNO_INPUT_KEY_NUM0 = 26,
    ALLUNO_INPUT_KEY_NUM1 = 27,
    ALLUNO_INPUT_KEY_NUM2 = 28,
    ALLUNO_INPUT_KEY_NUM3 = 29,
    ALLUNO_INPUT_KEY_NUM4 = 30,
    ALLUNO_INPUT_KEY_NUM5 = 31,
    ALLUNO_INPUT_KEY_NUM6 = 32,
    ALLUNO_INPUT_KEY_NUM7 = 33,
    ALLUNO_INPUT_KEY_NUM8 = 34,
    ALLUNO_INPUT_KEY_NUM9 = 35,
    ALLUNO_INPUT_KEY_F1 = 36,
    ALLUNO_INPUT_KEY_F2 = 37,
    ALLUNO_INPUT_KEY_F3 = 38,
    ALLUNO_INPUT_KEY_F4 = 39,
    ALLUNO_INPUT_KEY_F5 = 40,
    ALLUNO_INPUT_KEY_F6 = 41,
    ALLUNO_INPUT_KEY_F7 = 42,
    ALLUNO_INPUT_KEY_F8 = 43,
    ALLUNO_INPUT_KEY_F9 = 44,
    ALLUNO_INPUT_KEY_F10 = 45,
    ALLUNO_INPUT_KEY_F11 = 46,
    ALLUNO_INPUT_KEY_F12 = 47,
    ALLUNO_INPUT_KEY_BACKSPACE = 48,
    ALLUNO_INPUT_KEY_TAB = 49,
    ALLUNO_INPUT_KEY_ENTER = 50,
    ALLUNO_INPUT_KEY_PAUSE = 51,
    ALLUNO_INPUT_KEY_CAPS_LOCK = 52,
    ALLUNO_INPUT_KEY_ESCAPE = 53,
    ALLUNO_INPUT_KEY_SPACE = 54,
    ALLUNO_INPUT_KEY_PAGE_UP = 55,
    ALLUNO_INPUT_KEY_PAGE_DOWN = 56,
    ALLUNO_INPUT_KEY_END = 57,
    ALLUNO_INPUT_KEY_HOME = 58,
    ALLUNO_INPUT_KEY_LEFT = 59,
    ALLUNO_INPUT_KEY_UP = 60,
    ALLUNO_INPUT_KEY_RIGHT = 61,
    ALLUNO_INPUT_KEY_DOWN = 62,
    ALLUNO_INPUT_KEY_INSERT = 63,
    ALLUNO_INPUT_KEY_DELETE = 64,
    ALLUNO_INPUT_KEY_MINUS = 65,
    ALLUNO_INPUT_KEY_EQUAL = 66,
    ALLUNO_INPUT_KEY_BRACKET_LEFT = 67,
    ALLUNO_INPUT_KEY_BRACKET_RIGHT = 68,
    ALLUNO_INPUT_KEY_SEMICOLON = 69,
    ALLUNO_INPUT_KEY_QUOTE = 70,
    ALLUNO_INPUT_KEY_BACKSLASH = 71,
    ALLUNO_INPUT_KEY_COMMA = 72,
    ALLUNO_INPUT_KEY_PERIOD = 73,
    ALLUNO_INPUT_KEY_SLASH = 74,
    ALLUNO_INPUT_KEY_BACKQUOTE = 75,
    ALLUNO_INPUT_KEY_NUMPAD0 = 76,
    ALLUNO_INPUT_KEY_NUMPAD1 = 77,
    ALLUNO_INPUT_KEY_NUMPAD2 = 78,
    ALLUNO_INPUT_KEY_NUMPAD3 = 79,
    ALLUNO_INPUT_KEY_NUMPAD4 = 80,
    ALLUNO_INPUT_KEY_NUMPAD5 = 81,
    ALLUNO_INPUT_KEY_NUMPAD6 = 82,
    ALLUNO_INPUT_KEY_NUMPAD7 = 83,
    ALLUNO_INPUT_KEY_NUMPAD8 = 84,
    ALLUNO_INPUT_KEY_NUMPAD9 = 85,
    ALLUNO_INPUT_KEY_NUMPAD_MULTIPLY = 86,
    ALLUNO_INPUT_KEY_NUMPAD_ADD = 87,
    ALLUNO_INPUT_KEY_NUMPAD_ENTER = 88,
    ALLUNO_INPUT_KEY_NUMPAD_SUBTRACT = 89,
    ALLUNO_INPUT_KEY_NUMPAD_DECIMAL = 90,
    ALLUNO_INPUT_KEY_NUMPAD_DIVIDE = 91,
    ALLUNO_INPUT_KEY_SHIFT_LEFT = 92,
    ALLUNO_INPUT_KEY_SHIFT_RIGHT = 93,
    ALLUNO_INPUT_KEY_CONTROL_LEFT = 94,
    ALLUNO_INPUT_KEY_CONTROL_RIGHT = 95,
    ALLUNO_INPUT_KEY_ALT_LEFT = 96,
    ALLUNO_INPUT_KEY_ALT_RIGHT = 97,
    ALLUNO_INPUT_KEY_META = 98,
    ALLUNO_INPUT_KEY_COUNT = 99
};

typedef struct alluno_input_host alluno_input_host;
typedef struct alluno_input_keyboard alluno_input_keyboard;
typedef struct alluno_input_mouse alluno_input_mouse;
typedef struct alluno_input_pen alluno_input_pen;
typedef struct alluno_input_touch alluno_input_touch;
typedef struct alluno_input_gamepad alluno_input_gamepad;

typedef struct alluno_input_capabilities {
    uint8_t keyboard;
    uint8_t mouse;
    uint8_t pen;
    uint8_t touch;
    uint8_t gamepads[ALLUNO_INPUT_PROFILE_SLOTS];
    int16_t max_gamepads;
    uint8_t midi;
    uint8_t camera;
    uint8_t microphone;
} alluno_input_capabilities;

typedef struct alluno_input_pen_state {
    uint16_t x;
    uint16_t y;
    uint16_t pressure;
    int8_t tilt_x;
    int8_t tilt_y;
    uint16_t twist;
    uint8_t down;
    uint8_t barrel;
    uint8_t eraser;
    uint8_t in_range;
} alluno_input_pen_state;

typedef struct alluno_input_touch_contact {
    uint8_t id;
    uint8_t down;
    uint16_t x;
    uint16_t y;
    uint16_t pressure;
    uint16_t width;
    uint16_t height;
} alluno_input_touch_contact;

typedef struct alluno_input_gamepad_state {
    uint16_t buttons;
    uint8_t left_trigger;
    uint8_t right_trigger;
    int16_t thumb_lx;
    int16_t thumb_ly;
    int16_t thumb_rx;
    int16_t thumb_ry;
} alluno_input_gamepad_state;

typedef struct alluno_input_gamepad_output {
    uint8_t kind;
    uint8_t a;
    uint8_t b;
    uint8_t c;
    const uint8_t* raw;
    size_t raw_len;
} alluno_input_gamepad_output;

typedef void (*alluno_input_output_callback)(void* user, const alluno_input_gamepad_output* output);

const char* alluno_input_version(void);
const char* alluno_input_last_error(void);
int32_t alluno_input_probe(alluno_input_capabilities* out);

alluno_input_host* alluno_input_open(const char* device_name);
alluno_input_host* alluno_input_open_recording(void);
size_t alluno_input_recording_len(const alluno_input_host* host);
void alluno_input_close(alluno_input_host* host);

alluno_input_keyboard* alluno_input_keyboard_open(const alluno_input_host* host);
void alluno_input_keyboard_close(alluno_input_keyboard* keyboard);
int32_t alluno_input_keyboard_key(alluno_input_keyboard* keyboard, uint8_t key, bool pressed);
int32_t alluno_input_keyboard_text(alluno_input_keyboard* keyboard, const char* text);

alluno_input_mouse* alluno_input_mouse_open(const alluno_input_host* host);
void alluno_input_mouse_close(alluno_input_mouse* mouse);
int32_t alluno_input_mouse_move_abs(alluno_input_mouse* mouse, uint16_t x, uint16_t y);
int32_t alluno_input_mouse_move_rel(alluno_input_mouse* mouse, int32_t dx, int32_t dy);
int32_t alluno_input_mouse_button(alluno_input_mouse* mouse, uint8_t button, bool pressed);
int32_t alluno_input_mouse_wheel(alluno_input_mouse* mouse, int32_t dx, int32_t dy);

alluno_input_pen* alluno_input_pen_open(const alluno_input_host* host);
void alluno_input_pen_close(alluno_input_pen* pen);
int32_t alluno_input_pen_report(alluno_input_pen* pen, const alluno_input_pen_state* state);

alluno_input_touch* alluno_input_touch_open(const alluno_input_host* host);
void alluno_input_touch_close(alluno_input_touch* touch);
int32_t alluno_input_touch_report(alluno_input_touch* touch, const alluno_input_touch_contact* contacts, size_t count);

alluno_input_gamepad* alluno_input_gamepad_open(const alluno_input_host* host, uint8_t profile);
void alluno_input_gamepad_close(alluno_input_gamepad* pad);
int32_t alluno_input_gamepad_submit(alluno_input_gamepad* pad, const alluno_input_gamepad_state* state);
int32_t alluno_input_gamepad_on_output(alluno_input_gamepad* pad, alluno_input_output_callback callback, void* user);
int32_t alluno_input_gamepad_slot(alluno_input_gamepad* pad);
bool alluno_input_recording_rumble(alluno_input_gamepad* pad, uint8_t large, uint8_t small);

#ifdef __cplusplus
}
#endif

#endif
