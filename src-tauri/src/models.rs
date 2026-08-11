//! Model discovery and validation (requirements §15).
//!
//! A profile's model mapping is only correct if the provider actually serves
//! those model ids to *this key*. Typing them by hand produces a config that
//! looks right and fails at call time.
//!
//! Evidence: on a live provider, `GET /v1/models` returned 14 models to both
//! keys and **not one Claude model among them**, while the machine's Claude
//! profiles were configured for `claude-opus-5`. Nothing in either CLI surfaces
//! that mismatch — it shows up as a failed call much later.

use crate::model::{CliKind, ModelMap};
use serde::{Deserialize, Serialize};

/// One entry from an OpenAI-compatible `GET /v1/models` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub owned_by: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelListResponse {
    data: Vec<ModelInfo>,
}

/// Parses an OpenAI-compatible model listing.
pub fn parse_model_list(body: &str) -> Result<Vec<ModelInfo>, String> {
    let parsed: ModelListResponse =
        serde_json::from_str(body).map_err(|e| format!("unexpected model list shape: {e}"))?;
    Ok(parsed.data)
}

/// Why a chosen model id cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelIssue {
    /// The id is not in the provider's list for this key.
    NotServed { role: String, id: String },
    /// A required role has no model assigned.
    Unset { role: String },
}

/// Checks a profile's model mapping against what the provider actually serves.
///
/// Only roles the CLI genuinely uses are checked — Codex has a single default
/// model and no opus/sonnet/haiku concept, so flagging those would be noise.
pub fn validate_mapping(cli: CliKind, map: &ModelMap, available: &[ModelInfo]) -> Vec<ModelIssue> {
    let served = |id: &str| available.iter().any(|m| m.id == id);
    let mut issues = Vec::new();

    let mut check = |role: &str, value: &Option<String>, required: bool| match value {
        Some(id) if !served(id) => issues.push(ModelIssue::NotServed {
            role: role.to_string(),
            id: id.clone(),
        }),
        None if required => issues.push(ModelIssue::Unset {
            role: role.to_string(),
        }),
        _ => {}
    };

    match cli {
        CliKind::Claude => {
            // Claude Code routes by tier. Opus is the one that must resolve;
            // the others fall back to provider defaults when unset.
            check("opus", &map.opus, true);
            check("sonnet", &map.sonnet, false);
            check("haiku", &map.haiku, false);
        }
        CliKind::Codex => {
            check("default", &map.default, true);
        }
    }

    issues
}

/// Suggests provider model ids for each role by matching on the role name.
///
/// A hint only — the user confirms. Returning nothing is correct and common:
/// a provider serving no Claude-family models has nothing to suggest for
/// `opus`, and inventing one would recreate the exact failure this guards.
pub fn suggest_mapping(cli: CliKind, available: &[ModelInfo]) -> ModelMap {
    let find = |needle: &str| {
        available
            .iter()
            .find(|m| m.id.to_lowercase().contains(needle))
            .map(|m| m.id.clone())
    };

    match cli {
        CliKind::Claude => ModelMap {
            opus: find("opus"),
            sonnet: find("sonnet"),
            haiku: find("haiku"),
            default: None,
        },
        CliKind::Codex => ModelMap {
            default: find("gpt").or_else(|| available.first().map(|m| m.id.clone())),
            ..ModelMap::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape taken verbatim from a live provider response.
    const LIVE_BODY: &str = r#"{
      "object": "list",
      "data": [
        {"id":"kimi-k2.7","object":"model","created":1784595814,"owned_by":"must1c"},
        {"id":"gpt-5.5","object":"model","created":1781881624,"owned_by":"must1c"},
        {"id":"gpt-5.4-mini","object":"model","created":1781881624,"owned_by":"must1c"},
        {"id":"deepseek-v4-pro","object":"model","created":1781959288,"owned_by":"must1c"}
      ]
    }"#;

    fn live_models() -> Vec<ModelInfo> {
        parse_model_list(LIVE_BODY).unwrap()
    }

    #[test]
    fn parses_a_real_model_listing() {
        let models = live_models();
        assert_eq!(models.len(), 4);
        assert_eq!(models[0].id, "kimi-k2.7");
        assert_eq!(models[0].owned_by, "must1c");
    }

    #[test]
    fn rejects_a_body_that_is_not_a_model_list() {
        assert!(parse_model_list("not json").is_err());
        assert!(parse_model_list(r#"{"error":{"message":"nope"}}"#).is_err());
    }

    /// The exact live situation: Claude profiles configured for `claude-opus-5`
    /// against a provider that serves no Claude models at all. Both CLIs accept
    /// this config silently; only a real call fails.
    #[test]
    fn flags_a_claude_model_the_provider_does_not_serve() {
        let map = ModelMap {
            opus: Some("claude-opus-5".into()),
            ..ModelMap::default()
        };

        let issues = validate_mapping(CliKind::Claude, &map, &live_models());
        assert_eq!(
            issues,
            vec![ModelIssue::NotServed {
                role: "opus".into(),
                id: "claude-opus-5".into()
            }]
        );
    }

    #[test]
    fn accepts_a_model_the_provider_serves() {
        let map = ModelMap {
            default: Some("gpt-5.5".into()),
            ..ModelMap::default()
        };
        assert!(validate_mapping(CliKind::Codex, &map, &live_models()).is_empty());
    }

    #[test]
    fn codex_requires_a_default_model() {
        let issues = validate_mapping(CliKind::Codex, &ModelMap::default(), &live_models());
        assert_eq!(issues, vec![ModelIssue::Unset { role: "default".into() }]);
    }

    /// Codex has no opus/sonnet/haiku concept — checking those would be noise.
    #[test]
    fn codex_ignores_claude_tier_roles() {
        let map = ModelMap {
            default: Some("gpt-5.5".into()),
            opus: Some("nonexistent-model".into()),
            ..ModelMap::default()
        };
        assert!(validate_mapping(CliKind::Codex, &map, &live_models()).is_empty());
    }

    /// A provider with no Claude-family models must suggest nothing rather than
    /// guess. Inventing a mapping here would recreate the failure above.
    #[test]
    fn suggests_nothing_when_no_matching_family_exists() {
        let suggested = suggest_mapping(CliKind::Claude, &live_models());
        assert_eq!(suggested.opus, None);
        assert_eq!(suggested.sonnet, None);
        assert_eq!(suggested.haiku, None);
    }

    #[test]
    fn suggests_matching_ids_when_the_family_is_present() {
        let models = parse_model_list(
            r#"{"data":[
                 {"id":"claude-opus-5","owned_by":"p"},
                 {"id":"claude-haiku-4.5","owned_by":"p"}
               ]}"#,
        )
        .unwrap();

        let suggested = suggest_mapping(CliKind::Claude, &models);
        assert_eq!(suggested.opus.as_deref(), Some("claude-opus-5"));
        assert_eq!(suggested.haiku.as_deref(), Some("claude-haiku-4.5"));
        assert_eq!(suggested.sonnet, None);
    }

    #[test]
    fn codex_falls_back_to_the_first_model_when_no_gpt_exists() {
        let models =
            parse_model_list(r#"{"data":[{"id":"glm-5.2","owned_by":"p"}]}"#).unwrap();
        assert_eq!(
            suggest_mapping(CliKind::Codex, &models).default.as_deref(),
            Some("glm-5.2")
        );
    }
}
