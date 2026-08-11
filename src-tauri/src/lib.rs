//! agentport — carry Claude Code / Codex CLI configuration to another machine.
//!
//! See `docs/requirements.md`. Each module below maps to a section there.

pub mod bundle; // §9  export/import, identity comparison
pub mod detect; // §10 is the CLI present on this machine
pub mod model; // §7  bundle types — intent, not files
pub mod models; // §15 fetch + validate provider model ids
pub mod scan; // §14 discover profiles already on this machine

// Not built yet (§5, §7, §10):
//   writer/  translate intent into each CLI's own config schema
//   shell    generate the script + register ONE line in the shell rc
//   probe    real generation call, then classify 401 / 402 / timeout

use model::{CliKind, ModelMap, Profile, ProfileState};
use models::{ModelInfo, ModelIssue};

#[tauri::command]
fn cli_state(cli: CliKind) -> ProfileState {
    detect::state_for(cli)
}

/// Read-only sweep of this machine for profiles that already exist.
#[tauri::command]
fn scan_machine() -> Vec<Profile> {
    scan::scan_machine()
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            cli_state,
            scan_machine,
            parse_model_list,
            validate_model_mapping,
            suggest_model_mapping
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
