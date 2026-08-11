import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { StatusBadge, probeTone } from "@/components/StatusBadge";
import { CheckIcon, InfoIcon, Spinner } from "@/components/Icons";
import type { CliKind, InstallReport, ProbeReport, Profile } from "@/types";

/** What the install actually did, plus a real probe per profile.
 *
 *  Importing without proving it works only moves uncertainty from one machine
 *  to the next, so this is where the proof lands. */
export function Summary({
  report,
  profiles,
  installed,
  probes,
  busy,
  onBack,
}: {
  report: InstallReport;
  profiles: Profile[];
  installed: Record<CliKind, boolean>;
  probes: Record<string, ProbeReport>;
  busy: string | null;
  onBack: () => void;
}) {
  const done = profiles.filter((p) => installed[p.cli]);
  const held = profiles.filter((p) => !installed[p.cli]);

  return (
    <div className="space-y-3">
      {done.map((p) => {
        const r = probes[p.alias];

        if (!r) {
          return (
            <Card key={p.alias} className="gap-0 p-3 sm:px-4">
              <div className="flex flex-wrap items-center gap-3">
                <code className="text-sm font-medium">{p.alias}</code>
                <StatusBadge tone="busy">testing</StatusBadge>
              </div>
            </Card>
          );
        }

        const { tone, label } = probeTone(r.result);
        const edge =
          tone === "ready"
            ? "border-l-(--color-ok)"
            : tone === "waiting"
              ? "border-l-(--color-warn)"
              : "border-l-destructive";

        return (
          <Card key={p.alias} className={`gap-2 border-l-2 p-3 sm:px-4 ${edge}`}>
            <div className="flex flex-wrap items-center gap-3">
              <code className="text-sm font-medium">{p.alias}</code>
              <StatusBadge tone={tone}>{label}</StatusBadge>
            </div>
            <p className="text-muted-foreground text-xs leading-relaxed">
              {r.advice}
            </p>
            {"detail" in r.result && r.result.detail && (
              <pre className="bg-background text-muted-foreground/80 max-h-32 overflow-auto rounded-md p-2.5 font-mono text-[11px] leading-relaxed whitespace-pre-wrap">
                {r.result.detail}
              </pre>
            )}
          </Card>
        );
      })}

      {held.map((p) => (
        <Card
          key={p.alias}
          className="gap-2 border-l-2 border-l-(--color-warn) p-3 sm:px-4"
        >
          <div className="flex flex-wrap items-center gap-3">
            <code className="text-sm font-medium">{p.alias}</code>
            <StatusBadge tone="waiting">not activated</StatusBadge>
          </div>
          <p className="text-muted-foreground text-xs leading-relaxed">
            Kept, but the CLI is not on this machine. Install it and run agentport
            again.
          </p>
        </Card>
      ))}

      {busy && (
        <div className="text-muted-foreground flex items-center gap-2 text-xs">
          <Spinner className="size-3.5" />
          {busy}
        </div>
      )}

      <div
        className={`flex items-start gap-2.5 rounded-lg border p-3 text-xs leading-relaxed ${
          report.rc_line_added
            ? "border-(--color-ok)/25 bg-(--color-ok)/6"
            : "border-border/70 bg-card"
        }`}
      >
        <span className="text-muted-foreground mt-px shrink-0">
          {report.rc_line_added ? (
            <CheckIcon className="size-3.5" />
          ) : (
            <InfoIcon className="size-3.5" />
          )}
        </span>
        <span className="text-muted-foreground">
          {report.rc_line_added ? (
            <>
              Added one line to <code className="text-foreground">{report.rc_file}</code>.{" "}
              <strong className="text-foreground font-medium">
                Open a new terminal
              </strong>
              , or run{" "}
              <code className="text-foreground">source {report.rc_file}</code> in
              the one you have.
            </>
          ) : (
            <>
              <code className="text-foreground">{report.rc_file}</code> already
              sourced agentport, so it was left untouched. Your aliases are live in
              any new terminal.
            </>
          )}
        </span>
      </div>

      <div className="border-border/70 bg-card text-muted-foreground flex items-start gap-2.5 rounded-lg border p-3 text-xs leading-relaxed">
        <span className="mt-px shrink-0">
          <InfoIcon className="size-3.5" />
        </span>
        <span>
          Default <code className="text-foreground">claude</code> and{" "}
          <code className="text-foreground">codex</code> configuration does not
          travel in a bundle — set that up separately if you need it.
        </span>
      </div>

      <Button variant="secondary" size="sm" onClick={onBack}>
        Back to profiles
      </Button>
    </div>
  );
}
