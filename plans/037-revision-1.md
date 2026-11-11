# 037 revision 1 - fixes required before the Linux branch can merge

> **Executor instructions.** This is a review follow-up to plan 037 (Linux
> platform support). The work under review is commit `770172e` on branch
> `t3/linux-platform-support-voicemeeter`. Your worktree starts at
> `0da95cd`; first run `git merge --ff-only 770172e`. If that does not
> fast-forward, STOP and report. Then fix the six review findings below, in
> order, one commit per finding. Touch only the files named per finding.
> Do not edit `plans/`. Do not push. STOP conditions are binding.

## Context you need

Pulpit is a Windows-first app (Tauri desktop + Rust server + Android
client). Plan 037 added Linux implementations behind
`#[cfg(target_os = "linux")]` that shell out to desktop tools through
`run_tool` in `crates/os/src/linux/mod.rs`. Windows is the platform real
users run; CI runs `cargo fmt --check` and, on Windows,
`cargo clippy --workspace --all-targets --locked -- -D warnings`. A
warning on Windows therefore fails CI.

## Commands (repo root unless stated)

| Purpose | Command |
| --- | --- |
| Rust tests | `CARGO_BUILD_JOBS=2 timeout 1500 cargo test -p pulpit-os -p pulpit-sysinfo -p pulpit-actions -p pulpit-vm --locked` |
| Clippy (Linux) | `CARGO_BUILD_JOBS=2 timeout 1500 cargo clippy -p pulpit-os -p pulpit-sysinfo -p pulpit-actions -p pulpit-vm --all-targets --locked -- -D warnings` |
| Windows compile check | `CARGO_BUILD_JOBS=2 timeout 900 cargo check -p pulpit-os --locked --target x86_64-pc-windows-msvc` - **`pulpit-os` only**. The reviewer ran exactly this command on this machine and it works (23 s). Do not add other crates: anything that depends on `pulpit-db` pulls in `ring`, whose C build needs the MSVC toolchain that is not installed here. |
| Format | `cargo fmt --check` |
| Desktop crate | `CARGO_BUILD_JOBS=2 timeout 1500 cargo test -p pulpit-desktop --locked` |
| Frontend (in `apps/desktop`) | `npm ci` once, then `npx vite build` and `npx vitest run` |

Every cargo command uses `CARGO_BUILD_JOBS=2` and `timeout`. Other builds
share this machine.

## Finding 1 - Windows build warning (fails Windows CI)

`crates/os/src/media.rs` line 8-9 now reads:

```rust
#[cfg(not(target_os = "linux"))]
use crate::OsError;
```

At `0da95cd` the gate was `#[cfg(not(windows))]`: on Windows `OsError` is
only used inside the non-Windows stub branches. The new gate makes it an
unused import on Windows (reviewer confirmed with the Windows compile
check: `warning: unused import: crate::OsError --> crates/os/src/media.rs:9:5`).

Fix: `#[cfg(not(any(windows, target_os = "linux")))]`.

Verify: the Windows compile check prints **no** `warning:` line for
`pulpit-os`. If it prints any other warning in a file plan 037 touched,
fix that too and say so. Also run the Clippy (Linux) command. The changes
in `crates/sysinfo` and `crates/vm` cannot be compile-checked for Windows
here; for those, re-read every `#[cfg(...)]` line that plan 037 or this
revision touched and confirm in your report that no item used on Windows
lost its definition or import.

Commit: `os: keep the Windows build of media.rs warning-free`.

## Finding 2 - MPRIS parser does not match real `busctl` output

`crates/os/src/linux/media.rs`. The `GET_ALL` test fixture was hand-written
and its shape is wrong, so the parser built against it drops all track
metadata from real players (no title, artist, duration, cover, track id).

How `busctl --json=short` really renders values: **only the top-level reply
wraps its arguments in an array. Inside a variant, `data` is the value
itself.** Real capture from this machine (a `GetAll` whose values are
variants holding string arrays):

```json
{"type":"a{sv}","data":[{"Features":{"type":"as","data":[]},"Interfaces":{"type":"as","data":["org.freedesktop.DBus.Monitoring"]}}]}
```

So a real MPRIS `GetAll` looks like this (note `Metadata.data` is an
**object**, and `xesam:artist.data` is a **flat** array):

```json
{"type":"a{sv}","data":[{
  "PlaybackStatus":{"type":"s","data":"Playing"},
  "LoopStatus":{"type":"s","data":"Track"},
  "Shuffle":{"type":"b","data":true},
  "Metadata":{"type":"a{sv}","data":{
    "mpris:trackid":{"type":"o","data":"/org/mpris/MediaPlayer2/track/1"},
    "mpris:length":{"type":"x","data":354000000},
    "mpris:artUrl":{"type":"s","data":"file:///home/u/Music/cov%20er.jpg"},
    "xesam:title":{"type":"s","data":"Bohemian Rhapsody"},
    "xesam:artist":{"type":"as","data":["Queen"]},
    "xesam:album":{"type":"s","data":"A Night at the Opera"},
    "xesam:albumArtist":{"type":"as","data":["Queen"]},
    "xesam:trackNumber":{"type":"i","data":11}
  }},
  "Position":{"type":"x","data":60000000},
  "CanGoNext":{"type":"b","data":true},
  "CanGoPrevious":{"type":"b","data":true},
  "CanPlay":{"type":"b","data":true},
  "CanPause":{"type":"b","data":true},
  "CanSeek":{"type":"b","data":true}
}]}
```

Fix in `parse_player`:
- `Metadata`: accept the object directly; also accept a one-element array
  holding the object (the shape the old fixture assumed) so the parser is
  tolerant either way.
- `xesam:artist` / `xesam:albumArtist`: take the first string of a flat
  array; also accept the old nested `[["…"]]` shape.
- `mpris:length` may be signed (`x`) or unsigned (`t`); keep reading it as
  an integer and keep the clamp to `>= 0`.
- Replace the `GET_ALL` fixture with the real shape above and update its
  comment (shape verified against live `busctl` output for variants;
  values hand-written). Keep one extra small test that the old wrapped
  shape still parses.
- Keep every existing assertion's expected values (title, artist, 354000 ms,
  position 60000 ms, track id, art URL).

Optional, only if it works without installing anything: if
`dbus-run-session` and a Python D-Bus binding (`python3 -c "import dbus"`
or `gi`) are present, start a fake MPRIS player on a **private** bus
(`dbus-run-session -- …`), run the same `busctl --user --json=short call …
GetAll s org.mpris.MediaPlayer2.Player`, and paste the captured reply in
your report. Never register anything on the user's real session bus.

Verify: Rust tests command. Commit: `os: parse MPRIS metadata in busctl's real variant shape`.

## Finding 3 - `run_tool` can hang forever on tools that fork a daemon

`crates/os/src/linux/mod.rs`. After the child exits, `run_tool` does
`stdout_thread.join()` / `stderr_thread.join()`. `read_to_end` only
returns when **every** holder of the pipe closes it. `wl-copy` and
`xclip -i` fork a background process that keeps serving the clipboard and
inherits those pipes, so the join can block until something else replaces
the clipboard - possibly forever. The timeout does not cover this (the
comment in `linux/clipboard.rs` that says it does is wrong). `paste_text`
calls `set_text` on the action thread, so every later tile press would
queue behind the hang.

Fix:
- Reader threads send their buffer over a channel; after the child exits,
  wait for each with a short bound (`recv_timeout`, 500 ms is enough) and
  use whatever arrived (empty on timeout). Never `join()` the readers.
  The same applies on the timeout/kill path (it already returns without
  joining - keep it that way).
- Add a way to run a tool with stdout and stderr sent to `Stdio::null()`
  (for example a `run_tool_detached_output` wrapper or a parameter; keep
  the existing call sites readable) and use it for clipboard **writes**
  (`wl-copy`, `xclip -i`), so a paste does not pay the 500 ms wait twice.
  On non-zero exit in this mode the error text is `exit status …`.
- Fix the wrong sentence in the `linux/clipboard.rs` module doc.

New tests in `linux/mod.rs` (they run real `sh`, no display needed):
- `a_daemonizing_tool_does_not_block_the_runner`: `sh -c "sleep 4 & echo hi"`
  (the background `sleep` inherits both pipes) returns `hi` (trimmed) in
  under 2 s.
- `discarded_output_mode_returns_as_soon_as_the_child_exits`: the same
  script through the null-output mode returns `Ok` in under 1 s.
- The five existing tests keep passing unchanged.

Verify: Rust tests command. Commit: `os: bound pipe reads so daemonizing tools cannot wedge the runner`.

## Finding 4 - the startup support probe launches Voicemeeter on Windows

`apps/desktop/src/api.js` now calls `invoke("vm_status")` at module load.
On Windows `vm_status` is **not** side-effect free: see the doc comment
above `async fn vm_status` in `apps/desktop/src-tauri/src/lib.rs` ("The
first query opens and logs into the remote DLL (its login starts
Voicemeeter when the app is closed)") and `VoicemeeterState::status` →
`with_remote` → `ensure_connected` → `login()` in `crates/vm/src/lib.rs`.
Before plan 037 this only ran when the user opened the Voicemeeter
settings page. The probe would start Voicemeeter on every Pulpit launch.
(This was the plan's own instruction and the plan was wrong; the fix is
below.)

Fix - a side-effect-free flag:
- `crates/vm/src/lib.rs`: add
  `/// Whether this platform can run the Voicemeeter integration at all.`
  `pub const SUPPORTED: bool = cfg!(windows);` and use `SUPPORTED` for the
  three `supported:` fields in `status()` (the struct field stays).
- `apps/desktop/src-tauri/src/lib.rs` - **only** these two additions, no
  other line in this file may change:
  1. a new command placed directly **above** the doc comment of `vm_status`:
     ```rust
     /// Whether Voicemeeter can exist on this platform. Unlike
     /// [`vm_status`] this touches nothing (no DLL, no login), so the UI
     /// may ask at startup.
     #[tauri::command]
     fn vm_supported() -> bool {
         pulpit_vm::SUPPORTED
     }
     ```
  2. `vm_supported,` added to the `tauri::generate_handler![…]` list
     immediately before `vm_status,`.
- `apps/desktop/src/api.js`: `probeVmSupport` calls `invoke("vm_supported")`
  and sets `vmSupported.value = supported !== false`; update the comment.
  Add `vmSupported: () => invoke("vm_supported")` to `api` only if the
  existing `tests/api.test.js` table pattern needs it - otherwise leave
  `api` alone.
- Tests in `apps/desktop/tests`: update anything that relied on the
  startup `vm_status` call; add one assertion that loading the app's API
  module never invokes `vm_status` by itself (follow the existing mock
  pattern - `cmds("vm_status")` in `SettingsOverlay.test.js`). Do not
  weaken or delete existing tests.

Verify: Rust tests command, Desktop crate command, `grep -n '"vm_status"' apps/desktop/src/api.js`
shows only the `vmStatus:` entry of `api`, then both Frontend commands.
Commit: `vm: ask platform support without logging into Voicemeeter`.

## Finding 5 - only ship cover bytes that are images

`crates/os/src/linux/media.rs`, `load_art` / `art_ext`: any `file://` path
a player on the session bus names is read and its bytes are pushed to
tablets as cover art; `art_ext` falls back to `"jpg"` for unknown content.
Make unknown content `None`: `art_ext` returns `Option<&'static str>`
(`jpg`, `png`, `webp` by magic bytes), `load_art` returns `None` when it is
`None`. Add a test: a file with text content yields no art; a file starting
with the PNG magic yields `("png")`. Use `tempfile` only if it is already a
dev-dependency of `pulpit-os`; otherwise test `art_ext` alone on byte
slices.

Verify: Rust tests command. Commit: `os: ship MPRIS cover bytes only when they are an image`.

## Finding 6 - formatting (fails the CI fmt job)

`cargo fmt --check` fails in `crates/os/src/linux/{mod,media,play,speaker}.rs`.
After findings 1-5 run `cargo fmt -p pulpit-os -p pulpit-sysinfo -p pulpit-vm -p pulpit-actions -p pulpit-desktop`
and confirm with `git status --short` that it touched **only** files plan
037 or this revision already changed. If it reformats any other file,
revert that file (`git checkout -- <file>`) and report it.

Verify: `cargo fmt --check` exits 0 (if it fails only because of files you
are not allowed to touch, report which). Commit: `Format the Linux platform modules`.

## Done criteria

- [ ] Rust tests, Clippy (Linux) and Desktop crate commands exit 0.
- [ ] Windows compile check prints no `warning:` lines.
- [ ] `cargo fmt --check` exits 0.
- [ ] `npx vite build` and `npx vitest run` exit 0 with at least 508 tests.
- [ ] `git diff --stat 770172e..HEAD` lists only: `crates/os/src/media.rs`,
      files under `crates/os/src/linux/`, `crates/vm/src/lib.rs`,
      `apps/desktop/src-tauri/src/lib.rs` (two additions only),
      `apps/desktop/src/api.js`, files under `apps/desktop/tests/`, and
      files reformatted by finding 6 that plan 037 already touched.
- [ ] `git diff 770172e..HEAD -- apps/desktop/src-tauri/src/lib.rs` shows only added lines (no `-` lines).

## STOP conditions

- The fast-forward fails.
- Fixing a finding needs a change in a file not listed for it.
- The `pulpit-os`-only Windows compile check fails to run: report the
  exact error and **continue with the remaining findings** (do not install
  toolchains, do not stop for this).

## Report

STATUS; one line per finding (DONE + commit, or why not); each done
criterion with the exact command and observed result; `git diff --stat
770172e..HEAD`; for finding 2, whether you captured a real MPRIS reply and
its text if so.
