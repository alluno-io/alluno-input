# AllunoVHID for macOS

A DriverKit system extension that publishes the virtual game controllers, pen and touch
screen `alluno-input-macos` streams input into. It speaks the same bus contract as the Windows
driver: `vhid_wire.h` here, `vhid_ioctl.h` there and `alluno_input_core::hid::wire` in Rust carry
the same bytes.

Status: not compiled yet. There is no Mac in the build, and loading it needs the
`com.apple.developer.driverkit.family.hid.device` entitlement, which Apple grants per
developer account on request. Until then `crates/alluno-input-macos/src/vhid.rs` answers
`Unavailable` and the rest of the library does not wait on it. Expect the first build to
surface drift against the DriverKit SDK it is built with.

## Files

| File | Role |
|---|---|
| `AllunoVHIDBus.iig` / `.cpp` | The service matched on `IOUserResources`; owns sixteen slots and hands out user clients |
| `AllunoVHIDDevice.iig` / `.cpp` | One `IOUserHIDDevice` per slot: descriptor, identity, feature table, input forwarding, output queue |
| `AllunoVHIDUserClient.iig` / `.cpp` | Six external methods: version, plug, unplug, input, poll output, set feature |
| `vhid_wire.h` | The packed headers and limits |
| `Info.plist` | Bundle id `io.alluno.AllunoVHID`; the `AllunoVHIDBus` and `AllunoVHIDUserClient` personalities |
| `AllunoVHID.entitlements` | DriverKit, the HID device family, user-client access for `io.alluno.desktop`, app sandbox |

## Building and activating

Xcode with the DriverKit SDK and an account holding the entitlements above.

1. Create a DriverKit target with bundle id `io.alluno.AllunoVHID`, add the files, and set
   the entitlements file. The host app (`io.alluno.desktop`) needs
   `com.apple.developer.system-extension.install` and
   `com.apple.developer.driverkit.userclient-access` naming the extension.
2. Embed the `.dext` in the app under `Contents/Library/SystemExtensions`.
3. Activate it with `OSSystemExtensionRequest.activationRequest`; the user approves it once
   in System Settings. `alluno-input probe` then reports the controller profiles and touch as `Bus`.

## Client

The Rust client finds the service with `IOServiceMatching("AllunoVHIDBus")`, plugs a device
by sending the packed header, descriptor and feature table, and streams input reports with
the input selector. Output and set-feature reports are polled every 4 ms through the
poll-output selector rather than pushed, which keeps the client free of a run loop.

## License

MIT
