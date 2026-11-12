# Validation report

Initial implementation/verification date: 2026-11-09; selectable-engine follow-up
2026-11-10. Host: Windows x64, Node 22,
stable Rust, WebView2. Camera fixtures were synthetic localhost RTSP sources;
synthetic-media tests used no physical camera credentials or LAN configuration changes. A separate, explicitly selected-interface discovery probe is recorded below.

| Check | Evidence |
| --- | --- |
| `pnpm test` | Vitest shell/layout/fallback logic and Rust workspace tests pass |
| `pnpm lint` | Strict TypeScript, ESLint, cargo fmt and Clippy with warnings denied pass |
| Real sidecar integration | RTSP video probe, MSE binary delivery/producer bytes, blocked gateway management routes, forced crash/restart and registration replay pass |
| Real camera-service integration | Connection receipt required; one-use/stale receipt guards; credentials excluded from snapshots; locked keychain creates no camera; crash between test and save preserves one camera and restores playback |
| Persistence/security | SQLite reopen/settings/order/identity/delete checks pass; database bytes and redacted logs contain no test password or full credential URL |
| Native Windows credential store | Temporary OS keychain write/read/delete round trip passes |
| Identity recovery | Swapped-IP, UUID/serial fallback, conflicting identity, ambiguity/proxy ARP and backoff tests pass; fake-network persistence/change notification integration passes |
| Physical discovery | Explicit physical Wi-Fi interface, ONVIF/mDNS only: one device advertised model C530WS in 5,149 ms with zero warnings; no credentials or configuration changes |
| Discovery | Namespaced WS-Discovery identity, sender-address checks, MAC parsing, scan caps and DNS-SD parser/pointer bounds pass |
| UI with real media | Nine consumers receive advancing video; expansion renders 1280×720 main rather than 640×360 sub; Escape, reorder, layout and hidden-window pause pass |
| Forced HLS fallback | With WebRTC and MSE signalling disabled, fragmented-MP4 HLS renders through hls.js; no uncaught browser errors |
| Setup/discovery UI | Test-before-save, invalidation after edits, explicit interface selection and preservation of discovered RTSP port/name pass |
| Main-only edit regression | Opening an existing camera with no substream preserves the empty path |
| Production assets | Vite production build passes; hls.js is lazy loaded. Its 599 kB fallback chunk produces an informational size warning |
| Packaging | Windows MSI and per-user NSIS installers built, with the verified sidecar, application MIT license, go2rtc license and third-party notices in the bundle manifest |

Real-media checks are reproducible using `pnpm test:media` with FFmpeg/FFprobe and
the fetched sidecar. Ordinary tests intentionally skip OS-keychain and external
fixture checks; they were run explicitly here. The headless IPC harness uses
Tauri's official mocks for frontend command responses. The actual Rust service,
SQLite/keychain and go2rtc code have separate integration tests; the harness is
excluded from production assets.

Native M1 playback was observed in the Tauri window through MSE before the later
workflow changes. Further native computer-use inspection was stopped with the
physical Escape key; no more native UI input was issued. Subsequent UI checks
used the repository's headless test runner, including real video from go2rtc.

## Open acceptance checks

- CI execution: the workspace has no Git repository/remote, so configured GitHub
  workflows have not run. Local Windows checks do not establish a passing matrix.
- Native macOS and Linux launch, lifecycle, keychain and playback, especially
  Linux/WebKitGTK codec/transport behavior. Chromium results do not prove these.
- Add/play the discovered physical C530WS using its camera account, and verify recovery from a real DHCP move. Discovery itself completed in 5.1 seconds.
- Hardware decoding/performance and clean-machine installer usability review.
- Windows signing and macOS notarization: no signing identity was available;
  generated Windows installers are unsigned.
- IPv6 multicast discovery/NDP recovery is not implemented; manual IPv6 RTSP
  addressing is accepted, and unavailable tracking is shown in the camera list.

The user approved Tauri's five transitive MPL-2.0 dependencies with notices and
source links. The exception is recorded in ADR 0004; the resolved
license inventory and generated original notices are included in the repository.
No release, package name, account, repository or installer has been published.

## Review artifacts

- `target/release/bundle/nsis/Watchtower_0.2.0_x64-setup.exe` —
  current-user NSIS installer; Authenticode status `NotSigned`.
- `target/release/bundle/msi/Watchtower_0.2.0_x64_en-US.msi` —
  Authenticode status `NotSigned`.
- `target/release/bundle/SHA256SUMS.txt` — checksums for both final installers.

The installer manifest includes go2rtc and MediaMTX executables and all four
license resources. Synthetic camera processes and the test gateway were stopped;
the temporary playback-capability JSON file was removed. First-launch and live
grid screenshots were visually inspected after rendering.

## RTSP follow-up

Full URLs now test only the supplied stream instead of inferring `/stream2`.
Explicit substream failures identify the successful main stream and explain how
to clear the optional substream. Probe failures expose fixed diagnostic messages,
with bounded upstream bodies kept only in memory. Literal percent sequences in
credentials survive URL reconstruction. Regression checks cover these changes,
including a real RTSP server rejecting a missing stream and a successful retry
after clearing a failing substream. The reported physical-camera failure still
requires a retry with the updated build; its cause has not been confirmed.

The 0.1.1 follow-up disables unnecessary ONVIF talkback negotiation for all
source registrations, including restarts and playback reconciliation. A
socket-level RTSP fixture rejecting talkback and subsequent connections failed
with the previous defaults and passes with receive-only registration on the
first connection. Rejected requests now identify the allowlisted RTSP stage
(DESCRIBE, SETUP, or OPTIONS) without returning upstream text or credentials.

## C530WS diagnostic follow-up

A read-only probe on the selected Wi-Fi interface discovered one C530WS. An
unauthenticated DESCRIBE for `/stream1` returned 401 with Basic and Digest
challenges. Separate credential prompts supplied credentials in memory only;
authenticated Basic and Digest DESCRIBE probes both returned RTSP 406. No
PLAY or SETUP requests were sent by these diagnostics. At the connection-count
check, this Windows host had no established TCP connections to remote port 554.
The Digest calculation passed the standard reference vector. These observations
do not establish whether the camera currently refuses every new RTSP session
or only this request pattern; a fresh VLC comparison is still required.

The diagnostic is a development helper, not bundled application code. Build
`cargo build -p watchtower-discovery --example rtsp_status`, then run
`powershell.exe -NoProfile -STA -File scripts/diagnose-camera.ps1` on Windows.
It currently selects Wi-Fi and requires exactly one discovered C530WS. It never
writes credentials to arguments, files, or tool output. Cancel the native prompt
to stop. Run its pure Digest reference check with `-SelfTest`.

## Selectable engines, 0.2.0

- Native Retina 0.4.20 plus webrtc-rs 0.14.0, go2rtc 1.9.15, and MediaMTX 1.21.3
  are selectable globally and through nullable camera overrides.
- `pnpm test` and `pnpm lint` pass. Migration tests cover old JSON settings and
  existing cameras with no saved preferences, preserving go2rtc in both cases.
- The real-media suite passes 22 browser checks across the three engines,
  with explicit skips for unsupported transports and the separate saved-source
  test. It covers nine advancing videos, quality switching, expansion, reorder,
  pause/resume, preference persistence, override invalidation and explicit WHEP
  playback. Supported HLS fallbacks are forced separately.
- Additional Rust checks pass for native viewer sharing/idle cleanup, native
  playback with both executable paths absent, runtime config privacy, source
  alias sharing, adapter switching, and MediaMTX crash/replay.
- A proxy allowing only one RTSP connection confirms that testing an already-live
  Retina source receives fresh frames without opening a second camera connection.
- With user authorization, three saved cameras were read without changing their
  configuration. They resolve to one distinct credential-free RTSP source. A
  real three-tile grid passed against each engine, with advancing video and
  screenshots in `docs/screenshots/saved-{retina,go2rtc,mediamtx}.png`.
- Source aliases share one physical registration per engine, preventing duplicate
  upstream connections for repeated URLs. Removing an alias preserves other
  viewers; releasing the last alias removes the physical registration.
- All 899 resolved dependency versions have regenerated notices. The five
  approved Tauri MPL dependencies remain the only copyleft exceptions. MediaMTX
  archives and its MIT license are verified from the official release digests.
- The final Windows 0.2.0 release build was checked in its native WebView2 UI.
  All three saved tiles displayed live 1280×720 WebRTC video with Retina selected.
  Saving the global default and restarting preserved Retina and restored all
  three live tiles. Process/socket inspection confirmed no media sidecars and
  one shared upstream RTSP connection. The app remains open for user review.
- Unsigned NSIS and MSI installers were built under `target/release/bundle`,
  with final artifact digests recorded in `SHA256SUMS.txt` in that directory.

Retina currently supports H.264 video over WebRTC without transcoding or audio.
Other codecs require another engine. These Windows/Chromium checks do not
establish macOS or Linux runtime compatibility. No physical C530WS connection
success, signing, clean-machine installation, or CI execution is claimed.

## Retina-only packaging, 2026-11-12

- Standard `pnpm package` succeeds without fetching sidecars. The Windows NSIS
  installer is 7.11 MiB and the MSI is 10.75 MiB (previously 36.57/46.16 MiB).
- Read-only inspection of the resulting MSI File table confirms its only app
  executable is `watchtower-desktop.exe`. The other payload files are the app MIT
  license, vendored go2rtc browser-player MIT license, and dependency notices.
  Neither media executable nor the MediaMTX license is present. The generated
  NSIS installation script likewise lists no media sidecars.
- Four packaging-profile tests pass. Desktop engine capability/fallback tests
  pass with no features and with both optional features enabled. `cargo test
  --workspace`, frontend tests (8), frontend lint, formatting, and workspace
  Clippy all pass. A focused Playwright test confirms the native-only picker,
  disabled explanation for an omitted saved engine, and saving Retina.
- Optional Tauri overlays pair Cargo capabilities with the exact requested
  external binaries. Optional installer profiles were not rebuilt in this
  follow-up. Their adapters retain the prior real-stream verification above.
- Notices were regenerated for the same 899 dependency versions; only the
  profile-dependent sidecar description changed. No dependencies were added.
  Installers remain unsigned; no installation or publication was performed.
