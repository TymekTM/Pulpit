//! Linux desktop-tool plumbing. Every OS-facing feature shells out to a
//! standard desktop tool through [`run_tool`]: one place enforces the
//! `LC_ALL=C` environment (translated tool output would break parsing),
//! the timeout bound, and the missing-binary contract. Tools are always
//! spawned with argument vectors - never through a shell - because tile
//! data (device ids, file paths, app names) flows into them.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::{OsError, Result};

pub(crate) mod capture;
pub(crate) mod clipboard;
pub(crate) mod media;
pub(crate) mod play;
pub(crate) mod speaker;

/// Default bound for one tool call: everything invoked here answers in
/// tens of milliseconds; three seconds only guards a wedged daemon.
pub(crate) const TOOL_TIMEOUT: Duration = Duration::from_secs(3);

/// How often the runner polls the child while waiting for it to exit.
const WAIT_TICK: Duration = Duration::from_millis(20);

/// How long the runner waits for the pipe readers after the child
/// exits. `read_to_end` only returns when EVERY holder of the pipe
/// closes it, and tools that fork a daemon (`wl-copy`, `xclip -i`)
/// leave the write end open in the child - whatever the readers have
/// collected by the end of this bound is what the caller gets.
const DRAIN_TIMEOUT: Duration = Duration::from_millis(500);

/// Run `program args` with `LC_ALL=C`, feeding `stdin` (if any) and
/// closing it, and return the collected stdout.
///
/// - binary not on disk -> [`OsError::Unsupported`] with `missing`
///   (callers pass a static "what: tool not found" sentence)
/// - non-zero exit -> [`OsError::Failed`] with the first 200 chars of
///   stderr
/// - still running after `timeout` -> kill, reap, [`OsError::Failed`]
///
/// The pipe readers are never joined: see [`DRAIN_TIMEOUT`]. Use
/// [`run_tool_detached_output`] for tools known to fork a daemon.
pub(crate) fn run_tool(
    missing: &'static str,
    program: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    run_tool_inner(missing, program, args, stdin, timeout, false)
}

/// [`run_tool`] with stdout and stderr sent to `Stdio::null()`: nothing
/// is read, so the call returns as soon as the child exits even when
/// the tool forks a daemon that keeps the pipes open (`wl-copy`,
/// `xclip -i` clipboard writes). On a non-zero exit the error text is
/// the bare `exit status …` - there is no stderr to report.
pub(crate) fn run_tool_detached_output(
    missing: &'static str,
    program: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    run_tool_inner(missing, program, args, stdin, timeout, true)
}

fn run_tool_inner(
    missing: &'static str,
    program: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
    timeout: Duration,
    discard_output: bool,
) -> Result<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(if discard_output {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stderr(if discard_output {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => OsError::Unsupported(missing),
            _ => OsError::Failed(format!("{program}: {e}")),
        })?;

    // Take the pipe handles out of `child` so the reader threads own
    // them; `try_wait` below needs only the process handle.
    let mut stdin_pipe = child.stdin.take();

    // Drain stdout/stderr on their own threads: a tool that fills one
    // pipe while this loop ignores it must not deadlock on a full pipe.
    // Each chunk crosses the channel as soon as it is read, so a reader
    // stuck waiting for EOF (a daemon holds the write end open) has
    // still delivered everything the child wrote.
    let stdout_rx = child.stdout.take().map(|mut pipe| {
        let (tx, rx) = mpsc::channel::<Option<Vec<u8>>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(Some(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            // end of pipe: everything that will ever arrive has
            let _ = tx.send(None);
        });
        rx
    });
    let stderr_rx = child.stderr.take().map(|mut pipe| {
        let (tx, rx) = mpsc::channel::<Option<Vec<u8>>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(Some(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = tx.send(None);
        });
        rx
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
                let stdout = recv_drained(stdout_rx.as_ref());
                let stderr = recv_drained(stderr_rx.as_ref());
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
                    // kill and reap the child; the reader threads are
                    // abandoned, not joined - a daemon holding the pipes
                    // open must not wedge the caller past the kill
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(OsError::Failed(format!("{program} timed out")));
                }
                std::thread::sleep(WAIT_TICK);
            }
            Err(e) => return Err(OsError::Failed(format!("{program}: {e}"))),
        }
    }
}

/// What a reader thread delivered within [`DRAIN_TIMEOUT`]: every chunk
/// that arrived, ending early at end-of-pipe. `None` (discarded-output
/// mode) reads nothing and waits for nobody.
fn recv_drained(rx: Option<&mpsc::Receiver<Option<Vec<u8>>>>) -> Vec<u8> {
    let Some(rx) = rx else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let deadline = Instant::now() + DRAIN_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break; // the bound is up: keep what arrived so far
        }
        match rx.recv_timeout(remaining) {
            Ok(Some(chunk)) => out.extend_from_slice(&chunk),
            // end of pipe or dropped reader: nothing more is coming
            Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => break,
        }
    }
    out
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
        let err = run_tool(
            NO_SH,
            "sh",
            &["-c", "echo boom >&2; exit 3"],
            None,
            TOOL_TIMEOUT,
        )
        .unwrap_err();
        let OsError::Failed(message) = err else {
            panic!("expected Failed");
        };
        assert!(message.contains("boom"), "message: {message}");
    }

    #[test]
    fn timeout_kills_the_child_and_fails_quickly() {
        let started = Instant::now();
        let err = run_tool(
            NO_SH,
            "sh",
            &["-c", "sleep 5"],
            None,
            Duration::from_millis(100),
        )
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

    #[test]
    fn a_daemonizing_tool_does_not_block_the_runner() {
        // the background sleep inherits both pipes and keeps them open
        // after sh exits, the way wl-copy's clipboard daemon does
        let started = Instant::now();
        let out = run_tool(
            NO_SH,
            "sh",
            &["-c", "sleep 4 & echo hi"],
            None,
            TOOL_TIMEOUT,
        )
        .expect("run");
        assert_eq!(String::from_utf8_lossy(&out).trim(), "hi");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn discarded_output_mode_returns_as_soon_as_the_child_exits() {
        // same daemonizing script, but nothing is read: the runner has
        // no pipe to wait for, so the call ends with the child
        let started = Instant::now();
        run_tool_detached_output(
            NO_SH,
            "sh",
            &["-c", "sleep 4 & echo hi"],
            None,
            TOOL_TIMEOUT,
        )
        .expect("run");
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
