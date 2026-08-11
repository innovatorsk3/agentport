import { useEffect, useState } from "react";
import * as api from "../api";
import {
  DANGER_LABELS,
  dangerLevelsFor,
  PRESETS,
  type CliKind,
  type DangerLevel,
  type ModelInfo,
  type ModelIssue,
  type Profile,
} from "../types";

interface Props {
  initial?: Profile;
  onSave: (p: Profile) => void;
  onCancel: () => void;
}

function blank(cli: CliKind): Profile {
  return {
    provider: "",
    base_url: "",
    api_key: "",
    origin: "manual",
    ...PRESETS[cli],
  } as Profile;
}

export function ProfileForm({ initial, onSave, onCancel }: Props) {
  const [p, setP] = useState<Profile>(initial ?? blank("claude"));
  const [aliasError, setAliasError] = useState<string | null>(null);
  const [shadows, setShadows] = useState(false);
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const [issues, setIssues] = useState<ModelIssue[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);

  const set = <K extends keyof Profile>(k: K, v: Profile[K]) =>
    setP((prev) => ({ ...prev, [k]: v }));

  // An alias is typed at a shell prompt, so it is validated as you go rather
  // than at save time.
  useEffect(() => {
    let live = true;
    api
      .validateAlias(p.alias)
      .then((sh) => live && (setAliasError(null), setShadows(sh)))
      .catch((e) => live && (setAliasError(String(e)), setShadows(false)));
    return () => {
      live = false;
    };
  }, [p.alias]);

  // Re-check the mapping whenever either side changes: a model that was valid
  // for one provider may not exist on another.
  useEffect(() => {
    if (!models) return setIssues([]);
    api.validateModelMapping(p.cli, p.model_map, models).then(setIssues);
  }, [models, p.cli, p.model_map]);

  function switchCli(cli: CliKind) {
    setP((prev) => ({ ...prev, ...PRESETS[cli], cli } as Profile));
    setModels(null);
  }

  async function loadModels() {
    setLoadingModels(true);
    setModelError(null);
    try {
      const list = await api.fetchModels(p.base_url, p.api_key);
      setModels(list);
      // Suggest, never auto-apply. Where a provider serves no model of a
      // family the suggestion is empty — guessing there is what produces a
      // config that looks right and fails at call time.
      const suggested = await api.suggestModelMapping(p.cli, list);
      setP((prev) => ({
        ...prev,
        model_map: { ...suggested, ...prev.model_map },
      }));
    } catch (e) {
      setModelError(String(e));
      setModels(null);
    } finally {
      setLoadingModels(false);
    }
  }

  const canLoadModels = p.base_url.length > 0 && p.api_key.length > 0;
  const canSave =
    !aliasError && p.alias && p.provider && p.base_url && p.api_key;

  const roles: Array<[keyof Profile["model_map"], string]> =
    p.cli === "claude"
      ? [
          ["opus", "Opus"],
          ["sonnet", "Sonnet"],
          ["haiku", "Haiku"],
        ]
      : [["default", "Model"]];

  function issueFor(role: string) {
    return issues.find((i) => i.role === role);
  }

  return (
    <div>
      <h2>{initial ? "Edit profile" : "New profile"}</h2>

      <div className="two-col">
        <div className="field">
          <label>CLI</label>
          <select
            value={p.cli}
            onChange={(e) => switchCli(e.target.value as CliKind)}
          >
            <option value="claude">Claude Code</option>
            <option value="codex">Codex</option>
          </select>
        </div>

        <div className="field">
          <label>Alias — what you type in a terminal</label>
          <input
            value={p.alias}
            onChange={(e) => set("alias", e.target.value)}
            placeholder="cht"
          />
          {aliasError && <div className="hint bad">{aliasError}</div>}
          {shadows && !aliasError && (
            <div className="hint warn">
              This shadows an existing command. It will still work, but the
              original becomes harder to reach.
            </div>
          )}
        </div>
      </div>

      <div className="two-col">
        <div className="field">
          <label>Provider</label>
          <input
            value={p.provider}
            onChange={(e) => set("provider", e.target.value)}
            placeholder="my-provider.example"
          />
        </div>

        <div className="field">
          <label>Base URL</label>
          <input
            value={p.base_url}
            onChange={(e) => set("base_url", e.target.value)}
            placeholder="https://my-provider.example/v1"
          />
        </div>
      </div>

      <div className="field">
        <label>API key</label>
        <input
          value={p.api_key}
          onChange={(e) => set("api_key", e.target.value)}
          placeholder="sk-…"
        />
      </div>

      <div className="two-col">
        <div className="field">
          <label>Environment variable carrying the key</label>
          <input
            value={p.env_var}
            onChange={(e) => set("env_var", e.target.value)}
          />
          <div className="hint">
            Each profile gets its own. Sharing one across profiles sends a key
            to the wrong provider.
          </div>
        </div>

        <div className="field">
          <label>Permissions</label>
          <select
            value={p.danger}
            onChange={(e) => set("danger", e.target.value as DangerLevel)}
          >
            {dangerLevelsFor(p.cli).map((d) => (
              <option key={d} value={d}>
                {DANGER_LABELS[d]}
              </option>
            ))}
          </select>
        </div>
      </div>

      {p.cli === "codex" && (
        <div className="field">
          <label>Wire API</label>
          <select
            value={p.wire_api ?? "responses"}
            onChange={(e) => set("wire_api", e.target.value)}
          >
            <option value="responses">responses</option>
            <option value="chat">chat completions</option>
          </select>
        </div>
      )}

      <h2 style={{ marginTop: 24 }}>Models</h2>
      <div className="field">
        <button onClick={loadModels} disabled={!canLoadModels || loadingModels}>
          {loadingModels ? "Loading…" : "Load models this key can use"}
        </button>
        {!canLoadModels && (
          <div className="hint">Fill in the base URL and key first.</div>
        )}
        {modelError && <div className="hint bad">{modelError}</div>}
        {models && (
          <div className="hint">
            Provider serves {models.length} model
            {models.length === 1 ? "" : "s"} to this key.
          </div>
        )}
      </div>

      {roles.map(([role, label]) => {
        const issue = issueFor(role as string);
        return (
          <div className="field" key={role as string}>
            <label>{label}</label>
            {models ? (
              <select
                value={p.model_map[role] ?? ""}
                onChange={(e) =>
                  set("model_map", {
                    ...p.model_map,
                    [role]: e.target.value || undefined,
                  })
                }
              >
                <option value="">— none —</option>
                {models.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.id}
                  </option>
                ))}
              </select>
            ) : (
              <input
                value={p.model_map[role] ?? ""}
                onChange={(e) =>
                  set("model_map", {
                    ...p.model_map,
                    [role]: e.target.value || undefined,
                  })
                }
                placeholder="load models to choose from a list"
              />
            )}
            {issue?.kind === "not_served" && (
              <div className="hint bad">
                This provider does not serve <code>{issue.id}</code> to this
                key. Both CLIs accept it silently and fail at call time.
              </div>
            )}
            {issue?.kind === "unset" && (
              <div className="hint warn">Required — nothing will run without it.</div>
            )}
          </div>
        );
      })}

      <div className="actions">
        <button onClick={onCancel}>Cancel</button>
        <div className="spacer" />
        <button
          className="primary"
          disabled={!canSave}
          onClick={() => onSave(p)}
        >
          Save profile
        </button>
      </div>
    </div>
  );
}
