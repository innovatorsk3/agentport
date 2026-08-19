//! End-to-end: scan a machine, export, import onto a second machine, install.
//!
//! Exercises the same functions the UI calls, so a regression here breaks the
//! product flow rather than just a unit.

use innovport_lib::model::*;
use innovport_lib::{bundle, scan, shell, writer};
use std::fs;
use std::path::{Path, PathBuf};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("innovport_it_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Builds a machine that looks like one configured by hand.
fn seed_machine(home: &Path) {
    fs::create_dir_all(home.join(".claude/profiles")).unwrap();
    fs::write(
        home.join(".claude/profiles/htmustc.json"),
        r#"{
          "env": {
            "ANTHROPIC_BASE_URL": "https://provider.example",
            "ANTHROPIC_AUTH_TOKEN": "key-claude",
            "ANTHROPIC_DEFAULT_OPUS_MODEL": "big-model"
          },
          "permissions": { "defaultMode": "bypassPermissions" }
        }"#,
    )
    .unwrap();

    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex/ht.config.toml"),
        "model = \"gpt-5.5\"\napproval_policy = \"never\"\n\
         sandbox_mode = \"danger-full-access\"\n\n\
         [model_providers.p]\nbase_url = \"https://provider.example/v1\"\n\
         env_key = \"PROVIDER_HT_KEY\"\nwire_api = \"responses\"\n",
    )
    .unwrap();
    // Tier-1 config: must never be adopted.
    fs::write(
        home.join(".codex/config.toml"),
        "[model_providers.base]\nbase_url = \"https://base.example/v1\"\nenv_key = \"BASE\"\n",
    )
    .unwrap();
}

#[test]
fn scan_export_import_install_round_trip() {
    let machine_a = tmp("a");
    let machine_b = tmp("b");
    seed_machine(&machine_a);

    // --- machine A: scan ------------------------------------------------
    let mut found = scan::scan_claude_dir(&machine_a.join(".claude/profiles"));
    found.extend(scan::scan_codex_dir(&machine_a.join(".codex")));

    assert_eq!(found.len(), 2, "tier-1 config.toml must not be adopted");
    assert!(found.iter().all(|p| p.origin == Origin::Scanned));

    // --- export ----------------------------------------------------------
    let exported = Bundle::new(found.clone());
    let json = serde_json::to_string(&exported).unwrap();

    // A bundle must carry no absolute paths, or it cannot cross machines.
    assert!(!json.contains("/.claude/"), "bundle leaked a path");
    assert!(!json.contains("/.codex/"), "bundle leaked a path");
    assert!(!json.contains(machine_a.to_str().unwrap()));

    // --- machine B: import onto an empty machine -------------------------
    let incoming: Bundle = serde_json::from_str(&json).unwrap();
    let plans = bundle::plan_import(&incoming, &[]);
    assert!(plans
        .iter()
        .all(|p| matches!(p, bundle::ImportPlan::Add { .. })));

    let mut installed: Vec<Profile> = incoming.profiles.clone();
    for p in &mut installed {
        // Codex keys live outside the TOML, so a scan cannot recover them.
        if p.api_key.is_empty() {
            p.api_key = "key-supplied-by-user".into();
        }
    }

    // --- install ---------------------------------------------------------
    shell::check_unique_aliases(&installed).unwrap();
    for p in &installed {
        writer::write_profile(&machine_b, p).unwrap();
    }
    shell::write_script(&machine_b, &installed, shell::ShellKind::Posix).unwrap();

    let rc = machine_b.join(".zshrc");
    for i in 0..3 {
        let outcome = shell::ensure_rc_line(&rc, &machine_b, shell::ShellKind::Posix).unwrap();
        let expected = if i == 0 {
            shell::RcOutcome::Added
        } else {
            shell::RcOutcome::AlreadyPresent
        };
        assert_eq!(outcome, expected);
    }

    // --- machine B now matches machine A ---------------------------------
    let claude = fs::read_to_string(machine_b.join(".claude/profiles/htmustc.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&claude).unwrap();
    assert_eq!(v["permissions"]["defaultMode"], "bypassPermissions");
    assert!(v.get("defaultMode").is_none(), "top-level copy is inert");

    let codex = fs::read_to_string(machine_b.join(".codex/ht.config.toml")).unwrap();
    let t: toml::Value = toml::from_str(&codex).unwrap();
    assert_eq!(t["approval_policy"].as_str(), Some("never"));
    assert_eq!(t["sandbox_mode"].as_str(), Some("danger-full-access"));

    // Each profile kept its own env var — sharing one sends a key to the wrong
    // provider.
    let script = fs::read_to_string(machine_b.join(".innovport/profiles.sh")).unwrap();
    assert!(script.contains("ANTHROPIC_AUTH_TOKEN="));
    assert!(script.contains("PROVIDER_HT_KEY="));
    assert!(
        !script.contains("export "),
        "keys must not leak into the shell"
    );

    // Machine B's tier-1 config was never created by us.
    assert!(!machine_b.join(".codex/config.toml").exists());
}

/// Re-importing the same bundle must not pile up copies — the seven duplicate
/// `# Added by Antigravity` lines in a real .zshrc are what this prevents.
#[test]
fn reimporting_the_same_bundle_changes_nothing() {
    let home = tmp("reimport");
    seed_machine(&home);

    let mut existing = scan::scan_claude_dir(&home.join(".claude/profiles"));
    existing.extend(scan::scan_codex_dir(&home.join(".codex")));

    let again = Bundle::new(existing.clone());
    let plans = bundle::plan_import(&again, &existing);

    assert!(
        plans
            .iter()
            .all(|p| matches!(p, bundle::ImportPlan::Skip { .. })),
        "identical profiles must be skipped, not duplicated"
    );
}

/// Installing twice must leave the machine in the same state, not accumulate.
#[test]
fn installing_twice_is_idempotent() {
    let home = tmp("twice");
    let profiles = vec![Profile {
        alias: "cht".into(),
        profile_name: None,
        cli: CliKind::Claude,
        provider: "provider.example".into(),
        base_url: "https://provider.example".into(),
        api_key: "k".into(),
        env_var: "ANTHROPIC_AUTH_TOKEN".into(),
        danger: DangerLevel::Bypass,
        model_map: ModelMap {
            opus: Some("claude-opus-5".into()),
            ..ModelMap::default()
        },
        wire_api: None,
        origin: Origin::Manual,
    }];

    let rc = home.join(".zshrc");
    fs::write(&rc, "export PATH=/usr/bin\n").unwrap();

    for _ in 0..2 {
        for p in &profiles {
            writer::write_profile(&home, p).unwrap();
        }
        shell::write_script(&home, &profiles, shell::ShellKind::Posix).unwrap();
        shell::ensure_rc_line(&rc, &home, shell::ShellKind::Posix).unwrap();
    }

    let rc_text = fs::read_to_string(&rc).unwrap();
    let active = rc_text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter(|l| l.contains("innovport/profiles"))
        .count();
    assert_eq!(active, 1);

    let script = fs::read_to_string(home.join(".innovport/profiles.sh")).unwrap();
    assert_eq!(script.matches("cht() {").count(), 1);
}
