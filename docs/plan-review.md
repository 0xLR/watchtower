# Plan review

M0 is the first implementation slice. Milestones stay gated on acceptance.

Before M1, configure all go2rtc listeners explicitly: binding the HTTP API to
loopback alone does not constrain RTSP or WebRTC. Decide how the player obtains
short-lived playback access without receiving camera credentials, and document
that protocol. Suppress or redact sidecar diagnostics before capturing logs.

Before M2, measure codec and transport support on all three platform webviews.
Webview hardware decoding is conditional, not an architectural guarantee. HLS
may need an explicit client implementation on platforms without native support.

Before M3, represent full stream URLs without persisting secrets in userinfo,
query strings or path segments. Restrict unsupported credential formats with a
clear error. Test keychain failures and roll back partial saves. Avoid deriving
is_local from a hostname string alone; identity tracking needs validated local
addresses and interface boundaries.

Before M5, reject ambiguous identity matches. A fallback identity must not
silently override a contradictory stronger identity. Add swapped-IP, proxy-ARP,
multiple-interface and duplicate-UUID fixtures before network integration.

These are implementation details to resolve within their milestones, not changes
to the accepted v0.1 scope.
