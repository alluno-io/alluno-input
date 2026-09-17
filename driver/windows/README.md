# Windows drivers

Two KMDF drivers, three `.sys` files, one solution.

| Directory | Driver | Output |
|---|---|---|
| `src/` | AllunoInput, upper filters on the keyboard and mouse class drivers | `KeyboardAllunoInput.sys`, `MouseAllunoInput.sys` |
| `vhid/` | AllunoVHID, a bus on the Virtual HID Framework with its own Xbox 360 USB device | `AllunoVHID.sys` |

Both target Windows 10 or later, x64 only, KMDF 1.15. The filters ship with the Alluno
desktop today. AllunoVHID builds but has not been loaded on a machine yet.

## Building

Visual Studio 2022 with the Windows Driver Kit (10.0.26100 is known good):

```
msbuild src\AllunoInput.sln /p:Configuration=Release /p:Platform=x64
```

`AllunoVHID.inf` is a plain file in the project on purpose. Stamping it, building the
catalog and signing happen in `alluno-windows-drivers`, whose `build.ps1 -Package` builds the
solution, runs `stampinf` and `inf2cat`, signs the three drivers, compiles `src/Installer.cs`
into `Install.exe` and `Uninstall.exe` with the framework C# compiler, and zips the result.

## Installing

`Install.exe` asks for elevation, then:

1. Checks Secure Boot. With Secure Boot off it turns test signing on (`bcdedit /set testsigning on`).
   With Secure Boot on it continues only when Custom Kernel Signers is active; otherwise it
   stops, because an unsigned filter under Secure Boot leaves the machine without keyboard
   and mouse after the reboot.
2. Copies the two filter drivers into `System32\drivers`, creates their kernel services and
   adds them to the `UpperFilters` of the keyboard and mouse device classes.
3. Creates the root device `Root\AllunoVHID` and installs `AllunoVHID.inf` on it.
4. Asks for a reboot.

`Uninstall.exe` removes the filters from `UpperFilters`, deletes the services and files,
removes the AllunoVHID device node and its driver package, and turns test signing off again
where it turned it on.

A kernel driver loads only with a signature the machine trusts: test signing with Secure Boot
off, Custom Kernel Signers, or Microsoft attestation signing for a release.

## AllunoInput

The filter service names must start with `Keyboard` and `Mouse` (`KeyboardAllunoInput`,
`MouseAllunoInput`). Injected input arrives as hardware input does, with no
`LLKHF_INJECTED` flag and no extra-info marker. A send goes to one filter device, never to
every filter in the class.

| Device path | IOCTL | Value |
|---|---|---|
| `\\.\KeyboardAllunoInput` | `IOCTL_ALLUNO_SEND` | `CTL_CODE(0x0B, 0x820, 0, 0)` = `0x000B2080` |
| `\\.\MouseAllunoInput` | `IOCTL_ALLUNO_SEND` | `CTL_CODE(0x0F, 0x820, 0, 0)` = `0x000F2080` |

### Keyboard input (`KEYBOARD_INPUT_DATA`, 12 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 2 | UnitId | 0 |
| 2 | 2 | MakeCode | PS/2 Set 1 scan code |
| 4 | 2 | Flags | `0x00` key down, `0x01` key up, `0x02` E0 prefix, `0x04` E1 prefix |
| 6 | 2 | Reserved | 0 |
| 8 | 4 | ExtraInformation | 0 |

Flags combine: `0x03` is E0 plus key up, the release of Right Ctrl for example.

### Mouse input (`MOUSE_INPUT_DATA`, 24 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 2 | UnitId | 0 |
| 2 | 2 | Flags | `0x00` relative, `0x01` absolute, `0x03` absolute on the virtual desktop |
| 4 | 2 | ButtonFlags | see below |
| 6 | 2 | ButtonData | wheel delta, signed, 120 per notch |
| 8 | 4 | RawButtons | 0 |
| 12 | 4 | LastX | x delta, or 0 to 65535 when absolute |
| 16 | 4 | LastY | y delta, or 0 to 65535 when absolute |
| 20 | 4 | ExtraInformation | 0 |

| Flag | Value |
|---|---|
| `LEFT_BUTTON_DOWN` / `UP` | `0x0001` / `0x0002` |
| `RIGHT_BUTTON_DOWN` / `UP` | `0x0004` / `0x0008` |
| `MIDDLE_BUTTON_DOWN` / `UP` | `0x0010` / `0x0020` |
| `BUTTON_4_DOWN` / `UP` | `0x0040` / `0x0080` |
| `BUTTON_5_DOWN` / `UP` | `0x0100` / `0x0200` |
| `WHEEL` | `0x0400`, delta in ButtonData |
| `HWHEEL` | `0x0800`, delta in ButtonData |

### Scan codes (PS/2 Set 1)

| Key | Code | Key | Code | Key | Code |
|---|---|---|---|---|---|
| Esc | `0x01` | Tab | `0x0F` | Space | `0x39` |
| A to Z | `0x1E` to `0x32` | 1 to 0 | `0x02` to `0x0B` | F1 to F12 | `0x3B` to `0x58` |
| Enter | `0x1C` | Backspace | `0x0E` | CapsLock | `0x3A` |
| LShift | `0x2A` | RShift | `0x36` | LCtrl | `0x1D` |
| LAlt | `0x38` | LWin | E0 + `0x5B` | | |

Right Ctrl, Right Alt, the arrows, Insert, Delete, Home, End, Page Up and Page Down use the
E0 flag with their base code. `crates/alluno-input-windows/src/scan.rs` holds the full map.

## AllunoVHID

A root-enumerated device (`Root\AllunoVHID`, class `System`, service `AllunoVHID`) that
publishes up to sixteen virtual devices. A HID slot is a Virtual HID Framework device: the
caller hands over the report descriptor, the identity and the feature-report table, and the
driver answers `GET_FEATURE` from that table, keeps the last input for
`GET_INPUT_REPORT`, and delivers output and set-feature reports through a pending
`WAIT_OUTPUT`. An XUSB slot is instead a child device with the wired Xbox 360 controller's
USB identity, which the inbox `xusb22` driver claims; `vhid/AllunoXusb.c` answers its URBs.
Closing the file handle that plugged a slot unplugs it.

The contract is `vhid/vhid_ioctl.h`: device type `0x8A11`, functions `0x800` to `0x805`,
device interface `{7F3A6C2E-4B1D-4E8A-9C0B-5A2D3F6E1B90}`. The repository README has the
IOCTL table and the profiles the Rust side plugs; `crates/alluno-input-windows/src/vhid.rs` is the
client and `crates/alluno-input-windows/tests/vhid_contract.rs` pins the codes and layouts.

## Rust bindings

`crates/alluno-input-windows`: `kernel` for the filters, `vhid` for the bus, behind the `alluno-input`
facade.

## License

MIT
