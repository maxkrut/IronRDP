# Virtual desktop control wire format

No Microsoft Open Specification documents `Microsoft::Windows::RDS::VirtualDesktopControl::Switch`.
This note records where each layout and rule in this crate comes from, so the `oracle: reference` markers in `crates/ironrdp-testsuite-core/tests/vdcswitch` can be checked against the binaries.

## Pinned binaries

Windows App 2.0.1485.0, package `MicrosoftCorporationII.Windows365_2.0.1485.0_x64__8wekyb3d8bbwe`.
Paths are relative to the package root.

| File | Version | SHA-256 |
| --- | --- | --- |
| `msrdc/rdclientax.dll` | 1.2.7423.0 | `188F1B416B3ECB8B508CF4B4BDE69DC062C9F2114D03E93AD935AAE8E6207BDE` |
| `wnc/localsettingsproxyplugin.dll` | none | `D6ED5ED6E4BDE8DBADAE845DF4F142DAA45FA734EA1E0E3D4B61BD7A99EAA2B9` |
| `wnc/Microsoft.CloudManagedDesktop.Clients.NxtClient.Common.dll` | 1.2610.03566.2401 | `3767769F28D8E1291F0B063BFAAF02290B9CE6CA258A59955FCCDA5FF5EB971A` |
| `wnc/Microsoft.CloudManagedDesktop.Clients.NxtClient.Process.Client.dll` | 1.2610.03566.2401 | `56FF2456F7F22A57C54FE2AA8CA4E4207252EFD9D805199CB34291AB8D7150D1` |

The native DLLs ship without symbols.
Each function below is named after the trace string it logs, and located by its relative virtual address (RVA); the image base of both DLLs is `0x180000000`.
The .NET assemblies were read with ILSpy.

### `rdclientax.dll` (`VirtualDesktopControlDVC`)

| RVA | Function | Identified by |
| --- | --- | --- |
| `0x47DF40` | `OnDataReceived` | dispatches on the PDU type; logs `OnPerformLocalActionPDU Request from Host with Action:%d)` |
| `0x47E3E0` | `SendVersionPdu` | `Entered SendVersionPdu with LaunchUri:%s)` |
| `0x47E6E0` | version response handler | `Version response PDU returned an unupported version:%d)` |
| `0x47E8F0` | `HandleSwitchToLocalRequestPDU` | `SendSwitchToLocalResponsePDU(hrResponse:0x%x) complete` |
| `0x47EE00` | `OnSwitchToCloudPCRequest` | `OnSwitchToCloudPCRequest:Sending SwitchToCloud Request PDU` |
| `0x47EFA0` | `PerformLocalAction` | `PerformLocalAction request recieved from Host` |

### `localsettingsproxyplugin.dll` (`PerformLocalActionChannel`)

| RVA | Function | Identified by |
| --- | --- | --- |
| `0x12100` | `OnDataReceived` | `Malformed VIRTUALDESKTOPCONTROL_PERFORM_LOCALACTION_PDU received from server {}` |
| `0x12A30` | `OnPerformLocalActionPDU` | `OnPerformLocalActionPDU Expected from VIRTUALDESKTOPCONTROL_PROTOCOL_VERSION_200 onwards only {}` |
| `0x137B0` | `SendVersionPdu` | `PerformLocalActionChannel::SendVersionPdu` |

## Header

Every PDU starts with a 6-byte header, all fields little-endian.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 2 | `pduType` |
| 2 | 4 | `pduLength`: the whole PDU, header included |

`OnDataReceived` rejects a buffer shorter than 6 bytes and a `pduLength` greater than the buffer.
It checks each type only against a minimum `pduLength` and ignores bytes past the fields it reads.

## PDUs

| Type | Name | Direction | Minimum length | Body after the header |
| --- | --- | --- | --- | --- |
| 1 | Version request | client to server | 20 + URI | `minVersion` u32, `maxVersion` u32, `cbLaunchUri` u32, launch URI |
| 2 | Version response | server to client | 10 | `version` u32 |
| 3 | Switch-to-local request | server to client | 6 | none |
| 4 | Switch-to-local response | client to server | 10 | `hresult` u32 |
| 5 | Switch-to-cloud request | client to server | 6 | none |
| 6 | Switch-to-cloud response | server to client | 10 | `hresult` u32 |
| 7 | Perform local action | server to client | 7 | `action` u8 |

### Version request (type 1)

`SendVersionPdu` (`0x47E3E0`) allocates `2 * wcslen(launchUri) + 20` bytes and writes:

- `minVersion` `0x00010000` at offset 6 and `maxVersion` `0x00020000` at offset 10.
- `cbLaunchUri` at offset 14: the launch URI's UTF-16 size, including its terminating NUL.
- The launch URI at offset 18, as NUL-terminated UTF-16.

The client sends it as soon as the channel opens.

### Version response (type 2)

The handler at `0x47E6E0` stores `version` from offset 6 and accepts it when `version - 0x10000` is at most `0x10000`, so only versions 1 (`0x00010000`) through 2 (`0x00020000`) are accepted.
Any other version logs `VirtualDesktopControlClientChannel version negotiation failed. Closing the channel` and closes the channel.
This crate cannot close the channel, so it ignores every later PDU instead (see the README).

### Switch-to-local request and response (types 3 and 4)

When the core property `EnableConnectionAction` is set, `OnDataReceived` hands the switch to the Windows App container (`NotifyContainerSwitchToLocal`) and sends no response.
Otherwise `HandleSwitchToLocalRequestPDU` (`0x47E8F0`) switches and sends a 10-byte response: type 4, length 10, then the HRESULT of the switch.
`VdcSwitchHandler::switch_to_local_requested` chooses between the two by returning `None` or `Some(hresult)`.

### Switch-to-cloud request and response (types 5 and 6)

`OnSwitchToCloudPCRequest` (`0x47EE00`) writes the 6-byte header alone: type 5, length 6.
It needs an open channel but not a negotiated version.
`OnDataReceived` reads the response's HRESULT at offset 6 and only logs it.

### Perform local action (type 7)

`OnDataReceived` in `rdclientax.dll` handles type 7 only when `pduLength` is at least 7, a version was negotiated, and the negotiated version is at least `0x00020000`.
`OnPerformLocalActionPDU` (`0x12A30`) in `localsettingsproxyplugin.dll` applies the same version 2 requirement and reads the action as the byte at offset 6.

The action values follow the `RemoteDesktopAction` enum in `Microsoft.CloudManagedDesktop.Clients.NxtClient.Common.dll`, and `LocalSettingsActionRequestedHandler` in `Microsoft.CloudManagedDesktop.Clients.NxtClient.Process.Client.dll` maps each to a settings page:

| Value | `RemoteDesktopAction` | Settings URI | `LocalAction` |
| --- | --- | --- | --- |
| 0 | `ShowBluetoothSettings` | `ms-settings:bluetooth?launchedFromRemoteSession=1` | `BLUETOOTH_SETTINGS` |
| 1 | `ShowSystemSoundSettings` | `ms-settings:sound?launchedFromRemoteSession=1` | `SOUND_SETTINGS` |
| 2 | `ShowSystemDisplaySettings` | `ms-settings:display?launchedFromRemoteSession=1` | `DISPLAY_SETTINGS` |
| 3 | `ShowSystemAccountSettings` | `ms-settings:signinoptions?launchedFromRemoteSession=1` | `ACCOUNT_SETTINGS` |
| 4 | `ShowLocalSettings` | `ms-settings:system?launchedFromRemoteSession=1` | `SYSTEM_SETTINGS` |

When the container does not implement local actions, `PerformLocalAction` (`0x47EFA0`) falls back to `ShellExecuteW` for action 0 alone, opening `ms-settings:devices?launchedFromRemoteSession=1`, and fails other actions with `E_UNEXPECTED`.

## Not verified

- No layout has been checked against a live Cloud PC session; every byte here comes from the client binaries.
- What the server does with the launch URI in the version request was not traced.
