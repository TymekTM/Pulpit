//! Clipboard text via the desktop's clipboard tools: `wl-paste`/
//! `wl-copy` on Wayland, `xclip` on X11. `wl-copy` and `xclip -i` fork a
//! child that keeps owning the selection after the parent exits, which
//! the runner's timeout bounds.

use crate::linux::{run_tool, TOOL_TIMEOUT};
use crate::{OsError, Result};

const NO_SESSION: &str = "clipboard: no display session";

/// Which desktop session's clipboard tools to use. Pure over the two
/// env values so the choice is unit-testable without a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionBackend {
    Wayland,
    X11,
}

/// Wayland wins when both are set (XWayland clips would not reach
/// native windows); empty values count as unset.
pub(crate) fn session_backend(
    wayland_display: Option<&str>,
    x11_display: Option<&str>,
) -> Option<SessionBackend> {
    let set = |v: Option<&str>| v.is_some_and(|s| !s.trim().is_empty());
    if set(wayland_display) {
        Some(SessionBackend::Wayland)
    } else if set(x11_display) {
        Some(SessionBackend::X11)
    } else {
        None
    }
}

fn backend() -> Option<SessionBackend> {
    session_backend(
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

pub(crate) fn get_text() -> Result<String> {
    let backend = backend().ok_or(OsError::Unsupported(NO_SESSION))?;
    let (program, args, missing): (&str, &[&str], &'static str) = match backend {
        SessionBackend::Wayland => (
            "wl-paste",
            &["--no-newline"],
            "clipboard read: wl-paste not found",
        ),
        SessionBackend::X11 => (
            "xclip",
            &["-selection", "clipboard", "-o"],
            "clipboard read: xclip not found",
        ),
    };
    match run_tool(missing, program, args, None, TOOL_TIMEOUT) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        // an empty clipboard makes both tools exit non-zero: that is
        // "no text", not an error
        Err(OsError::Failed(_)) => Ok(String::new()),
        Err(e) => Err(e),
    }
}

pub(crate) fn set_text(text: &str) -> Result<()> {
    let backend = backend().ok_or(OsError::Unsupported(NO_SESSION))?;
    let (program, args, missing): (&str, &[&str], &'static str) = match backend {
        SessionBackend::Wayland => ("wl-copy", &[], "clipboard write: wl-copy not found"),
        SessionBackend::X11 => (
            "xclip",
            &["-selection", "clipboard", "-i"],
            "clipboard write: xclip not found",
        ),
    };
    run_tool(missing, program, args, Some(text.as_bytes()), TOOL_TIMEOUT).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_display_wins_over_x11() {
        assert_eq!(
            session_backend(Some("wayland-0"), Some(":0")),
            Some(SessionBackend::Wayland)
        );
        assert_eq!(
            session_backend(Some("wayland-0"), None),
            Some(SessionBackend::Wayland)
        );
    }

    #[test]
    fn x11_used_without_wayland() {
        assert_eq!(session_backend(None, Some(":0")), Some(SessionBackend::X11));
    }

    #[test]
    fn no_or_empty_display_values_mean_no_session() {
        assert_eq!(session_backend(None, None), None);
        assert_eq!(session_backend(Some(""), Some("")), None);
        assert_eq!(session_backend(Some("   "), None), None);
    }
}
