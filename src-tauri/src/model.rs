//! Core types — a bundle stores INTENT, never files (requirements §7).
//!
//! No absolute paths live here. The destination machine translates intent into
//! whatever schema its installed CLI version expects — that is what makes
//! macOS → Windows work at all.

use serde::{Deserialize, Serialize};

/// Bump when the bundle shape changes. Old bundles still sit on disk somewhere.
pub const BUNDLE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliKind {
    Claude,
    Codex,
}

/// The "dangerous" setting as a NEUTRAL scale — not a boolean (requirements §8).
///
/// Each CLI exposes a different number of rungs; `writer/` translates this into
/// each one's own schema. `WorkspaceWrite` has no Claude equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DangerLevel {
    /// Prompt before anything dangerous.
    Ask,
    /// Auto-accept file edits, prompt for everything else.
    AcceptEdits,
    /// Never prompt, but block writes outside the workspace. Codex only.
    WorkspaceWrite,
    /// Never prompt, no sandbox.
    Bypass,
}

/// Maps a third-party provider's model names onto standard roles.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMap {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sonnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haiku: Option<String>,
    /// Codex: the default model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Where a profile came from — used by the first-run scan (requirements §14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Typed into the app by hand.
    Manual,
    /// Read out of an imported bundle.
    Imported,
    /// Discovered on this machine by scanning existing CLI config.
    Scanned,
}

impl Default for Origin {
    fn default() -> Self {
        Origin::Manual
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Alias name — this is also what gets TYPED in a terminal, so it must not
    /// contain spaces (§6).
    pub alias: String,

    pub cli: CliKind,

    /// Provider metadata, e.g. "htmustc".
    pub provider: String,

    pub base_url: String,

    /// The key travels inside the bundle in plaintext — DELIBERATE, this is an
    /// internal tool (§7).
    pub api_key: String,

    /// Environment variable that carries the key. MUST be stored per profile,
    /// never hardcoded — hardcoding it is exactly what made a second profile
    /// load its key into the wrong variable (evidence #3).
    pub env_var: String,

    pub danger: DangerLevel,

    #[serde(default)]
    pub model_map: ModelMap,

    /// Codex: "responses" | "chat". Ignored for Claude.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_api: Option<String>,

    #[serde(default)]
    pub origin: Origin,
}

impl Profile {
    /// IDENTITY = provider + base_url + cli (§9).
    ///
    /// Import compares this, NOT the alias — two profiles sharing a name can be
    /// completely different things.
    pub fn identity(&self) -> String {
        format!("{:?}|{}|{}", self.cli, self.provider, self.base_url)
    }

    /// Identical = same identity, same key, same danger level.
    ///
    /// Re-importing the very same bundle must skip silently and NOT spawn a
    /// copy — the lesson from seven duplicated `# Added by Antigravity` lines
    /// found in a real `.zshrc`.
    pub fn is_identical_to(&self, other: &Profile) -> bool {
        self.identity() == other.identity()
            && self.api_key == other.api_key
            && self.danger == other.danger
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub version: u32,
    pub profiles: Vec<Profile>,
}

impl Bundle {
    pub fn new(profiles: Vec<Profile>) -> Self {
        Self { version: BUNDLE_VERSION, profiles }
    }
}

/// COMPUTED on every app launch, never persisted as a flag (§10).
///
/// Persisting it means installing Codex leaves the profile greyed out forever
/// because a stale flag says otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProfileState {
    Ready,
    /// "not ready" — NOT "disabled". The user never turned it off; the machine
    /// is missing the CLI. Different wording drives a different next action.
    CliMissing { cli: CliKind },
}
