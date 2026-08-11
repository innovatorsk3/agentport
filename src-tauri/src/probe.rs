//! Tests a profile with a REAL generation call and classifies the failure
//! (requirements §10).
//!
//! The point is the classification. Three failures look identical to a user —
//! "it does not work" — but demand completely different responses:
//!
//!   401           wrong key            paste a different key
//!   402           out of credit        top up
//!   timeout/500   model not mapped     fix it in the provider's admin panel
//!
//! The third one cost twenty minutes of `curl` to identify by hand, and is the
//! most valuable thing this app does.
//!
//! Crucially this is NOT a reachability ping. A live provider returned
//! `200 OK` on `GET /v1/models` while `POST /v1/responses` hung indefinitely
//! for the same key (evidence #4) — checking the cheap endpoint yields a lying
//! green tick.

use crate::model::{CliKind, Profile};
use serde::Serialize;
use std::time::Duration;

/// How long to wait before calling a request hung.
///
/// Generous: a cold model can legitimately take a while, and a false "timeout"
/// would send the user to the wrong fix.
pub const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProbeResult {
    /// The provider generated text.
    Ok { millis: u64 },
    /// 401 — the key is not accepted.
    BadKey { detail: String },
    /// 402 — authenticated, but out of credit.
    NoCredit { detail: String },
    /// Hung or 5xx — reached the provider but no model answered. Almost always
    /// means the key has no model mapped to this endpoint.
    ModelUnavailable { detail: String },
    /// Anything else, reported verbatim rather than guessed at.
    Other { status: u16, detail: String },
    /// Never reached the provider at all.
    Unreachable { detail: String },
}

/// The endpoint that actually generates text for this CLI kind.
///
/// Deliberately not `/v1/models`: that endpoint answers from auth alone and
/// proves nothing about whether a model can be called.
///
/// The two CLIs disagree about where `/v1` lives, and real profiles on disk
/// prove it: Claude stores `https://host` and appends `/v1/...` itself, while
/// Codex stores `https://host/v1` and appends only the path. Naively appending
/// a full path to either produced `/v1/v1/messages` and a 404 — caught by
/// probing a live provider, not by any unit test written beforehand. So strip
/// a trailing `/v1` first and build the whole path from a known root.
pub fn generation_endpoint(profile: &Profile) -> String {
    let root = profile
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .trim_end_matches('/');

    match profile.cli {
        CliKind::Claude => format!("{root}/v1/messages"),
        CliKind::Codex => match profile.wire_api.as_deref() {
            Some("chat") => format!("{root}/v1/chat/completions"),
            _ => format!("{root}/v1/responses"),
        },
    }
}

/// The smallest request that still forces real generation.
pub fn probe_body(profile: &Profile) -> serde_json::Value {
    let model = match profile.cli {
        CliKind::Claude => profile
            .model_map
            .opus
            .clone()
            .or_else(|| profile.model_map.sonnet.clone())
            .or_else(|| profile.model_map.haiku.clone()),
        CliKind::Codex => profile.model_map.default.clone(),
    }
    .unwrap_or_default();

    match profile.cli {
        CliKind::Claude => serde_json::json!({
            "model": model,
            "max_tokens": 4,
            "messages": [{ "role": "user", "content": "hi" }],
        }),
        CliKind::Codex => match profile.wire_api.as_deref() {
            Some("chat") => serde_json::json!({
                "model": model,
                "max_tokens": 4,
                "messages": [{ "role": "user", "content": "hi" }],
            }),
            _ => serde_json::json!({
                "model": model,
                "input": "hi",
                "stream": false,
            }),
        },
    }
}

/// Maps an HTTP status and body onto an outcome.
///
/// 5xx lands on `ModelUnavailable` rather than a generic error because that is
/// what it means in practice: the gateway accepted the key, then found nothing
/// behind it. A live provider returned `500 internal_error` for exactly this.
///
/// A 404 is ambiguous on its own — it can mean a wrong base URL or a model the
/// provider does not serve. Probing live surfaced both from the same gateway:
/// `Unknown endpoint: POST /v1/v1/messages` versus
/// `Model 'claude-opus-5' not found`. Same status, opposite fixes, so the body
/// decides.
pub fn classify_status(status: u16, body: &str) -> ProbeResult {
    let detail = truncate(body, 300);
    match status {
        200..=299 => ProbeResult::Ok { millis: 0 },
        401 | 403 => ProbeResult::BadKey { detail },
        402 => ProbeResult::NoCredit { detail },
        429 => ProbeResult::Other { status, detail },
        404 if mentions_model(body) => ProbeResult::ModelUnavailable { detail },
        500..=599 => ProbeResult::ModelUnavailable { detail },
        other => ProbeResult::Other { status: other, detail },
    }
}

/// True when an error body blames the model rather than the route.
fn mentions_model(body: &str) -> bool {
    let lower = body.to_lowercase();
    (lower.contains("model") && !lower.contains("endpoint")) || lower.contains("model_not_found")
}

fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    trimmed.chars().take(max).collect::<String>() + "…"
}

/// Runs the probe against a live provider.
pub async fn probe(profile: &Profile) -> ProbeResult {
    let client = match reqwest::Client::builder().timeout(TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult::Unreachable {
                detail: format!("cannot build HTTP client: {e}"),
            }
        }
    };

    let mut req = client
        .post(generation_endpoint(profile))
        .json(&probe_body(profile));

    // Claude's own API uses x-api-key + a version header; OpenAI-compatible
    // gateways in front of it accept a bearer token. Send both so a proxy and a
    // first-party endpoint both work.
    req = match profile.cli {
        CliKind::Claude => req
            .header("x-api-key", &profile.api_key)
            .header("anthropic-version", "2023-06-01")
            .bearer_auth(&profile.api_key),
        CliKind::Codex => req.bearer_auth(&profile.api_key),
    };

    let started = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            match classify_status(status, &body) {
                ProbeResult::Ok { .. } => ProbeResult::Ok {
                    millis: started.elapsed().as_millis() as u64,
                },
                other => other,
            }
        }
        Err(e) if e.is_timeout() => ProbeResult::ModelUnavailable {
            detail: format!("no response within {}s", TIMEOUT.as_secs()),
        },
        Err(e) => ProbeResult::Unreachable {
            detail: e.to_string(),
        },
    }
}

/// One line of guidance per outcome. The whole point of classifying.
pub fn advice(result: &ProbeResult) -> &'static str {
    match result {
        ProbeResult::Ok { .. } => "Working.",
        ProbeResult::BadKey { .. } => "The provider rejected this key. Paste a different one.",
        ProbeResult::NoCredit { .. } => "The key is valid but out of credit. Top up.",
        ProbeResult::ModelUnavailable { .. } => {
            "The provider accepted the key but no model answered. \
             Check that this model is mapped to this endpoint in the provider's admin panel."
        }
        ProbeResult::Other { .. } => "Unexpected response — see the detail below.",
        ProbeResult::Unreachable { .. } => "Could not reach the provider. Check the base URL.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DangerLevel, ModelMap, Origin};

    fn profile(cli: CliKind, wire: Option<&str>) -> Profile {
        Profile {
            alias: "t".into(),
            cli,
            provider: "htmustc.id.vn".into(),
            base_url: "https://htmustc.id.vn/v1".into(),
            api_key: "mk-live-secret".into(),
            env_var: "K".into(),
            danger: DangerLevel::Bypass,
            model_map: ModelMap {
                default: Some("gpt-5.5".into()),
                opus: Some("claude-opus-5".into()),
                ..ModelMap::default()
            },
            wire_api: wire.map(str::to_string),
            origin: Origin::Manual,
        }
    }

    /// The core lesson of evidence #4: never probe the cheap endpoint.
    #[test]
    fn never_probes_the_models_endpoint() {
        for p in [
            profile(CliKind::Claude, None),
            profile(CliKind::Codex, Some("responses")),
            profile(CliKind::Codex, Some("chat")),
        ] {
            assert!(
                !generation_endpoint(&p).contains("/models"),
                "a models listing answers from auth alone and proves nothing"
            );
        }
    }

    #[test]
    fn picks_the_endpoint_matching_the_wire_api() {
        assert!(generation_endpoint(&profile(CliKind::Codex, Some("responses")))
            .ends_with("/v1/responses"));
        assert!(generation_endpoint(&profile(CliKind::Codex, Some("chat")))
            .ends_with("/v1/chat/completions"));
        assert!(generation_endpoint(&profile(CliKind::Claude, None)).ends_with("/v1/messages"));
    }

    /// Codex defaults to the responses API when the field is absent.
    #[test]
    fn missing_wire_api_defaults_to_responses() {
        assert!(generation_endpoint(&profile(CliKind::Codex, None)).ends_with("/v1/responses"));
    }

    #[test]
    fn a_trailing_slash_does_not_double_up() {
        let mut p = profile(CliKind::Codex, Some("responses"));
        p.base_url = "https://htmustc.id.vn/v1/".into();
        assert_eq!(generation_endpoint(&p), "https://htmustc.id.vn/v1/responses");
    }

    /// Found by probing a live provider: appending `/v1/messages` to a base URL
    /// that already ended in `/v1` produced `/v1/v1/messages` and a 404. Both
    /// storage conventions occur in real profiles on disk.
    #[test]
    fn a_base_url_already_ending_in_v1_is_not_doubled() {
        for base in [
            "https://htmustc.id.vn",
            "https://htmustc.id.vn/",
            "https://htmustc.id.vn/v1",
            "https://htmustc.id.vn/v1/",
        ] {
            let mut c = profile(CliKind::Claude, None);
            c.base_url = base.into();
            assert_eq!(
                generation_endpoint(&c),
                "https://htmustc.id.vn/v1/messages",
                "claude base {base}"
            );

            let mut x = profile(CliKind::Codex, Some("responses"));
            x.base_url = base.into();
            assert_eq!(
                generation_endpoint(&x),
                "https://htmustc.id.vn/v1/responses",
                "codex base {base}"
            );
        }
    }

    // ---- classification: three symptoms, three fixes ---------------------

    #[test]
    fn unauthorised_is_a_key_problem() {
        let r = classify_status(401, r#"{"error":{"message":"invalid key"}}"#);
        assert!(matches!(r, ProbeResult::BadKey { .. }));
        assert!(advice(&r).contains("key"));
    }

    /// Taken from a real 402 body seen during this project.
    #[test]
    fn payment_required_is_a_credit_problem() {
        let r = classify_status(
            402,
            r#"{"error":{"message":"Out of credit","code":"insufficient_quota"}}"#,
        );
        assert!(matches!(r, ProbeResult::NoCredit { .. }));
        assert!(advice(&r).contains("credit"));
    }

    /// A live provider returned exactly this while `/v1/models` was 200 OK.
    #[test]
    fn server_error_points_at_model_mapping_not_the_key() {
        let r = classify_status(500, r#"{"error":{"code":"internal_error"}}"#);
        assert!(matches!(r, ProbeResult::ModelUnavailable { .. }));
        assert!(advice(&r).contains("admin panel"));
    }

    /// Each outcome must give DIFFERENT guidance — identical advice would make
    /// the classification pointless.
    #[test]
    fn every_failure_class_gives_distinct_guidance() {
        let advices = [
            advice(&classify_status(401, "")),
            advice(&classify_status(402, "")),
            advice(&classify_status(500, "")),
            advice(&ProbeResult::Unreachable { detail: String::new() }),
        ];
        for (i, a) in advices.iter().enumerate() {
            for (j, b) in advices.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "outcomes {i} and {j} give identical advice");
                }
            }
        }
    }

    /// Live gateway body, verbatim. A model the provider does not serve is a
    /// model problem even though the status is 404.
    #[test]
    fn model_not_found_is_a_model_problem_despite_the_404() {
        let r = classify_status(
            404,
            r#"{"error":{"message":"Model 'claude-opus-5' not found","code":"model_not_found"}}"#,
        );
        assert!(matches!(r, ProbeResult::ModelUnavailable { .. }));
        assert!(advice(&r).contains("admin panel"));
    }

    /// The other 404 from the same gateway: a wrong URL, which needs a
    /// completely different fix and must NOT be blamed on the model.
    #[test]
    fn unknown_endpoint_404_is_not_blamed_on_the_model() {
        let r = classify_status(
            404,
            r#"{"error":{"message":"Unknown endpoint: POST /v1/v1/messages","code":"not_found"}}"#,
        );
        assert!(matches!(r, ProbeResult::Other { status: 404, .. }));
    }

    /// Rate limiting is not a misconfiguration — do not send the user to the
    /// admin panel over a temporary throttle.
    #[test]
    fn rate_limiting_is_not_a_model_problem() {
        let r = classify_status(429, "slow down");
        assert!(matches!(r, ProbeResult::Other { status: 429, .. }));
    }

    #[test]
    fn success_is_success() {
        assert!(matches!(classify_status(200, "{}"), ProbeResult::Ok { .. }));
    }

    #[test]
    fn error_bodies_are_truncated_not_dumped() {
        let huge = "x".repeat(5000);
        let r = classify_status(500, &huge);
        if let ProbeResult::ModelUnavailable { detail } = r {
            assert!(detail.chars().count() <= 301);
        } else {
            panic!("expected ModelUnavailable");
        }
    }

    // ---- request body ----------------------------------------------------

    #[test]
    fn body_matches_the_wire_api() {
        let responses = probe_body(&profile(CliKind::Codex, Some("responses")));
        assert_eq!(responses["input"], "hi");
        assert!(responses.get("messages").is_none());

        let chat = probe_body(&profile(CliKind::Codex, Some("chat")));
        assert!(chat["messages"].is_array());
    }

    #[test]
    fn body_asks_for_the_smallest_possible_generation() {
        let b = probe_body(&profile(CliKind::Claude, None));
        assert_eq!(b["max_tokens"], 4);
        assert_eq!(b["model"], "claude-opus-5");
    }

    /// A Claude profile with only a haiku mapping must still probe something
    /// rather than sending an empty model id.
    #[test]
    fn falls_back_through_the_claude_tiers() {
        let mut p = profile(CliKind::Claude, None);
        p.model_map = ModelMap {
            haiku: Some("claude-haiku-4.5".into()),
            ..ModelMap::default()
        };
        assert_eq!(probe_body(&p)["model"], "claude-haiku-4.5");
    }
}
