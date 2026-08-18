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
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));

    home.map(PathBuf::from)
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
            profile_name: None,
            cli: CliKind::Claude,
            provider: provider_from_url(base_url),
            base_url: base_url.to_string(),
            api_key: env["ANTHROPIC_AUTH_TOKEN"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            // Claude reads the key from a fixed variable, unlike Codex.
            env_var: "ANTHROPIC_AUTH_TOKEN".to_string(),
            danger: claude_danger(&json),
            model_map: ModelMap {
                opus: env["ANTHROPIC_DEFAULT_OPUS_MODEL"]
                    .as_str()
                    .map(str::to_string),
                sonnet: env["ANTHROPIC_DEFAULT_SONNET_MODEL"]
                    .as_str()
                    .map(str::to_string),
                haiku: env["ANTHROPIC_DEFAULT_HAIKU_MODEL"]
                    .as_str()
                    .map(str::to_string),
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
            profile_name: None,
            cli: CliKind::Codex,
            provider: provider_from_url(&base_url),
            base_url,
            // The key lives outside the TOML, in whatever `env_key` names. The
            // scanner reports what it found; resolving the value is a separate
            // step the user confirms.
            api_key: std::env::var(&env_var).unwrap_or_default(),
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

/// Pulls `base_url` / `env_key` / `wire_api` out of the selected
/// `[model_providers.*]` table. TOML parsing is safe here: unknown hook and
/// project tables are values, not a schema this module needs to model.
fn codex_provider_block(text: &str) -> Option<(String, String, Option<String>)> {
    let document = toml::from_str::<toml::Value>(text).ok()?;
    let providers = document.get("model_providers")?.as_table()?;
    let selected_name = document.get("model_provider").and_then(toml::Value::as_str);
    let provider = selected_name
        .and_then(|name| providers.get(name))
        .or_else(|| {
            providers
                .values()
                .find(|value| value.get("base_url").is_some())
        })?
        .as_table()?;

    Some((
        provider.get("base_url")?.as_str()?.to_string(),
        provider.get("env_key")?.as_str()?.to_string(),
        provider
            .get("wire_api")
            .and_then(toml::Value::as_str)
            .map(str::to_string),
    ))
}

/// Finds a top-level `key = "value"` (outside any table header).
fn toml_scalar(text: &str, key: &str) -> Option<String> {
    toml::from_str::<toml::Value>(text)
        .ok()?
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
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
    scan_machine_at(&h)
}

pub fn scan_machine_at(h: &Path) -> Vec<Profile> {
    let mut all = scan_claude_dir(&h.join(".claude").join("profiles"));
    all.extend(scan_codex_dir(&h.join(".codex")));
    let shell_aliases = scan_shell_aliases(h);
    for profile in &mut all {
        let cli_profile_name = profile.alias.clone();
        if let Some(alias) = shell_aliases.iter().find(|candidate| {
            candidate.cli == profile.cli && candidate.profile_name == cli_profile_name
        }) {
            profile.alias = alias.alias.clone();
            profile.profile_name = Some(cli_profile_name);
        }
    }
    for profile in &mut all {
        if profile.cli == CliKind::Codex && profile.api_key.is_empty() {
            let cli_profile_name = profile.cli_profile_name().to_string();
            profile.api_key = find_key_file(h, &cli_profile_name)
                .or_else(|| find_key_in_shell_files(h, &profile.env_var))
                .unwrap_or_default();
        }
    }
    all.sort_by(|a, b| a.alias.cmp(&b.alias));
    all
}

/// Some existing Codex wrappers keep one key per named profile instead of
/// exporting it in the shell. Read that file only to populate the in-memory
/// profile for export; the app never rewrites or prints it.
fn find_key_file(home: &Path, profile_name: &str) -> Option<String> {
    let path = home.join(".codex").join("keys").join(profile_name);
    fs::read_to_string(path)
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellAlias {
    alias: String,
    cli: CliKind,
    profile_name: String,
}

/// Reads the user's explicit aliases that point at a named Claude/Codex
/// profile. The config filename is the CLI profile name; the shell alias is
/// what the user expects to see and type, such as `co-ht`.
fn scan_shell_aliases(home: &Path) -> Vec<ShellAlias> {
    let candidates = [home.join(".zshrc"), home.join(".bashrc")];
    candidates
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .flat_map(|text| parse_shell_aliases(&text))
        .collect()
}

fn parse_shell_aliases(text: &str) -> Vec<ShellAlias> {
    let mut found = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("alias ") else {
            continue;
        };
        let rest = rest.strip_prefix("-- ").unwrap_or(rest);
        let Some((alias, value)) = rest.split_once('=') else {
            continue;
        };
        let alias = alias.trim();
        if alias.is_empty() || alias.chars().any(|c| c.is_whitespace()) {
            continue;
        }
        let Some(command) = parse_shell_value(value) else {
            continue;
        };
        let mut parts = command.split_whitespace();
        let Some(wrapper) = parts.next() else {
            continue;
        };
        let Some(profile_name) = parts.next() else {
            continue;
        };
        if parts.next().is_some() {
            continue;
        }

        let cli = match wrapper {
            "cp_claude" => CliKind::Claude,
            "cp_codex" => CliKind::Codex,
            _ => continue,
        };
        found.push(ShellAlias {
            alias: alias.to_string(),
            cli,
            profile_name: profile_name.to_string(),
        });
    }
    found
}

/// GUI apps launched from Finder/Explorer do not inherit the user's shell
/// exports. Recover keys from the generated script and simple profile
/// assignments so a scanned Codex profile can actually be exported.
fn find_key_in_shell_files(home: &Path, env_var: &str) -> Option<String> {
    let candidates = [
        home.join(".agentport/profiles.sh"),
        home.join(".agentport/profiles.ps1"),
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
        home.join("Documents/WindowsPowerShell/Microsoft.PowerShell_profile.ps1"),
    ];

    candidates
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .find_map(|text| find_key_assignment(&text, env_var))
}

fn find_key_assignment(text: &str, env_var: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let assignment = trimmed
            .strip_prefix("export ")
            .unwrap_or(trimmed)
            .strip_prefix(env_var)
            .and_then(|rest| rest.strip_prefix('='));

        if let Some(value) = assignment.and_then(parse_shell_value) {
            return Some(value);
        }

        let ps = trimmed
            .strip_prefix("$env:")
            .and_then(|rest| rest.strip_prefix(env_var))
            .and_then(|rest| rest.trim_start().strip_prefix('='));
        if let Some(value) = ps.and_then(parse_shell_value) {
            return Some(value);
        }
    }
    None
}

fn parse_shell_value(value: &str) -> Option<String> {
    let value = value.trim_start();
    if let Some(value) = value.strip_prefix('"') {
        let mut out = String::new();
        let mut escaped = false;
        for ch in value.chars() {
            if escaped {
                out.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some(out);
            } else {
                out.push(ch);
            }
        }
        return None;
    }
    if !value.starts_with('\'') {
        return value
            .split_whitespace()
            .next()
            .filter(|v| !v.is_empty())
            .map(str::to_string);
    }

    let mut out = String::new();
    let mut i = 1;
    while i < value.len() {
        let rest = &value[i..];
        if rest.starts_with("'\\''") {
            out.push('\'');
            i += 4;
            continue;
        }
        if rest.starts_with("''") {
            out.push('\'');
            i += 2;
            continue;
        }
        let ch = rest.chars().next()?;
        if ch == '\'' {
            return Some(out);
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    None
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

    #[test]
    fn follows_the_selected_codex_provider_when_several_exist() {
        let d = tmpdir("codex_multiple");
        write(
            &d,
            "multi.config.toml",
            "model_provider = \"chosen\"\n\n[model_providers.other]\n\
             base_url = \"https://wrong.example/v1\"\nenv_key = \"WRONG\"\n\n\
             [model_providers.chosen]\nbase_url = \"https://right.example/v1\"\n\
             env_key = \"RIGHT\"\nwire_api = \"chat\"\n",
        );

        let found = scan_codex_dir(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].base_url, "https://right.example/v1");
        assert_eq!(found[0].env_var, "RIGHT");
        assert_eq!(found[0].wire_api.as_deref(), Some("chat"));
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

    #[test]
    fn reads_keys_from_generated_posix_assignments() {
        let body = "AGENTPORT_HT_API_KEY='abc' command codex --profile ht \"$@\"\n";
        assert_eq!(
            find_key_assignment(body, "AGENTPORT_HT_API_KEY").as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn reads_escaped_quotes_from_generated_scripts() {
        let body = "AGENTPORT_HT_API_KEY='a'\\''b' command codex --profile ht \"$@\"\n";
        assert_eq!(
            find_key_assignment(body, "AGENTPORT_HT_API_KEY").as_deref(),
            Some("a'b")
        );
    }

    #[test]
    fn reads_keys_from_generated_powershell_assignments() {
        let body = "$env:AGENTPORT_HT_API_KEY = 'a''b'\n";
        assert_eq!(
            find_key_assignment(body, "AGENTPORT_HT_API_KEY").as_deref(),
            Some("a'b")
        );
    }

    #[test]
    fn machine_scan_recovers_a_codex_key_for_export() {
        let d = tmpdir("machine_key");
        let env_var = format!("AGENTPORT_SCAN_TEST_{}_KEY", std::process::id());
        fs::create_dir_all(d.join(".codex")).unwrap();
        write(
            &d.join(".codex"),
            "ht.config.toml",
            &format!(
                "model = \"gpt-5.5\"\nmodel_provider = \"p\"\n\n\
                 [model_providers.p]\nbase_url = \"https://provider.example/v1\"\n\
                 env_key = \"{env_var}\"\n"
            ),
        );
        fs::create_dir_all(d.join(".agentport")).unwrap();
        fs::write(
            d.join(".agentport/profiles.sh"),
            format!("{env_var}='key-from-mac' command codex --profile ht \"$@\"\n"),
        )
        .unwrap();

        let found = scan_machine_at(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].api_key, "key-from-mac");
    }

    #[test]
    fn shell_aliases_are_kept_separate_from_cli_profile_names() {
        let d = tmpdir("shell_aliases");
        let env_var = format!("AGENTPORT_SHELL_ALIAS_{}_KEY", std::process::id());
        fs::create_dir_all(d.join(".claude/profiles")).unwrap();
        fs::create_dir_all(d.join(".codex")).unwrap();
        fs::create_dir_all(d.join(".codex/keys")).unwrap();
        write(
            &d.join(".claude/profiles"),
            "htmustc.json",
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://htmustc.id.vn"}}"#,
        );
        write(
            &d.join(".codex"),
            "ht.config.toml",
            &format!(
                "model = \"gpt-5.5\"\n\n[model_providers.p]\nbase_url = \"https://htmustc.id.vn/v1\"\nenv_key = \"{env_var}\"\n"
            ),
        );
        write(&d.join(".codex/keys"), "ht", "key-from-profile-file\n");
        write(
            &d,
            ".zshrc",
            "alias cht='cp_claude htmustc'\nalias co-ht='cp_codex ht'\nalias c='claude'\n",
        );

        let found = scan_machine_at(&d);
        assert_eq!(
            found.iter().map(|p| p.alias.as_str()).collect::<Vec<_>>(),
            ["cht", "co-ht"]
        );
        let codex = found.iter().find(|p| p.alias == "co-ht").unwrap();
        assert_eq!(codex.profile_name.as_deref(), Some("ht"));
        assert_eq!(codex.api_key, "key-from-profile-file");
        let claude = found.iter().find(|p| p.alias == "cht").unwrap();
        assert_eq!(claude.profile_name.as_deref(), Some("htmustc"));
    }
}
