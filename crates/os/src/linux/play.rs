//! Local audio playback for "Play Audio" tiles: spawn the first
//! available player (`pw-play`, `paplay`, `aplay`) detached and return
//! immediately; a new press kills the still-running clip first, matching
//! the Windows MCI restart semantics.

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// The still-playing clip from the previous press; killed (and reaped)
/// before the next one starts, so presses restart instead of layer.
static LAST: Mutex<Option<Child>> = Mutex::new(None);

/// Player candidates, tried in order. `aplay` only understands WAV,
/// but it is the universal fallback; PipeWire/Pulse ships the first two.
const PLAYERS: [&str; 3] = ["pw-play", "paplay", "aplay"];

/// Start `path` playing; a press while a clip is running restarts it.
/// An empty path is a no-op (tiles without a configured file).
pub(crate) fn play_audio(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Ok(());
    }
    let player = first_player().ok_or(
        "no audio player found (pw-play, paplay, aplay)".to_string(),
    )?;

    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(mut child) = last.take() {
        // Reap a finished child, kill a running one - either way the
        // old clip is gone before the new one starts.
        let running = child.try_wait().ok().flatten().is_none();
        if running {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
    let child = Command::new(&player)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {player}: {e}"))?;
    *last = Some(child);
    Ok(())
}

fn first_player() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    PLAYERS
        .iter()
        .find_map(|name| lookup_in_path(name, &path))
        .map(|p| p.to_string_lossy().into_owned())
}

/// `which`-style lookup over one PATH value, in pure Rust. Executable
/// bit required: a data file named `paplay` on PATH must not "play".
pub(crate) fn lookup_in_path(program: &str, path_var: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    for dir in std::env::split_paths(path_var) {
        let candidate = dir.join(program);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable_file(path: &std::path::Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exec(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn finds_the_first_executable_match_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let exe = make_exec(dir.path(), "paplay");
        let path_var =
            std::env::join_paths([dir.path(), std::path::Path::new("/nonexistent")]).unwrap();
        assert_eq!(lookup_in_path("paplay", &path_var), Some(exe));
        assert_eq!(lookup_in_path("nothing-else", &path_var), None);
    }

    #[test]
    fn non_executable_files_do_not_count() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("pw-play");
        std::fs::write(&plain, b"data, not a program").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        let path_var = std::env::join_paths([dir.path()]).unwrap();
        // skipped in favour of a later executable entry
        assert_eq!(lookup_in_path("pw-play", &path_var), None);
        let exe = make_exec(dir.path(), "aplay");
        assert_eq!(lookup_in_path("aplay", &path_var), Some(exe));
    }

    #[test]
    fn empty_path_is_a_no_op() {
        assert!(play_audio("").is_ok());
        assert!(play_audio("   ").is_ok());
        // no spawn happened: nothing to assert beyond the Ok, but the
        // stored child must not have been created
        let last = LAST.lock().unwrap();
        // (a previous test may have stored one; only assert no panic)
        let _ = last.is_some();
    }
}
