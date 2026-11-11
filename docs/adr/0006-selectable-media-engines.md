# ADR 0006: Selectable media engines

Accepted 2026-11-10. The user approved keeping go2rtc and MediaMTX executables
while adding a native Rust engine.

`watchtower-media::Media` is a facade over lazy engine adapters. Global settings
choose the default; nullable camera overrides take priority. Old settings missing
the field deserialize to go2rtc; a new profile defaults to Retina. A forward-only
SQLite migration adds the nullable override. Changing the default invalidates
connection-test receipts. UI engine changes reconnect players.

The migration also preserves go2rtc for an existing camera database with no
preferences row, so an implicit old default is not changed by upgrading.

Retina 0.4.20 receives RTSP over TCP and emits H.264 Annex B access units with
parameter sets inserted at keyframes. webrtc-rs 0.14.0 delivers video through a
loopback-only WHEP endpoint. The versions follow the API demonstrated by Retina's
WebRTC proxy example, with independent lifecycle and timestamp handling:
https://github.com/scottlamb/retina/tree/main/examples/webrtc-proxy
https://docs.rs/retina/0.4.20/retina/codec/struct.FrameFormat.html

Each native source has a bounded broadcast queue and one reader shared by its
viewers. Slow consumers resume at a keyframe. The reader exits when unused;
removal cancels readers and peers. WHEP sessions are deleted when a player
unmounts, with ICE failure cleanup for interrupted clients. RTP sample intervals
derive from the source's 90 kHz timestamps with bounds for discontinuities.
Native playback currently supports H.264 video only, with no transcoding or audio.
Unsupported streams receive an explicit engine-selection error.

The facade maps camera stream IDs onto opaque physical stream IDs. Identical
source URLs on the same engine share one physical registration, including
main/sub aliases and duplicate camera entries. Removing or changing an alias
releases the physical stream only when its final alias leaves. Matching happens
only in memory; credentials and credential-bearing URLs are not persisted.
Connection tests on an already-live native source consume a fresh frame from
that reader rather than opening a second RTSP session. A one-connection proxy
fixture verifies this behavior for cameras with strict session limits.

go2rtc retains WebRTC/MSE/HLS. MediaMTX 1.21.3 exposes WebRTC/WHEP and fMP4 HLS
through the playback-only gateway. Its release archives are pinned by official
GitHub SHA-256 digests in `scripts/mediamtx-manifest.json`. Both sidecars are
supervised with replay; only selected engines launch. Failed old engines do not
block switching to a working engine. No automatic cross-engine retries open
additional camera connections.

Camera URLs stay in memory. MediaMTX receives them through its authenticated
loopback API. Its temporary config stub contains only `{}` and is deleted on
drop; runtime API mutations do not update that file. All upstream listeners bind
to loopback, management requires a generated secret, playback requires a random
gateway capability, and public STUN/ICE servers are disabled. WHEP session
locations are replaced with gateway locations. Source URLs and raw media errors
never cross IPC. Credentials are separated before Retina receives the URL.

The facade advertises supported transports in `Playback`; fallback skips
unsupported modes. WebRTC and HLS selection remain independent of engine choice.

Upstream references:
https://mediamtx.org/docs/features/configuration
https://github.com/bluenviron/mediamtx/blob/v1.21.3/api/openapi.yaml
https://github.com/webrtc-rs/webrtc/blob/v0.14.0/webrtc/src/api/setting_engine/mod.rs

New crates are permissively licensed. The five existing Tauri MPL exceptions
remain the only approved copyleft dependencies. MediaMTX's MIT license is bundled
alongside go2rtc's; generated crate notices include Retina and WebRTC dependencies.
