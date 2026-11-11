# 037 - Linux platform support for the server (Voicemeeter greyed out)

> **Executor instructions.** This plan is a sequence of independent
> **phases**. Do them in order; **commit after every phase** so a later
> phase that has to stop does not lose earlier work. Touch only in-scope
> files. Run each Verify. STOP conditions are binding - a STOP in one phase
> means "skip that phase, write down why, go on to the next" unless it says
> otherwise. Do not edit `plans/`.
> **Drift check first:** open the files named in "Current state" and confirm
> the described stubs are still there.

## Status

- Priority: P1 (owner request) | Effort: L | Risk: medium | Depends on: none (see "Coordination")
- Category: feature / portability
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

Pulpit's server (the thing tablets connect to; headless binary in
`apps/server`, also embedded in the desktop app) builds on Linux today, but
almost every OS-facing feature is a stub that answers "unsupported": volume,
now-playing media, system-info tiles, clipboard, screenshot, sound playback.
The owner wants the server to work on Linux "with most of its features".
Voicemeeter (a Windows-only audio mixer) is postponed, but the desktop app
must show it **greyed out** on Linux instead of offering tiles that cannot
work.

The code was written for this: OS access hides behind small seams, and the
crate docs say "a Linux port is an implementation swap, not a rewrite".

## Design decisions (made by the planner - do not redesign)

1. **Shell out to standard desktop tools** rather than binding native
   libraries, for this first port. Each tool call goes through one small
   runner (below). A missing tool yields `OsError::Unsupported("<what>: <tool> not found")`,
   never a panic. Rationale: no new heavy dependencies, every parser is a
   pure function with unit tests, and a later native rewrite stays behind
   the same seams.
2. **Always run tools with `LC_ALL=C`** - `pactl` and friends translate
   their output (the owner's system is Polish), which would break parsing.
3. Tools per feature: audio `pactl` (works on PulseAudio and
   PipeWire-pulse); media `busctl --user` (ships with systemd) talking MPRIS
   over D-Bus; clipboard `wl-copy`/`wl-paste` when `WAYLAND_DISPLAY` is set,
   else `xclip -selection clipboard`; screenshot `grim -` on Wayland, else
   `import -window root png:-` (ImageMagick); sound playback first available
   of `pw-play`, `paplay`, `aplay`.
4. Gate Linux code with `#[cfg(target_os = "linux")]`; keep the existing
   stubs for every other non-Windows OS (`#[cfg(not(any(windows, target_os = "linux")))]`).
   **Windows code paths must not change at all.**

This machine has `pactl`, `busctl`, `wl-copy`, `wl-paste`, `grim`, `import`,
`pw-play`, `paplay`, `aplay`, a PipeWire session and a D-Bus user bus
(`/run/user/1000/bus`), but your shell may have no `WAYLAND_DISPLAY` /
`DISPLAY`. Unit tests must therefore test **parsers and decision logic on
captured text**, never require a live session. Live smoke checks are extra
and reported separately.

## Coordination with concurrent work

Other agents are editing these areas right now - do not touch them:
- `crates/backend/src/lib.rs` (`with_input`) and, in
  `crates/actions/src/lib.rs`, the `EnigoInput` struct, `EnigoInput::new`
  and the `key_*`/`text`/`mouse_*`/`media` methods.
- `apps/desktop/src-tauri/src/lib.rs` (no change is needed there).
- `crates/v2`, `crates/legacy`, `crates/db`, `crates/i18n`, `crates/obs`,
  `crates/spotify`, `apps/desktop/package.json`, `package-lock.json`,
  `apps/desktop/src/i18n.js`, `apps/mobile`.

Known baseline on Linux at this commit: three tests fail or hang for
environment reasons and are fixed elsewhere. Use this filter whenever you
run tests of `pulpit-backend`, `pulpit-db` or `pulpit-v2`:
`-- --skip http_step_inside_a_multiaction_runs_the_native_chain --skip open_read_write_still_errors_on_an_unwritable_path --skip hub_teardown_ends_the_session_and_releases_held_keys`

## Current state (all verified at `0da95cd`)

- `crates/os/src/lib.rs`: `OsError { Unsupported(&'static str), Failed(String) }`,
  `AudioDevice { id, name, is_default }`, `pub trait Speaker` with
  `volume`, `muted`, `set_volume`, `devices`, `active_device`,
  `set_active_device` (and a default `status`). `platform_speaker()` returns
  `win::WinSpeaker::new()` on Windows and `unsupported::StubSpeaker`
  otherwise. `crates/os/src/unsupported.rs` is the stub.
  Note `Unsupported` carries a `&'static str`: use static messages per tool
  (e.g. `"speaker: pactl not found"`), or `Failed(String)` for dynamic text.
- `crates/os/src/media.rs`: platform-neutral types and builders
  (`MediaProperties`, `PlaybackStatus`, `PlaybackInfo`, `TimelineProperties`,
  `build_snapshot`, `build_snapshot_keyed`, `attach_art_data`, `PushGate`),
  and three functions whose non-Windows branch is a stub (~:467-505):
  `control(target_app, action, slider_value) -> Result<bool>`,
  `list_sessions() -> Result<Vec<String>>`,
  `spawn_push(consumers) -> UnboundedReceiver<Value>` (returns a dead
  channel off Windows). The Windows implementation to mirror is
  `crates/os/src/media/win_media.rs` (628 lines): read `control` (~:327),
  `list_sessions` (~:394) and `spawn_push` (~:413) to learn the action
  names, the snapshot shape, the polling cadence and how `consumers()` gates
  work when nobody is listening.
- `crates/os/src/clipboard.rs`: off Windows `sequence_number()` is `0`,
  `get_text()` returns `Ok(String::new())`, `set_text` returns
  `Err(Unsupported("clipboard write"))`.
- `crates/os/src/capture.rs`: `screenshot_to_dir(dir)` is cross-platform and
  already resolves the Pictures folder on Linux; only
  `grab_screen_rgba() -> Result<image::RgbaImage>` (~:160) is a stub. The
  `image` crate is already a dependency with the `png` feature.
- `crates/os/src/play.rs`: `play_audio(path: &str) -> Result<(), String>`;
  off Windows it returns `Err("audio playback is Windows-only")`. Windows
  semantics (~:27): starts playing and returns; a press while a clip is
  running restarts it; an empty path is a no-op.
- `crates/sysinfo/src/lib.rs`: off Windows `cpu_times()` (~:145) returns
  `(0, 0, 0)`, `mem_info()` (~:193) `(0, 0)`, `cpu_brand()` (~:258) `"CPU"`.
  Contract (see `cpu_load_pct` ~:200): `cpu_times` returns
  `(idle, kernel, user)` where **`kernel` includes idle time** (the Windows
  convention): busy = `user + kernel - idle`, total = `user + kernel`.
  `mem_info` returns `(total_bytes, available_bytes)`.
- `crates/actions/src/lib.rs`: the `Input` trait has default
  `paste_text` (falls back to `self.text`) and `screenshot` (returns
  `Unsupported`); `impl Input for EnigoInput` overrides both only under
  `#[cfg(windows)]` (~:707, ~:732). The Windows `paste_text` saves the
  clipboard, sets the text, taps Ctrl+V via `self.key_tap(...)`, then
  restores the previous text if `clipboard::sequence_number()` is unchanged.
- `crates/vm/src/lib.rs`: Voicemeeter; `pub struct VoicemeeterStatus`
  (~:97: `installed, dll_path, logged_in, vm_type, version, strip_count,
  bus_count, dll_override`), built in `status()` (~:521). Off Windows the
  DLL loader is stubbed, so the integration is simply never "installed".
- Desktop UI (`apps/desktop/src`):
  - `catalog.js`: Voicemeeter actions are the entries after
    `{ header: "Voicemeeter" }` (~:260-378), all with `value` starting
    `vm-`. Entries may carry `unavailable: "<tooltip>"` (see the
    `slobs-*`/`twitch-*` entries ~:405-428).
  - `components/ActionPicker.vue`: entries with a truthy `unavailable` are
    listed greyed (`.item.unavailable`, `aria-disabled`, lock icon,
    tooltip = the `unavailable` text), skipped by keyboard navigation, and
    not pickable (`pick` returns early).
  - `components/SettingsOverlay.vue` (~:34-42): `PAGES` list; the
    Voicemeeter page has `id: "voicemeeter"`.
  - `api.js` (~:76): `vmStatus: () => invoke("vm_status")`, which returns
    the `VoicemeeterStatus` JSON.
  - i18n: English sentences are the keys; Polish translations live in JSON
    files next to `i18n.js` (`pl.json`, `pl-chrome.json`, `pl-modals.json`).
    Find the right file by grepping for a neighbouring string.
- `apps/server/src/main.rs`: the headless server. Takes the DB path as an
  argument (~:51); env overrides `PULPIT_PORT`, `PULPIT_DEVICES`,
  `PULPIT_ASSETS`, `PULPIT_EXT_DIR`, `PULPIT_NO_DISCOVERY=1`. It already
  handles SIGTERM on unix.

## Commands

| Purpose | Command (repo root unless noted) |
| --- | --- |
| Test one crate | `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p <crate> --locked` |
| Lint one crate | `CARGO_BUILD_JOBS=3 cargo clippy -p <crate> --all-targets --locked` |
| Build the server | `CARGO_BUILD_JOBS=3 cargo build -p pulpit-server --locked` (check the real package name in `apps/server/Cargo.toml`) |
| Frontend (in `apps/desktop`) | `npm ci`, then `npx vite build` and `npx vitest run` |

Crates: `pulpit-os`, `pulpit-sysinfo`, `pulpit-actions`, `pulpit-vm`.
Always keep `CARGO_BUILD_JOBS=3` (other builds run on this machine). If you
add a dependency, `--locked` will fail once: run the build once without
`--locked`, then commit the `Cargo.lock` change.

## Scope

In scope:
- `crates/os/src/**` (new `linux/` modules; the `#[cfg]` dispatch points in
  `lib.rs`, `media.rs`, `clipboard.rs`, `capture.rs`, `play.rs`),
  `crates/os/Cargo.toml`
- `crates/sysinfo/src/lib.rs`
- `crates/actions/src/lib.rs` - **only** new `#[cfg(target_os = "linux")]`
  overrides of `paste_text` and `screenshot` inside `impl Input for EnigoInput`
- `crates/vm/src/lib.rs` (one new field)
- `apps/desktop/src/api.js`, `apps/desktop/src/components/ActionPicker.vue`,
  `apps/desktop/src/components/SettingsOverlay.vue`,
  `apps/desktop/src/components/settings/VoicemeeterPage.vue`, one Polish
  translation JSON, and vitest files for those components
- `README.md` - one short "Linux" subsection (Phase 8)
- `Cargo.lock` if a dependency is added

Out of scope: everything listed under "Coordination"; `crates/discord`
(Phase 9 is investigate-only); the Windows updater; packaging (.deb, AUR,
systemd unit files); keyboard synthesis on native Wayland (see Phase 9).

## Git workflow

Branch: stay on the branch of your checkout. One commit per phase, message
style of this repo, e.g. `sysinfo: real CPU and memory readings on Linux`.
No push.

## Phases

### Phase 0 - Tool runner (`crates/os/src/linux/mod.rs`)

Create `crates/os/src/linux/` (declared from `lib.rs` as
`#[cfg(target_os = "linux")] pub(crate) mod linux;`) with:

```rust
/// Runs a desktop tool with `LC_ALL=C` and a timeout. `Unsupported` when
/// the binary is missing, `Failed` on a non-zero exit or timeout.
pub(crate) fn run_tool(
    missing: &'static str,          // e.g. "speaker: pactl not found"
    program: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
    timeout: std::time::Duration,
) -> crate::Result<Vec<u8>>   // stdout
```

Requirements: `io::ErrorKind::NotFound` on spawn → `OsError::Unsupported(missing)`;
write `stdin` (if any) then close it; read stdout/stderr on threads; poll
`try_wait` every 20 ms; kill on timeout → `Failed("<program> timed out")`;
non-zero exit → `Failed` with the first 200 chars of stderr. Default timeout
used by callers: 3 s.

Tests: `run_tool` with `sh -c "printf hi"` returns `b"hi"`; a missing binary
name returns `Unsupported`; `sh -c "sleep 5"` with a 100 ms timeout returns
`Failed` quickly; `sh -c "cat"` with stdin echoes it.

Verify: `cargo test -p pulpit-os` passes. Commit.

### Phase 1 - System info (`crates/sysinfo/src/lib.rs`)

Split the `#[cfg(not(windows))]` stubs into `target_os = "linux"`
implementations plus the old stub for other systems. Write each as a pure
parser over the file's text plus a thin reader:

- `/proc/stat` first line `cpu  user nice system idle iowait irq softirq steal …`
  → `idle = idle + iowait`, `user = user + nice`,
  `kernel = system + irq + softirq + steal + idle` (kernel **includes**
  idle - see the contract above).
- `/proc/meminfo`: `MemTotal:` and `MemAvailable:` in kB → bytes.
- `/proc/cpuinfo`: first `model name\t: …` value, trimmed; fall back to
  `"CPU"`.

Tests (plain text fixtures in the test module): the three parsers; and
`cpu_load_pct` over two parsed `/proc/stat` samples gives the expected
percentage (e.g. 25% busy).

Verify: `cargo test -p pulpit-sysinfo` passes. Commit.

### Phase 2 - Speaker via `pactl` (`crates/os/src/linux/speaker.rs`)

`LinuxSpeaker` implementing `Speaker`; `platform_speaker()` returns it on
Linux. Because `platform_speaker()` returns `impl Speaker`, use three
`#[cfg]` return branches (windows / linux / other).

| Method | Command | Parse |
| --- | --- | --- |
| `volume` | `pactl get-sink-volume @DEFAULT_SINK@` | first `NN%` in the output |
| `muted` | `pactl get-sink-mute @DEFAULT_SINK@` | `Mute: yes` / `Mute: no` |
| `set_volume` | `pactl set-sink-volume @DEFAULT_SINK@ NN%` | clamp to 0..=100, round |
| `active_device` | `pactl get-default-sink` | trimmed line |
| `devices` | `pactl -f json list sinks` | JSON array; `id` = `name`, `name` = `description` (fallback `name`), `is_default` = equals default sink |
| `set_active_device` | `pactl set-default-sink <id>` | - |

Real output captured on this machine (use as test fixtures):

```
Volume: front-left: 26214 /  40% / -23.88 dB,   front-right: 26214 /  40% / -23.88 dB
        balance 0.00
```
```
Mute: no
```
```
[{"index":58,"state":"SUSPENDED","name":"alsa_output.pci-0000_00_1f.3.analog-stereo","description":"Built-in Audio Analog Stereo","driver":"PipeWire","mute":false,"volume":{"front-left":{"value":26214,"value_percent":"40%","db":"-23.88 dB"}}}]
```

Tests: the parsers above, including volume above 100% (`153%` → report
`100.0`? No - report the real number clamped to 100) and a malformed string
→ `Failed`.

Also check how mute is *toggled* on Windows (search callers of `Speaker` in
`crates/backend` - read only): if it goes through a media key, nothing more
is needed; if a trait method is missing for Linux, report it, do not change
the trait.

Verify: `cargo test -p pulpit-os` passes; live smoke (report the output):
a tiny `#[ignore]`d test or `cargo run --example` is not required - instead
run `LC_ALL=C pactl get-sink-volume @DEFAULT_SINK@` by hand and confirm your
parser handles exactly that text. Commit.

### Phase 3 - Clipboard (`crates/os/src/linux/clipboard.rs`)

- `get_text`: Wayland → `wl-paste --no-newline` (an empty clipboard makes it
  exit non-zero: map that to `Ok(String::new())`); X11 → `xclip -selection clipboard -o`.
- `set_text`: Wayland → `wl-copy` with the text on stdin; X11 →
  `xclip -selection clipboard -i` with stdin. These tools fork to keep
  owning the selection: do not wait on inherited pipes after the parent
  exits (the runner's timeout handles it; if `wl-copy` makes `run_tool`
  wait, spawn it with stdout/stderr set to null).
- Neither `WAYLAND_DISPLAY` nor `DISPLAY` set → `Unsupported("clipboard: no display session")`.
- `sequence_number()` stays `0` on Linux (no cheap equivalent).

Put the "which backend" choice in a pure function taking the two env values
(`Option<&str>`) and test it.

Verify + commit.

### Phase 4 - Screenshot (`crates/os/src/capture.rs` + `linux/capture.rs`)

`grab_screen_rgba` on Linux: Wayland → `grim -` (PNG on stdout); else X11 →
`import -window root png:-`; decode with
`image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.to_rgba8()`.
Timeout 10 s. No session → `Unsupported("screen capture: no display session")`.

Test: decoding a tiny PNG generated in the test with the `image` crate
through the same decode helper; backend choice via the pure function from
Phase 3 (share it).

Verify + commit.

### Phase 5 - `paste_text` and `screenshot` actions on Linux (`crates/actions/src/lib.rs`)

Add `#[cfg(target_os = "linux")]` overrides inside `impl Input for EnigoInput`:

- `screenshot`: identical body to the Windows one (`pulpit_os::capture::screenshot_to_dir`).
- `paste_text`: same flow as Windows, but (a) call **trait methods only**
  (`self.key_tap(...)`, `self.sleep(...)`) - never `self.enigo` directly,
  another change is restructuring that field; (b) if
  `pulpit_os::clipboard::set_text` fails (no clipboard tool), fall back to
  `self.text(text)`; (c) restore the previous clipboard text only if
  `get_text()` still equals the pasted text (there is no sequence number).

Do not touch anything else in this file; do not reformat it.

Verify: `cargo test -p pulpit-actions` passes. Commit.

### Phase 6 - Sound playback (`crates/os/src/play.rs` + `linux/play.rs`)

`play_audio(path)` on Linux: empty path → `Ok(())`; pick the first existing
of `pw-play`, `paplay`, `aplay` (probe with `which`-style lookup over `PATH`
in pure Rust); spawn it detached (stdio null) and return immediately. Keep
the "a press while a clip is running restarts it" semantics: remember the
last `Child` in a `static Mutex<Option<Child>>`, `kill()` + `wait()` it
before spawning the next. No player → `Err("no audio player found (pw-play, paplay, aplay)")`.
Reap finished children (call `try_wait` on the stored child before
replacing it).

Test: the PATH lookup helper with a temp dir containing a fake executable.

Verify + commit.

### Phase 7 - Now-playing media via MPRIS (`crates/os/src/linux/media.rs`)

Read `win_media.rs` first; mirror its behaviour, not its API calls.

- Players: `busctl --user --json=short call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ListNames`
  → names starting with `org.mpris.MediaPlayer2.`. Real output shape:
  `{"type":"as","data":[["org.freedesktop.DBus",":1.0",…]]}`.
- State: `busctl --user --json=short call <name> /org/mpris/MediaPlayer2 org.freedesktop.DBus.Properties GetAll s org.mpris.MediaPlayer2.Player`
  → a JSON dict with `PlaybackStatus` (`Playing`/`Paused`/`Stopped`),
  `Metadata` (`xesam:title`, `xesam:artist` (array), `xesam:album`,
  `mpris:length` (µs), `mpris:artUrl`), `Position` (µs), `Volume`,
  `CanGoNext` … Each value is wrapped as `{"type":"…","data":…}` - write the
  unwrapping helper once. Capture one real sample if a player is running
  (`playerctl` is not installed; start nothing yourself) - otherwise write
  the fixture by hand from the D-Bus MPRIS specification and say so in the
  report.
- Friendly app name: the `Identity` property of `org.mpris.MediaPlayer2`
  (fallback: the bus-name suffix).
- `list_sessions`: friendly names.
- `control`: map the same action names the Windows `control` accepts to
  MPRIS methods (`PlayPause`, `Play`, `Pause`, `Stop`, `Next`, `Previous`,
  `SetPosition`/`Seek`, and `Volume` property via
  `org.freedesktop.DBus.Properties Set` if the Windows side supports a
  volume/seek slider). `target_app` selects a player by friendly name
  (case-insensitive); `None` → the first `Playing` player, else the first.
  An action Windows supports and MPRIS cannot do → `Ok(false)` (or whatever
  the Windows code returns for "not handled" - check).
- `spawn_push`: a thread polling once per second **only while
  `consumers() > 0`** (sleep otherwise), building the snapshot with the
  existing `build_snapshot`/`build_snapshot_keyed` and gating sends with the
  existing `PushGate`, exactly like the Windows poller. Album art: if
  `mpris:artUrl` is a `file://` URL, read the file and use
  `attach_art_data` like Windows does; ignore `http(s)` URLs for now.
  The thread ends when the receiver is dropped (send error).

Tests: JSON unwrapping, player-name filtering, metadata → `MediaProperties`
mapping, action → method mapping, player selection. All on fixtures.

STOP for this phase (skip, report, continue) if the Windows module turns out
to feed data that MPRIS cannot provide in the same shape without changing
`media.rs`'s public types.

Verify: `cargo test -p pulpit-os` passes; `cargo clippy -p pulpit-os --all-targets`
has no new warnings. Commit.

### Phase 8 - Voicemeeter greyed out on Linux + README

Backend:
- `crates/vm/src/lib.rs`: add `#[serde(default)] pub supported: bool` to
  `VoicemeeterStatus`, set to `cfg!(windows)` everywhere the struct is built
  (`grep -rn "VoicemeeterStatus {" crates apps --include=*.rs` and fix every
  literal, tests included - if a literal lives in a file listed under
  "Coordination", add the field there with a one-line edit and mention it in
  the report).

Frontend (`apps/desktop/src`):
- `api.js`: `export const vmSupported = ref(true);` and a function that
  calls `api.vmStatus()` once at startup (find where `refreshVmDevices` or
  similar startup probes are triggered and follow that pattern) and sets
  `vmSupported.value = status?.supported !== false` (errors leave it `true`).
- `ActionPicker.vue`: where the flat item list is computed, give every item
  whose `value` starts with `vm-` an
  `unavailable: t("Voicemeeter is not available on this system")` when
  `vmSupported.value` is `false`. The existing greyed-out rendering then
  applies unchanged. A tile that already uses a `vm-` action keeps showing
  its current selection.
- `SettingsOverlay.vue`: when unsupported, the Voicemeeter nav entry is
  rendered greyed (same visual language: reduced opacity, `not-allowed`
  cursor, `aria-disabled="true"`, tooltip with the same message) and does
  not navigate; if it was the remembered page, fall back to the first page.
- `VoicemeeterPage.vue`: if it is still reachable (e.g. `initialPage`),
  show the same one-line message instead of the controls.
- Add the Polish translation
  (`"Voicemeeter nie jest dostępny w tym systemie"`) to the right JSON file.
- vitest: one test for the picker (unsupported → `vm-set-strip` item has
  class `unavailable` and clicking does not emit; supported → pickable) and
  one for the settings nav, following the existing component tests in the
  repo (`grep -rln "ActionPicker" apps/desktop/src --include=*.test.js --include=*.spec.js`).

README: add a short "Linux" subsection near the install section: what works
(list the phases that landed), required tools per feature (table from
"Design decisions"), what does not (Voicemeeter, Discord integration,
auto-update, keyboard synthesis on native Wayland windows - see Phase 9
result).

Verify: `cargo test -p pulpit-vm`; in `apps/desktop`: `npm ci`,
`npx vite build`, `npx vitest run` all green. Commit.

### Phase 9 - Investigate only, write findings in the report (no code)

1. **Discord on Linux.** `crates/discord/src/lib.rs` is stubbed off Windows
   (`request_auth_code` → `Err(NotRunning)`, the client task drops its
   channel). Discord's IPC on Linux is a unix socket
   `$XDG_RUNTIME_DIR/discord-ipc-{0..9}` (also under
   `app/com.discordapp.Discord/` and `snap.discord/`) speaking the same
   framed protocol as the Windows named pipe. Report: is pipe I/O isolated
   behind a small seam (name the functions/lines), and roughly how many
   `#[cfg(windows)]` blocks would need a unix twin?
2. **Keyboard on Wayland.** `enigo` is `0.2.1` with default features (X11
   via libxdo): key presses reach X11/XWayland windows only. Report which
   features this enigo version offers (`cargo metadata --format-version 1 | grep -o '"wayland[^,]*'`
   or read the crate's `Cargo.toml` in `~/.cargo/registry`), and whether
   `cargo check -p pulpit-actions --features …`-style enabling of a Wayland
   backend compiles here. Do not commit any change for this.

### Final smoke check - the server boots on Linux

Build the headless server, then run it against scratch data and stop it:

```
D=$(mktemp -d)
PULPIT_PORT=18555 PULPIT_NO_DISCOVERY=1 PULPIT_DEVICES=$D/devices.json PULPIT_ASSETS=$D/assets PULPIT_EXT_DIR=$D/ext \
  timeout 15 ./target/debug/<server-binary> $D/pulpit.db > $D/out.log 2>&1 &
sleep 6; curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:18555/ ; wait
```

(Check `apps/server/src/main.rs` for the exact argument form; any HTTP
status proves it listens.) Quote the HTTP code and the last 20 log lines in
the report. Never use port 8500 and never touch `~/pulpitApp`.

## Done criteria

- [ ] `CARGO_BUILD_JOBS=3 timeout 1500 cargo test -p pulpit-os -p pulpit-sysinfo -p pulpit-actions -p pulpit-vm --locked` exits 0.
- [ ] `CARGO_BUILD_JOBS=3 cargo clippy -p pulpit-os -p pulpit-sysinfo -p pulpit-actions -p pulpit-vm --all-targets --locked` shows no new warnings in touched files.
- [ ] `CARGO_BUILD_JOBS=3 cargo build --workspace --locked` succeeds (nothing else broke).
- [ ] In `apps/desktop`: `npx vite build` and `npx vitest run` exit 0.
- [ ] `git diff 0da95cd --stat -- '*win*.rs'` is empty, and no line inside a `#[cfg(windows)]` block changed (state how you checked).
- [ ] One commit per completed phase; `git diff 0da95cd --stat` lists only in-scope files.
- [ ] The report lists, per phase: DONE / SKIPPED (reason), the tests added, and what was verified live vs. only by fixtures.

## STOP conditions (whole plan)

- A phase needs a change to a public trait or type used by Windows code
  (`Speaker`, `Input`, `media.rs` public structs): skip the phase, report.
- A needed edit falls in a file listed under "Coordination" (other than the
  one-line `VoicemeeterStatus` literal case): skip, report.
- The workspace does not build at `0da95cd` before your first change: STOP
  completely and report the error.

## Maintenance notes

- Every Linux feature degrades to `Unsupported` when its tool is missing;
  the tool list in the README is the support contract.
- Native replacements (PipeWire/zbus/arboard) can later replace individual
  `linux/*.rs` files without touching callers.
- CI builds and tests Rust on Windows only; these Linux modules are compiled
  and tested by nobody in CI until a Linux job is added - worth doing once
  this lands.
- Reviewer focus: no behaviour change on Windows; no shell injection (all
  tools are spawned with argument vectors, never through `sh -c` with
  interpolated strings - device ids and file paths come from tiles).
