//! Translates a profile into a Claude Code overlay (requirements §7, §8).
//!
//! The overlay is what `claude --settings <file>` consumes. It is written to
//! `<claude_dir>/profiles/<alias>.json` — never to `settings.json`, which is
//! tier-1 config the app must not touch (§4).

use crate::model::{DangerLevel, Profile};
use serde_json::{json, Map, Value};

/// Claude Code reads the key from this fixed variable, unlike Codex which names
/// one per provider.
pub const AUTH_ENV: &str = "ANTHROPIC_AUTH_TOKEN";

/// Renders the overlay JSON.
///
/// `existing` is the current file contents, if any. Unknown keys are preserved:
/// Claude Code writes back into these files when the user changes the model via
/// `/config`, so blindly replacing the file would discard their edits (§5).
pub fn render(profile: &Profile, existing: Option<&Value>) -> Value {
    let mut root: Map<String, Value> = match existing {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };

    // --- env block -------------------------------------------------------
    let mut env: Map<String, Value> = match root.get("env") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    env.insert("ANTHROPIC_BASE_URL".into(), json!(profile.base_url));
    env.insert(AUTH_ENV.into(), json!(profile.api_key));

    // Only write model roles that are actually set. Writing an empty string
    // would pin the CLI to a model id that resolves to nothing.
    for (key, value) in [
        ("ANTHROPIC_DEFAULT_OPUS_MODEL", &profile.model_map.opus),
        ("ANTHROPIC_DEFAULT_SONNET_MODEL", &profile.model_map.sonnet),
        ("ANTHROPIC_DEFAULT_HAIKU_MODEL", &profile.model_map.haiku),
    ] {
        match value {
            Some(id) => {
                env.insert(key.into(), json!(id));
            }
            None => {
                env.remove(key);
            }
        }
    }
    root.insert("env".into(), Value::Object(env));

    // --- permissions -----------------------------------------------------
    // defaultMode MUST be nested inside `permissions`. At the top level Claude
    // Code ignores it with no error at all — evidence #2, the single most
    // expensive silent failure behind this project.
    let mut permissions: Map<String, Value> = match root.get("permissions") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    permissions.insert("defaultMode".into(), json!(default_mode(profile.danger)));
    root.insert("permissions".into(), Value::Object(permissions));

    // A stale top-level copy would sit there looking authoritative while doing
    // nothing. Remove it so the file cannot mislead a later reader.
    root.remove("defaultMode");

    if matches!(profile.danger, DangerLevel::Bypass) {
        root.insert("skipDangerousModePermissionPrompt".into(), json!(true));
    } else {
        root.remove("skipDangerousModePermissionPrompt");
    }

    Value::Object(root)
}

/// Maps the neutral scale onto Claude's modes (§8).
///
/// Claude has no equivalent of Codex's `workspace-write` rung, so it collapses
/// to the nearest safe mode rather than silently becoming full bypass.
fn default_mode(level: DangerLevel) -> &'static str {
    match level {
        DangerLevel::Ask => "default",
        DangerLevel::AcceptEdits => "acceptEdits",
        DangerLevel::WorkspaceWrite => "acceptEdits",
        DangerLevel::Bypass => "bypassPermissions",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CliKind, ModelMap, Origin};

    fn profile(danger: DangerLevel) -> Profile {
        Profile {
            alias: "htmustc".into(),
            profile_name: None,
            cli: CliKind::Claude,
            provider: "htmustc.id.vn".into(),
            base_url: "https://htmustc.id.vn".into(),
            api_key: "mk-live-secret".into(),
            env_var: AUTH_ENV.into(),
            danger,
            model_map: ModelMap {
                opus: Some("claude-opus-5".into()),
                ..ModelMap::default()
            },
            wire_api: None,
            origin: Origin::Manual,
        }
    }

    /// The key that cost half a session: nested, never top level.
    #[test]
    fn default_mode_is_nested_under_permissions() {
        let out = render(&profile(DangerLevel::Bypass), None);
        assert_eq!(out["permissions"]["defaultMode"], "bypassPermissions");
        assert!(out.get("defaultMode").is_none());
    }

    /// A pre-existing top-level copy is misleading dead weight — strip it.
    #[test]
    fn strips_a_stale_top_level_default_mode() {
        let existing = json!({ "defaultMode": "bypassPermissions" });
        let out = render(&profile(DangerLevel::Ask), Some(&existing));
        assert!(out.get("defaultMode").is_none());
        assert_eq!(out["permissions"]["defaultMode"], "default");
    }

    #[test]
    fn writes_base_url_and_key_into_env() {
        let out = render(&profile(DangerLevel::Bypass), None);
        assert_eq!(out["env"]["ANTHROPIC_BASE_URL"], "https://htmustc.id.vn");
        assert_eq!(out["env"][AUTH_ENV], "mk-live-secret");
    }

    #[test]
    fn only_writes_model_roles_that_are_set() {
        let out = render(&profile(DangerLevel::Bypass), None);
        assert_eq!(out["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"], "claude-opus-5");
        assert!(out["env"].get("ANTHROPIC_DEFAULT_SONNET_MODEL").is_none());
    }

    /// Clearing a role must remove the variable, not leave the old id behind.
    #[test]
    fn clearing_a_role_removes_the_variable() {
        let existing = json!({
            "env": { "ANTHROPIC_DEFAULT_SONNET_MODEL": "stale-model" }
        });
        let out = render(&profile(DangerLevel::Bypass), Some(&existing));
        assert!(out["env"].get("ANTHROPIC_DEFAULT_SONNET_MODEL").is_none());
    }

    /// Claude Code writes back into these files via `/config`. Unknown keys the
    /// app does not manage must survive a rewrite.
    #[test]
    fn preserves_keys_the_app_does_not_manage() {
        let existing = json!({
            "enabledPlugins": { "gopls-lsp@claude-plugins-official": true },
            "model": "opus",
            "env": { "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1" }
        });
        let out = render(&profile(DangerLevel::Bypass), Some(&existing));

        assert_eq!(
            out["enabledPlugins"]["gopls-lsp@claude-plugins-official"],
            true
        );
        assert_eq!(out["model"], "opus");
        assert_eq!(out["env"]["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"], "1");
    }

    #[test]
    fn skip_prompt_only_accompanies_full_bypass() {
        let bypass = render(&profile(DangerLevel::Bypass), None);
        assert_eq!(bypass["skipDangerousModePermissionPrompt"], true);

        let ask = render(&profile(DangerLevel::Ask), None);
        assert!(ask.get("skipDangerousModePermissionPrompt").is_none());
    }

    /// Claude has no workspace-write rung. It must land on the nearest SAFE
    /// mode — silently upgrading to full bypass would hand the user more
    /// permission than the bundle asked for.
    #[test]
    fn workspace_write_does_not_become_bypass_on_claude() {
        let out = render(&profile(DangerLevel::WorkspaceWrite), None);
        assert_eq!(out["permissions"]["defaultMode"], "acceptEdits");
        assert!(out.get("skipDangerousModePermissionPrompt").is_none());
    }

    /// Downgrading a profile must clear the escape hatch, not leave it set.
    #[test]
    fn downgrading_removes_the_skip_prompt_flag() {
        let existing = json!({ "skipDangerousModePermissionPrompt": true });
        let out = render(&profile(DangerLevel::Ask), Some(&existing));
        assert!(out.get("skipDangerousModePermissionPrompt").is_none());
    }
}
