# ADR 0003: Tested saves and identity recovery

Camera configuration is separated from secrets. CameraInput is deliberately not
Debug, and camera passwords are never included in Camera/CameraView or snapshots.
A successful main/substream probe produces a short-lived, one-use in-memory save
receipt. Save verifies that an edited camera still has the original configuration
before committing; a removed camera cannot be resurrected by a stale receipt.
Keychain failures stop saving, and database failures attempt credential rollback.

ONVIF device information uses WS-Security UsernameToken digest as specified at
https://docs.oasis-open.org/wss/v1.1/wss-v1.1-spec-pr-UsernameTokenProfile-01.htm .
The device operation/schema follows
https://www.onvif.org/ver10/device/wsdl/devicemgmt.wsdl . Responses are size-capped,
DTD parsing is disabled, redirects/proxies are disabled and credential-bearing
responses/URLs never become diagnostics. PTZ remains out of scope.

WS-Discovery uses the ONVIF-compatible 2005 discovery/addressing namespaces.
Reference protocol: https://docs.oasis-open.org/ws-dd/discovery/1.1/os/wsdd-discovery-1.1-spec-os.html .
All scans require explicit interface selection; name-based virtual-interface hints
are advisory. This avoids pretending the interface library can prove that an
adapter is physical. IPv4 sweeps never exceed /24 or require raw sockets.

Recovery follows identity evidence and rejects contradictions or ambiguous hosts.
A commit rechecks the full original camera and identity before replacing sources
and SQLite addressing, preventing a scan result from overwriting later edits.
The hardware/platform cases listed in validation.md remain explicit acceptance
work rather than assumptions.

If media registration fails after SQLite commits, the camera is still saved.
Return its ID and mark it unreachable; a later playback request reconciles the
exact stored source. Do not return an unsaved-camera error after persistence.
A real-sidecar regression test covers a crash between connection test and save.
