# ADR 0002: Memory-only media engine and playback gateway

Upstream go2rtc v1.9.15 sources reviewed: internal/app/config.go,
internal/streams/api.go, internal/api/api.go, internal/webrtc/webrtc.go and
www/video-rtc.js at https://github.com/AlexxIT/go2rtc/tree/v1.9.15 .

Launch with inline JSON only. ConfigPath remains disabled. PUT creates a stream
then returns `config file disabled`; accept only this exact error after creation.
DELETE also mutates memory before attempting persistence. Never create a config
file containing camera URLs. Drop child stdout/stderr and disable upstream logs.

HTTP and WebRTC bind only to 127.0.0.1; disable the RTSP and SRT listeners. Both
server and player use empty ICE server lists: no external STUN or cloud traffic.
A Rust playback gateway holds upstream Basic auth and issues a random path
capability per launch. It accepts known stream IDs only, filters WebSocket message
types, sanitizes upstream errors, and exposes no management or static web pages.
The capability is launch-scoped playback access, never a camera credential.

All dependencies added for media use MIT/Apache/BSD/ISC-compatible licenses:
Tokio, axum, reqwest, futures-util, tungstenite, uuid, tracing, async-trait,
thiserror, serde_json. No FFmpeg dependency is bundled.

The supervisor initializes only go2rtc's API, WebSocket, RTSP source, WebRTC,
MP4/MSE and HLS modules through `app.modules`; unrelated protocol/server modules
are not initialized. Upstream main.go initializes the API listener before the
transport modules, so an API response alone is not readiness. Startup and crash
replay require the registered MP4 and HLS handlers to return their expected
`stream not found` health responses before exposing playback.

Vendored MIT VideoRTC lifecycle adaptations guard queued MSE update callbacks
against detached SourceBuffers, bound its pending byte queue, and revoke blob
URLs during teardown. The wrapper owns transport fallback/retry and preserves a
successful WebRTC peer after its signalling socket closes. The upstream source
and full original license remain alongside the vendored file.

For HLS, prefer hls.js when its MediaSource support check succeeds, and use native
HLS otherwise. The local Chromium runtime advertised native HLS as "maybe" and
fetched playlists/segments but did not render the fixture; capability advertising
alone was not sufficient evidence for selecting that path. Forced-fallback tests
exercise actual video rendering and fail on uncaught browser errors.

HLS URLs request `&mp4` so go2rtc uses fragmented MP4 rather than its MPEG-TS
variant. FFprobe accepted the TS fixture, but the webview fallback required fMP4
for reliable fragment parsing. Production CSP permits blob workers for hls.js.
