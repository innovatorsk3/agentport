//! Translates profile intent into each CLI's own config schema (requirements §7).
//!
//! This is the layer that makes a bundle portable: the bundle says *"permission
//! bypass on"*, and this module knows that Claude spells it
//! `permissions.defaultMode` while Codex needs two separate TOML keys.
//!
//! When a CLI moves a key — as Claude Code did with `defaultMode` — only this
//! module changes. Every existing bundle keeps working.

pub mod claude;
pub mod codex;

use crate::model::{CliKind, Profile};
use std::fs;
use std::path::{Path, PathBuf};

/// Where a profile's config file belongs, relative to the user's home.
pub fn config_path(home: &Path, profile: &Profile) -> PathBuf {
    let name = profile.cli_profile_name();
    match profile.cli {
        CliKind::Claude => home
            .join(".claude")
            .join("profiles")
            .join(format!("{name}.json")),
        CliKind::Codex => home.join(".codex").join(format!("{name}.config.toml")),
    }
}

/// Writes one profile's config, merging into any existing file.
///
/// Reads the current contents first rather than caching: Claude Code writes
/// back into these files when the user changes the model via `/config`, so a
/// stale in-memory copy would discard their edits (§5).
pub fn write_profile(home: &Path, profile: &Profile) -> Result<PathBuf, String> {
    let path = config_path(home, profile);
    let parent = path.parent().ok_or("config path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;

    let existing = fs::read_to_string(&path).ok();

    let rendered = match profile.cli {
        CliKind::Claude => {
            let parsed = match existing.as_deref() {
                Some(text) => Some(
                    serde_json::from_str::<serde_json::Value>(text)
                        .map_err(|e| format!("existing config is not valid JSON: {e}"))?,
                ),
                None => None,
            };
            let doc = claude::render(profile, parsed.as_ref());
            serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "\n"
        }
        CliKind::Codex => codex::render(profile, existing.as_deref())?,
    };

    crate::file_security::write_private(&path, rendered.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DangerLevel, ModelMap, Origin};

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("innovport_writer_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn profile(alias: &str, cli: CliKind) -> Profile {
        Profile {
            alias: alias.into(),
            profile_name: None,
            cli,
            provider: "htmustc.id.vn".into(),
            base_url: "https://htmustc.id.vn/v1".into(),
            api_key: "mk-live-secret".into(),
            env_var: "INNOVPORT_TEST_API_KEY".into(),
            danger: DangerLevel::Bypass,
            model_map: ModelMap {
                default: Some("gpt-5.5".into()),
                opus: Some("claude-opus-5".into()),
                ..ModelMap::default()
            },
            wire_api: Some("responses".into()),
            origin: Origin::Manual,
        }
    }

    #[test]
    fn claude_lands_in_the_profiles_directory_not_settings_json() {
        let home = Path::new("/home/u");
        let p = config_path(home, &profile("htmustc", CliKind::Claude));

        assert!(p.ends_with(".claude/profiles/htmustc.json"));
        // settings.json is tier-1 config — never a write target (§4).
        assert!(!p.ends_with("settings.json"));
    }

    /// The base config.toml is tier 1 and must never be a target either.
    #[test]
    fn codex_lands_beside_but_not_on_the_base_config() {
        let home = Path::new("/home/u");
        let p = config_path(home, &profile("ht", CliKind::Codex));

        assert!(p.ends_with(".codex/ht.config.toml"));
        assert!(!p.ends_with(".codex/config.toml"));
    }

    #[test]
    fn writes_a_claude_overlay_to_disk() {
        let home = tmpdir("claude");
        let path = write_profile(&home, &profile("htmustc", CliKind::Claude)).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["permissions"]["defaultMode"], "bypassPermissions");
        assert_eq!(v["env"]["ANTHROPIC_BASE_URL"], "https://htmustc.id.vn/v1");
    }

    #[test]
    fn writes_a_codex_overlay_to_disk() {
        let home = tmpdir("codex");
        let path = write_profile(&home, &profile("ht", CliKind::Codex)).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let v: toml::Value = toml::from_str(&text).unwrap();
        assert_eq!(v["approval_policy"].as_str(), Some("never"));
        assert_eq!(v["sandbox_mode"].as_str(), Some("danger-full-access"));
    }

    /// A second write must not duplicate anything or lose foreign keys.
    #[test]
    fn rewriting_preserves_foreign_keys_and_stays_stable() {
        let home = tmpdir("merge");
        let p = profile("htmustc", CliKind::Claude);

        let path = write_profile(&home, &p).unwrap();

        // Simulate Claude Code writing back via /config.
        let mut v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        v["model"] = serde_json::json!("opus");
        fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

        write_profile(&home, &p).unwrap();
        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(after["model"], "opus");
        assert_eq!(after["permissions"]["defaultMode"], "bypassPermissions");
    }

    /// The whole round trip must survive the app writing over its own output.
    #[test]
    fn codex_write_is_idempotent_on_disk() {
        let home = tmpdir("codex_idem");
        let p = profile("ht", CliKind::Codex);

        let path = write_profile(&home, &p).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        write_profile(&home, &p).unwrap();
        let second = fs::read_to_string(&path).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn creates_missing_parent_directories() {
        let home = tmpdir("nodirs");
        assert!(!home.join(".claude").exists());

        let path = write_profile(&home, &profile("new", CliKind::Claude)).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn refuses_to_overwrite_a_malformed_claude_config() {
        let home = tmpdir("claude_bad");
        let path = home.join(".claude/profiles/bad.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();

        let err = write_profile(&home, &profile("bad", CliKind::Claude)).unwrap_err();
        assert!(err.contains("not valid JSON"));
        assert_eq!(fs::read_to_string(path).unwrap(), "{ not json");
    }
}
