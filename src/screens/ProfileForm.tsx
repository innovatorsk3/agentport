import { useEffect, useState } from "react";
import * as api from "@/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/Icons";
import {
  DANGER_LABELS,
  dangerLevelsFor,
  PRESETS,
  type CliKind,
  type DangerLevel,
  type ModelInfo,
  type ModelIssue,
  type Profile,
} from "@/types";

const CLI_LABELS: Record<CliKind, string> = {
  claude: "Claude Code",
  codex: "Codex",
};

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

/** Opens inline beneath the list rather than as its own screen, so the context
 *  stays on screen and there is no way to get stranded. */
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

  // An alias is typed at a shell prompt, so it is checked as you go rather than
  // at save time.
  useEffect(() => {
    let live = true;
    api
      .validateAlias(p.alias)
      .then((sh) => live && (setAliasError(null), setShadows(sh)))
      .catch((e) => {
        if (!live) return;
        // Distinguish "this alias is invalid" from "the check could not run".
        // Rendering a transport failure as a validation message blames the user
        // for something they did not do.
        const msg = String(e);
        setAliasError(msg.includes("invoke") ? null : msg);
        setShadows(false);
      });
    return () => {
      live = false;
    };
  }, [p.alias]);

  // Re-check whenever either side moves: a model valid for one provider may not
  // exist on another.
  useEffect(() => {
    if (!models) return setIssues([]);
    api.validateModelMapping(p.cli, p.model_map, models).then(setIssues);
  }, [models, p.cli, p.model_map]);

  function switchCli(cli: CliKind) {
    setP((prev) => ({ ...prev, ...PRESETS[cli], cli }) as Profile);
    setModels(null);
  }

  async function loadModels() {
    setLoadingModels(true);
    setModelError(null);
    try {
      const list = await api.fetchModels(p.base_url, p.api_key);
      setModels(list);
      // Suggest, never auto-apply. Where a provider serves no model of a family
      // the suggestion is empty — guessing there is what produces a config that
      // looks right and fails at call time.
      const suggested = await api.suggestModelMapping(p.cli, list);
      setP((prev) => ({ ...prev, model_map: { ...suggested, ...prev.model_map } }));
    } catch (e) {
      setModelError(String(e));
      setModels(null);
    } finally {
      setLoadingModels(false);
    }
  }

  const canLoadModels = p.base_url.length > 0 && p.api_key.length > 0;
  const canSave = !aliasError && p.alias && p.provider && p.base_url && p.api_key;

  const roles: Array<[keyof Profile["model_map"], string]> =
    p.cli === "claude"
      ? [
          ["opus", "Opus"],
          ["sonnet", "Sonnet"],
          ["haiku", "Haiku"],
        ]
      : [["default", "Model"]];

  const issueFor = (role: string) => issues.find((i) => i.role === role);

  return (
    <div className="bg-card/60 border-border animate-in fade-in slide-in-from-top-1 space-y-5 rounded-xl border p-4 duration-150 sm:p-5">
      <h3 className="text-sm font-medium">
        {initial ? `Edit ${initial.alias}` : "New profile"}
      </h3>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="CLI">
          <Select value={p.cli} onValueChange={(v) => v && switchCli(v as CliKind)}>
            <SelectTrigger className="w-full">
              {/* SelectValue renders the raw enum unless given a child, which
                  would show "claude" instead of "Claude Code". */}
              <SelectValue>{CLI_LABELS[p.cli]}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="claude">Claude Code</SelectItem>
              <SelectItem value="codex">Codex</SelectItem>
            </SelectContent>
          </Select>
        </Field>

        <Field
          label="Alias — what you type in a terminal"
          error={aliasError}
          warn={
            shadows && !aliasError
              ? "Shadows an existing command — it will work, but the original gets harder to reach."
              : null
          }
        >
          <Input
            value={p.alias}
            onChange={(e) => set("alias", e.target.value)}
            placeholder="cht"
            spellCheck={false}
          />
        </Field>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Provider">
          <Input
            value={p.provider}
            onChange={(e) => set("provider", e.target.value)}
            placeholder="my-provider.example"
            spellCheck={false}
          />
        </Field>

        <Field label="Base URL">
          <Input
            value={p.base_url}
            onChange={(e) => set("base_url", e.target.value)}
            placeholder="https://my-provider.example/v1"
            spellCheck={false}
          />
        </Field>
      </div>

      <Field label="API key">
        <Input
          value={p.api_key}
          onChange={(e) => set("api_key", e.target.value)}
          placeholder="sk-…"
          spellCheck={false}
          className="font-mono text-xs"
        />
      </Field>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field
          label="Environment variable carrying the key"
          hint="One per profile. Sharing a variable sends a key to the wrong provider."
        >
          <Input
            value={p.env_var}
            onChange={(e) => set("env_var", e.target.value)}
            spellCheck={false}
            className="font-mono text-xs"
          />
        </Field>

        <Field
          label="Permissions"
          hint={
            p.cli === "claude"
              ? "Claude has no equivalent of Codex's workspace-only rung."
              : undefined
          }
        >
          <Select
            value={p.danger}
            onValueChange={(v) => v && set("danger", v as DangerLevel)}
          >
            <SelectTrigger className="w-full">
              <SelectValue>{DANGER_LABELS[p.danger]}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {dangerLevelsFor(p.cli).map((d) => (
                <SelectItem key={d} value={d}>
                  {DANGER_LABELS[d]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
      </div>

      {p.cli === "codex" && (
        <Field label="Wire API">
          <Select
            value={p.wire_api ?? "responses"}
            onValueChange={(v) => v && set("wire_api", v)}
          >
            <SelectTrigger className="w-full sm:w-64">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="responses">responses</SelectItem>
              <SelectItem value="chat">chat completions</SelectItem>
            </SelectContent>
          </Select>
        </Field>
      )}

      <div className="space-y-4 border-t pt-4">
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="secondary"
            size="sm"
            onClick={loadModels}
            disabled={!canLoadModels || loadingModels}
          >
            {loadingModels && <Spinner className="size-3.5" />}
            {loadingModels ? "Loading…" : "Load models this key can use"}
          </Button>
          <span className="text-muted-foreground text-xs">
            {models
              ? `${models.length} available to this key`
              : canLoadModels
                ? "verifies the mapping against the provider"
                : "fill in the base URL and key first"}
          </span>
        </div>

        {modelError && <p className="text-destructive text-xs">{modelError}</p>}

        <div className="grid gap-4 sm:grid-cols-3">
          {roles.map(([role, label]) => {
            const issue = issueFor(role as string);
            const value = p.model_map[role] ?? "";
            const setModel = (v: string) =>
              set("model_map", { ...p.model_map, [role]: v || undefined });

            return (
              <Field
                key={role as string}
                label={label}
                error={
                  issue?.kind === "not_served"
                    ? `This provider does not serve ${issue.id} to this key — both CLIs accept it silently and fail at call time.`
                    : null
                }
                warn={
                  issue?.kind === "unset" ? "Required — nothing runs without it." : null
                }
              >
                {models ? (
                  <Select
                    value={value || "__none__"}
                    onValueChange={(v) => setModel(!v || v === "__none__" ? "" : v)}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__none__">— none —</SelectItem>
                      {models.map((m) => (
                        <SelectItem key={m.id} value={m.id} className="font-mono text-xs">
                          {m.id}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : (
                  <Input
                    value={value}
                    onChange={(e) => setModel(e.target.value)}
                    placeholder="load models to pick"
                    spellCheck={false}
                    className="font-mono text-xs"
                  />
                )}
              </Field>
            );
          })}
        </div>
      </div>

      <div className="flex items-center gap-2 border-t pt-4">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <div className="flex-1" />
        <Button size="sm" disabled={!canSave} onClick={() => onSave(p)}>
          {initial ? "Save changes" : "Add profile"}
        </Button>
      </div>
    </div>
  );
}

function Field({
  label,
  hint,
  warn,
  error,
  children,
}: {
  label: string;
  hint?: string;
  warn?: string | null;
  error?: string | null;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-muted-foreground text-xs font-normal">{label}</Label>
      {children}
      {error ? (
        <p className="text-destructive text-xs leading-relaxed">{error}</p>
      ) : warn ? (
        <p className="text-(--color-warn) text-xs leading-relaxed">{warn}</p>
      ) : hint ? (
        <p className="text-muted-foreground/80 text-xs leading-relaxed">{hint}</p>
      ) : null}
    </div>
  );
}
