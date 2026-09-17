# alluno-input

Virtual input devices for Windows, Linux and macOS behind one Rust API: keyboard, mouse,
pen, touch screen and game controllers. A host imports `alluno-input`, asks what the machine can
emulate, opens devices by kind and feeds them normalized state. The kernel filter, the HID
bus, `uinput`, `uhid` and Core Graphics sit under it and never show through the API.

## Using the library

```toml
[dependencies]
alluno-input = { git = "https://github.com/alluno-io/alluno-input.git" }
```

```rust
use alluno_input::{Host, Key, Options, Input};

let caps = Input::probe();
let host = Input::open(Options::default())?;
let mut keyboard = host.keyboard()?;
keyboard.key(Key::A, true)?;
keyboard.key(Key::A, false)?;
```

`Input::probe()` reports how every device kind would be backed before anything is opened.
`Input` and every device it hands out are not `Send`: open them on the thread that injects.
Coordinates are `0..=65535` across the virtual desktop; gamepad state uses the XInput layout
whatever the profile.

What each platform needs:

| Platform | Without drivers | With drivers |
|---|---|---|
| Windows 10 or later, x64 | `SendInput` for keyboard and mouse, the Synthetic Pointer API for pen and touch, ViGEmBus for Xbox 360 and DualShock 4 | AllunoInput filters for keyboard and mouse, AllunoVHID for every controller profile, pen and touch |
| Linux | write access to `/dev/uinput`, or an X server with XTest for keyboard and mouse only; `/dev/uhid` for the HID controller profiles | |
| macOS | the Accessibility permission for the process (Core Graphics drops posts silently without it) | the AllunoVHID extension for controllers and touch |

## Workspace

Rust 1.97, edition 2024.

| Crate | Role |
|---|---|
| `alluno-input` | The facade every consumer depends on: the port plus the `Input` host for the target |
| `alluno-input-core` | The port: `Host`, the device traits, the state vocabulary, `Capabilities`; `hid`, the shared report descriptors, codecs and bus wire layouts |
| `alluno-input-windows` | The AllunoInput filter client, `SendInput`, the Synthetic Pointer API, the AllunoVHID bus client and the ViGEmBus client |
| `alluno-input-linux` | `uinput` devices, `uhid` controllers and XTest keyboard and mouse |
| `alluno-input-macos` | Core Graphics keyboard, mouse and pen; the AllunoVHID DriverKit client |
| `alluno-input-ffi` | The C ABI and `include/alluno_input.h` |
| `alluno-input-testkit` | A recording `Host` for consumers' tests |
| `alluno-input-cli` | `alluno-input probe`, `key <name>`, `mouse`, `pad <profile> [seconds]`, `pen`, `touch` |

| Driver | Role |
|---|---|
| `driver/windows/src` | AllunoInput, the KMDF keyboard and mouse filters |
| `driver/windows/vhid` | AllunoVHID, the KMDF bus on the Virtual HID Framework, with its own Xbox 360 USB device |
| `driver/macos/AllunoVHID` | AllunoVHID as a DriverKit extension |

## Building

`just check` is the gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`. CI runs the same three commands on Windows, Linux and macOS. Tests
that need a driver skip when it is absent, and no test sends input a person would notice.

`cargo run -q --bin alluno-input -- probe` prints what this machine answers. The `pad` profiles are
`xbox360`, `xboxone`, `xboxseries`, `ds4`, `dualsense`, `switch` and `generic`.

The Windows drivers build with Visual Studio 2022 and the WDK (10.0.26100 is known good):

```
msbuild driver\windows\src\AllunoInput.sln /p:Configuration=Release /p:Platform=x64
```

That produces `KeyboardAllunoInput.sys`, `MouseAllunoInput.sys` and `AllunoVHID.sys`.
Stamping the INF, the catalog, signing, the installer and the release package are done by
`alluno-windows-drivers`, which consumes this repository as a submodule. Installing is
described in `driver/windows/README.md`; the macOS extension in `driver/macos/README.md`.

## Backends per platform

| Kind | Windows | Linux | macOS |
|---|---|---|---|
| Keyboard | `Kernel` with the filter, else `Bus` with AllunoVHID, else `UserApi` (`SendInput`) | `Bus` (uinput), else `UserApi` (XTest); `bus_keyboard` and `user_keyboard` pick one | `UserApi` (Core Graphics); `bus_keyboard` with the extension |
| Mouse | `Kernel` with the filter, else `UserApi` (`SendInput`); `bus_mouse` adds a relative AllunoVHID node | `Bus` (uinput), else `UserApi` (XTest); `bus_mouse` and `user_mouse` pick one | `UserApi` (Core Graphics); `bus_mouse` with the extension |
| Pen | `Bus` with AllunoVHID, else `UserApi` (Synthetic Pointer) | `Bus` (uinput tablet) | `UserApi` (Core Graphics tablet) |
| Touch | `Bus` with AllunoVHID, else `UserApi` (Synthetic Pointer) | `Bus` (uinput, protocol B) | `Bus` with the extension, else `Unavailable` |
| Xbox 360 | `Bus` (AllunoVHID's XUSB device, or ViGEmBus without it), an XInput slot | `Bus` (uinput, `EV_FF` rumble) | `Unavailable` |
| DualShock 4 | `Bus` (AllunoVHID, or ViGEmBus without it) | `Bus` (uinput; `uhid_gamepad` for the HID node) | `Bus` with the extension |
| DualSense, Switch Pro, generic | `Bus` (AllunoVHID) | `Bus` (uhid) | `Bus` with the extension |
| Xbox One, Xbox Series | `Bus` (AllunoVHID), a HID gamepad | `Bus` (uhid, bound by `hid-microsoft`) | `Bus` with the extension |
| MIDI, camera, microphone | `Unavailable` until a backend lands | same | same |

## Windows: the AllunoInput filters

A KMDF upper filter on `kbdclass` and `mouclass`. What it injects arrives the way hardware
input does: no `LLKHF_INJECTED` flag, no extra-info marker. The filter service names must
start with `Keyboard` and `Mouse` (`KeyboardAllunoInput`, `MouseAllunoInput`), and an
`IOCTL_ALLUNO_SEND` goes to exactly one filter device, never to all of them.

| Device path | IOCTL | Packet |
|---|---|---|
| `\\.\KeyboardAllunoInput` | `0x000B2080` | `KEYBOARD_INPUT_DATA`, 12 bytes: unit, PS/2 Set 1 make code, flags (`0x01` break, `0x02` E0, `0x04` E1), reserved, extra |
| `\\.\MouseAllunoInput` | `0x000F2080` | `MOUSE_INPUT_DATA`, 24 bytes: unit, flags (`0x01` absolute, `0x02` virtual desktop), button flags, wheel data, raw buttons, x, y, extra |


## Windows: the AllunoVHID bus

A KMDF driver on the Virtual HID Framework. One root-enumerated device (`Root\AllunoVHID`)
offers sixteen slots behind the device interface `{7F3A6C2E-4B1D-4E8A-9C0B-5A2D3F6E1B90}`.
Each slot is a HID node with the descriptor, identity and feature-report table the profile
declares. The IOCTL contract is `driver/windows/vhid/vhid_ioctl.h`, mirrored by
`alluno_input_core::hid::wire`:

| IOCTL | Code | In | Out |
|---|---|---|---|
| `VERSION` | `0x8A112000` | | `u32` contract version (1) |
| `PLUG` | `0x8A112004` | packed header (VID, PID, version, descriptor length, feature count, kind), descriptor, features (`u16` length + bytes, id first) | `u32` slot |
| `UNPLUG` | `0x8A112008` | `u32` slot | |
| `INPUT` | `0x8A11200C` | `u32` slot, `u16` length, `u16` reserved, report bytes | |
| `WAIT_OUTPUT` | `0x8A112010` | `u32` slot | pends until an output arrives: `u8` kind (0 report, 1 set feature), `u8` reserved, `u16` length, bytes |
| `SET_FEATURE` | `0x8A112014` | as `INPUT` | |

## HID profiles

`alluno_input_core::hid` holds one descriptor, codec and feature table per profile, published
unchanged by AllunoVHID, `uhid` and the macOS extension:

| Profile | Identity | Input | Output decoded | Feature reports |
|---|---|---|---|---|
| DualShock 4 | `054C:09CC` | `0x01`, 64 bytes | rumble, light bar (`0x05`) | calibration `0x02`, addresses `0x12`/`0x81`, firmware `0xA3` |
| DualSense | `054C:0CE6` | `0x01`, 64 bytes | rumble, trigger rumble, player lamps, light bar (`0x02`) | calibration `0x05`, pairing `0x09`, firmware `0x20` |
| Switch Pro | `057E:2009` | `0x30`, 64 bytes | rumble (`0x10`), player lamps; subcommands and USB commands acknowledged on `0x21`/`0x81`, SPI calibration reads answered | none |
| Generic | `1234:5690` | `0x01`, 14 bytes | none | none |
| Xbox One, Xbox Series (Bluetooth layout) | `045E:02FD`, `045E:0B13` | `0x01`, 17 bytes, guide on `0x02` | rumble and trigger rumble (`0x03`) | none |
| Pen | `1234:5680` | `0x07`, 12 bytes | | |
| Touch | `1234:5681` | `0x08`, 122 bytes, ten parallel contacts | | contact count maximum `0x09` |
| Keyboard | `1234:5682` | `0x0A`, 18 bytes: modifiers and a 128-key bitmap | lamps (`0x0A`) | |
| Mouse | `1234:5683` | `0x0B`, 8 bytes: five buttons, relative x and y, wheel, pan | | |

## License

MIT.
