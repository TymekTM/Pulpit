//! Linux desktop-tool plumbing. Every OS-facing feature shells out to a
//! standard desktop tool through [`run_tool`]: one place enforces the
//! `LC_ALL=C` environment (translated tool output would break parsing),
//! the timeout bound, and the missing-binary contract. Tools are always
//! spawned with argument vectors - never through a shell - because tile
//! data (device ids, file paths, app names) flows into them.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{OsError, Result};

pub(crate) mod speaker;
pub(crate) mod clipboard;
pub(crate) mod capture;



/// Default bound for one tool call: everything invoked here answers in
/// tens of milliseconds; three seconds only guards a wedged daemon.
pub(crate) const TOOL_TIMEOUT: Duration = Duration::from_secs(3);

/// How often the runner polls the child while waiting for it to exit.
const WAIT_TICK: Duration = Duration::from_millis(20);

/// Run `program args` with `LC_ALL=C`, feeding `stdin` (if any) and
/// closing it, and return the collected stdout.
///
/// - binary not on disk -> [`OsError::Unsupported`] with `missing`
///   (callers pass a static "what: tool not found" sentence)
/// - non-zero exit -> [`OsError::Failed`] with the first 200 chars of
///   stderr
/// - still running after `timeout` -> kill, reap, [`OsError::Failed`]
pub(crate) fn run_tool(
    missing: &'static str,
    program: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => OsError::Unsupported(missing),
            _ => OsError::Failed(format!("{program}: {e}")),
        })?;

    // Take the pipe handles out of `child` so the reader threads own
    // them; `try_wait` below needs only the process handle.
    let mut stdin_pipe = child.stdin.take();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    // Drain stdout/stderr on their own threads: a tool that fills one
    // pipe while this loop ignores it must not deadlock on a full pipe.
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    // stdin payloads are tiny (clipboard text); write, then close so
    // tools waiting for EOF (`cat`, `xclip -i`) see the end.
    if let Some(mut pipe) = stdin_pipe.take() {
        let _ = pipe.write_all(stdin.unwrap_or_default());
        drop(pipe);
    }

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_thread.join().unwrap_or_default();
                let stderr = stderr_thread.join().unwrap_or_default();
                if status.success() {
                    return Ok(stdout);
                }
                let mut message = String::from_utf8_lossy(&stderr).trim().to_string();
                if message.is_empty() {
                    message = format!("exit status {status}");
                }
                // 200 *characters*, never slicing inside a UTF-8 sequence
                message = message.chars().take(200).collect();
                return Err(OsError::Failed(format!("{program}: {message}")));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait(); // reap the killed child
                    return Err(OsError::Failed(format!("{program} timed out")));
                }
                std::thread::sleep(WAIT_TICK);
            }
            Err(e) => return Err(OsError::Failed(format!("{program}: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_SH: &str = "test: sh not found";

    #[test]
    fn captures_stdout() {
        let out = run_tool(NO_SH, "sh", &["-c", "printf hi"], None, TOOL_TIMEOUT).expect("run");
        assert_eq!(out, b"hi");
    }

    #[test]
    fn missing_binary_reports_unsupported() {
        let err = run_tool(
            "test: pulpit-missing-tool not found",
            "pulpit-missing-tool-037",
            &[],
            None,
            TOOL_TIMEOUT,
        )
        .unwrap_err();
        assert!(
            matches!(err, OsError::Unsupported(m) if m == "test: pulpit-missing-tool not found"),
            "wrong error: {err:?}"
        );
    }

    #[test]
    fn nonzero_exit_reports_trimmed_stderr() {
        let err = run_tool(NO_SH, "sh", &["-c", "echo boom >&2; exit 3"], None, TOOL_TIMEOUT)
            .unwrap_err();
        let OsError::Failed(message) = err else {
            panic!("expected Failed");
        };
        assert!(message.contains("boom"), "message: {message}");
    }

    #[test]
    fn timeout_kills_the_child_and_fails_quickly() {
        let started = Instant::now();
        let err = run_tool(NO_SH, "sh", &["-c", "sleep 5"], None, Duration::from_millis(100))
            .unwrap_err();
        assert!(err.to_string().contains("timed out"), "error: {err}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn stdin_is_written_and_closed() {
        let out = run_tool(
            NO_SH,
            "sh",
            &["-c", "cat"],
            Some(b"pulpit \xe5\x9b\x9b".as_slice()),
            TOOL_TIMEOUT,
        )
        .expect("run");
        assert_eq!(out, "pulpit 四".as_bytes());
    }
}
