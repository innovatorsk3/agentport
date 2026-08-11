import { AlertIcon, CheckIcon, Spinner } from "./Icons";
import type { ProbeResult } from "@/types";

/** Every state a profile row can be in. Kept in one place so nothing invents
 *  its own green, and so "not ready" always reads the same way. */
export type Tone = "ready" | "waiting" | "failed" | "busy" | "idle";

const TONE_CLASS: Record<Tone, string> = {
  ready: "text-(--color-ok)",
  waiting: "text-(--color-warn)",
  failed: "text-destructive",
  busy: "text-muted-foreground",
  idle: "text-muted-foreground",
};

export function StatusBadge({
  tone,
  children,
  className = "",
}: {
  tone: Tone;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 text-xs whitespace-nowrap ${TONE_CLASS[tone]} ${className}`}
    >
      {tone === "busy" ? (
        <Spinner className="size-3.5" />
      ) : tone === "ready" ? (
        <CheckIcon className="size-3.5" />
      ) : tone === "idle" ? null : (
        <AlertIcon className="size-3.5" />
      )}
      {children}
    </span>
  );
}

/** Turns a probe outcome into a tone plus a short label.
 *
 *  Three failures look identical to a user — "it does not work" — but need
 *  completely different fixes, so they never share a label. */
export function probeTone(r: ProbeResult): { tone: Tone; label: string } {
  switch (r.outcome) {
    case "ok":
      return { tone: "ready", label: `replied in ${(r.millis / 1000).toFixed(1)}s` };
    case "bad_key":
      return { tone: "failed", label: "key rejected" };
    case "no_credit":
      return { tone: "waiting", label: "out of credit" };
    case "model_unavailable":
      return { tone: "failed", label: "no model answered" };
    case "unreachable":
      return { tone: "failed", label: "unreachable" };
    default:
      return { tone: "failed", label: `HTTP ${r.status}` };
  }
}
