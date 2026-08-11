//! agentport — carry Claude Code / Codex CLI configuration to another machine.
//!
//! See `docs/requirements.md`. Each module below maps to a section there.

pub mod bundle; // §9  export/import, identity comparison
pub mod detect; // §10 is the CLI present on this machine
pub mod model; // §7  bundle types — intent, not files
pub mod models; // §15 fetch + validate provider model ids
pub mod probe; // §10 real generation call, classify the failure
pub mod scan; // §14 discover profiles already on this machine
pub mod shell; // §5,§6 generate the alias script + ONE rc line
pub mod writer; // §7,§8 intent -> each CLI's config schema

use bundle::ImportPlan;
use model::{Bundle, CliKind, ModelMap, Profile, ProfileState};
use models::{ModelInfo, ModelIssue};
use probe::ProbeResult;
use serde::Serialize;
use std::path::PathBuf;

fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "cannot determine the home directory".to_string())
}

/// The shell family this machine uses.
fn host_shell() -> shell::ShellKind {
    if cfg!(windows) {
        shell::ShellKind::PowerShell
    } else {
        shell::ShellKind::Posix
    }
}

/// Startup file for the host shell.
fn rc_path() -> Result<PathBuf, String> {
    let home = home_dir()?;
    Ok(if cfg!(windows) {
        home.join("Documents")
            .join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1")
    } else if std::env::var("SHELL").unwrap_or_default().contains("bash") {
        home.join(".bashrc")
    } else {
        home.join(".zshrc")
    })
}

// ---- detection & scanning ------------------------------------------------

#[tauri::command]
fn cli_state(cli: CliKind) -> ProfileState {
    detect::state_for(cli)
}

/// Read-only sweep of this machine for profiles that already exist.
#[tauri::command]
fn scan_machine() -> Vec<Profile> {
    scan::scan_machine()
}

// ---- models --------------------------------------------------------------

/// Parses an OpenAI-compatible model listing fetched by the frontend.
#[tauri::command]
fn parse_model_list(body: String) -> Result<Vec<ModelInfo>, String> {
    models::parse_model_list(&body)
}

/// Checks a model mapping against what the provider actually serves.
#[tauri::command]
fn validate_model_mapping(
    cli: CliKind,
    map: ModelMap,
    available: Vec<ModelInfo>,
) -> Vec<ModelIssue> {
    models::validate_mapping(cli, &map, &available)
}

/// Suggests model ids per role. Returning empty is a valid answer.
#[tauri::command]
fn suggest_model_mapping(cli: CliKind, available: Vec<ModelInfo>) -> ModelMap {
    models::suggest_mapping(cli, &available)
}

// ---- bundles -------------------------------------------------------------

#[tauri::command]
fn plan_import(incoming: Bundle, existing: Vec<Profile>) -> Vec<ImportPlan> {
    bundle::plan_import(&incoming, &existing)
}

// ---- install -------------------------------------------------------------

/// What an install actually did, so the UI can be specific rather than vague.
#[derive(Debug, Serialize)]
pub struct InstallReport {
    pub configs: Vec<String>,
    pub script: String,
    pub rc_file: String,
    /// True when the rc line was added just now — the only case where the user
    /// must open a new terminal.
    pub rc_line_added: bool,
}

/// Writes every profile's config, regenerates the script, and registers the
/// single rc line if it is not already there.
#[tauri::command]
fn install_profiles(profiles: Vec<Profile>) -> Result<InstallReport, String> {
    // Validate every alias before writing anything — a half-installed set is
    // worse than a refused one.
    for p in &profiles {
        shell::validate_alias(&p.alias)?;
    }

    let home = home_dir()?;
    let sh = host_shell();

    let mut configs = Vec::new();
    for p in &profiles {
        configs.push(writer::write_profile(&home, p)?.display().to_string());
    }

    let script = shell::write_script(&home, &profiles, sh)?;
    let rc = rc_path()?;
    let outcome = shell::ensure_rc_line(&rc, &home, sh)?;

    Ok(InstallReport {
        configs,
        script: script.display().to_string(),
        rc_file: rc.display().to_string(),
        rc_line_added: outcome == shell::RcOutcome::Added,
    })
}

/// Returns true when the alias shadows a common command; errors when it cannot
/// be typed at a shell prompt at all.
#[tauri::command]
fn validate_alias(alias: String) -> Result<bool, String> {
    shell::validate_alias(&alias)?;
    Ok(shell::shadows_common_command(&alias))
}

// ---- probing -------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProbeReport {
    pub alias: String,
    pub endpoint: String,
    pub result: ProbeResult,
    pub advice: String,
}

#[tauri::command]
async fn probe_profile(profile: Profile) -> ProbeReport {
    let result = probe::probe(&profile).await;
    ProbeReport {
        alias: profile.alias.clone(),
        endpoint: probe::generation_endpoint(&profile),
        advice: probe::advice(&result).to_string(),
        result,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            cli_state,
            scan_machine,
            parse_model_list,
            validate_model_mapping,
            suggest_model_mapping,
            plan_import,
            install_profiles,
            validate_alias,
            probe_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
