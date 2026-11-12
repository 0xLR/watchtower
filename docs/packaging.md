# Packaging and release review

`pnpm package` builds a Retina-only installer: no go2rtc or MediaMTX download,
executable, or MediaMTX license resource is required. Retina runs inside the app.
Artifacts are under `target/release/bundle/`.

Optional build profiles retain the sidecar adapters:

| Command | Included engines |
| --- | --- |
| `pnpm package` | Retina |
| `pnpm package:go2rtc` | Retina + go2rtc |
| `pnpm package:mediamtx` | Retina + MediaMTX |
| `pnpm package:all-engines` | Retina + both sidecars |

Each optional command fetches and checksum-verifies only its selected binaries,
regenerates notices, and applies a Tauri config overlay that enables the matching
Cargo features and external binaries together. For development, fetch the desired
sidecar with its `scripts/fetch-*.mjs` helper and run, for example,
`pnpm --filter @watchtower/desktop tauri dev --config src-tauri/tauri.go2rtc.conf.json`.
Using Cargo features alone does not add executables to an installer; use the profile.
These are build-time choices, with no runtime downloads or external executable picker.
Existing files in the source `binaries` directory are not bundled by the native profile.

The engine picker exposes only engines included in the build. Saved preferences
for omitted engines are preserved; playback and connection tests use Retina with
a visible notice. H.264 video is required for that fallback. Reinstalling a build
with the selected sidecar restores its use without editing camera preferences.

The manual GitHub packaging workflow offers the same four profiles and does not
publish releases. All installers include the application MIT license, the go2rtc
MIT license for the vendored browser player, and generated dependency notices.
MediaMTX's MIT license is added with its executable. Dependency notices remain a
conservative superset across profiles. The five Tauri MPL dependencies are the
approved exceptions (ADR 0004). No dependency versions changed for this work.

Signing credentials were not available during implementation. Windows builds
are unsigned. macOS signing/notarization and Windows certificate configuration
must be supplied through the standard Tauri signing environment/configuration
before a signed release. No certificate is generated or identity fabricated.
See https://v2.tauri.app/distribute/sign/windows/ and
https://v2.tauri.app/distribute/sign/macos/ for maintainer setup.

Runtime targets remain Windows 10/11 and macOS 12+. Linux needs a compatible
WebKitGTK 4.1 stack and a running, unlocked Secret Service. No browser product,
public server, updater service or telemetry is added. Installer prerequisites
may install the platform webview runtime when necessary.

Before a public release: run the CI matrix; review installers on clean machines;
verify a physical Tapo camera and a DHCP move; verify Linux playback and macOS
lifecycle; supply signing/notarization; then check project-name/package collisions.
The existing containrrr/watchtower Docker updater is unrelated. Nothing has been
published or registered under this name by this implementation.

MediaMTX upstream prebuilt archives cover Windows x64, macOS x64/arm64 and Linux
x64/arm64 in the checked-in manifest. A Windows ARM64 packaging target would need
a separately verified MediaMTX build; it is not advertised as a completed target.
