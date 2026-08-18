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
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));

    home.map(PathBuf::from)
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

/// Startup files for the host shell.
fn rc_paths() -> Result<Vec<PathBuf>, String> {
    let home = home_dir()?;
    Ok(if cfg!(windows) {
        powershell_profile_paths(&home)
    } else if std::env::var("SHELL").unwrap_or_default().contains("bash") {
        vec![home.join(".bashrc")]
    } else {
        vec![home.join(".zshrc")]
    })
}

/// Resolve every profile path that an installed PowerShell may load. Windows
/// PowerShell 5 and PowerShell 7 use different profile directories, and
/// Documents may be redirected to OneDrive or another known-folder location.
/// Asking each executable for `$PROFILE` handles both cases without guessing.
fn powershell_profile_paths(home: &Path) -> Vec<PathBuf> {
    let fallback = home
        .join("Documents")
        .join("PowerShell")
        .join("Microsoft.PowerShell_profile.ps1");
    let mut paths = Vec::new();

    for executable in ["pwsh.exe", "powershell.exe"] {
        let Ok(output) = Command::new(executable)
            .args(["-NoProfile", "-NonInteractive", "-Command", "$PROFILE"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.to_ascii_lowercase().ends_with(".ps1") {
            continue;
        }

        let path = PathBuf::from(path);
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }

    if paths.is_empty() {
        paths.push(fallback);
    }
    paths
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
fn plan_import(incoming: Bundle, existing: Vec<Profile>) -> Result<Vec<ImportPlan>, String> {
    bundle::validate_bundle(&incoming)?;
    Ok(bundle::plan_import(&incoming, &existing))
}

#[tauri::command]
fn write_bundle(path: String, bundle: Bundle) -> Result<(), String> {
    bundle::validate_bundle(&bundle)?;
    let path = PathBuf::from(path);
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("agentport"))
    {
        return Err("bundle filename must end with .agentport".into());
    }
    let text = serde_json::to_string_pretty(&bundle)
        .map_err(|e| format!("cannot serialize bundle: {e}"))?;
    fs::write(&path, format!("{text}\n"))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

// ---- install -------------------------------------------------------------

/// What an install actually did, so the UI can be specific rather than vague.
#[derive(Debug, Serialize)]
pub struct InstallReport {
    pub configs: Vec<String>,
    pub script: String,
    pub rc_files: Vec<String>,
    /// True when the rc line was added just now — the only case where the user
    /// must open a new terminal.
    pub rc_line_added: bool,
}

/// Writes every profile's config, regenerates the script, and registers one
/// startup line in each supported shell profile if it is not already there.
#[tauri::command]
fn install_profiles(profiles: Vec<Profile>) -> Result<InstallReport, String> {
    // Validate every alias before writing anything — a half-installed set is
    // worse than a refused one.
    for p in &profiles {
        shell::validate_alias(&p.alias)?;
        if let Some(name) = &p.profile_name {
            shell::validate_alias(name).map_err(|e| {
                format!("profile '{}' has an invalid CLI profile name: {e}", p.alias)
            })?;
        }
        validate_profile(p)?;
    }
    shell::check_unique_aliases(&profiles)?;

    let home = home_dir()?;
    let sh = host_shell();

    let mut configs = Vec::new();
    for p in &profiles {
        configs.push(writer::write_profile(&home, p)?.display().to_string());
    }

    let script = shell::write_script(&home, &profiles, sh)?;
    let rc_paths = rc_paths()?;
    let mut rc_line_added = false;
    for rc in &rc_paths {
        let outcome = shell::ensure_rc_line(rc, &home, sh)?;
        rc_line_added |= outcome == shell::RcOutcome::Added;
    }

    Ok(InstallReport {
        configs,
        script: script.display().to_string(),
        rc_files: rc_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        rc_line_added,
    })
}

/// Reject incomplete profiles before touching any file. An empty Codex key or
/// model otherwise produces a plausible-looking install that can never work.
fn validate_profile(profile: &Profile) -> Result<(), String> {
    if profile.provider.trim().is_empty() {
        return Err(format!("profile '{}' has no provider", profile.alias));
    }
    if profile.base_url.trim().is_empty()
        || !(profile.base_url.starts_with("http://") || profile.base_url.starts_with("https://"))
        || profile.base_url.chars().any(char::is_whitespace)
    {
        return Err(format!(
            "profile '{}' has an invalid base URL",
            profile.alias
        ));
    }
    if profile.api_key.trim().is_empty() {
        return Err(format!("profile '{}' has no API key", profile.alias));
    }
    if !valid_env_var(&profile.env_var) {
        return Err(format!(
            "profile '{}' has an invalid environment variable name",
            profile.alias
        ));
    }
    if matches!(profile.cli, CliKind::Claude) && profile.env_var != "ANTHROPIC_AUTH_TOKEN" {
        return Err(format!(
            "profile '{}' must use ANTHROPIC_AUTH_TOKEN for Claude Code",
            profile.alias
        ));
    }

    let model = match profile.cli {
        CliKind::Claude => profile.model_map.opus.as_deref(),
        CliKind::Codex => profile.model_map.default.as_deref(),
    };
    if model.is_none_or(|value| value.trim().is_empty()) {
        let role = if matches!(profile.cli, CliKind::Claude) {
            "opus"
        } else {
            "default"
        };
        return Err(format!("profile '{}' has no {role} model", profile.alias));
    }
    Ok(())
}

fn valid_env_var(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            cli_state,
            scan_machine,
            parse_model_list,
            validate_model_mapping,
            suggest_model_mapping,
            plan_import,
            write_bundle,
            install_profiles,
            validate_alias,
            probe_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
