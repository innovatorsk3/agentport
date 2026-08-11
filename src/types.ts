// Mirrors the Rust types in src-tauri/src/model.rs.
// Serde renames enums to lowercase / snake_case, so the string literals here
// must match those exactly.

/** Deliberately distinctive so it does not slip into a repo unnoticed —
 *  bundles carry API keys in plaintext by design. */
export const BUNDLE_EXT = ".agentport";

export type CliKind = "claude" | "codex";

/** The neutral danger scale. Each CLI translates it differently (§8). */
export type DangerLevel = "ask" | "accept_edits" | "workspace_write" | "bypass";

export type Origin = "manual" | "imported" | "scanned";

export interface ModelMap {
  opus?: string;
  sonnet?: string;
  haiku?: string;
  default?: string;
}

export interface Profile {
  alias: string;
  cli: CliKind;
  provider: string;
  base_url: string;
  api_key: string;
  env_var: string;
  danger: DangerLevel;
  model_map: ModelMap;
  wire_api?: string;
  origin: Origin;
}

export interface Bundle {
  version: number;
  profiles: Profile[];
}

/** Computed per launch, never stored (§10). */
export type ProfileState =
  | { state: "ready" }
  | { state: "cli_missing"; cli: CliKind };

export interface ModelInfo {
  id: string;
  owned_by: string;
}

export type ModelIssue =
  | { kind: "not_served"; role: string; id: string }
  | { kind: "unset"; role: string };

export type ImportPlan =
  | { kind: "skip"; alias: string }
  | { kind: "add"; alias: string }
  | { kind: "rename"; from: string; to: string };

export type ProbeResult =
  | { outcome: "ok"; millis: number }
  | { outcome: "bad_key"; detail: string }
  | { outcome: "no_credit"; detail: string }
  | { outcome: "model_unavailable"; detail: string }
  | { outcome: "other"; status: number; detail: string }
  | { outcome: "unreachable"; detail: string };

export interface ProbeReport {
  alias: string;
  endpoint: string;
  result: ProbeResult;
  advice: string;
}

export interface InstallReport {
  configs: string[];
  script: string;
  rc_file: string;
  /** Only when true does the user need to open a new terminal. */
  rc_line_added: boolean;
}

/** Presets for a first run. The alias is a starting point, always editable. */
export const PRESETS: Record<CliKind, Partial<Profile>> = {
  claude: {
    cli: "claude",
    alias: "cc",
    // Claude Code reads the key from this fixed variable.
    env_var: "ANTHROPIC_AUTH_TOKEN",
    danger: "bypass",
    model_map: {},
  },
  codex: {
    cli: "codex",
    alias: "cx",
    // Codex names one variable per provider; a per-profile default avoids the
    // shared-variable mix-up that broke a second profile in practice.
    env_var: "AGENTPORT_CX_API_KEY",
    danger: "bypass",
    wire_api: "responses",
    model_map: {},
  },
};

export const DANGER_LABELS: Record<DangerLevel, string> = {
  ask: "Ask before anything dangerous",
  accept_edits: "Auto-accept file edits",
  workspace_write: "Never ask, but block writes outside the workspace",
  bypass: "Never ask, no sandbox",
};

/** Codex has an intermediate rung Claude has no equivalent of. */
export function dangerLevelsFor(cli: CliKind): DangerLevel[] {
  return cli === "codex"
    ? ["ask", "accept_edits", "workspace_write", "bypass"]
    : ["ask", "accept_edits", "bypass"];
}
