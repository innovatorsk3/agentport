//! Translates a profile into a Codex overlay (requirements §7, §8).
//!
//! Written to `<codex_dir>/<alias>.config.toml`, which `codex --profile <alias>`
//! layers over `config.toml`. The base `config.toml` is tier-1 config and is
//! never touched (§4).

use crate::model::{DangerLevel, Profile};
use toml_edit::{DocumentMut, Item, Table, value};

/// Provider table key, derived from the alias so two profiles never collide.
pub fn provider_key(alias: &str) -> String {
    let sanitized: String = alias
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("agentport_{sanitized}")
}

/// Default env var name for a profile's key.
///
/// Each profile gets its OWN variable. Sharing one is exactly what made a
/// second profile load its key into the wrong variable (evidence #3).
pub fn default_env_var(alias: &str) -> String {
    let sanitized: String = alias
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("AGENTPORT_{sanitized}_API_KEY")
}

/// Renders the overlay TOML.
///
/// `existing` is the current file text, if any. It is edited in place rather
/// than regenerated: these files accumulate state the app knows nothing about
/// (hook trust hashes, per-project trust levels), and rewriting from scratch
/// would silently delete it.
pub fn render(profile: &Profile, existing: Option<&str>) -> Result<String, String> {
    let mut doc: DocumentMut = match existing {
        Some(text) => text
            .parse()
            .map_err(|e| format!("existing config is not valid TOML: {e}"))?,
        None => DocumentMut::new(),
    };

    let key = provider_key(&profile.alias);
    doc["model_provider"] = value(key.clone());

    if let Some(model) = &profile.model_map.default {
        doc["model"] = value(model.clone());
    }

    // Codex splits danger across TWO axes, unlike Claude's single mode (§8).
    let (approval, sandbox) = danger_axes(profile.danger);
    doc["approval_policy"] = value(approval);
    doc["sandbox_mode"] = value(sandbox);

    // --- [model_providers.<key>] ----------------------------------------
    if !doc.contains_key("model_providers") {
        let mut t = Table::new();
        t.set_implicit(true);
        doc["model_providers"] = Item::Table(t);
    }
    let providers = doc["model_providers"]
        .as_table_mut()
        .ok_or("model_providers is not a table")?;

    let mut provider = Table::new();
    provider["name"] = value(profile.provider.clone());
    provider["base_url"] = value(profile.base_url.clone());
    provider["env_key"] = value(profile.env_var.clone());
    provider["wire_api"] = value(profile.wire_api.clone().unwrap_or_else(|| "responses".into()));
    providers[&key] = Item::Table(provider);

    Ok(doc.to_string())
}

/// Maps the neutral scale onto Codex's two axes (§8).
fn danger_axes(level: DangerLevel) -> (&'static str, &'static str) {
    match level {
        DangerLevel::Ask => ("untrusted", "read-only"),
        DangerLevel::AcceptEdits => ("on-request", "workspace-write"),
        DangerLevel::WorkspaceWrite => ("never", "workspace-write"),
        DangerLevel::Bypass => ("never", "danger-full-access"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CliKind, ModelMap, Origin};

    fn profile(alias: &str, danger: DangerLevel) -> Profile {
        Profile {
            alias: alias.into(),
            cli: CliKind::Codex,
            provider: "htmustc.id.vn".into(),
            base_url: "https://htmustc.id.vn/v1".into(),
            api_key: "mk-live-secret".into(),
            env_var: default_env_var(alias),
            danger,
            model_map: ModelMap {
                default: Some("gpt-5.5".into()),
                ..ModelMap::default()
            },
            wire_api: Some("responses".into()),
            origin: Origin::Manual,
        }
    }

    /// `toml` v1's `FromStr` parses a single VALUE, not a document — use
    /// `from_str` or every multi-line render looks like a syntax error.
    fn parse(text: &str) -> toml::Value {
        toml::from_str(text).expect("output must be valid TOML")
    }

    #[test]
    fn renders_valid_toml_with_a_provider_table() {
        let out = render(&profile("ht", DangerLevel::Bypass), None).unwrap();
        let v = parse(&out);

        assert_eq!(v["model_provider"].as_str(), Some("agentport_ht"));
        assert_eq!(v["model"].as_str(), Some("gpt-5.5"));

        let p = &v["model_providers"]["agentport_ht"];
        assert_eq!(p["base_url"].as_str(), Some("https://htmustc.id.vn/v1"));
        assert_eq!(p["env_key"].as_str(), Some("AGENTPORT_HT_API_KEY"));
        assert_eq!(p["wire_api"].as_str(), Some("responses"));
    }

    /// Full bypass needs BOTH axes unrestricted.
    #[test]
    fn bypass_sets_both_axes() {
        let out = render(&profile("ht", DangerLevel::Bypass), None).unwrap();
        let v = parse(&out);
        assert_eq!(v["approval_policy"].as_str(), Some("never"));
        assert_eq!(v["sandbox_mode"].as_str(), Some("danger-full-access"));
    }

    /// The intermediate rung Claude has no equivalent of: never prompts, but
    /// still refuses writes outside the workspace.
    #[test]
    fn workspace_write_keeps_the_sandbox() {
        let out = render(&profile("ht", DangerLevel::WorkspaceWrite), None).unwrap();
        let v = parse(&out);
        assert_eq!(v["approval_policy"].as_str(), Some("never"));
        assert_eq!(v["sandbox_mode"].as_str(), Some("workspace-write"));
    }

    #[test]
    fn ask_is_the_most_restrictive_pairing() {
        let out = render(&profile("ht", DangerLevel::Ask), None).unwrap();
        let v = parse(&out);
        assert_eq!(v["approval_policy"].as_str(), Some("untrusted"));
        assert_eq!(v["sandbox_mode"].as_str(), Some("read-only"));
    }

    /// Real config files carry hook trust hashes and per-project trust levels.
    /// Regenerating from scratch would delete them silently.
    #[test]
    fn preserves_unrelated_tables_in_an_existing_file() {
        let existing = r#"
model = "old-model"

[hooks.state]

[hooks.state."/Users/mac/.codex/hooks.json:session_start:0:0"]
trusted_hash = "sha256:0888d0d0"

[projects."/Users/mac/code"]
trust_level = "trusted"
"#;
        let out = render(&profile("ht", DangerLevel::Bypass), Some(existing)).unwrap();
        let v = parse(&out);

        assert_eq!(v["model"].as_str(), Some("gpt-5.5")); // updated
        assert!(v["hooks"]["state"].get("/Users/mac/.codex/hooks.json:session_start:0:0").is_some());
        assert_eq!(v["projects"]["/Users/mac/code"]["trust_level"].as_str(), Some("trusted"));
    }

    /// Each profile must carry its own variable — evidence #3.
    #[test]
    fn each_profile_gets_a_distinct_env_var() {
        assert_eq!(default_env_var("ht"), "AGENTPORT_HT_API_KEY");
        assert_eq!(default_env_var("cse"), "AGENTPORT_CSE_API_KEY");
        assert_ne!(default_env_var("ht"), default_env_var("cse"));
    }

    /// An alias with a dash is legal in a shell but not in a bare TOML key.
    #[test]
    fn dashed_aliases_produce_usable_keys() {
        let out = render(&profile("co-ht", DangerLevel::Bypass), None).unwrap();
        let v = parse(&out);
        assert_eq!(v["model_provider"].as_str(), Some("agentport_co_ht"));
        assert!(v["model_providers"].get("agentport_co_ht").is_some());
        assert_eq!(default_env_var("co-ht"), "AGENTPORT_CO_HT_API_KEY");
    }

    /// A key the user set by hand must survive, not be replaced by the default.
    #[test]
    fn honours_a_custom_env_var() {
        let mut p = profile("ht", DangerLevel::Bypass);
        p.env_var = "MUST1C_HT_API_KEY".into();
        let out = render(&p, None).unwrap();
        let v = parse(&out);
        assert_eq!(
            v["model_providers"]["agentport_ht"]["env_key"].as_str(),
            Some("MUST1C_HT_API_KEY")
        );
    }

    #[test]
    fn malformed_existing_toml_is_an_error_not_a_silent_overwrite() {
        let err = render(&profile("ht", DangerLevel::Bypass), Some("[[[ not toml"));
        assert!(err.is_err());
    }

    /// Rendering twice must converge, not accumulate duplicate tables.
    #[test]
    fn rendering_is_idempotent() {
        let once = render(&profile("ht", DangerLevel::Bypass), None).unwrap();
        let twice = render(&profile("ht", DangerLevel::Bypass), Some(&once)).unwrap();
        assert_eq!(once, twice);
    }
}
