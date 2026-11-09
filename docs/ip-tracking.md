# Identity-based address tracking

Camera UUID is internal identity; addressing may change. The network matcher uses
MAC, ONVIF endpoint UUID and serial evidence, never a successful connection alone.
Any contradictory known identity vetoes a weaker match. Multiple matching hosts
are ambiguous and cannot trigger an address update.

## Capture

After a successful camera connection test, local addresses receive a normal TCP
probe and an OS neighbor-table lookup. Multicast/zero MACs and duplicated/proxy-ARP
MACs are rejected. Locally administered MACs are treated as unreliable so device
UUID or serial can provide identity for randomized addressing. No raw ARP socket
or administrator privileges are required. Selected-interface discovery supplies
ONVIF UUID/model information; GetDeviceInformation attempts serial/model on the
advertised port, Tapo's 2020, or 80/8080 using a WS-Security password digest.

Local hostnames are resolved during setup. Non-local hosts skip identity scanning.
The camera list distinguishes missing identity from missing interface selection.
An explicit camera-network selection is required for recovery scans, including
VPNs and virtual adapters. Up to four selected interfaces are supported.

## Detection and recovery

A backend task checks active producer counters every five seconds. Three failures
spanning at least 15 seconds trigger recovery. Healthy progress resets failures.
Hidden-window pause clears active monitoring. Optional background sweeps run
roughly every three minutes and default to off.

1. Probe the saved address and inspect fresh neighbors. Matching identity at the
   old host means the address did not change; show a quiet stream/settings notice.
2. Run selected-interface WS-Discovery/DNS-SD and match endpoint identity.
3. Fall back to bounded TCP sweeps and neighbor MACs. Custom RTSP ports are also
   considered. Serial fallback enriches candidates with ONVIF device information.
4. Before commit, recheck that the saved camera and identity were not edited or
   removed. Rebuild/push in-memory sources, transactionally update camera and
   last-seen identity, and emit `camera:address-changed`. Roll back media sources
   if persistence fails. The UI quietly reports the address change and reconnects.
5. No or ambiguous match leaves the camera unreachable. Retry delays back off to
   five minutes; stored camera data is retained.

All sweeps are limited to the interface's /24 (at most 254 hosts), even on larger
subnets. The UI warns when the actual subnet is wider. WS/mDNS replies and source
addresses must belong to selected local interfaces. Scans have a 55-second outer
limit, and a shared lock prevents concurrent recovery/manual scans.

The pure matcher and retry policy have offline tests for swapped IPs, missing
identity, conflicts, ambiguity and serial fallback. Fake-network integration
verifies host/last-seen persistence and one change notification. Physical DHCP
reassignment remains a hardware acceptance check.

Automatic discovery and sweep recovery currently enumerate IPv4 interfaces.
Manual IPv6 RTSP addresses are accepted, but IPv6 multicast discovery/NDP recovery
is not claimed; tracking is unavailable when no stable identity is captured.
