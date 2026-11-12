# ADR 0007: Retina-only default and optional sidecar profiles

Accepted 2026-11-12.

Retina works well for the user's streams. Bundling two unused executables adds
downloads and installer size. The standard desktop build therefore includes no
external binaries and needs neither sidecar present in the source checkout.

Keep the existing adapters and offer three explicit Tauri overlays: go2rtc,
MediaMTX, and both. Each overlay sets `build.features` and `bundle.externalBin`
together; root packaging scripts fetch only its selected binaries. Desktop
features control selectable engines, not dependency versions. Adapter source
remains in the media crate for shared fixtures and optional builds.

The backend supplies available engines in the snapshot. The UI offers those
engines, while showing a disabled explanation for an unavailable saved choice.
Resolve unavailable choices to Retina for registration, playback, and probes;
show a warning and retain original stored preferences. This avoids rewriting
camera configuration when switching installer profiles. It does not add codec
conversion: Retina still requires H.264 video.

Use build-time options rather than runtime downloads, arbitrary executable paths,
or a new plugin manager. Optional builds retain pinned archive verification,
private loopback gateways, supervision, and license resources. The vendored
go2rtc browser player still requires its MIT notice in the native build.

Verified upstream configuration behavior:

- [Tauri config extension](https://v2.tauri.app/develop/configuration-files/#extending-the-configuration)
  supports CLI config overlays using JSON Merge Patch; arrays replace, objects merge.
- [BuildConfig](https://v2.tauri.app/reference/config/#buildconfig) supports
  `features` passed to Cargo.
- [External binaries](https://v2.tauri.app/develop/sidecar/) are explicitly listed
  in `bundle.externalBin`, with target-triple suffixed input files.

No new dependencies or license exceptions are introduced. Existing generated
notices intentionally cover all optional profiles.
