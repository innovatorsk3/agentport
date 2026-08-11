import { useEffect, useState } from "react";
import * as api from "./api";
import { ProfileForm } from "./screens/ProfileForm";
import {
  BUNDLE_EXT,
  type Bundle,
  type CliKind,
  type InstallReport,
  type ProbeReport,
  type Profile,
} from "./types";
import "./App.css";

type Screen = "start" | "list" | "form" | "summary";

export default function App() {
  const [screen, setScreen] = useState<Screen>("start");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [editing, setEditing] = useState<Profile | undefined>();
  const [installed, setInstalled] = useState<Record<CliKind, boolean>>({
    claude: false,
    codex: false,
  });
  const [report, setReport] = useState<InstallReport | null>(null);
  const [probes, setProbes] = useState<ProbeReport[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // CLI presence is recomputed on every mount, never stored. A stored flag
  // would leave a profile greyed out forever after the CLI is installed.
  useEffect(() => {
    void refreshCliState();
  }, []);

  async function refreshCliState() {
    const [claude, codex] = await Promise.all([
      api.cliState("claude"),
      api.cliState("codex"),
    ]);
    setInstalled({
      claude: claude.state === "ready",
      codex: codex.state === "ready",
    });
  }

  async function doScan() {
    setBusy("Scanning this machine…");
    setError(null);
    try {
      const found = await api.scanMachine();
      if (found.length === 0) {
        setError("No existing profiles found. Create one instead.");
      } else {
        setProfiles(found);
        setScreen("list");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function doImport(file: File) {
    setBusy("Reading bundle…");
    setError(null);
    try {
      const bundle: Bundle = JSON.parse(await file.text());
      if (!Array.isArray(bundle.profiles)) {
        throw new Error("that file is not an agentport bundle");
      }
      // Compare identity, not name: identical entries are skipped silently so
      // re-importing the same bundle cannot pile up copies.
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
      if (skipped || renamed) {
        setError(
          `${added.length} imported` +
            (skipped ? `, ${skipped} already present` : "") +
            (renamed ? `, ${renamed} renamed to avoid a clash` : ""),
        );
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  function doExport() {
    const bundle: Bundle = { version: 1, profiles };
    const blob = new Blob([JSON.stringify(bundle, null, 2)], {
      type: "application/json",
    });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `profiles${BUNDLE_EXT}`;
    a.click();
    URL.revokeObjectURL(a.href);
  }

  async function doInstall() {
    setBusy("Writing configuration…");
    setError(null);
    try {
      // Only profiles whose CLI is present get installed; the rest stay in the
      // list, not ready, so they are not lost.
      const ready = profiles.filter((p) => installed[p.cli]);
      const rep = await api.installProfiles(ready);
      setReport(rep);
      setScreen("summary");

      setBusy("Testing each profile…");
      const results = await Promise.all(ready.map(api.probeProfile));
      setProbes(results);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  function saveProfile(p: Profile) {
    setProfiles((prev) => {
      const i = prev.findIndex((x) => x.alias === editing?.alias);
      if (i >= 0) {
        const next = [...prev];
        next[i] = p;
        return next;
      }
      return [...prev, p];
    });
    setEditing(undefined);
    setScreen("list");
  }

  // ---- screens ----------------------------------------------------------

  if (screen === "form") {
    return (
      <div className="app">
        <Header />
        <ProfileForm
          initial={editing}
          onSave={saveProfile}
          onCancel={() => {
            setEditing(undefined);
            setScreen(profiles.length ? "list" : "start");
          }}
        />
      </div>
    );
  }

  if (screen === "summary" && report) {
    return (
      <div className="app">
        <Header />
        <h2>Done</h2>

        {busy && <p className="hint">{busy}</p>}

        {probes.map((r) => (
          <ProbeRow key={r.alias} report={r} />
        ))}

        {profiles
          .filter((p) => !installed[p.cli])
          .map((p) => (
            <div className="result warn" key={p.alias}>
              <div className="head">
                <code>{p.alias}</code>
                <span className="state missing">
                  {p.cli === "claude" ? "Claude Code" : "Codex"} is not
                  installed
                </span>
              </div>
              <div className="advice">
                Kept, not activated. Install the CLI and run agentport again.
              </div>
            </div>
          ))}

        <div className="note">
          {report.rc_line_added ? (
            <>
              Added one line to <code>{report.rc_file}</code>.{" "}
              <strong>Open a new terminal</strong> — or run{" "}
              <code>source {report.rc_file}</code> in the one you have.
            </>
          ) : (
            <>
              <code>{report.rc_file}</code> already sourced agentport, so it was
              left untouched. Your aliases are live in any new terminal.
            </>
          )}
          <br />
          <br />
          Default <code>claude</code> and <code>codex</code> configuration is
          not carried by a bundle — set that up separately if you need it.
        </div>

        <div className="actions">
          <button onClick={() => setScreen("list")}>Back to profiles</button>
        </div>
      </div>
    );
  }

  if (screen === "list") {
    return (
      <div className="app">
        <Header />
        <h2>Profiles</h2>

        <div className="rows">
          {profiles.map((p) => {
            const ready = installed[p.cli];
            return (
              <div className={`row ${ready ? "" : "not-ready"}`} key={p.alias}>
                <code className="alias">{p.alias}</code>
                <span className="meta">
                  {p.cli === "claude" ? "Claude" : "Codex"} · {p.provider}
                </span>
                <span className="row-actions">
                  <span className={`state ${ready ? "ready" : "missing"}`}>
                    {ready
                      ? "ready"
                      : `${p.cli === "claude" ? "Claude Code" : "Codex"} not installed`}
                  </span>
                  <button
                    className="small"
                    onClick={() => {
                      setEditing(p);
                      setScreen("form");
                    }}
                  >
                    edit
                  </button>
                  <button
                    className="small"
                    onClick={() =>
                      setProfiles((prev) =>
                        prev.filter((x) => x.alias !== p.alias),
                      )
                    }
                  >
                    remove
                  </button>
                </span>
              </div>
            );
          })}
        </div>

        {profiles.some((p) => !installed[p.cli]) && (
          <div className="note">
            Profiles whose CLI is missing are kept but not activated — install
            the CLI, then{" "}
            <button className="small" onClick={refreshCliState}>
              check again
            </button>
            .
          </div>
        )}

        {error && <p className="error">{error}</p>}
        {busy && <p className="hint">{busy}</p>}

        <div className="actions">
          <button onClick={() => setScreen("form")}>Add profile</button>
          <button onClick={doExport} disabled={!profiles.length}>
            Export bundle
          </button>
          <div className="spacer" />
          <button
            className="primary"
            onClick={doInstall}
            disabled={!profiles.some((p) => installed[p.cli]) || !!busy}
          >
            Install and test
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <Header />
      <h2>Start</h2>
      <div className="choices">
        <button className="choice" onClick={doScan}>
          <strong>Scan this machine</strong>
          <span>
            Find profiles you already set up by hand and adopt them. Nothing is
            written until you say so.
          </span>
        </button>

        <label className="choice" style={{ display: "block" }}>
          <strong>Import a bundle</strong>
          <span>Carry a set of profiles over from another machine.</span>
          <input
            type="file"
            accept={`${BUNDLE_EXT},application/json`}
            style={{ display: "none" }}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void doImport(f);
            }}
          />
        </label>

        <button className="choice" onClick={() => setScreen("form")}>
          <strong>Create a profile</strong>
          <span>Start from a Claude Code or Codex preset.</span>
        </button>
      </div>

      {error && <p className="error">{error}</p>}
      {busy && <p className="hint">{busy}</p>}

      <div className="note">
        Detected on this machine:{" "}
        {installed.claude ? "Claude Code ✓" : "Claude Code ✗"} ·{" "}
        {installed.codex ? "Codex ✓" : "Codex ✗"}
      </div>
    </div>
  );
}

function Header() {
  return (
    <>
      <h1>agentport</h1>
      <p className="tagline">
        Carry your Claude Code and Codex CLI setup to another machine.
      </p>
    </>
  );
}

/** One probe outcome. The classification is the point: three failures look
 *  identical to a user but need completely different fixes. */
function ProbeRow({ report }: { report: ProbeReport }) {
  const r = report.result;
  const tone =
    r.outcome === "ok" ? "ok" : r.outcome === "no_credit" ? "warn" : "bad";
  const label =
    r.outcome === "ok"
      ? `replied in ${(r.millis / 1000).toFixed(1)}s`
      : r.outcome === "bad_key"
        ? "key rejected"
        : r.outcome === "no_credit"
          ? "out of credit"
          : r.outcome === "model_unavailable"
            ? "no model answered"
            : r.outcome === "unreachable"
              ? "unreachable"
              : `HTTP ${r.status}`;

  return (
    <div className={`result ${tone}`}>
      <div className="head">
        <code>{report.alias}</code>
        <span className={`state ${tone === "ok" ? "ready" : tone === "warn" ? "missing" : "bad"}`}>
          {label}
        </span>
      </div>
      <div className="advice">{report.advice}</div>
      {"detail" in r && r.detail && <div className="detail">{r.detail}</div>}
    </div>
  );
}
