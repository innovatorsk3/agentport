//! Scans this machine for profiles that already exist (requirements §14).
//!
//! Read-only. Nothing here writes, and nothing here touches tier-1 config
//! (`~/.claude/settings.json`, `~/.codex/config.toml`) — the app may read those
//! for context but never adopts or rewrites them (§4).
//!
//! Scanning turns a manual first-run into a confirmation step: the app shows
//! what it found and the user picks which ones to keep.

use crate::model::{CliKind, DangerLevel, ModelMap, Origin, Profile};
use std::fs;
use std::path::{Path, PathBuf};

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Reads Claude overlay profiles from `~/.claude/profiles/*.json`.
///
/// The overlay shape is what `claude --settings <file>` consumes: an `env`
/// block plus an optional `permissions.defaultMode`.
pub fn scan_claude_dir(dir: &Path) -> Vec<Profile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };

        let env = &json["env"];
        let base_url = env["ANTHROPIC_BASE_URL"].as_str().unwrap_or_default();
        // No base URL means it rides on the user's own Anthropic auth — that is
        // tier-1 default config, which the app never adopts (§4).
        if base_url.is_empty() {
            continue;
        }

        found.push(Profile {
            alias: stem.to_string(),
            cli: CliKind::Claude,
            provider: provider_from_url(base_url),
            base_url: base_url.to_string(),
            api_key: env["ANTHROPIC_AUTH_TOKEN"].as_str().unwrap_or_default().to_string(),
            // Claude reads the key from a fixed variable, unlike Codex.
            env_var: "ANTHROPIC_AUTH_TOKEN".to_string(),
            danger: claude_danger(&json),
            model_map: ModelMap {
                opus: env["ANTHROPIC_DEFAULT_OPUS_MODEL"].as_str().map(str::to_string),
                sonnet: env["ANTHROPIC_DEFAULT_SONNET_MODEL"].as_str().map(str::to_string),
                haiku: env["ANTHROPIC_DEFAULT_HAIKU_MODEL"].as_str().map(str::to_string),
                default: json["model"].as_str().map(str::to_string),
            },
            wire_api: None,
            origin: Origin::Scanned,
        });
    }

    found.sort_by(|a, b| a.alias.cmp(&b.alias));
    found
}

/// `permissions.defaultMode` — note the nesting. A top-level `defaultMode` is
/// silently ignored by Claude Code, which is evidence #2 in the requirements:
/// misplacing this key produced no error at all, just a setting that never
/// applied.
fn claude_danger(json: &serde_json::Value) -> DangerLevel {
    match json["permissions"]["defaultMode"].as_str() {
        Some("bypassPermissions") => DangerLevel::Bypass,
        Some("acceptEdits") => DangerLevel::AcceptEdits,
        _ => DangerLevel::Ask,
    }
}

/// Reads Codex overlay profiles from `~/.codex/<name>.config.toml`.
///
/// `config.toml` itself is skipped on purpose — that is the tier-1 default.
pub fn scan_codex_dir(dir: &Path) -> Vec<Profile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".config.toml") else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };

        let Some((base_url, env_var, wire_api)) = codex_provider_block(&text) else {
            continue;
        };

        found.push(Profile {
            alias: stem.to_string(),
            cli: CliKind::Codex,
            provider: provider_from_url(&base_url),
            base_url,
            // The key lives outside the TOML, in whatever `env_key` names. The
            // scanner reports what it found; resolving the value is a separate
            // step the user confirms.
            api_key: String::new(),
            env_var,
            danger: codex_danger(&text),
            model_map: ModelMap {
                default: toml_scalar(&text, "model"),
                ..ModelMap::default()
            },
            wire_api,
            origin: Origin::Scanned,
        });
    }

    found.sort_by(|a, b| a.alias.cmp(&b.alias));
    found
}

/// Codex splits danger across TWO axes, unlike Claude's single mode (§8).
/// Both must say "unrestricted" before this counts as full bypass.
fn codex_danger(text: &str) -> DangerLevel {
    let approval = toml_scalar(text, "approval_policy");
    let sandbox = toml_scalar(text, "sandbox_mode");

    match (approval.as_deref(), sandbox.as_deref()) {
        (Some("never"), Some("danger-full-access")) => DangerLevel::Bypass,
        (Some("never"), Some("workspace-write")) => DangerLevel::WorkspaceWrite,
        _ => DangerLevel::Ask,
    }
}

/// Pulls `base_url` / `env_key` / `wire_api` out of the `[model_providers.*]`
/// table.
///
/// Deliberately a line scanner rather than a full TOML parse: these files are
/// hand-edited and may carry unrelated tables (hook state, trust hashes) that a
/// strict parse would have to model. Only the fields below matter here.
fn codex_provider_block(text: &str) -> Option<(String, String, Option<String>)> {
    let mut inside = false;
    let mut base_url = None;
    let mut env_key = None;
    let mut wire_api = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed.starts_with("[model_providers.");
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(v) = scalar_on_line(trimmed, "base_url") {
            base_url = Some(v);
        } else if let Some(v) = scalar_on_line(trimmed, "env_key") {
            env_key = Some(v);
        } else if let Some(v) = scalar_on_line(trimmed, "wire_api") {
            wire_api = Some(v);
        }
    }

    Some((base_url?, env_key?, wire_api))
}

/// Finds a top-level `key = "value"` (outside any table header).
fn toml_scalar(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            break; // top-level scalars all precede the first table
        }
        if let Some(v) = scalar_on_line(trimmed, key) {
            return Some(v);
        }
    }
    None
}

fn scalar_on_line(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    Some(rest.trim_matches('"').to_string())
}

/// Derives a readable provider label from a base URL host.
fn provider_from_url(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .split(':')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// Scans every known location on this machine.
pub fn scan_machine() -> Vec<Profile> {
    let Some(h) = home() else {
        return Vec::new();
    };
    let mut all = scan_claude_dir(&h.join(".claude").join("profiles"));
    all.extend(scan_codex_dir(&h.join(".codex")));
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("agentport_scan_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(dir: &Path, name: &str, body: &str) {
        let mut f = fs::File::create(dir.join(name)).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    /// Modelled on a real `~/.claude/profiles/*.json` overlay.
    #[test]
    fn reads_a_claude_overlay() {
        let d = tmpdir("claude_ok");
        write(
            &d,
            "htcse.json",
            r#"{
              "env": {
                "ANTHROPIC_BASE_URL": "https://htmustc.id.vn",
                "ANTHROPIC_AUTH_TOKEN": "mk-live-secret",
                "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-5"
              },
              "model": "opus",
              "permissions": { "defaultMode": "bypassPermissions" }
            }"#,
        );

        let found = scan_claude_dir(&d);
        assert_eq!(found.len(), 1);
        let p = &found[0];
        assert_eq!(p.alias, "htcse");
        assert_eq!(p.cli, CliKind::Claude);
        assert_eq!(p.provider, "htmustc.id.vn");
        assert_eq!(p.api_key, "mk-live-secret");
        assert_eq!(p.danger, DangerLevel::Bypass);
        assert_eq!(p.model_map.opus.as_deref(), Some("claude-opus-5"));
        assert_eq!(p.origin, Origin::Scanned);
    }

    /// A `defaultMode` sitting at the top level instead of inside `permissions`
    /// is exactly the misplacement Claude Code ignores without any error
    /// (evidence #2). The scanner must report Ask, not Bypass — reporting the
    /// intended value would launder a broken config into a working-looking one.
    #[test]
    fn top_level_default_mode_is_not_treated_as_bypass() {
        let d = tmpdir("claude_misplaced");
        write(
            &d,
            "broken.json",
            r#"{
              "env": { "ANTHROPIC_BASE_URL": "https://x.example" },
              "defaultMode": "bypassPermissions"
            }"#,
        );

        let found = scan_claude_dir(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].danger, DangerLevel::Ask);
    }

    /// No base URL = the user's own Anthropic auth = tier 1. Never adopted.
    #[test]
    fn skips_overlay_without_base_url() {
        let d = tmpdir("claude_tier1");
        write(&d, "plain.json", r#"{ "model": "opus" }"#);
        assert!(scan_claude_dir(&d).is_empty());
    }

    #[test]
    fn malformed_json_is_skipped_not_fatal() {
        let d = tmpdir("claude_bad");
        write(&d, "broken.json", "{ this is not json");
        write(
            &d,
            "good.json",
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://ok.example"}}"#,
        );

        let found = scan_claude_dir(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].alias, "good");
    }

    /// Modelled on a real `~/.codex/<name>.config.toml`, including the trailing
    /// hook tables that a naive parse would trip over.
    #[test]
    fn reads_a_codex_overlay() {
        let d = tmpdir("codex_ok");
        write(
            &d,
            "ht.config.toml",
            r#"# comment line
model = "gpt-5.5"
model_provider = "must1c_ht"

approval_policy = "never"
sandbox_mode = "danger-full-access"

[model_providers.must1c_ht]
name = "Must1c (ht)"
base_url = "https://htmustc.id.vn/v1"
env_key = "MUST1C_HT_API_KEY"
wire_api = "responses"

[hooks.state]
"#,
        );

        let found = scan_codex_dir(&d);
        assert_eq!(found.len(), 1);
        let p = &found[0];
        assert_eq!(p.alias, "ht");
        assert_eq!(p.cli, CliKind::Codex);
        assert_eq!(p.base_url, "https://htmustc.id.vn/v1");
        assert_eq!(p.env_var, "MUST1C_HT_API_KEY");
        assert_eq!(p.wire_api.as_deref(), Some("responses"));
        assert_eq!(p.danger, DangerLevel::Bypass);
        assert_eq!(p.model_map.default.as_deref(), Some("gpt-5.5"));
    }

    /// Each profile must carry its OWN env var. Assuming one shared name is
    /// what made a second profile load its key into the wrong variable
    /// (evidence #3).
    #[test]
    fn each_codex_profile_keeps_its_own_env_var() {
        let d = tmpdir("codex_two");
        for (file, provider, var) in [
            ("ht.config.toml", "must1c_ht", "MUST1C_HT_API_KEY"),
            ("cse.config.toml", "must1c_cse", "MUST1C_CSE_API_KEY"),
        ] {
            write(
                &d,
                file,
                &format!(
                    "model = \"gpt-5.5\"\n\n[model_providers.{provider}]\n\
                     base_url = \"https://htmustc.id.vn/v1\"\n\
                     env_key = \"{var}\"\n"
                ),
            );
        }

        let found = scan_codex_dir(&d);
        assert_eq!(found.len(), 2);
        let vars: Vec<&str> = found.iter().map(|p| p.env_var.as_str()).collect();
        assert!(vars.contains(&"MUST1C_HT_API_KEY"));
        assert!(vars.contains(&"MUST1C_CSE_API_KEY"));
    }

    /// Codex needs BOTH axes unrestricted; one alone is not full bypass.
    #[test]
    fn codex_needs_both_axes_for_bypass() {
        let d = tmpdir("codex_partial");
        write(
            &d,
            "half.config.toml",
            "approval_policy = \"never\"\nsandbox_mode = \"workspace-write\"\n\n\
             [model_providers.p]\nbase_url = \"https://x.example/v1\"\nenv_key = \"K\"\n",
        );

        let found = scan_codex_dir(&d);
        assert_eq!(found[0].danger, DangerLevel::WorkspaceWrite);
    }

    /// `config.toml` is tier-1 default config and must never be adopted (§4).
    #[test]
    fn skips_the_base_config_toml() {
        let d = tmpdir("codex_base");
        write(
            &d,
            "config.toml",
            "[model_providers.p]\nbase_url = \"https://x.example/v1\"\nenv_key = \"K\"\n",
        );
        assert!(scan_codex_dir(&d).is_empty());
    }

    #[test]
    fn missing_directory_yields_nothing() {
        let missing = std::env::temp_dir().join("agentport_no_such_dir_xyz");
        assert!(scan_claude_dir(&missing).is_empty());
        assert!(scan_codex_dir(&missing).is_empty());
    }
}
