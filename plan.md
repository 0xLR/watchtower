# Watchtower: Implementation Plan

A plan for an AI coding agent to build an open-source desktop app that discovers IP cameras, lets the user add RTSP streams, and shows them in a live grid. Target cameras: Tapo (RTSP on 554, ONVIF on 2020). Recording, PTZ, rewind and detection come later, so the architecture must leave room for them without building them now.

Implementation exception approved by the user on 2026-11-09: retain Tauri's
transitive MPL-2.0 crates (`cssparser`, `cssparser-macros`, `dtoa-short`,
`option-ext`, `selectors`), with original notices and versioned source links.
All other new dependencies must follow the permissive-license rule.

## 1. Goals and non-goals

**v0.1 goals**

- Discover cameras on the local network.
- Add a camera by IP or full stream URL.
- Keep a camera reachable when its DHCP address changes, with no manual edits.
- View any number of cameras in a grid; expand one tile to full size.
- A distinctive, restrained UI (section 7).

**Non-goals for v0.1:** recording, PTZ, timeline or rewind, motion or object detection, cloud anything, accounts, telemetry.

## 2. Architecture decisions

| Concern | Decision | Why |
| --- | --- | --- |
| App shell | Tauri 2 (Rust backend, webview frontend) | The webview decodes video with hardware acceleration; no GPU video code to write |
| Media pipeline | go2rtc as a bundled sidecar binary | Pulls each camera once, serves WebRTC/MSE/HLS, handles reconnects, has an HTTP API to add or change streams at runtime |
| Frontend | React + TypeScript + Vite, plain CSS with design tokens | Agents know it well; no component library, so the look stays custom |
| Storage | SQLite via `sqlx` (migrations checked in) | Single file, easy to extend for recordings and events later |
| Secrets | OS keychain via the `keyring` crate | Camera passwords never go in SQLite or logs |
| Backend structure | Cargo workspace of small crates | Clear boundaries; future features become new crates |

Key rules:

- go2rtc binds to `127.0.0.1` on a random free port, with its API credentials generated at launch. Nothing is exposed to the LAN.
- The Rust side owns all state. The frontend only talks to Rust through typed Tauri commands and events, and to go2rtc only for video playback.
- Stream URLs with credentials are built in memory at the moment they are pushed to go2rtc. They are never persisted or logged.
- Every camera has a stable internal ID (UUID). IPs are attributes, not identity.

**Linux risk:** Tauri uses WebKitGTK there, and its WebRTC and MSE support can be flaky. Test early (milestone M2). The player layer must support falling back from WebRTC to MSE to HLS.

## 3. Repository layout

```
watchtower/
├── README.md
├── LICENSE                      # MIT
├── AGENTS.md                    # Rules for coding agents (section 9)
├── Cargo.toml                   # Workspace root
├── package.json                 # JS workspace root (pnpm)
├── pnpm-workspace.yaml
├── rust-toolchain.toml
├── justfile                     # dev, build, test, lint, sidecar fetch
├── .github/workflows/           # CI: lint, test, build matrix (win, mac, linux)
├── docs/
│   ├── architecture.md
│   ├── design-system.md
│   ├── ip-tracking.md
│   ├── adr/                     # Architecture decision records, one file each
│   └── roadmap.md
├── scripts/
│   ├── fetch-go2rtc.sh          # Downloads pinned go2rtc per target triple
│   └── dev-fake-camera.sh       # Local RTSP test source via ffmpeg/mediamtx
├── crates/
│   ├── watchtower-core/         # Domain types, traits, errors; no I/O
│   ├── watchtower-store/        # SQLite repositories + migrations
│   │   └── migrations/
│   ├── watchtower-secrets/      # Keychain wrapper behind a trait
│   ├── watchtower-discovery/    # WS-Discovery (ONVIF), mDNS, ARP/neighbor table
│   ├── watchtower-netid/        # Identity resolution and IP tracking (section 5)
│   ├── watchtower-media/        # go2rtc supervisor + API client
│   └── watchtower-onvif/        # Stub now; PTZ and device info later
├── apps/
│   └── desktop/
│       ├── src-tauri/
│       │   ├── Cargo.toml
│       │   ├── tauri.conf.json
│       │   ├── capabilities/
│       │   ├── binaries/        # go2rtc sidecars (gitignored, fetched by script)
│       │   └── src/
│       │       ├── main.rs
│       │       ├── commands/    # One file per command group
│       │       ├── events.rs    # Typed events pushed to the frontend
│       │       └── state.rs     # App state wiring
│       └── src/                 # Frontend
│           ├── main.tsx
│           ├── app/             # Shell, routing, providers
│           ├── features/
│           │   ├── grid/        # Tile layout, expand, ordering
│           │   ├── cameras/     # Add/edit/remove, list
│           │   ├── discovery/   # Scan UI and results
│           │   └── settings/
│           ├── player/          # Stream player with transport fallback
│           ├── design/          # Tokens, base CSS, primitives (Panel, Button, Field, Readout)
│           ├── lib/             # Typed Tauri client, utilities
│           └── tests/
└── packages/
    └── ipc-types/               # Generated TS types from Rust (specta or ts-rs)
```

Future crates slot in beside the current ones: `watchtower-ptz`, `watchtower-record`, `watchtower-timeline`, `watchtower-detect`. Future frontend features go in `features/ptz`, `features/timeline`, and so on.

## 4. Data model (v0.1)

**cameras**

- `id` (UUID, primary key), `name`, `created_at`, `sort_order`
- `host` (current IP or hostname), `port`, `path_main`, `path_sub`
- `username` (password lives in the keychain under the camera ID)
- `source_kind` (`rtsp` | `onvif_discovered` | `manual_url`)
- `is_local` (bool: host is a private-range address)

**camera\_identity** (one row per camera, all nullable except where noted)

- `camera_id`, `mac` (normalized lowercase), `onvif_endpoint_uuid`, `serial`, `model`
- `last_seen_at`, `last_seen_host`

**settings**: key/value for grid layout, default substream behavior, theme options.

All schema changes are forward-only migrations in `watchtower-store/migrations/`.

## 5. IP change tracking (the important part)

**Principle:** identity is the MAC address (with ONVIF endpoint UUID and serial as backups), and the IP is just where the camera currently is.

**Capturing identity when a camera is added**

1. For a local IP, read the MAC from the OS neighbor table (ARP/NDP). Send a TCP connect or ICMP probe first so the entry exists.
2. If ONVIF responds (port 2020 for Tapo, 80/8080 elsewhere), also store the endpoint UUID and serial from `GetDeviceInformation` or the WS-Discovery response.
3. Skip all of this for non-local hosts. Remote URLs and hostnames outside private ranges get no tracking.

**Detecting a change**

- A health watcher in `watchtower-netid` subscribes to go2rtc stream status. When a stream fails repeatedly (suggested: 3 failures over about 15 seconds), it starts recovery for that camera.
- A slow background sweep (every few minutes, off by default for battery, configurable) also rechecks identity so a changed IP is caught even before a stream drops.

**Recovery procedure**

1. Probe the last known IP. If a device answers with the same MAC, the problem is not an IP change; surface the real error.
2. Run WS-Discovery on all local interfaces and match by endpoint UUID.
3. Otherwise sweep the local subnet(s) with a fast TCP probe on the camera's port (554, 2020), then read the neighbor table and match by MAC.
4. On a match: update `host` and `last_seen_*` in SQLite, rebuild the source URL, push it to go2rtc via its API (`PUT /api/streams`), and emit a `camera:address-changed` event. The UI shows a quiet notice.
5. No match: mark the camera `unreachable`, keep retrying with backoff (cap around 5 minutes), and never delete data.

**Edge cases the agent must handle**

- Multiple network interfaces, VPNs, and Docker bridges: scan only physical or user-selected interfaces.
- Subnet size: cap sweeps (for example /24 by default; warn above /22).
- Two cameras swapping IPs: match strictly by identity, never by "something answered at the old IP".
- MAC randomization and routers that proxy ARP: if the MAC cannot be read reliably, fall back to ONVIF UUID, then serial; if none exist, disable tracking for that camera and say so in the UI.
- Privileges: raw ARP scanning may need elevated rights. Prefer reading the OS neighbor table after ordinary TCP probes.

The logic must be pure and unit-testable: the matcher takes a list of observed devices and a camera identity, and returns a decision. Network access sits behind traits so tests use fakes.

## 6. Milestones

Each milestone ends with a working build, passing CI, and updated docs. The agent should not start the next milestone until the acceptance criteria are met.

**M0: Scaffold**

- Create the workspace and layout from section 3, CI, `justfile`, `AGENTS.md`, and `fetch-go2rtc.sh` with a pinned version and checksum.
- Tauri window opens with an empty shell using the design tokens.
- *Accept:* `just dev` runs on Windows, macOS and Linux; `just test` and `just lint` pass.

**M1: Media pipeline**

- `watchtower-media` launches go2rtc as a supervised sidecar on a random localhost port, restarts it on crash, and exposes `add_stream`, `update_stream`, `remove_stream`, `stream_status`.
- *Accept:* a hard-coded RTSP test stream (from `dev-fake-camera.sh`) can be added and plays in a bare `<video>` inside the app.

**M2: Player and grid**

- `player/` wraps go2rtc's `video-stream` component with transport fallback (WebRTC, then MSE, then HLS) and clear loading, error and retry states.
- `features/grid/`: auto-layout (1, 2x2, 3x3, 4x4, and fit-to-count), click to expand, Esc to return, drag to reorder.
- Tiles use the substream; the expanded view switches to the main stream.
- Tiles pause when the window is hidden.
- *Accept:* 9 test streams play smoothly; expanding switches quality; Linux/WebKitGTK behavior documented in `docs/architecture.md` with the fallback that works.

**M3: Cameras and storage**

- SQLite store, keychain wrapper, add/edit/remove camera UI. Accept a bare IP (with Tapo-aware default paths `/stream1` and `/stream2`), a host, or a full `rtsp://` URL.
- A "test connection" step before saving.
- *Accept:* cameras and credentials survive restart; passwords never appear in the DB file or logs (add a test that scans both).

**M4: Discovery**

- WS-Discovery probe on selected interfaces, results listed with name, IP, model if available. One click adds a discovered camera and prompts for credentials.
- Manual rescan, and a subnet-sweep fallback for devices that don't answer ONVIF.
- *Accept:* a Tapo camera on the LAN is found and added in under 60 seconds.

**M5: IP tracking**

- Implement section 5 end to end, with the pure matcher fully unit-tested and an integration test using a fake network.
- *Accept:* changing a camera's DHCP address while the app is running restores the tile without user action, and the stored IP updates.

**M6: Polish and packaging**

- Empty states, error copy, keyboard navigation, settings, signed installers where possible, README with screenshots.
- *Accept:* a new user can go from install to a working grid without reading docs.

## 7. Design direction

The feel to aim for is a quiet instrument panel, like the screens in well-art-directed sci-fi films: dark, precise, information-dense, calm. Not neon, not terminal green, and not the generic AI look.

**Do**

- **Palette:** near-black neutrals with a slight cool tint (for example surfaces around `#0B0D0F`, `#111418`, `#171B20`), hairline borders at low opacity, off-white text with two dimmer levels. One accent only, used sparingly for focus, active state and live indicators. Amber, tried first: a muted amber (`#D9A441`). It lives in one token so it can be swapped if it doesn't land. A separate desaturated red for fault states only.
- **Shape:** sharp or near-sharp corners (2px radius at most). 1px hairlines. Corner tick marks or small bracket details on video tiles instead of rounded cards.
- **Type:** a clean grotesque for UI (Inter, IBM Plex Sans or Geist) and a monospace for readouts (JetBrains Mono or IBM Plex Mono) with tabular numerals. Small uppercase labels with generous letter spacing for metadata. Tile overlays show camera name, a quiet live clock, and resolution or latency in mono.
- **Layout:** video is the hero. Chrome stays thin and recedes; panels slide in from the edge and don't cover tiles unless needed.
- **Motion:** slow, linear or gentle ease; short fades; an unhurried reveal when a tile connects. No bounce, no glow pulses, no typing effects.
- **States:** disconnected tiles show a flat dark field with a thin diagonal-hatch or grid texture and a precise status line, not a spinner.

**Avoid (explicitly)**

- Rounded pill badges, gradient chips, multicolor tag clusters, sparkle icons, emoji in the UI.
- Neon cyan or green glows, scanlines, glitch effects, fake hex dumps, matrix rain, skull or hooded-hacker imagery.
- Gradient hero backgrounds, glassmorphism everywhere, purple-to-blue gradients.
- Drop shadows as the main depth cue; use tone steps and hairlines.

Put these in `src/design/tokens.css` as CSS variables first, build the primitives (`Panel`, `Button`, `Field`, `Readout`, `StatusLine`, `Tile`) in `design/`, and document them in `docs/design-system.md`. No feature code may use raw color values; everything goes through tokens.

## 8. Quality bar

- **Rust:** `cargo fmt`, `cargo clippy -D warnings`, `cargo test` per crate. No `unwrap()` in non-test code. Errors via `thiserror` in libraries.
- **Frontend:** strict TypeScript, ESLint, Vitest for logic, Playwright smoke test of the shell where feasible.
- **Types across the boundary** are generated, never hand-duplicated (`packages/ipc-types`).
- **Logging:** `tracing`, with a redaction layer that masks credentials in any URL.
- **Network code behind traits** (`NeighborTable`, `Discoverer`, `StreamBackend`) so everything is testable offline.
- **Dependencies:** permissive licenses only (MIT, Apache-2.0, BSD). go2rtc is MIT; confirm any new dependency's license before adding it.

## 9. Instructions for the coding agent (copy into `AGENTS.md`)

1. Work one milestone at a time, in order. Finish and verify before moving on.
2. Before writing code for a milestone, restate its acceptance criteria and list the files you intend to create.
3. Keep crates small and single-purpose. Do not put network or database access in `watchtower-core`.
4. Never log, persist or display passwords or credential-bearing URLs. Use the redaction layer.
5. Do not add recording, PTZ, detection or any non-v0.1 feature, but keep the seams the plan describes (traits, crate layout, schema room).
6. Write the test first for pure logic (the IP matcher, URL parsing, layout math).
7. When an assumption is needed (for example an API detail of go2rtc), check the current docs rather than guessing, and record the decision as an ADR in `docs/adr/`.
8. If a requirement conflicts with another, stop and ask instead of choosing silently.
9. Keep the UI within the design rules in section 7; reject any shortcut that introduces avoided patterns.

## 10. Future roadmap (design for it, don't build it)

| Feature | Where it lands | Notes |
| --- | --- | --- |
| PTZ | `watchtower-onvif` + `features/ptz` | ONVIF `ContinuousMove`, `Stop`, presets; Tapo pan/tilt models on port 2020 |
| Recording | `watchtower-record` | Segment go2rtc's stream to disk (fMP4); retention policy; schema adds `recordings` |
| Rewind on live | `watchtower-timeline` | Needs a rolling buffer, so builds on recording; UI scrubber in the expanded tile |
| Motion detection | `watchtower-detect` | Start with frame-difference on substreams; events table; optional camera-side ONVIF events |
| Object detection | `watchtower-detect` | Pluggable backend trait (ONNX Runtime first); runs on substreams only |
| Multi-site / remote | later | Out of scope; keep identity separate from addressing so it stays possible |

## 11. Decisions

- **Name:** Watchtower. Crates use the `watchtower-` prefix. Before publishing, check GitHub, crates.io and npm for clashes (the Docker updater containrrr/watchtower is widely known) and give the repo a distinctive description so searches find this project.
- **License:** MIT.
- **Accent color:** muted amber (`#D9A441`), tried first and swappable through a single token.
- **Platforms:** Windows 10 and 11, Linux, and macOS 12 (Monterey) or later.
- **Desktop only:** no browser mode. go2rtc listens on localhost only, its own web page is never opened or linked, and the app window is the only interface.
