# Windows 365 redirection wire formats

No Microsoft Open Specification documents the battery status, power request, or screen capture redirection channels.
This note records where each layout and rule in this crate comes from, so the `oracle: reference` markers in `crates/ironrdp-testsuite-core/tests/w365` can be checked against the binaries.

## Pinned binaries

Windows App 2.0.1485.0, package `MicrosoftCorporationII.Windows365_2.0.1485.0_x64__8wekyb3d8bbwe`.
Paths are relative to the package root.

| File | Version | SHA-256 | Channel |
| --- | --- | --- | --- |
| `msrdc/rdclientax.dll` | 1.2.7423.0 | `188F1B416B3ECB8B508CF4B4BDE69DC062C9F2114D03E93AD935AAE8E6207BDE` | battery status |
| `wnc/powerrequestredirectionplugin.dll` | none | `E317C55B3371632A3D05E63F221612CF61E8206C92DC56BB277B45A0EA330131` | power request |
| `RDCoreSdk.dll` | 11.0.9.719 | `CAF8724CC7FBCB8CA66EB3B00575F0D32671F255CF65CB4FB88F4269C351D9A4` | screen capture |

The package also ships a different `wnc/RDCoreSdk.dll` build; only the root copy was analyzed.

The DLLs ship without symbols.
Each function below is named after the trace string it logs, and located by its relative virtual address (RVA); the image base of every DLL is `0x180000000`.
All fields are little-endian.

## Battery status redirection

`Microsoft::Windows::RDS::BatteryStatusRedirection`, implemented by `RdpBatteryStatusRedirectionClientChannel` in `rdclientax.dll`.

| RVA | Function | Identified by |
| --- | --- | --- |
| `0x48DA90` | `OnNewChannelConnection` | `Failed to create RdpBatteryStatusRedirectionClientChannel` |
| `0x48DC50` | RemoteApp check | `GetBoolProperty(TS_PROP_CORE_CONNECTION_IS_RAIL) failed` |
| `0x48F640` | `OnDataReceived` | `RdpBatteryStatusRedirectionClientChannel DispatchAsyncCallWithBuffer failed!`, `HandleServerReadyPdu failed!` |
| `0x48EB40` | `HandleServerReadyPdu`, which sends the client ready PDU | `SendClientReadyPdu failed!` |
| `0x48F4D0` | battery info request handler | called back with the 16 bytes `OnDataReceived` dispatches |
| `0x48EFB0` | battery update check | `SendBatteryInfoChangedPdu Failed` |
| `0x48EDF0` | `SendBatteryInfoChangedPdu` | writes type 4 |

### Header

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 2 | `pduType` |
| 2 | 4 | `pduLength`: the whole PDU, header included |

`OnDataReceived` rejects a buffer shorter than 6 bytes and a `pduLength` greater than the buffer, then checks each type against a minimum `pduLength`.

### PDUs

| Type | Name | Direction | Length | Body after the header |
| --- | --- | --- | --- | --- |
| 1 | Server ready | server to client | at least 14 | `protocolVersion` u32, `protocolFlags` u32 |
| 2 | Client ready | client to server | 16 | `protocolVersion` `0x00010000`, `protocolFlags` 0, reserved u16 0 |
| 3 | Battery info request | server to client | at least 16 | `batteryState` u32, `batteryLifePercent` u16, `batteryLifeMinutes` u32 |
| 4 | Battery info changed | client to server | 16 | same fields as type 3 |

`HandleServerReadyPdu` logs the server's version and flags and answers with version 1 whatever the server announced.

### Battery state

`0x48EFB0` fills the fields from `GetSystemPowerStatus`:

- When `BatteryFlag` is `0x80` (no battery) or `0xFF` (unknown), the state is 1 and the percentage and minutes are 0.
- Otherwise the state is 2 when `ACLineStatus` is 1 and 0 when it is not; the percentage is `BatteryLifePercent`; the minutes are `BatteryLifeTime / 60`, or 0 when `BatteryLifeTime` is `0xFFFFFFFF`.

`BatteryState::ON_BATTERY`, `NO_BATTERY`, and `AC_POWER` are 0, 1, and 2.
This crate leaves reading the battery to the embedder.

### Pacing

The request handler (`0x48F4D0`) stores the state, percentage, and minutes the server sent as the server's current view, allows one update, and runs the update check.
The update check also runs when the local battery changes, and sends a type 4 PDU only when an update is allowed and either:

- no update was sent on this channel yet, or
- the state or the percentage differs from the server's view.

The minutes alone never trigger an update.
Sending clears the permission until the next type 3 request.

### RemoteApp

`OnNewChannelConnection` (`0x48DA90`) accepts the channel only when the check at `0x48DC50` finds the core property `RailMode` false, so Windows App refuses the channel in RemoteApp sessions.

## Power request redirection

`Microsoft::Windows::RDS::PowerRequestRedirection`, implemented by `PowerRequestRedirectionChannel` in `powerrequestredirectionplugin.dll`.

| RVA | Function | Identified by |
| --- | --- | --- |
| `0x129C0` | `OnDataReceived` | `PowerRequestRedirectionChannel::OnDataReceived` |
| `0x119C0` | `HandleServerReadyPdu` | `PowerRequestRedirectionChannel::HandleServerReadyPdu` |
| `0x13320` | `SendClientReadyPdu` | `PowerRequestRedirectionChannel::SendClientReadyPdu` |
| `0x11510` | `HandlePowerRequestInfoChangedPdu` | `PowerRequestRedirectionChannel::HandlePowerRequestInfoChangedPdu` |
| `0x13E60` | `SetPowerDisplayRequest` | `SetPowerDisplayRequest failed! {}`, logged by its caller |

The header is the same 6-byte type and length as the battery channel.
`OnDataReceived` logs `Not enough data to read POWER_REQUEST_HEADER!` below 6 bytes and rejects a `pduLength` greater than the buffer.

| Type | Name | Direction | Length | Body after the header |
| --- | --- | --- | --- | --- |
| 1 | Server ready | server to client | at least 14 | `protocolVersion` u32, `protocolFlags` u32 |
| 2 | Client ready | client to server | 14 | `protocolVersion` `0x00010000`, `protocolFlags` 0 |
| 3 | Power request info changed | server to client | at least 10 | `powerRequestFlags` u32 |

`SendClientReadyPdu` (`0x13320`) always writes version 1, whatever the server announced.

`HandlePowerRequestInfoChangedPdu` (`0x11510`) reads `powerRequestFlags` at offset 6:

- Bit `0x1` means the Cloud PC holds power requests, and bit `0x2` means it holds none.
- Neither bit fails with `One of 'Current Requests' and 'No current requests' must be set!`, and both bits fail with `'Current Requests' and 'No current requests' cannot both be set!`.
- Otherwise it calls `SetPowerDisplayRequest` (`0x13E60`) with whether bit `0x1` is set; that function sets or clears `PowerRequestSystemRequired` and `PowerRequestDisplayRequired` with `PowerSetRequest` and `PowerClearRequest`.

Windows App's behavior when the channel closes while requests are held was not traced; this crate reports the requests as released.

## Screen capture redirection

`Microsoft::Windows::RDS::ScreenCaptureRedirection`, implemented by `RdpScreenCaptureRedirectionClientChannel` in `RDCoreSdk.dll`.

| RVA | Function | Identified by |
| --- | --- | --- |
| `0x556640` | `OnDataReceived` | `Malformed ScreenCaptureRedirection PDU received. cbSize={}.` |
| `0x559990` | `SendClientReadyPdu` | `RdpScreenCaptureRedirectionClientChannel::SendClientReadyPdu` |
| `0x552600` | `CreateScreenCapturePdu` | `RdpScreenCaptureRedirectionClientChannel::CreateScreenCapturePdu` |
| `0x55AFC0` | channel `UploadScreenCapture` | `CreateScreenCapturePdu failed! {}` |
| `0x55D000` | plugin `UploadScreenCapture` | `ScreenCaptureRedirection upload rejected because client channel is unavailable. …` |
| `0x552D40` | `HandleSaveResultPdu` | `RdpScreenCaptureRedirectionClientChannel::HandleSaveResultPdu` |

### Header

This channel uses an 8-byte header.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 2 | `operationType` |
| 2 | 2 | `flags` |
| 4 | 4 | `payloadLength`: the bytes after the header |

`OnDataReceived` rejects a PDU unless it is at least 8 bytes and `payloadLength + 8` equals its size exactly.

### PDUs

| Type | Name | Direction | `flags` | Payload |
| --- | --- | --- | --- | --- |
| 1 | Server ready | server to client | 0 | `protocolVersion` u32, `protocolFlags` u32 |
| 2 | Client ready | client to server | 0 | `protocolVersion` `0x00010000`, `protocolFlags` 0 |
| 3 | Screen capture | client to server | image type | `captureId` u16, image bytes |
| 4 | Save result | server to client | 0 | `captureId` u16, `hresult` u32, `cbFileName` u32, `cbFilePath` u32, file name, file path |

A server ready PDU is accepted only when `flags` is 0, `payloadLength` is 8, and `protocolVersion` is `0x00010000`; only then does the client answer with the client ready PDU and allow uploads.

### Screen capture (type 3)

`CreateScreenCapturePdu` (`0x552600`) stores the image type in the header `flags` and sizes the PDU as the image plus 10 bytes, so the payload is the 2-byte capture ID followed by the image.
`UploadScreenCapture` (`0x55AFC0`) rejects an empty image, uses image type 1 for PNG and 2 for JPEG, and takes capture IDs from a counter that wraps to 0 after `0x7FFF`.
The plugin (`0x55D000`) rejects an upload while the channel is unavailable.

### Save result (type 4)

`HandleSaveResultPdu` (`0x552D40`) reads:

- `captureId` at offset 8, which must not exceed `0x7FFF`.
- `hresult` at offset 10, `cbFileName` at offset 14, and `cbFilePath` at offset 18.
- The file name at offset 22 and the file path right after it, as UTF-16.

It requires `cbFileName + cbFilePath` to equal `payloadLength - 14`.

## Not verified

- No layout has been checked against a live Cloud PC session; every byte here comes from the client binaries.
- Whether the server NUL-terminates the save result's file name and path was not determined, so the decoder drops trailing NULs.
- Windows App's screen capture counter is shared by the whole process and its starting value was not determined; each `ScreenCaptureClient` starts at 0.
