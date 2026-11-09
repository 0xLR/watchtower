# Validation report

Local implementation/verification date: 2026-11-09. Host: Windows x64, Node 22,
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
source links. The exception is recorded in plan.md and ADR 0004; the resolved
license inventory and generated original notices are included in the repository.
No release, package name, account, repository or installer has been published.

## Review artifacts

- `target/release/bundle/nsis/Watchtower_0.1.0_x64-setup.exe` — 11,152,558 bytes,
  current-user NSIS installer; Authenticode status `NotSigned`.
- `target/release/bundle/msi/Watchtower_0.1.0_x64_en-US.msi` — 15,253,504 bytes;
  Authenticode status `NotSigned`.
- `target/release/bundle/SHA256SUMS.txt` — checksums for both final installers.

The installer manifest includes the bundled go2rtc executable and all three
license resources. Synthetic camera processes and the test gateway were stopped;
the temporary playback-capability JSON file was removed. First-launch and live
grid screenshots were visually inspected after rendering.
