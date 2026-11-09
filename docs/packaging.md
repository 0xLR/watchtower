# Packaging and release review

`pnpm package` downloads and verifies the pinned go2rtc sidecar, regenerates
third-party notices and invokes Tauri bundling. Artifacts are under
`target/release/bundle/`. The manual GitHub workflow builds Windows MSI/NSIS,
macOS app/DMG, and Linux DEB/RPM artifacts; it does not publish a release.

The sidecar is bundled as an external binary. All installers include the
application MIT license, go2rtc's MIT license, and a generated dependency notice
file with original texts and versioned source links. The five MPL-2.0 transitive
Tauri dependencies are an explicit user-approved exception (ADR 0004).

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
