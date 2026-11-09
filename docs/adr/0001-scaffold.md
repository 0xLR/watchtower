# ADR 0001: Desktop scaffold and sidecar pin

Accepted for M0, 2026-11-09.

Use Tauri 2's Vite integration with devUrl on 127.0.0.1:1420 and frontendDist
../dist, following https://v2.tauri.app/start/frontend/vite/ . Production connects
only through Tauri IPC. Bundling stays disabled until packaging is implemented.

Pin go2rtc v1.9.15. Digests were obtained from the upstream GitHub release API:
https://api.github.com/repos/AlexxIT/go2rtc/releases/tags/v1.9.15 .
The manifest stores upstream asset SHA-256 digests, including archive digests.
Verify before extraction. This provides reproducibility and corruption detection;
it does not substitute for upstream release signing.

Use a Node fetcher behind the requested shell wrapper so Windows does not need
Unix checksum tools. macOS/Linux extraction uses unzip; Windows uses PowerShell.
Dependencies use permissive licenses: Tauri (MIT/Apache-2.0), React (MIT), Vite
(MIT), TypeScript (Apache-2.0), ESLint (MIT), Vitest (MIT), serde and ts-rs
(MIT/Apache-2.0), go2rtc (MIT). Recheck licenses when adding dependencies.
