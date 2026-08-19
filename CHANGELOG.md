# Changelog

## InnovPort 0.2.1 - 2026-08-19

### Fixed
- Preserve the legacy Tauri application identifier so existing installs can upgrade in place.
- Remove stale Agentport shell source lines when both legacy and InnovPort paths are present.
- Restrict generated bundle, script, and CLI overlay files to the owner on Unix systems.

## InnovPort 0.2.0 - 2026-08-19

### Changed
- Renamed the product, package, binary, Tauri app, generated scripts, and release assets from Agentport to InnovPort.
- New exports use `.innovport`, and new generated files live under `~/.innovport`.

### Compatibility
- Existing `.agentport` bundles remain importable and writable.
- Existing `~/.agentport` profile scripts and shell startup lines migrate to the InnovPort paths on install.

### Security
- Bundle files remain git-ignored because they contain API keys in plaintext by design.
