# Milestone status

The user requested implementation through M6 on 2026-11-09, with final review
following implementation. Local evidence is recorded in validation.md; external
platform/hardware checks remain acceptance gates rather than assumed successes.

| Milestone | Implementation | Verification remaining |
| --- | --- | --- |
| M0 | Workspaces, shell, design primitives, generated IPC, tooling and CI matrix | CI execution and native launch on macOS/Linux |
| M1 | Pinned bundled go2rtc, supervisor, runtime stream management, playback-only gateway | Native lifecycle checks on macOS/Linux |
| M2 | Transport fallback, responsive grid, main/sub switching, expansion, order and visibility pause | Linux/WebKitGTK and macOS playback; hardware decode/performance measurement |
| M3 | SQLite migrations, keychain, tested camera CRUD, settings | Native keychain checks on macOS/Linux |
| M4 | WS-Discovery, RTSP DNS-SD, selected interfaces and capped TCP fallback | Physical camera account/add/play verification; C530WS discovery observed in 5.1 seconds |
| M5 | Pure matcher, backend health watcher, backoff, recovery, transactional updates and events | Physical DHCP reassignment while watching |
| M6 | Empty/error states, keyboard paths, preferences, notices, installer configuration, screenshots and user docs | Signing/notarization credentials and clean-machine installer review |

Recording, PTZ, rewind/timeline, detection and multi-site remain future work.
Future Rust crates and frontend feature directories fit the existing boundaries.
