# Watchtower agent rules

1. Work one milestone at a time, in order. Finish and verify before moving on.
2. Before coding, restate acceptance criteria and list intended files.
3. Keep crates small. No network or database I/O in watchtower-core.
4. Never log, persist or display passwords or credential-bearing URLs. Use redaction.
5. No recording, PTZ, detection or other future features in v0.1.
6. Write tests first for pure logic (identity matcher, URL parsing, layout math).
7. Verify API assumptions against current upstream docs; record decisions in docs/adr.
8. Ask when requirements conflict.
9. Follow docs/design-system.md; all feature colors must use design tokens.

10. License policy: permissive dependencies, plus the user-approved Tauri MPL-2.0 exceptions. Regenerate third-party notices when dependencies change.
