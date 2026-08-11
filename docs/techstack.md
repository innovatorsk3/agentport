# Tech Stack & Build

**Date:** 2026-08-11
**Status:** Agreed for v1 · scaffold verified end to end

---

## 1. Decisions

| Layer | Choice | Why |
|---|---|---|
| Desktop shell | **Tauri v2** | ~10 MB app / ~3 MB DMG vs ~80–150 MB for Electron. This app writes a few files and makes a few HTTP calls — it does not need a bundled Chromium. |
| UI | **React + TypeScript + Vite** | Node 22 and pnpm 10 already on the machine. Three form screens; nothing exotic required. |
| Core | **Rust** (Tauri backend) | Filesystem, HTTP, CLI detection. Small surface. |
| Packaging | **Portable, no installer** | The app runs twice in its lifetime (requirements §1) — demanding an install and an uninstall contradicts that. |
| CI | **GitHub Actions** → Releases | macOS runners solve "you cannot build a Mac app on Windows". |

### Why Tauri over Electron

What the app does: write config files, make HTTP test calls, check whether a CLI is on PATH. No heavy computation, no complex UI.

150 MB to write a few config files is disproportionate. The trade is that Tauri uses the **system webview** (WebView2 on Windows, WKWebView on macOS), so the UI can render slightly differently across platforms — acceptable for three form screens.

### Why Rust over Go or Python

**Python was ruled out** — not for the language but for its desktop packaging story. PyInstaller output is [routinely flagged by Windows Defender](https://github.com/pyinstaller/pyinstaller/issues/5854) because malware ships the same way. Combined with an unsigned binary, that is two layers of suspicion on a tool people are asked to download.

**Go + Wails** was the pragmatic alternative — comparable size, gentler learning curve. It was declined because the developer has not written Go either, so its main advantage disappeared.

**Rust + Tauri** won on ecosystem maturity: a first-party `tauri-action` for CI, thorough signing documentation, and a large community. Roughly 90% of the code is TypeScript regardless; the Rust portion is filesystem and HTTP work, which is the gentlest corner of the language.

### Known risks

**Rust learning curve.** If it slows delivery, the fallback is Electron — trading 15× the size for shipping speed. **This decision is reversible.**

**WebView2 on older Windows.** Present on Windows 11 and recent Windows 10. Tauri can bundle it if a real machine reports otherwise.

---

## 2. Layout

```
agentport/
├── src/                      # React UI
│   ├── screens/              # source · list · summary
│   ├── components/
│   └── types/                # bundle types mirroring the Rust side
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs            # Tauri commands
│   │   ├── model.rs          # §7  bundle types — intent, not files
│   │   ├── detect.rs         # §10 is the CLI installed
│   │   ├── scan.rs           # §14 discover existing profiles
│   │   ├── models.rs         # §15 fetch + validate provider model ids
│   │   ├── bundle.rs         # §9  export/import, identity comparison
│   │   ├── writer/           # §7,§8 intent → each CLI's config schema
│   │   ├── shell.rs          # §5,§6 script + ONE rc line
│   │   └── probe.rs          # §10 real call, classify failures
│   └── tests/round_trip.rs   # scan → export → import → install
│   ├── icons/
│   ├── tauri.conf.json
│   └── Cargo.toml
├── .github/workflows/release.yml
├── docs/
└── .gitignore
```

Each core module maps to one requirements section, so the reason it exists stays legible.

---

## 3. Rust ↔ React boundary

**Rust does** everything touching the system:
- Read and write CLI config files
- Scan for existing profiles
- Check whether `claude` / `codex` are on PATH
- Generate the shell script and register one rc line
- Make HTTP test calls
- Read and write bundle files

**React does** rendering, input, and screen state. It **never** touches the filesystem directly.

Rationale: every dangerous operation sits behind a narrow, auditable Rust surface — especially writing to the shell rc (§5), the exact place seven duplicate Antigravity lines came from.

---

## 4. Local build

```bash
# once, if Rust is not installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

pnpm install
pnpm dev                # frontend only
pnpm tauri dev          # full app
pnpm tauri build        # release bundle for the host OS

cd src-tauri && cargo test    # Rust unit tests
```

**Verified on 2026-08-11:** Node 22.22.0 · pnpm 10.15.1 · Rust 1.97.1 · 94 Rust tests passing (91 unit + 3 integration) · release build produced `agentport.app` (9.7 MB) and `agentport_0.1.0_aarch64.dmg` (2.8 MB).

---

## 5. CI → GitHub Releases

### Matrix

| Platform | Runner | Target | Output |
|---|---|---|---|
| macOS Apple Silicon | `macos-latest` | `aarch64-apple-darwin` | `.dmg`, `.app` |
| macOS Intel | `macos-latest` | `x86_64-apple-darwin` | `.dmg`, `.app` |
| Windows x64 | `windows-latest` | `x86_64-pc-windows-msvc` | `.exe` (NSIS) |
| Linux x64 | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `.AppImage`, `.deb` |

Uses `tauri-apps/tauri-action@v1`, triggered by pushing a `v*` tag.

`fail-fast: false` — one platform failing does not cancel the others.

### Stable download URLs

`tauri-action` names artifacts after the app version — `agentport_0.1.0_x64-setup.exe`
— so the filename moves with every release and cannot be fetched blind. A second
upload step copies each artifact to a fixed, platform-labelled name:

```
agentport-windows-x64-setup.exe
agentport-macos-arm64.dmg
agentport-macos-intel.dmg
agentport-linux-x64.AppImage
agentport-linux-x64.deb
```

That is what makes `releases/latest/download/<name>` resolve, so a fresh machine
can `curl` a build without opening a browser.

The release is **published, not drafted**. A draft is invisible to
`latest/download` and to anyone but the repository owner — which would defeat the
purpose.

### Signing — v1 is unsigned

No certificate purchased. Consequences users must know:

**macOS:** Gatekeeper blocks the first launch. Right-click → Open, or Privacy & Security → "Open Anyway".

⚠️ **Required even when unsigned:** `"signingIdentity": "-"` in `tauri.conf.json` (ad-hoc signing). Without it, Apple Silicon builds downloaded from GitHub Releases are reported as **"damaged"** — users read that as a corrupt download rather than a security prompt. Ad-hoc signing does not remove the Gatekeeper warning, but it does remove that misleading one.

Verified present in the local build: `codesign -dv` reports `Signature=adhoc`.

**Windows:** SmartScreen shows "Windows protected your PC" → More info → Run anyway.

Acceptable for internal use. For wider distribution: Apple Developer Program (~$99/year) and a Windows code-signing certificate (a few hundred dollars/year) — *figures worth re-checking.*

---

## 6. Security: bundles carry keys

Per requirements §7, bundles carry keys in **plaintext** — deliberate, for internal use.

Consequences for a public repository:

- `.gitignore` blocks `*.agentport`, `*.agentport.json`, `**/agentport-bundle*`
- The extension is deliberately **distinctive**, not `config.json` — `git add .` forgives nobody
- The README states plainly: **do not commit bundle files**
- Any sample files use fake keys and an `.example` suffix

This is not a theoretical risk: a live API key was found sitting in plaintext in the developer's `.zshrc`.

**Verified:** `git check-ignore` confirms `test.agentport`, `sub/agentport-bundle-2026.json`, and `.env` are all blocked, and `git add -A` does not pick them up.

---

## 7. Still open

- **Linux** — Tauri can produce AppImage/deb, but there is no requirement yet
- **Bundling WebView2** for older Windows — wait for a real machine to report a failure
- **Auto-update** — Tauri offers an updater. For an app that runs twice in its lifetime this is close to pointless; likely dropped
- **Licence** — undecided
