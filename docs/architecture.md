# Architecture

The Tauri 2 desktop window hosts React. Rust owns cameras, settings, credentials,
discovery observations, media registrations and recovery state. The frontend
uses typed IPC commands/events and connects to a local playback-only gateway.
Rust domain structs generate TypeScript through ts-rs into `packages/ipc-types`
when `cargo test` runs. CI checks that regeneration produces no diff.

| Boundary | Responsibility |
| --- | --- |
| watchtower-core | Pure domain types, address parsing, safe URL construction, redaction and network/media traits |
| watchtower-store | SQLite, transactional camera/identity updates, ordering and forward-only migrations |
| watchtower-secrets | Native Windows Credential Manager, macOS Keychain, Linux Secret Service; memory fake for tests |
| watchtower-discovery | Selected-interface IPv4 WS-Discovery, RTSP DNS-SD/mDNS, bounded TCP probes and neighbor tables |
| watchtower-netid | Pure identity matcher, retry policy and recovery orchestration behind fakeable traits |
| watchtower-media | Supervised go2rtc, runtime stream API, safe producer counters and playback gateway |
| watchtower-onvif | GetDeviceInformation with WS-Security UsernameToken digest; no PTZ |
| desktop backend | Application wiring, tested-save receipts, IPC, health watcher and events |

## Media isolation

The pinned go2rtc binary is bundled beside the executable. Development fetches
use upstream SHA-256 digests checked into the repository. The supervisor retries
port selection/startup, restarts crashed children and replays in-memory stream
registrations. Stream mutations and replay are serialized.

API and WebRTC listeners bind to random ports on 127.0.0.1. RTSP and SRT server
listeners are disabled. Server and browser ICE server lists are empty: no public
STUN servers. API Basic credentials are generated for each launch and checked
for loopback requests as well (`local_auth: true`).

Launch uses inline configuration, never a camera config file. Upstream PUT and
DELETE mutate memory before returning `config file disabled`; that precise
condition is handled without enabling persistence. Child output is discarded,
SQL statement logging is disabled, and complete tracing records pass through a
URL-redacting writer. Network errors are mapped to fixed user-facing messages.

The gateway binds to another random loopback port with a random path capability.
It exposes known stream IDs only, filters WebSocket message kinds, replaces
upstream error payloads, and allows only playback/HLS routes. It does not expose
management endpoints, source URLs or API Basic credentials. Browser origins are
restricted to the Tauri application and local dev origin. Production CSP permits
local assets, IPC, loopback media and blob media; there are no remote assets.

Graceful exit explicitly stops go2rtc. Windows children are assigned to a job
with kill-on-close so termination of the parent also kills the sidecar. Linux
sets a parent-death signal. macOS graceful shutdown is implemented; abrupt parent
termination still needs platform testing.

## Player and grid

The player adapts the pinned MIT go2rtc VideoRTC implementation using a
`video-stream` custom element with Watchtower's own controls. It tries WebRTC,
then MSE, then native HLS or BSD-licensed hls.js. Each attempt has a timeout;
stalled playback advances fallback and ultimately offers an explicit retry plus
automatic retry after 15 seconds. Successful WebRTC closes signalling without
closing the peer connection. MSE/HLS buffers remain live rather than accumulating
recordings.

Tiles prefer substreams; expansion switches to main and Escape returns. Automatic
layout fits the count; explicit column choices and responsive widths can produce
additional scrollable rows. Drag and keyboard-accessible arrow controls persist
order. Document visibility and Tauri minimized/visible state unmount active
transports while hidden. Pause state also suspends failure-driven recovery.

Windows native WebView2 displayed a real synthetic RTSP source through MSE.
Headless Chromium is used for reproducible nine-consumer, quality-switch and
fallback tests. Linux/WebKitGTK and macOS runtime playback have not been verified
in this environment. On Linux the implemented candidate fallback is MSE, then
native HLS/hls.js; codec support and hardware decoding depend on installed webview
and media libraries. Do not claim Linux runtime acceptance from Chromium tests.

## Data and save semantics

Every camera has a UUID. IP addresses are mutable attributes. Camera credentials
are sent through IPC only when entered and are never returned to the frontend.
A successful connection test creates a two-minute in-memory receipt containing
the exact tested configuration. Save consumes that receipt once, checks for stale
edits/removal, writes the keychain and transactionally saves camera/identity data.
A failed database save restores prior credentials. If the engine stops after a
successful test and persistence, saving still returns the saved camera; playback
reconciles its source after restart, avoiding duplicate cameras from a misleading
save error. Playback reconciliation is serialized with configuration changes. Keychain failure does not
create a camera. Removing a camera deletes its credential and cascades identity;
a database failure attempts credential restoration.

Passwords remain only in the keychain and necessary transient memory. URL
userinfo is parsed into separate credential fields. Query keys are allowlisted,
fragments and credential-shaped paths are rejected, and SQLite never stores a
full source URL. Local DNS names resolving exclusively to private addresses are
resolved during setup before identity capture. Non-local hosts receive no scans
or address tracking. Secrets in arbitrary proprietary path formats are unsupported.

Test-only fixture entrypoints and the debug data-directory override are excluded
from release behavior. Recording, PTZ, timeline and detection remain separate
future boundaries rather than partial features hidden in the current code.
