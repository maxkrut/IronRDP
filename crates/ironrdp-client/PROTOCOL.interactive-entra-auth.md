# Interactive Entra authentication channel

No Microsoft Open Specification documents `Microsoft::Windows::RDS::InteractiveEntraAuth`.
This note records where each rule in `src/interactive_entra.rs` comes from, so the `oracle: reference` markers in that module's tests can be checked against the binary.

## Pinned binary

Windows App 2.0.1485.0, package `MicrosoftCorporationII.Windows365_2.0.1485.0_x64__8wekyb3d8bbwe`.

| File | Version | SHA-256 |
| --- | --- | --- |
| `msrdc/rdclientax.dll` | 1.2.7423.0 | `188F1B416B3ECB8B508CF4B4BDE69DC062C9F2114D03E93AD935AAE8E6207BDE` |

The DLL ships without symbols.
Each function below is located by its relative virtual address (RVA) and identified by a string it uses; the image base is `0x180000000`.
The client's source file is `InteractiveRdsAadAuthClient.cpp`, which its logging cites.

| RVA | Role | Identified by |
| --- | --- | --- |
| `0x4AD360` | channel acceptance | `EnableInteractiveRdsAadAuth`, `InteractiveRdsAadEnabled` |
| `0x4ABD10` | data handler, dispatching on the exchange state | `error_code` |
| `0x4AC260` | state 0: request received | calls the request parser |
| `0x4AC030` | request parser | `version`, `scenario_id`, `username_hint` |
| `0x4AC470` | state 1: TLS handshake | calls the token step once the handshake completes |
| `0x4AC650` | token step | `2.5.4.3`, `ServerAadDeviceId`, `InteractiveRdsAadAuthMode`, `AadNonceRequest` |
| `0x4ACF30` | server certificate check | `ServerAadDeviceId`, `ServerAadTenantRootCerts` |
| `0x289220` | P2P certificate chain validation | `CertHelper::ValidateAadP2PCertificate failed!` |
| `0x4AD0F0` | states 2 and 3: data inside TLS | — |

## Acceptance

`0x4AD360` accepts the channel only when the registry value `EnableInteractiveRdsAadAuth` is non-zero under `Software\Microsoft\AzureVirtualDesktopClient\FeatureFlags` or `Software\Microsoft\MSRDC\FeatureFlags`, in HKLM or HKCU, and the session property `InteractiveRdsAadEnabled` is set.
This client leaves the channel off unless the embedder enables it.

## Request

The first message on the channel is plain JSON, which `0x4AC030` parses:

- `version` must be 1; any other value fails with `E_NOTIMPL`.
- `scenario_id` is a GUID string, required, and must be one of the two GUIDs stored at RVAs `0xA79400` and `0xA793F0`: `75cc6d06-3a48-43a1-bcd0-cb46eb8f9aae` and `b419e9ce-25cd-44bc-8da5-6ff75edd1e28`. Any other GUID fails with `E_NOTIMPL`. The parser stores the ID and nothing reads it afterwards.
- `username_hint` is optional.

A request that is not a JSON object or lacks `scenario_id` fails; this client reports both as `E_INVALIDARG`.

## TLS

After the request, `0x4AC470` runs a TLS handshake inside the channel as the TLS client.
Every later message on the channel goes through that TLS layer.

## Server certificate

`0x4AC650` reads the session host's Entra device ID from the common name (OID `2.5.4.3`) of the server's TLS certificate and stores it as `ServerAadDeviceId`.

`0x4ACF30` checks the certificate when both `ServerAadDeviceId` and the tenant's P2P root certificates (`ServerAadTenantRootCerts`) are known, with `0x289220` requiring the certificate to chain to one of those roots.
It runs once before the token is acquired, where missing values skip the check, and again after, where missing values fail with `E_ACCESSDENIED`.
This client checks only after the token is granted, because the embedder supplies the roots with the token.

## Token and RDS AAD exchange

`0x4AC650` asks the credential UI for a token with `InteractiveRdsAadAuthMode` set to 1 and, when present, `username_hint` as `UserName`, bound to the session's proof-of-possession key (`RdsAadBindingKey`).
The exchange inside TLS is then the RDS AAD Auth exchange of [MS-RDPBCGR] section 5.4.5.4.1: the server sends `ts_nonce`, the client sends `rdp_assertion`, and the server answers with `authentication_result`.

## Errors

On any failure, `0x4ABD10` sends `{"error_code": <HRESULT>}` and closes the channel.
The message goes through TLS once the handshake has completed and is plain before it.
Whether the HRESULT is written signed was not determined; this client writes it as a signed 32-bit integer.

## Not verified

- No exchange has been run against a live session host.
- Where Windows App obtains `ServerAadTenantRootCerts` was not traced.
- The meaning of the two scenario IDs is not documented anywhere in the package.

[MS-RDPBCGR]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-rdpbcgr/
