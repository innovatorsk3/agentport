import { Badge } from "@/components/ui/badge";
import { ImportIcon, PlusIcon, ScanIcon, Spinner } from "@/components/Icons";
import { BUNDLE_EXT, type CliKind } from "@/types";

interface Props {
  installed: Record<CliKind, boolean>;
  /** How many profiles the opening scan already found on this machine. */
  found: number;
  scanning: boolean;
  onScan: () => void;
  onImport: (f: File) => void;
  onCreate: () => void;
}

export function StartScreen({
  installed,
  found,
  scanning,
  onScan,
  onImport,
  onCreate,
}: Props) {
  // The three ways in are not equal. A machine that already has profiles wants
  // to adopt them; an empty one wants a bundle. The scan has already run by the
  // time this renders, so the app points at the right one instead of asking a
  // question it can answer itself.
  const recommend: "scan" | "import" = found > 0 ? "scan" : "import";

  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <Choice
          icon={scanning ? <Spinner className="size-4" /> : <ScanIcon />}
          title={
            scanning
              ? "Looking for existing profiles…"
              : found > 0
                ? `Adopt ${found} profile${found === 1 ? "" : "s"} already on this machine`
                : "Nothing to adopt on this machine"
          }
          sub={
            found > 0
              ? "Read from your existing Claude Code and Codex config. Nothing is written until you say so."
              : "No hand-made profiles were found here."
          }
          recommended={recommend === "scan" && !scanning}
          disabled={scanning || found === 0}
          onClick={onScan}
        />

        <Choice
          as="label"
          icon={<ImportIcon />}
          title="Import a bundle"
          sub="Carry a set of profiles over from another machine — any operating system."
          recommended={recommend === "import" && !scanning}
        >
          <input
            type="file"
            accept={`${BUNDLE_EXT},application/json`}
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) onImport(f);
              e.target.value = "";
            }}
          />
        </Choice>

        <Choice
          icon={<PlusIcon />}
          title="Create one by hand"
          sub="Start from a Claude Code or Codex preset."
          onClick={onCreate}
        />
      </div>

      <div className="flex flex-wrap gap-2">
        <CliChip name="Claude Code" found={installed.claude} />
        <CliChip name="Codex" found={installed.codex} />
      </div>
    </div>
  );
}

function Choice({
  as = "button",
  icon,
  title,
  sub,
  recommended = false,
  disabled = false,
  onClick,
  children,
}: {
  as?: "button" | "label";
  icon: React.ReactNode;
  title: string;
  sub: string;
  recommended?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children?: React.ReactNode;
}) {
  const Tag = as as React.ElementType;

  return (
    <Tag
        onClick={disabled ? undefined : onClick}
        aria-disabled={disabled || undefined}
        className={[
          "group flex w-full items-center gap-3 rounded-xl border p-4 text-left transition-all sm:gap-4 sm:p-5",
          "focus-visible:ring-ring/60 outline-none focus-visible:ring-[3px]",
          disabled
            ? "pointer-events-none opacity-45"
            : "cursor-pointer active:translate-y-px",
          recommended
            ? "border-primary/45 from-primary/10 bg-linear-to-b to-transparent hover:border-primary/70"
            : "bg-card hover:border-border hover:bg-accent/40 border-border/70",
        ].join(" ")}
      >
        <span
          className={[
            "grid size-9 shrink-0 place-items-center rounded-lg transition-colors sm:size-10",
            recommended
              ? "bg-primary/15 text-primary"
              : "bg-accent text-muted-foreground group-hover:text-foreground",
          ].join(" ")}
        >
          {icon}
        </span>

        <span className="min-w-0 flex-1">
          <span className="block text-sm font-medium">{title}</span>
          <span className="text-muted-foreground mt-0.5 block text-xs leading-relaxed">
            {sub}
          </span>
        </span>

        {recommended && (
          <Badge
            variant="secondary"
            className="bg-primary/15 text-primary hidden shrink-0 border-none text-[10px] tracking-wider uppercase sm:inline-flex"
          >
            start here
          </Badge>
        )}
      {children}
    </Tag>
  );
}

function CliChip({ name, found }: { name: string; found: boolean }) {
  return (
    <span className="border-border/70 bg-card text-muted-foreground inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs">
      <span
        className={[
          "size-1.5 rounded-full",
          found
            ? "bg-(--color-ok) ring-(--color-ok)/20 ring-3"
            : "bg-muted-foreground/60",
        ].join(" ")}
      />
      {name} {found ? "found" : "not found"}
    </span>
  );
}
