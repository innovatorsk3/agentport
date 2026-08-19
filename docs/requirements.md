# InnovPort — Requirements

**Date:** 2026-08-11
**Status:** Scope agreed — ready to build

---

## 1. What it is

> A **GUI installer** that carries agent CLI configuration (Claude Code, Codex) from one machine to another — across operating systems — including awkward settings like permission bypass, and proves it works before you walk away.

**Not** an alias manager. Aliases are the surface; the substance is **portability** and **proof**.

**Lifecycle:** open → scan, type, or import → write config to disk → test → close.
The terminal keeps working afterwards, **even if the app is deleted**. Reopen only to change something.

### Success metric

From opening the app on a blank machine to `cht` working in a terminal: **≤ 2 minutes**.
(Doing it by hand: an afternoon, with four silent failures on the way.)

---

## 2. Why — evidence, not assumption

Every major requirement below traces to a **real failure** observed during a debugging session on 2026-08-11:

| # | What actually happened | Requirement it drives |
|---|---|---|
| 1 | Switching profiles overwrote `~/.claude/settings.json` → affected **the whole machine**; the user noticed, not the tooling | §5 Ownership boundary |
| 2 | `defaultMode` placed at the top level instead of inside `permissions` → **silently ignored**, no error anywhere | §7 Bundles store intent · §8 Danger is a scale |
| 3 | A shell function hardcoded `MUST1C_CSE_API_KEY` → the second profile loaded its key into the wrong variable | §6 One env var per profile |
| 4 | A key returned **200 OK** on `/v1/models` but **timed out** on `/v1/responses` | §10 Test with a real generation call |
| 5 | **Seven** duplicate `# Added by Antigravity` lines in one `.zshrc` (plus two duplicate `postgresql@17` lines and two duplicate `NVM_DIR` blocks) | §5 One immutable line · §9 Never spawn copies |
| 6 | Claude profiles configured for `claude-opus-5` against a provider serving **no Claude models at all** — neither CLI reports it | §15 Model setup must be verified |

**Four of these six were silent failures** (#2, #3, #4, #6) — no crash, no log. They looked like success.

---

## 3. Scope

### In scope
- Type in a new profile by hand (first run)
- **Scan this machine for profiles that already exist** and offer them for adoption
- Export a bundle (selectable)
- Import a bundle (cross-OS: macOS ↔ Windows ↔ Linux)
- Generate a shell script and register one line in the shell rc
- Detect whether each CLI is installed
- Real connection test with failure classification
- **Fetch the provider's actual model list** and validate the mapping against it

### Out of scope
- **Installing CLIs** — detect and report only. Becoming a package manager is a bottomless pit.
- **Bundle encryption / passphrases** — internal use; keys travel readable.
- **A resident dashboard that polls** — burns the user's credit; test on demand instead.
- **Editing tier-1 CLI defaults** (see §4).

---

## 4. The three tiers

```
Tier 1 — CLI default config      bare `claude` / `codex`
Tier 2 — named profile           htmustc, htcse
Tier 3 — alias                   cht, co-ht, c
```

**The app only touches tiers 2 and 3.**

**Tier 1** may only be **given a name** (creating `c` → `claude`); its contents are never modified, and it never travels in a bundle.

> `alias c='claude'` ← the app creates this
> `claude` ← the app never touches this

The app owns **the name**, not **the thing being named**. Delete the alias and `claude` still works.

**Consequence to surface:** on a new machine, a bare `codex` will not work if it depends on an environment variable outside any profile. See §10, final screen.

---

## 5. Ownership boundary (non-negotiable)

| The app owns | The app never touches |
|---|---|
| Named profiles | The user's Anthropic auth |
| Aliases pointing at them | Default `~/.claude/settings.json` |
| Its own directory | Base `~/.codex/config.toml` |
| | Environment variables already in the shell rc |

### It does not own the shell rc

The app writes into **its own directory** and generates **one file**. The shell rc receives exactly **one line, once, and never changes it**:

```sh
# macOS/Linux — ~/.zshrc | ~/.bashrc | fish config
[ -f ~/.innovport/profiles.sh ] && . ~/.innovport/profiles.sh
```
```powershell
# Windows — $PROFILE
if (Test-Path ~/.innovport/profiles.ps1) { . ~/.innovport/profiles.ps1 }
```

**Why:** a GUI editing a shell startup file with regex is a foot-gun. Corrupt it once and **the terminal will not open** — and you need a terminal to fix it. Uninstall = delete one line. Blast radius zero.

**That line must be checked for before it is added.** Skipping that check is precisely what produced seven duplicate Antigravity lines.

### The generated file
- Clear header: `# GENERATED — DO NOT EDIT`
- **Must tolerate the user editing it anyway** (already observed: a hand-added `alias c='claude'` sitting inside the managed block)
- Re-read before writing, never cache blindly — Claude Code writes back to `settings.json` when the user changes the model via `/config`

---

## 6. Mechanism: generate shell scripts, NOT a binary launcher

**This decision was reversed mid-discussion**, once it became clear the app runs only once per machine:

| | Generated shell script ✅ | Binary launcher ❌ |
|---|---|---|
| Cost of supporting several shells | Paid **once** | None |
| Gatekeeper / SmartScreen signing | Not needed | Paid **forever** |
| `PATH` management | Not needed | Paid forever |
| After the app is deleted | Aliases **still work** | Dead commands |
| User can read / edit / delete it | Yes | No |

**The promise:** the app goes away, what it did stays.

### One env var per profile
The environment variable carrying the key must be **stored on the profile**, never hardcoded. That was failure #3: a hardcoded `MUST1C_CSE_API_KEY` made the second profile load its key into the wrong variable.

### Alias constraints
- **No spaces**, no characters needing escaping — an alias is **typed at a terminal**, unlike a filename which is only ever read
- Dashes are fine (`cht-cse` works today)
- Warn when it shadows an existing system command (`cd`, `ls`, …)
- PowerShell aliases **cannot take arguments** → generate a `function`, then `Set-Alias`

---

## 7. Bundles store INTENT, not FILES

**The central principle that makes cross-OS work.**

A bundle **excludes**:
- Absolute paths (`~/.claude/profiles/htcse.json` is a macOS notion; Windows is `C:\Users\…`)
- Verbatim Claude JSON or Codex TOML

A bundle **contains intent**:
> *"Profile `htcse`, provider htmustc, this base URL, CLI kind Claude, permission bypass on, this model mapping, this key."*

The destination machine **translates** that into files matching whatever CLI version is installed.

**Why:** Claude Code moved `defaultMode` (failure #2) and it cost half a session to diagnose. A bundle locked to a CLI's schema dies whenever that schema moves. With a translation layer, only the translation layer changes.

### Other requirements
- **Bundle version number** — the format will change and old bundles persist
- **Keys travel readable** — internal use; no encryption, no passphrase
- **A filename unlikely to slip into git** — not `config.json`; use a distinctive extension (`.innovport`). `git add .` forgives nobody, and this user has already had a key sitting in plaintext in `.zshrc`

InnovPort continues to import legacy `.agentport` bundles and migrate the old
`~/.agentport` shell source line when an existing installation is upgraded.
- **Selectable export** — the user picks which profiles travel

---

## 8. Danger is a SCALE, not a switch

Not a checkbox. It must be **translated per CLI**, and each CLI has a different number of rungs.

### Claude Code — `~/.claude/settings.json`
```json
{
  "permissions": { "defaultMode": "bypassPermissions" },
  "skipDangerousModePermissionPrompt": true
}
```
⚠️ **`defaultMode` must sit INSIDE `permissions`.** At the top level it is silently ignored with no error (failure #2).

Modes: `default` · `acceptEdits` · `plan` · `dontAsk` · `bypassPermissions`

### Codex — `~/.codex/<profile>.config.toml`
**Two independent axes:**
```toml
approval_policy = "never"            # untrusted | on-request | never
sandbox_mode = "danger-full-access"  # read-only | workspace-write | danger-full-access
```
Codex has an intermediate `workspace-write` rung (never prompts, but blocks writes outside the workspace) with **no Claude equivalent**.

---

## 9. Import compares IDENTITY, not name

### Two axes

- **Identity** = provider + base URL + CLI kind
- **Name** = alias

They can diverge in both directions:

| Identity | Name | Action |
|---|---|---|
| Same | Same | **Identical** → skip silently, spawn NO copy |
| Same | Different | Same provider under a local name → keep as is |
| **Different** | **Same** | **Real conflict** — the name is taken → auto-suffix |
| Different | Different | New → import |

### Suffix rule
A taken name gets `-1`, `-2` (like a downloaded file, but **dash, not parentheses**, because an alias must be typeable):

```
cht        ← already present
cht-1      ← the newly imported one
```

### No prompts — run straight through
Import shows a summary at the end; the user tidies up from there.

**But identical entries must be blocked.** The lesson from the seven Antigravity lines: re-importing the same bundle — an ordinary thing to do when you cannot remember whether you already did — must not produce `cht-1`, `cht-2`, `cht-3`.

---

## 10. State and proof

### Missing CLI → KEEP, show greyed out

```
  c          Claude · this machine's login              [rename]
  cht        Claude · htmustc            ✓ ready        [edit] [delete]
  co-ht      Codex  · htmustc            ⃠ Codex CLI not installed
                                           [ Check again ]
```

- Configuration is **stored**, the alias is **not yet created**
- Once the CLI appears → *"Activate `co-ht`?"* — one click
- **Wording:** *"not ready"*, not *"disabled"*. "Disabled" implies the user turned it off, sending them hunting for an on switch. "Not ready — Codex missing" sends them to install Codex.
- **Computed on every launch, NEVER stored as a flag.** A stored flag means installing Codex leaves the row greyed out forever.
- The tier-1 row (`c`) has **one** button, `[rename]` — no edit, no delete, no key or URL field

### Test with a REAL generation call

**Not** a reachability ping. **Not** an auth check alone.

The correct endpoint per CLI kind:
- Codex `wire_api = "responses"` → `POST /v1/responses`
- Claude → `/v1/messages`

**Why (failure #4):** `GET /v1/models` returning `200 OK` proves the key is valid — it proves **nothing** about whether a model can be called. Pinging the wrong door yields a lying green tick.

### Classify the failure — three kinds, one symptom, three responses

| Symptom | Meaning | User action |
|---|---|---|
| `401` | Wrong key | Paste a different key |
| `402` | Out of credit | Top up |
| Timeout / `500` | Provider has not mapped a model to this endpoint | Fix in the provider's admin panel |

**The third kind is what cost twenty minutes of `curl`**, and is the most valuable thing this app does.

### Final screen
```
✓ cht     — tested, replied in 1.2s
✗ cht-cse — key valid but provider timed out
⃠ co-ht   — Codex CLI not installed

3 profiles installed. Default `claude` and `codex` configuration does
not travel in a bundle — set that up separately on this machine.
```
The closing line is not a warning and not red. It is a fact worth knowing.

---

## 11. User flows

### A. First run, blank machine
```
No profiles yet.
  ▸ Scan this machine
  ▸ Import from a bundle file
  ▸ Create manually
```
Presets for **Claude** and **Codex** (alias editable). Manual fields: alias · provider · base URL · key · env var name · danger level · model mapping · wire_api (Codex).

### B. New machine with a bundle
```
Found 3 profiles: cht · cht-cse · co-ht
⚠ Codex CLI not installed — co-ht will be kept, not activated
```
→ paste any missing keys → install → test → close.

### C. Second import (sync)
```
9 profiles unchanged.
1 differs — cht (key has changed)  [review]
```

---

## 12. Three principles

> **A bundle stores intent, not files.**
> **"Not ready" ≠ "disabled".**
> **The app owns the name, not the thing being named.**

---

## 13. Still open

- **Where bundles live** — a file on disk, or another mechanism (decide during build)
- **Which shells ship in v1** — zsh certainly; bash / fish / PowerShell depending on Windows priority

---

## 14. Scanning an existing machine

The app must **discover profiles that already exist** and offer them for adoption, so a first run is a confirmation step rather than retyping.

### Read-only, and bounded by §4
- Reads `~/.claude/profiles/*.json` (Claude overlays) and `~/.codex/*.config.toml` (Codex overlays)
- **Skips tier 1**: `~/.codex/config.toml` and any Claude overlay with no `ANTHROPIC_BASE_URL` (that means it rides on the user's own Anthropic auth)
- Writes nothing during a scan

### It reports what is there, not what was intended
A `defaultMode` sitting at the top level instead of inside `permissions` is reported as **Ask**, not Bypass — because that is what the CLI actually does with it (failure #2). Reporting the intent would launder a broken config into one that looks correct.

### Each profile keeps its own env var
Codex names the key variable per provider (`MUST1C_HT_API_KEY`, `MUST1C_CSE_API_KEY`). The scanner reads `env_key` from each file rather than assuming a shared name — assuming one is failure #3.

### Codex keys live outside the config
The TOML names the variable; the value lives elsewhere. The scanner reports the variable name and leaves resolving the value to a step the user confirms.

### Verified against real data
Run against a live machine, the scanner found all four existing profiles with the correct per-profile env vars:

```
htcse    Claude  htmustc.id.vn  env=ANTHROPIC_AUTH_TOKEN  danger=Bypass
htmustc  Claude  htmustc.id.vn  env=ANTHROPIC_AUTH_TOKEN  danger=Bypass
cse      Codex   htmustc.id.vn  env=MUST1C_CSE_API_KEY    danger=Bypass
ht       Codex   htmustc.id.vn  env=MUST1C_HT_API_KEY     danger=Bypass
```

---

## 15. Model setup must be verified, not typed

A profile's model mapping is only correct if the provider actually serves those
model ids **to this key**. Typing them by hand produces a config that looks
right and fails at call time.

### Fetch the real list
`GET /v1/models` on the provider's base URL returns what that key may use. The
app fetches it during profile setup and offers the ids as a choice rather than a
free-text field.

### Roles differ per CLI
- **Claude Code** routes by tier: `ANTHROPIC_DEFAULT_OPUS_MODEL`,
  `..._SONNET_...`, `..._HAIKU_...`. Opus must resolve; the others fall back.
- **Codex** has a single default model and no tier concept. Checking tier roles
  there would be noise.

### Suggesting nothing is a valid answer
Where a provider serves no model of a family, the app suggests nothing for that
role. Guessing would recreate the exact failure this guards against.

### Found live on the developer's own machine
Both Claude profiles are configured for `claude-opus-5`, `claude-sonnet-5` and
`claude-haiku-4.5`. The provider serves **14 models and not one Claude model
among them**:

```
htcse    Claude  configured opus=claude-opus-5
                 issues: NotServed opus, NotServed sonnet, NotServed haiku
htmustc  Claude  (identical)
cse      Codex   configured default=gpt-5.5   issues: none
ht       Codex   configured default=gpt-5.5   issues: none
```

Neither CLI reports this. It surfaces only as a failed call, much later — the
same silent-success shape as evidence #2 and #4.

---

## Appendix: the state this replaces

```
~/.claude/settings.json            machine-wide default (Anthropic auth)
~/.claude/profiles/htmustc.json    overlay, invoked via --settings
~/.claude/profiles/htcse.json
~/.codex/config.toml               default + must1c provider
~/.codex/ht.config.toml            invoked via --profile ht
~/.codex/cse.config.toml
~/.codex/keys/{ht,cse}             plaintext keys, chmod 600
~/.zshrc                           cp_claude / cuse / cwho / cp_codex + aliases
```

| Alias | CLI | Provider | State |
|---|---|---|---|
| `c` | Claude | Anthropic auth | ✓ bypass on |
| `cht` | Claude | htmustc | ✓ bypass on |
| `cht-cse` | Claude | htcse | ✓ bypass on |
| `codex` | Codex | old key | ✗ out of credit |
| `co-ht` | Codex | htmustc | ✓ yolo on |
| `co-cse` | Codex | htcse | ✗ provider timeout |
