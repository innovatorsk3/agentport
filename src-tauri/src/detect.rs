//! Detects whether a CLI is present on this machine (requirements §10).
//!
//! The result is COMPUTED on every call — never cached, never persisted.

use crate::model::{CliKind, ProfileState};
use std::process::Command;

/// The command name to look for on PATH.
fn command_name(cli: CliKind) -> &'static str {
    match cli {
        CliKind::Claude => "claude",
        CliKind::Codex => "codex",
    }
}

/// `which` on unix, `where` on Windows.
fn lookup_tool() -> &'static str {
    if cfg!(windows) {
        "where"
    } else {
        "which"
    }
}

pub fn is_installed(cli: CliKind) -> bool {
    Command::new(lookup_tool())
        .arg(command_name(cli))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn state_for(cli: CliKind) -> ProfileState {
    if is_installed(cli) {
        ProfileState::Ready
    } else {
        ProfileState::CliMissing { cli }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tool_matches_platform() {
        let t = lookup_tool();
        if cfg!(windows) {
            assert_eq!(t, "where");
        } else {
            assert_eq!(t, "which");
        }
    }

    /// A command that certainly does not exist must report false, not panic.
    #[test]
    fn missing_command_is_not_installed() {
        let found = Command::new(lookup_tool())
            .arg("agentport_definitely_not_a_real_command_xyz")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(!found);
    }
}
