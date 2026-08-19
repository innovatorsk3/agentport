# Changelog

## InnovPort 0.2.0 - 2026-08-19

### Changed
- Renamed the product, package, binary, Tauri app, generated scripts, and release assets from Agentport to InnovPort.
- New exports use `.innovport`, and new generated files live under `~/.innovport`.

### Compatibility
- Existing `.agentport` bundles remain importable and writable.
- Existing `~/.agentport` profile scripts and shell startup lines migrate to the InnovPort paths on install.

### Security
- Bundle files remain git-ignored because they contain API keys in plaintext by design.
