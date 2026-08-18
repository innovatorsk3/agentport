import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { StatusBadge, probeTone } from "@/components/StatusBadge";
import {
  BoltIcon,
  PencilIcon,
  PlusIcon,
  TrashIcon,
} from "@/components/Icons";
import { ProfileForm } from "./ProfileForm";
import { profileConfigurationIssue, type CliKind, type ProbeReport, type Profile } from "@/types";

const CLI_NAME: Record<CliKind, string> = {
  claude: "Claude Code",
  codex: "Codex",
};

interface Props {
  profiles: Profile[];
  installed: Record<CliKind, boolean>;
  /** Per-alias probe outcome, shown inline under its own row. */
  probes: Record<string, ProbeReport>;
  testing: Set<string>;
  editing: string | null;
  adding: boolean;
  onEdit: (alias: string | null) => void;
  onAdd: () => void;
  onCancelForm: () => void;
  onSave: (p: Profile) => void;
  onRemove: (alias: string) => void;
  onTest: (p: Profile) => void;
  onRecheck: () => void;
  selectedForExport: Set<string>;
  onToggleExport: (alias: string) => void;
}

export function ProfileList({
  profiles,
  installed,
  probes,
  testing,
  editing,
  adding,
  onEdit,
  onAdd,
  onCancelForm,
  onSave,
  onRemove,
  onTest,
  onRecheck,
  selectedForExport,
  onToggleExport,
}: Props) {
  const missing = profiles.filter((p) => !installed[p.cli]);
  const incomplete = profiles.filter(
    (p) => installed[p.cli] && profileConfigurationIssue(p),
  );

  return (
    <div className="space-y-3">
      {profiles.map((p) => {
        const configIssue = profileConfigurationIssue(p);
        const ready = installed[p.cli] && !configIssue;
        const probe = probes[p.alias];
        const busy = testing.has(p.alias);
        const isEditing = editing === p.alias;

        return (
          <div key={p.alias} className="space-y-2">
            <Card
              className={`gap-0 overflow-hidden p-0 transition-colors ${
                isEditing ? "border-primary/40" : ""
              }`}
            >
              <div
                className={`flex flex-wrap items-center gap-x-4 gap-y-2 p-3 sm:px-4 ${
                  ready ? "" : "opacity-60"
                }`}
              >
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={selectedForExport.has(p.alias)}
                    onChange={() => onToggleExport(p.alias)}
                    aria-label={`Include ${p.alias} in export`}
                    className="accent-primary size-3.5"
                  />
                  <code className="text-sm font-medium">{p.alias}</code>
                </label>

                <span className="text-muted-foreground min-w-0 flex-1 truncate text-xs">
                  {CLI_NAME[p.cli]} · {p.provider}
                  {p.profile_name && p.profile_name !== p.alias
                    ? ` · profile ${p.profile_name}`
                    : ""}
                </span>

                {busy ? (
                  <StatusBadge tone="busy">testing</StatusBadge>
                ) : ready ? (
                  <StatusBadge tone="ready">ready</StatusBadge>
                ) : !installed[p.cli] ? (
                  <StatusBadge tone="waiting">
                    {CLI_NAME[p.cli]} not found
                  </StatusBadge>
                ) : (
                  <StatusBadge tone="waiting">needs setup</StatusBadge>
                )}

                <div className="flex items-center gap-0.5">
                  {/* Testing must not require installing first. Proving a
                      profile works is the reason this app exists, so it cannot
                      be gated behind writing to disk. */}
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-8"
                    title="Test this profile"
                    onClick={() => onTest(p)}
                    disabled={busy}
                  >
                    <BoltIcon className="size-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-8"
                    title="Edit"
                    onClick={() => onEdit(isEditing ? null : p.alias)}
                  >
                    <PencilIcon className="size-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="hover:text-destructive size-8"
                    title="Remove"
                    onClick={() => onRemove(p.alias)}
                  >
                    <TrashIcon className="size-4" />
                  </Button>
                </div>
              </div>

              {probe && !busy && <ProbeStrip report={probe} />}
            </Card>

            {isEditing && (
              <ProfileForm initial={p} onSave={onSave} onCancel={onCancelForm} />
            )}
          </div>
        );
      })}

      {profiles.length === 0 && !adding && (
        <div className="border-border/70 text-muted-foreground rounded-xl border border-dashed p-8 text-center text-sm">
          No profiles yet.
        </div>
      )}

      {adding && <ProfileForm onSave={onSave} onCancel={onCancelForm} />}

      {missing.length > 0 && (
        <div className="border-(--color-warn)/25 bg-(--color-warn)/6 text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-2 rounded-lg border p-3 text-xs leading-relaxed">
          <span>
            {missing.length} profile{missing.length === 1 ? " is" : "s are"} kept
            but not activated because the CLI is not on this machine. Install it,
            then
          </span>
          <Button variant="secondary" size="sm" className="h-7" onClick={onRecheck}>
            check again
          </Button>
        </div>
      )}

      {incomplete.length > 0 && (
        <div className="border-(--color-warn)/25 bg-(--color-warn)/6 text-muted-foreground rounded-lg border p-3 text-xs leading-relaxed">
          {incomplete.length} profile{incomplete.length === 1 ? " needs" : "s need"} setup
          (API key or model mapping) before it can be installed. Edit the highlighted
          profile and try again.
        </div>
      )}

      {!adding && !editing && (
        <Button variant="secondary" size="sm" onClick={onAdd}>
          <PlusIcon className="size-4" />
          Add profile
        </Button>
      )}
    </div>
  );
}

/** One probe outcome, shown under the row it belongs to — testing never costs
 *  you the list you were looking at. */
function ProbeStrip({ report }: { report: ProbeReport }) {
  const { tone, label } = probeTone(report.result);

  return (
    <div className="bg-accent/25 text-muted-foreground border-t px-3 py-2.5 text-xs leading-relaxed sm:px-4">
      <StatusBadge tone={tone} className="font-medium">
        {label}
      </StatusBadge>
      {report.result.outcome !== "ok" && (
        <span className="ml-1.5">— {report.advice}</span>
      )}
    </div>
  );
}
