import { useEffect, useState } from "react";
import * as api from "@/api";
import { Button } from "@/components/ui/button";
import { AlertIcon, CheckIcon, ExportIcon, Spinner } from "@/components/Icons";
import { Wordmark } from "@/components/Wordmark";
import { ProfileList } from "@/screens/ProfileList";
import { StartScreen } from "@/screens/StartScreen";
import { Summary } from "@/screens/Summary";
import {
  type Bundle,
  type CliKind,
  type InstallReport,
  type ProbeReport,
  type Profile,
  profileConfigurationIssue,
} from "@/types";
import "@/index.css";

type Screen = "start" | "list" | "summary";

/** Three tones, three meanings. A successful import printed in the error colour
 *  teaches people to distrust red, so success never borrows it. */
type Notice = { tone: "good" | "warn" | "bad"; text: string } | null;

const TITLES: Record<Screen, { title: string; sub?: string }> = {
  start: { title: "Start" },
  list: { title: "Profiles" },
  summary: { title: "Installed" },
};

export default function App() {
  const [screen, setScreen] = useState<Screen>("start");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [exportSelection, setExportSelection] = useState<Set<string>>(new Set());
  const [installed, setInstalled] = useState<Record<CliKind, boolean>>({
    claude: false,
    codex: false,
  });
  const [scanned, setScanned] = useState<Profile[]>([]);
  const [scanning, setScanning] = useState(true);
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [probes, setProbes] = useState<Record<string, ProbeReport>>({});
  const [testing, setTesting] = useState<Set<string>>(new Set());
  const [report, setReport] = useState<InstallReport | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice>(null);

  // Detect and scan on launch. The app has the answer before the first screen
  // renders, so it can point at the right choice instead of asking a question it
  // could answer itself.
  useEffect(() => {
    void (async () => {
      try {
        await refreshCliState();
      } catch {
        // A detection failure must not prevent scanning or leave the start
        // screen spinning forever.
        setInstalled({ claude: false, codex: false });
      }
      try {
        setScanned(await api.scanMachine());
      } catch {
        setScanned([]);
      } finally {
        setScanning(false);
      }
    })();
  }, []);

  async function refreshCliState() {
    // Recomputed every time, never stored — a stored flag would leave a profile
    // greyed out forever after the CLI is installed.
    const [claude, codex] = await Promise.all([
      api.cliState("claude"),
      api.cliState("codex"),
    ]);
    setInstalled({
      claude: claude.state === "ready",
      codex: codex.state === "ready",
    });
  }

  function adoptScanned() {
    setProfiles(scanned);
    // Export is opt-in: scanning a machine must not silently select every key.
    setExportSelection(new Set());
    setScreen("list");
    setNotice({
      tone: "good",
      text: `Adopted ${scanned.length} profile${scanned.length === 1 ? "" : "s"} from this machine. Nothing has been written yet.`,
    });
  }

  async function doImport(file: File) {
    setBusy("Reading bundle…");
    setNotice(null);
    try {
      const parsed: unknown = JSON.parse(await file.text());
      if (!isBundle(parsed)) {
        throw new Error("that file is not an agentport bundle");
      }
      const bundle: Bundle = {
        ...parsed,
        profiles: parsed.profiles.map((p) => ({
          ...p,
          model_map: p.model_map ?? {},
          origin: p.origin ?? "imported",
        })),
      };
      // Compares identity, not name: identical entries are skipped so importing
      // the same bundle twice cannot pile up copies.
      const plans = await api.planImport(bundle, profiles);
      const added: Profile[] = [];
      bundle.profiles.forEach((p, i) => {
        const plan = plans[i];
        if (plan.kind === "skip") return;
        added.push({
          ...p,
          alias: plan.kind === "rename" ? plan.to : plan.alias,
          origin: "imported",
        });
      });

      setProfiles((prev) => [...prev, ...added]);
      setScreen("list");

      const skipped = plans.filter((p) => p.kind === "skip").length;
      const renamed = plans.filter((p) => p.kind === "rename").length;
      const parts = [`${added.length} imported`];
      if (skipped) parts.push(`${skipped} already present`);
      if (renamed) parts.push(`${renamed} renamed to avoid a clash`);
      setNotice({ tone: "good", text: parts.join(" · ") });
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setBusy(null);
    }
  }

  async function doExport() {
    const selected = profiles.filter((p) => exportSelection.has(p.alias));
    if (!selected.length) {
      setNotice({ tone: "bad", text: "Select at least one profile to export." });
      return;
    }
    const incomplete = selected.find((p) => profileConfigurationIssue(p));
    if (incomplete) {
      setNotice({
        tone: "bad",
        text: `Cannot export ${incomplete.alias} yet: ${profileConfigurationIssue(incomplete)}.`,
      });
      return;
    }
    const bundle: Bundle = { version: 1, profiles: selected };
    setBusy("Choose where to save the bundle…");
    try {
      if (await api.saveBundle(bundle)) {
        setNotice({
          tone: "warn",
          text: "Bundle saved. It carries your API keys in plaintext — do not commit it to a repository.",
        });
      }
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setBusy(null);
    }
  }

  /** Probing one profile. Deliberately available before installing anything. */
  async function testOne(p: Profile) {
    setTesting((s) => new Set(s).add(p.alias));
    try {
      const r = await api.probeProfile(p);
      setProbes((prev) => ({ ...prev, [p.alias]: r }));
    } finally {
      setTesting((s) => {
        const n = new Set(s);
        n.delete(p.alias);
        return n;
      });
    }
  }

  async function doInstall() {
    setBusy("Writing configuration…");
    setNotice(null);
    try {
      // Only profiles whose CLI is present get installed; the rest stay in the
      // list, not ready, so they are never lost.
      const ready = profiles.filter(
        (p) => installed[p.cli] && !profileConfigurationIssue(p),
      );
      const rep = await api.installProfiles(ready);
      setReport(rep);
      setScreen("summary");

      setBusy("Testing each profile…");
      const results = await Promise.all(ready.map(api.probeProfile));
      setProbes((prev) => ({
        ...prev,
        ...Object.fromEntries(results.map((r) => [r.alias, r])),
      }));
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setBusy(null);
    }
  }

  function saveProfile(p: Profile) {
    setProfiles((prev) => {
      const i = prev.findIndex((x) => x.alias === editing);
      if (i >= 0) {
        const next = [...prev];
        next[i] = p;
        return next;
      }
      return [...prev, p];
    });
    setExportSelection((prev) => {
      const next = new Set(prev);
      if (editing && editing !== p.alias && next.delete(editing)) {
        // Preserve an explicit selection when the user renames a profile.
        next.add(p.alias);
      }
      return next;
    });
    setEditing(null);
    setAdding(false);
    if (screen === "start") setScreen("list");
  }

  const installable = profiles.filter(
    (p) => installed[p.cli] && !profileConfigurationIssue(p),
  ).length;
  const exportable = profiles.filter((p) => exportSelection.has(p.alias)).length;
  const showList = screen === "list" || (screen === "start" && adding);

  return (
    <div className="flex min-h-full flex-col">
      {/* Sticky so the identity stays put however far the list grows. */}
      <header className="bg-background/85 supports-backdrop-filter:bg-background/65 sticky top-0 z-20 border-b backdrop-blur-md">
        <div className="mx-auto flex w-full max-w-3xl items-center gap-4 px-4 py-3 sm:px-6">
          <Wordmark compact />
          <div className="flex-1" />
          {screen !== "start" && (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground"
              onClick={() => {
                setScreen("start");
                setAdding(false);
                setEditing(null);
              }}
            >
              Start over
            </Button>
          )}
        </div>
      </header>

      <main className="mx-auto w-full max-w-3xl flex-1 px-4 py-6 sm:px-6 sm:py-8">
        <div className="mb-5 flex items-baseline gap-3">
          <h1 className="text-muted-foreground text-xs font-semibold tracking-[0.1em] uppercase">
            {TITLES[screen].title}
          </h1>
          <span className="text-muted-foreground/70 ml-auto text-xs">
            {screen === "list" &&
              `${profiles.length} total${
                profiles.length - installable > 0
                  ? ` · ${profiles.length - installable} not ready`
                  : ""
              }`}
            {screen === "summary" && report && `${report.configs.length} written`}
          </span>
        </div>

        {screen === "start" && !adding && (
          <StartScreen
            installed={installed}
            found={scanned.length}
            scanning={scanning}
            onScan={adoptScanned}
            onImport={doImport}
            onCreate={() => setAdding(true)}
          />
        )}

        {showList && (
          <ProfileList
            profiles={profiles}
            installed={installed}
            probes={probes}
            testing={testing}
            editing={editing}
            adding={adding}
            onEdit={(a) => {
              setEditing(a);
              setAdding(false);
            }}
            onAdd={() => {
              setAdding(true);
              setEditing(null);
            }}
            onCancelForm={() => {
              setAdding(false);
              setEditing(null);
            }}
            onSave={saveProfile}
            onRemove={(alias) => {
              setProfiles((prev) => prev.filter((x) => x.alias !== alias));
              setExportSelection((prev) => {
                const next = new Set(prev);
                next.delete(alias);
                return next;
              });
            }}
            selectedForExport={exportSelection}
            onToggleExport={(alias) =>
              setExportSelection((prev) => {
                const next = new Set(prev);
                if (next.has(alias)) next.delete(alias);
                else next.add(alias);
                return next;
              })
            }
            onTest={testOne}
            onRecheck={refreshCliState}
          />
        )}

        {screen === "summary" && report && (
          <Summary
            report={report}
            profiles={profiles}
            installed={installed}
            probes={probes}
            busy={busy}
            onBack={() => setScreen("list")}
          />
        )}

        {notice && <NoticeBar notice={notice} onClose={() => setNotice(null)} />}

        {busy && screen !== "summary" && (
          <div className="text-muted-foreground mt-4 flex items-center gap-2 text-xs">
            <Spinner className="size-3.5" />
            {busy}
          </div>
        )}
      </main>

      {/* A sticky action bar keeps the primary action reachable no matter how
          long the list gets or how short the window is. */}
      {screen === "list" && !adding && !editing && (
        <footer className="bg-background/85 supports-backdrop-filter:bg-background/65 sticky bottom-0 border-t backdrop-blur-md">
          <div className="mx-auto flex w-full max-w-3xl flex-wrap items-center gap-2 px-4 py-3 sm:px-6">
            <Button
              variant="secondary"
              size="sm"
              onClick={doExport}
              disabled={!exportable || !!busy}
            >
              <ExportIcon className="size-4" />
              Export {exportable} profile{exportable === 1 ? "" : "s"}
            </Button>
            <div className="flex-1" />
            <Button onClick={doInstall} disabled={!installable || !!busy}>
              {busy && <Spinner className="size-4" />}
              {installable
                ? `Install ${installable} profile${installable === 1 ? "" : "s"}`
                : "Nothing to install"}
            </Button>
          </div>
        </footer>
      )}
    </div>
  );
}

function NoticeBar({
  notice,
  onClose,
}: {
  notice: NonNullable<Notice>;
  onClose: () => void;
}) {
  const style =
    notice.tone === "good"
      ? "border-(--color-ok)/25 bg-(--color-ok)/6"
      : notice.tone === "warn"
        ? "border-(--color-warn)/25 bg-(--color-warn)/6"
        : "border-destructive/30 bg-destructive/6";

  return (
    <div
      className={`text-muted-foreground animate-in fade-in mt-5 flex items-start gap-2.5 rounded-lg border p-3 text-xs leading-relaxed ${style}`}
    >
      <span className="mt-px shrink-0">
        {notice.tone === "good" ? (
          <CheckIcon className="size-3.5" />
        ) : (
          <AlertIcon className="size-3.5" />
        )}
      </span>
      <span className="flex-1">{notice.text}</span>
      <button
        onClick={onClose}
        className="hover:text-foreground shrink-0 leading-none"
        aria-label="Dismiss"
      >
        ×
      </button>
    </div>
  );
}

function isBundle(value: unknown): value is Bundle {
  if (!value || typeof value !== "object") return false;
  const candidate = value as { version?: unknown; profiles?: unknown };
  if (candidate.version !== 1 || !Array.isArray(candidate.profiles) || !candidate.profiles.length) {
    return false;
  }
  return candidate.profiles.every((profile) => {
    if (!profile || typeof profile !== "object") return false;
    const p = profile as Record<string, unknown>;
    return (
      typeof p.alias === "string" &&
      (p.cli === "claude" || p.cli === "codex") &&
      typeof p.provider === "string" &&
      typeof p.base_url === "string" &&
      typeof p.api_key === "string" &&
      typeof p.env_var === "string" &&
      typeof p.danger === "string" &&
      (p.model_map === undefined ||
        (p.model_map !== null && typeof p.model_map === "object"))
    );
  });
}
