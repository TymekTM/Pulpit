# 031 - Run `adb` without a console window and with a timeout

> **Executor instructions.** Follow the steps in order, touch only the
> in-scope file, run each Verify. STOP conditions are binding. Do not edit
> `plans/`. **Drift check first:** confirm the excerpt below still matches.

## Status

- Priority: P2 | Effort: S | Risk: low-medium (Windows-only code path cannot be run here) | Depends on: none
- Category: correctness / UX
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

The Tablets settings page and the tablet updater shell out to `adb`. Two
problems: (1) on Windows a GUI app that spawns a console program without
`CREATE_NO_WINDOW` flashes a black console window on every call - the repo
already avoids this for shell commands (`creation_flags(0x0800_0000)` in
`crates/backend/src/lib.rs` ~:1256); (2) there is no timeout, so an `adb`
that waits forever (unauthorised device prompt, wedged daemon) blocks the
Tauri command and the UI spinner never ends.

## Current state

`apps/desktop/src-tauri/src/lib.rs` (~:1511):

```rust
fn run_adb(args: &[&str]) -> Result<(bool, String), String> {
    let out = std::process::Command::new("adb")
        .args(args)
        .output()
        .map_err(|e| format!("adb nie znaleziony w PATH: {e}"))?;
    …
```

It returns `(success, combined text)`. Callers: `adb_devices` (~:1541),
`adb_install` (~:1555), `tablet_version` (~:1589), `install_tablet_update`
(~:1964). User-facing strings in this file are currently Polish; keep new
ones Polish too (plan 033 makes them all bilingual).

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` |
| Lint | `CARGO_BUILD_JOBS=3 cargo clippy -p pulpit-desktop --all-targets --locked` |

## Scope

In scope: `apps/desktop/src-tauri/src/lib.rs` - `run_adb`, a new helper
beside it, new tests in the existing test module (~:2098).

Out of scope: the callers' logic, the frontend, new crate dependencies.

## Git workflow

One commit: `Run adb hidden and with a timeout`. No push.

## Steps

### Step 1 - Generic "run with timeout" helper

Add a private function beside `run_adb`:

```rust
/// Runs `cmd` to completion or kills it after `timeout`. `Ok(None)` means
/// it was killed. Output is read on helper threads so a child that fills
/// its pipe cannot deadlock, and so a daemon that inherited the pipes
/// (adb's own server does) cannot keep us waiting after the child exited.
fn output_with_timeout(
    mut cmd: std::process::Command,
    timeout: std::time::Duration,
) -> std::io::Result<Option<(std::process::ExitStatus, Vec<u8>, Vec<u8>)>>
```

Implementation requirements:
- `stdin(Stdio::null())`, `stdout(Stdio::piped())`, `stderr(Stdio::piped())`, `spawn()`.
- One thread per pipe doing `read_to_end`, sending the bytes through a
  `std::sync::mpsc::channel`.
- Poll `child.try_wait()` every 50 ms until it exits or the deadline passes.
  On deadline: `child.kill()`, `child.wait()`, return `Ok(None)`.
- After exit, collect each pipe with `recv_timeout(Duration::from_secs(1))`;
  on timeout use an empty buffer (do not join the reader threads).

### Step 2 - `run_adb` uses it

- Build the command in one place:
  ```rust
  let mut cmd = std::process::Command::new("adb");
  cmd.args(args);
  #[cfg(windows)]
  {
      use std::os::windows::process::CommandExt;
      cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
  }
  ```
- Timeout: 180 s when `args` contains `"install"`, otherwise 20 s (the
  first `adb devices` may start the daemon, which takes a few seconds).
- Spawn failure keeps the existing message
  (`adb nie znaleziony w PATH: {e}`).
- Timeout returns
  `Err(format!("adb nie odpowiedział w ciągu {} s", timeout.as_secs()))`.
- Everything after the process finished (how `success` and the text are
  derived) stays exactly as it is now.

### Step 3 - Tests

In the test module add two tests for `output_with_timeout`, with a small
`#[cfg]`-switched helper building the command:
- unix: `sh -c "echo hi"` and `sh -c "sleep 5"`;
- windows: `cmd /C "echo hi"` and `cmd /C "ping -n 6 127.0.0.1 >NUL"`.

Assert: the echo run returns `Some` with stdout containing `hi`; the sleeper
with a 200 ms timeout returns `None` in under 3 s.

Verify: Test command passes; Lint shows no new warning in the file.

## Done criteria

- [ ] `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` exits 0.
- [ ] `grep -n "creation_flags(0x0800_0000)" apps/desktop/src-tauri/src/lib.rs` prints at least one line inside `run_adb`.
- [ ] `grep -n '\.output()' apps/desktop/src-tauri/src/lib.rs` shows no remaining call on an `adb` command.
- [ ] Report states explicitly: "Windows branch compiled only under cfg, not run".

## STOP conditions

- Some `adb` call in this file does not go through `run_adb` (e.g. a direct
  `Command::new("adb")` elsewhere): route it through `run_adb` only if it is
  a one-line change; otherwise report it.

## Maintenance notes

The Windows branch is verified only by CI (`windows-latest`). If CI fails on
the Windows test command syntax, fix the test helper, not the timeout logic.
