# agentport

**Carry your Claude Code and Codex CLI setup to another machine — across operating systems. Export, import, test, done.**

> ⚠️ Work in progress. No runnable release yet.

---

## The problem

You have Claude Code and Codex CLI dialled in on one machine: several providers, an alias per provider, permission bypass switched on so you never type `--dangerously-skip-permissions` again.

Now you are sitting at a different machine. Possibly a different operating system.

Redoing it by hand takes an afternoon — and the failures along the way are **silent**. A config key nested one level too high is ignored without an error. A key loaded into the wrong environment variable still runs. A provider returns `200 OK` on one endpoint and hangs on another.

## The approach

A desktop app you run **twice in its lifetime**: once to export on the old machine, once to import on the new one. It writes config to disk and closes. Your terminal keeps working afterwards — **even if you delete the app**.

## How this differs from provider switchers

Several tools already switch providers for Claude Code. agentport is not competing there:

| | Provider switchers | agentport |
|---|---|---|
| Switch provider on one machine | ✓ | — |
| **Carry setup across machines and operating systems** | ✗ | ✓ |
| **Scan a machine for profiles that already exist** | ✗ | ✓ |
| **Test with a real generation call, then classify the failure** | ✗ | ✓ |
| **Verify model ids against what the provider actually serves** | ✗ | ✓ |

The last two rows matter more than they look.

### Failures are classified, not just reported

These three produce the same "it does not work" symptom but demand completely different responses:

| Response | Meaning | What you do |
|---|---|---|
| `401` | Wrong key | Paste a different key |
| `402` | Out of credit | Top up |
| Timeout / `500` | Provider has not mapped a model to this endpoint | Fix it in the provider's admin panel |

The third one costs twenty minutes of `curl` to identify. agentport tells you in one line.

### Model mapping is verified, not typed

A profile pointing at a model the provider does not serve looks completely
correct in both CLIs and fails only at call time. agentport fetches the real
`/v1/models` list for that key and checks the mapping against it.

Found on the developer's own machine while building this: two Claude profiles
configured for `claude-opus-5`, against a provider serving 14 models and **not
one Claude model among them**. Neither CLI says a word about it.

## Principles

**It does not own your shell rc.** Your `.zshrc` (or `$PROFILE` on Windows) receives exactly **one line, once**. Uninstalling means deleting that line.

**A bundle stores intent, not files.** No absolute paths, no verbatim CLI JSON or TOML. The destination machine translates intent into whatever schema its installed CLI version expects — that is what makes macOS → Windows work.

**It owns the name, not the thing being named.** It creates aliases; it never rewrites your default CLI config or your existing auth.

## ⚠️ Bundles carry API keys in plaintext

This is **deliberate** — an internal tool where keys must travel and stay readable.

Which means: **never commit a bundle file.** `.gitignore` already blocks `*.agentport` and friends, but stay careful anyway.

## Install

Every tagged release publishes builds for macOS, Windows and Linux under stable
filenames, so a fresh machine can fetch one without opening a browser.

**Windows**
```powershell
curl.exe -L -o agentport-setup.exe https://github.com/innovatorsk3/agentport/releases/latest/download/agentport-windows-x64-setup.exe
.\agentport-setup.exe
```

**macOS** — Apple Silicon, or swap `arm64` for `intel`
```bash
curl -L -o agentport.dmg https://github.com/innovatorsk3/agentport/releases/latest/download/agentport-macos-arm64.dmg
open agentport.dmg
```

**Linux**
```bash
curl -L -o agentport.AppImage https://github.com/innovatorsk3/agentport/releases/latest/download/agentport-linux-x64.AppImage
chmod +x agentport.AppImage && ./agentport.AppImage
```

### Neither build is signed

**Windows** — SmartScreen blocks the first run: *More info → Run anyway*.
**macOS** — right-click the app → *Open*, or System Settings → Privacy &
Security → *Open Anyway*.

A code-signing certificate is the only thing that removes those prompts; for an
internal tool it is not worth the yearly fee. macOS builds are ad-hoc signed so
Apple Silicon does not report them as *damaged*, which reads as a corrupt
download rather than a security prompt.

## Carrying a setup to a new machine

On the machine that already works:

1. Open agentport — it scans and offers the profiles it finds
2. **Export bundle** → save the `.agentport` file

On the new machine:

3. Download and run agentport
4. **Import a bundle** → pick that file
5. **Install** → it writes the config, registers one line in your shell rc, and
   tests every profile against its provider

Open a new terminal and your aliases are there.

## Documentation

- [Requirements](docs/requirements.md) — full specification, with the evidence behind each decision
- [Tech stack & build](docs/techstack.md) — Tauri v2 + React, CI, code signing

## Licence

Undecided.
