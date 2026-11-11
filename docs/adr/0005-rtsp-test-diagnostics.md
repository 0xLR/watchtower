# ADR 0005: RTSP connection tests

Accepted 2026-11-09.

A full RTSP URL identifies one stream. Do not infer a second URL from
`/stream1`; only bare addresses receive Tapo main/sub defaults. Explicit
substreams are tested separately and failures identify the successful main
stream and explain how to clear the optional substream.

The pinned go2rtc probe endpoint returns upstream errors in HTTP failure bodies:
https://github.com/AlexxIT/go2rtc/blob/v1.9.15/internal/streams/api.go
Those bodies may contain credentials. Read at most 16 KiB in memory, remove
URLs, and map recognized errors to fixed user messages. Never return raw errors.
The RTSP client discards most non-200 status codes in favor of `wrong response
on METHOD`, so report a rejected request without asserting a specific status:
https://github.com/AlexxIT/go2rtc/blob/v1.9.15/pkg/rtsp/client.go

Escape literal percent signs before URL userinfo setters to preserve credentials
through the Rust URL encoder and Go URL decoder. Regression tests use synthetic
credentials only.

## Receive-only compatibility follow-up

Register sources with go2rtc's internal `#backchannel=0` option. Watchtower
receives streams and has no talkback feature. The pinned RTSP handler otherwise
adds an ONVIF backchannel requirement and reconnects if rejected, which can
consume a camera's remaining connection capacity. The option is consumed by
go2rtc and does not enter the camera's RTSP URL:
https://github.com/AlexxIT/go2rtc/blob/v1.9.15/internal/rtsp/rtsp.go

A socket-level camera fixture rejects backchannel requests and further connection
attempts. The previous configuration fails; receive-only registration succeeds
on the first connection. This reproduces the compatibility failure, without
claiming it is the cause of the user's physical camera failure. Preserve only
allowlisted DESCRIBE, SETUP, and OPTIONS stages in fixed diagnostic messages;
the pinned engine does not expose the rejected response status.
