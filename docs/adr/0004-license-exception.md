# ADR 0004: Approved Tauri license exception

Accepted by explicit user reply on 2026-11-09:
"Allow these Tauri dependencies and include notices (Recommended)".

The original permissive-only rule conflicts with Tauri's transitive dependency
licenses. The user approved retaining these unmodified MPL-2.0 crates:

| Crate | Locked version | Versioned source |
| --- | --- | --- |
| cssparser | 0.37.0 | https://crates.io/crates/cssparser/0.37.0 |
| cssparser-macros | 0.7.1 | https://crates.io/crates/cssparser-macros/0.7.1 |
| dtoa-short | 0.3.5 | https://crates.io/crates/dtoa-short/0.3.5 |
| option-ext | 0.2.0 | https://crates.io/crates/option-ext/0.2.0 |
| selectors | 0.38.0 | https://crates.io/crates/selectors/0.38.0 |

Original notices and source links are generated into THIRD_PARTY_NOTICES.md and
bundled with the application. The full resolved inventory is in
`docs/dependency-licenses.json`. Other new dependencies remain subject to the
permissive-license rule. Watchtower's own source remains MIT licensed.

Storage/keychain dependencies use sqlx (MIT/Apache-2.0) and keyring 3.6.3
(MIT/Apache-2.0). Keyring's native feature selection follows
https://docs.rs/keyring/3.6.3/keyring/ . UI HLS uses hls.js (BSD-2-Clause).
The fixture server is MediaMTX v1.21.3 (MIT), pinned to upstream release digests;
FFmpeg/FFprobe are separately installed development prerequisites, not bundled.
