//! Detects whether a CLI is present on this machine (requirements §10).
//!
//! The result is COMPUTED on every call — never cached, never persisted.
//!
//! # Why this is not simply `which`
//!
//! A GUI app launched from Finder or the Start menu does NOT inherit the shell's
//! `PATH`. It gets a minimal system one. Both CLIs on the developer's own
//! machine live in directories that only `.zshrc` adds:
//!
//! ```text
//! shell PATH:  /Users/mac/.local/bin/claude
//!              /Users/mac/.nvm/versions/node/v22.22.0/bin/codex
//! GUI PATH:    not found · not found
//! ```
//!
//! So the app reported "Claude Code not installed" for a CLI the user was
//! actively running, and — far worse — `install_profiles` skips any profile
//! whose CLI looks missing, so the install silently wrote nothing while
//! reporting success. Another silent failure of exactly the shape this project
//! exists to eliminate.
//!
//! The fix asks the user's **login shell** for its `PATH` once, then resolves
//! every CLI against it, with a directory sweep as a backstop for when no login
//! shell answers (Windows, a locked-down environment, a broken rc file).

use crate::model::{CliKind, ProfileState};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The command name to look for.
fn command_name(cli: CliKind) -> &'static str {
    match cli {
        CliKind::Claude => "claude",
        CliKind::Codex => "codex",
    }
}

/// Asks the user's login shell for the `PATH` it would give an interactive
/// session — the one that actually has nvm, homebrew and `~/.local/bin` on it.
///
/// `-l` reads the login files and `-i` the interactive ones, because different
/// setups export `PATH` from different files. Resolved once per process: it
/// costs about a second, and `PATH` does not change under a running app.
fn login_shell_path() -> Option<&'static str> {
    static CACHE: OnceLock<Option<String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            if cfg!(windows) {
                return None; // PowerShell inherits the machine PATH already
            }
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            let out = Command::new(shell)
                .args(["-lic", "printf %s \"$PATH\""])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(path)
            }
        })
        .as_deref()
}

/// Directories these CLIs are commonly installed into.
///
/// A backstop, not the primary mechanism: it is a fixed list and will go stale,
/// whereas the login shell always knows the truth. It only runs when no shell
/// answered.
fn fallback_dirs(home: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            home.join("AppData/Local/Programs"),
            home.join("AppData/Roaming/npm"),
            home.join(".local/bin"),
        ]
    } else {
        let mut dirs = vec![
            home.join(".local/bin"),
            home.join("bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ];
        // nvm keeps one bin directory per installed node version.
        if let Ok(versions) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            dirs.extend(versions.flatten().map(|e| e.path().join("bin")));
        }
        dirs
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// True when `dir` holds a runnable file called `name`.
fn executable_in(dir: &Path, name: &str) -> bool {
    if cfg!(windows) {
        // Windows resolves a bare name through PATHEXT.
        ["exe", "cmd", "bat", "ps1"]
            .iter()
            .any(|ext| dir.join(format!("{name}.{ext}")).is_file())
            || dir.join(name).is_file()
    } else {
        let p = dir.join(name);
        p.is_file() && is_executable(&p)
    }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_p: &Path) -> bool {
    true
}

/// Resolves a CLI to its full path, or `None` when it is genuinely absent.
pub fn resolve(cli: CliKind) -> Option<PathBuf> {
    let name = command_name(cli);

    // 1. The login shell's PATH — the only source that knows about nvm, a
    //    custom prefix, or anything else the user's rc file sets up.
    if let Some(path) = login_shell_path() {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path.split(sep).filter(|d| !d.is_empty()) {
            let dir = Path::new(dir);
            if executable_in(dir, name) {
                return Some(dir.join(name));
            }
        }
    }

    // 2. This process's own PATH. Correct when launched from a terminal, and
    //    the whole story on Windows.
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if executable_in(&dir, name) {
                return Some(dir.join(name));
            }
        }
    }

    // 3. Known install locations, for when no shell answered.
    let home = home()?;
    fallback_dirs(&home)
        .into_iter()
        .find(|d| executable_in(d, name))
        .map(|d| d.join(name))
}

pub fn is_installed(cli: CliKind) -> bool {
    resolve(cli).is_some()
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
    use std::fs;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("agentport_detect_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A file has to be runnable, not merely present — a stray `claude.md` in a
    /// bin directory must not count as an installed CLI.
    #[cfg(unix)]
    #[test]
    fn a_non_executable_file_does_not_count() {
        let d = tmpdir("nonexec");
        fs::write(d.join("claude"), "just text").unwrap();
        assert!(!executable_in(&d, "claude"));

        make_executable(&d.join("claude"));
        assert!(executable_in(&d, "claude"));
    }

    #[test]
    fn missing_directory_is_not_a_panic() {
        let missing = std::env::temp_dir().join("agentport_no_such_dir_xyz");
        assert!(!executable_in(&missing, "claude"));
    }

    /// The nvm layout that hid `codex` from the GUI build: one bin directory
    /// per node version, none of them on the system PATH.
    #[test]
    fn fallback_covers_per_version_nvm_directories() {
        let home = tmpdir("nvm");
        let bin = home.join(".nvm/versions/node/v22.22.0/bin");
        fs::create_dir_all(&bin).unwrap();

        let dirs = fallback_dirs(&home);
        assert!(dirs.contains(&bin), "nvm version bin must be searched");
    }

    /// `~/.local/bin` is where the developer's `claude` actually lives, and it
    /// is only on PATH because `.zshrc` puts it there.
    #[test]
    fn fallback_covers_local_bin() {
        let home = tmpdir("localbin");
        assert!(fallback_dirs(&home).contains(&home.join(".local/bin")));
    }

    /// Resolution must never depend on this process's PATH alone. Stripping it
    /// entirely still finds a CLI sitting in a known location — the exact
    /// situation of an app launched from Finder.
    #[cfg(unix)]
    #[test]
    fn resolves_without_relying_on_the_process_path() {
        let home = tmpdir("nopath");
        let bin = home.join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        make_executable(&bin.join("claude"));

        let found = fallback_dirs(&home)
            .into_iter()
            .find(|d| executable_in(d, "claude"));

        assert_eq!(found, Some(bin));
    }

    /// Whatever the mechanism, the answer must match reality on this machine.
    /// This test runs from a terminal, where `which` is authoritative.
    #[test]
    fn agrees_with_which_when_run_from_a_shell() {
        for cli in [CliKind::Claude, CliKind::Codex] {
            let via_which = Command::new("which")
                .arg(command_name(cli))
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if via_which {
                assert!(
                    is_installed(cli),
                    "{} is on PATH but detect says it is missing",
                    command_name(cli)
                );
            }
        }
    }
}
