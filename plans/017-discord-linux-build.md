# Plan 017: Make `pulpit-discord` (and its dependents) compile on non-Windows

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in the "STOP conditions" section occurs,
> stop and report — do not improvise. When done, update the status row
> for this plan in `plans/README.md` — unless a reviewer dispatched you
> and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 6dcddf3..HEAD -- crates/discord/src/lib.rs`
> If the file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `6dcddf3`, 2026-10-29

## Why this matters

`cargo test --workspace` and `cargo clippy --workspace` are this repo's
standard Rust verification (AGENTS.md), but they only work on Windows:
`pulpit-discord`'s connection transport is built on Win32 named pipes
(Discord IPC), the `pipe` module is `#[cfg(windows)]`, while several
users of it are NOT gated — so the crate fails with ~38 `unresolved
module pipe` errors on Linux, and it drags `pulpit-backend`,
`pulpit-host`, `pulpit-desktop` and `pulpit-server` down with it. This
blocked local verification during a real session on 2026-10-29 (the
advisor could only test two crates). Goal: the whole workspace type-
checks on Linux; Windows behavior is unchanged.

## Current state

All in `crates/discord/src/lib.rs` (~2000 lines). Key landmarks:

- Line 850 — ungated const referencing the gated module:

  ```rust
  /// Second wait slot in [`Pipe::read_some_wake`]: the wake event (the
  /// first slot, the read event, answers as plain WAIT_OBJECT_0).
  const WAIT_WAKE: u32 = pipe::WAIT_OBJECT_0 + 1;

  #[cfg(windows)]
  mod pipe {
      use std::ffi::c_void;
      pub const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;
      ...
  }
  ```

- Lines ~1783-1825 — `WakeHandle`/`WakeEvent` call `pipe::CreateEventW`,
  `pipe::SetEvent`, `pipe::CloseHandle` ungated:

  ```rust
  struct WakeHandle(*mut c_void);
  unsafe impl Send for WakeHandle {}
  struct WakeEvent(*mut c_void);
  unsafe impl Send for WakeEvent {}
  unsafe impl Sync for WakeEvent {}
  impl WakeEvent {
      fn new() -> WakeEvent {
          WakeEvent(unsafe { pipe::CreateEventW(std::ptr::null_mut(), 0, 0, std::ptr::null_mut()) })
      }
      fn handle(&self) -> WakeHandle { WakeHandle(self.0) }
      fn set(&self) {
          if !self.0.is_null() { unsafe { pipe::SetEvent(self.0) }; }
      }
  }
  impl Drop for WakeEvent {
      fn drop(&mut self) {
          if !self.0.is_null() { unsafe { pipe::CloseHandle(self.0) }; }
      }
  }
  ```

- Lines ~1213-1600 — the `Conn` transport (`connect`, `connect_with_push`,
  `serve` at ~1529 taking `wake: WakeHandle`, plus its pipe I/O
  helpers). All pipe-dependent.
- Line 1772 — `pub struct DiscordClient` with field `wake:
  std::sync::Arc<WakeEvent>` (~1777); `impl DiscordClient` at ~1837,
  `pub fn spawn_push` at ~1847; `fn actor_loop` at ~1958 (spawns the
  pipe actor thread).
- Line 470 — `pub fn probe_status` calls `Conn::connect`.
- Public API that MUST keep existing on all platforms (used by
  `crates/backend/src/lib.rs` and the desktop): `DiscordConfig`,
  `DiscordError`, `DiscordStatus`, `DiscordClient` + `spawn_push` +
  the exec/plan methods, `authorize`, `refresh`, `save_tokens`,
  `save_config`, `clear_tokens`, `probe_status`, `is_discord_action`,
  `input_declarations`, `MemberVoice` and its helpers,
  `encode_frame`/`decode_frame`, `plan`. These are pure (HTTP/fs/math)
  except where noted below.

**Repo exemplar for the stub pattern**: `crates/os/src/media.rs:356-370`
— `spawn_push` is `#[cfg(windows)]` real, and the non-Windows body
returns an immediately-empty channel:

```rust
#[cfg(not(windows))]
{
    let _ = consumers;
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    rx
}
```

and `crates/os/src/media.rs:331-342` — `control()` gates per-OS with a
non-Windows `Err(OsError::Unsupported(...))` arm.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Linux check (the point) | `cargo check -p pulpit-discord` | exit 0, no errors |
| Dependent check | `cargo check -p pulpit-backend -p pulpit-host -p pulpit-server` | exit 0 |
| Workspace check | `cargo check --workspace` | exit 0, or only failures OUTSIDE this plan's scope (report them) |
| Windows cross-check | `cargo check -p pulpit-discord --target x86_64-pc-windows-msvc --all-targets` | exit 0 (Windows target is installed) |
| Linux tests (newly possible) | `cargo test -p pulpit-discord` | all pass |
| Lint | `cargo clippy -p pulpit-discord --all-targets` | no new warnings vs. before |

## Suggested executor toolkit

- None needed; plain Rust + the repo's existing cfg patterns.

## Scope

**In scope**:
- `crates/discord/src/lib.rs`

**Out of scope** (do NOT touch):
- `crates/backend`, `crates/host`, `apps/*` — they must compile thanks
  to the crate's own gating, not their own. If they still fail on
  Linux for reasons INSIDE them, that is a report line, not a fix
  (STOP condition).
- Any behavior change on Windows: the `#[cfg(windows)]` paths must
  produce byte-identical behavior.
- Discord-over-unix-sockets support — explicitly not a goal; Linux is
  dev/CI-verification only.

## Git workflow

- Branch: `advisor/017-discord-linux-build`
- Commits per step; message style: imperative, e.g. `Gate the Discord pipe transport behind cfg(windows)`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Baseline the failure list

Run `cargo check -p pulpit-discord 2>&1 | grep -E "^error" | head -50`
and keep the list — these are the items to gate. Every one should
mention `pipe`, `WakeEvent`, `WakeHandle`, `WAIT_WAKE`, or a `Conn`
helper. **Verify**: list captured (expected ~38 errors mentioning
`pipe`).

### Step 2: Gate the transport, keep the types

Apply `#[cfg(windows)]` to every pipe-dependent item and provide the
non-Windows counterpart, in this shape:

1. `WAIT_WAKE` — real one `#[cfg(windows)]`; add
   `#[cfg(not(windows))] const WAIT_WAKE: u32 = 1;` (value never used:
   `Conn` cannot be constructed on non-Windows).
2. `WakeEvent`/`WakeHandle` struct defs stay shared (a raw pointer
   field compiles everywhere); cfg-split the impls:
   `#[cfg(windows)] impl WakeEvent` with the real `new`/`handle`/`set`
   and `Drop`; `#[cfg(not(windows))] impl WakeEvent` with
   `fn new() -> WakeEvent { WakeEvent(std::ptr::null_mut()) }`,
   `fn handle(&self) -> WakeHandle { WakeHandle(std::ptr::null_mut()) }`,
   `fn set(&self) {}`, and an empty `Drop`. The existing
   `if !self.0.is_null()` guards make the no-op safe.
3. The `Conn` transport (struct, `connect`, `connect_with_push`,
   `serve`, and every private helper the errors blame): `#[cfg(windows)]`
   on each. Add a `#[cfg(not(windows))]` stub with the SAME signature
   for `Conn::connect` only:
   `fn connect(_: &DiscordConfig, _: Instant) -> Result<Conn> { Err(DiscordError::NotRunning) }`
   — but ONLY if some shared code (e.g. `probe_status`) still calls it;
   if you gate `probe_status`'s connect arms per-OS instead, prefer
   whatever leaves less duplicated code. `NotRunning` is the honest
   signal: Discord IPC is unavailable on this platform.
4. `fn actor_loop` + the thread-spawning part of
   `DiscordClient::spawn_push`: `#[cfg(windows)]`. Non-Windows
   `spawn_push` mirrors the `crates/os/src/media.rs` exemplar: build
   and immediately drop the receiver (`let (_tx, rx) = ...; drop(rx);`
   return a client that simply never produces events) — match the
   exemplar's shape as closely as the signature allows.

Keep `probe_status`'s pure prefix (`NotConfigured`/`NeedsAuth` checks)
shared; only its `Conn::connect` calls differ per-OS.

**Verify**: `cargo check -p pulpit-discord` → exit 0. If errors remain,
they name the next item to gate — repeat until clean. If an error does
NOT fit the shapes above (e.g. a shared type needs pipe internals),
that is a STOP condition.

### Step 3: Confirm the dependents and Windows

**Verify**: `cargo check -p pulpit-backend -p pulpit-host -p pulpit-server` → exit 0.

**Verify**: `cargo check -p pulpit-discord --target x86_64-pc-windows-msvc --all-targets` → exit 0.

### Step 4: Tests and lint

**Verify**: `cargo test -p pulpit-discord` → all pass (the in-file
tests are pure: frames, plan, perceptual math, config).

**Verify**: `cargo clippy -p pulpit-discord --all-targets` → no new
warnings (dead-code warnings for cfg'd items must not appear; if they
do, the cfg split is wrong, not the lint).

### Step 5: Workspace sweep (informational)

Run `cargo check --workspace`. If further failures appear in OTHER
crates (likely candidates: `pulpit-desktop`'s WebView2 assumptions),
record them verbatim in your report and STOP there — out of scope.

## Test plan

- No new tests required; the existing pure unit tests becoming
  runnable on Linux IS part of the win. If trivial, one smoke test is
  welcome: `#[cfg(not(windows))] #[test] fn spawn_push_stubs_to_a_closed_channel()`
  asserting the receiver yields `None` immediately — model after
  `crates/os/src/media.rs` tests only if cheap; skip on any friction.

## Done criteria

- [ ] `cargo check -p pulpit-discord` exits 0 on Linux
- [ ] `cargo check -p pulpit-backend -p pulpit-host -p pulpit-server` exits 0
- [ ] `cargo check -p pulpit-discord --target x86_64-pc-windows-msvc --all-targets` exits 0
- [ ] `cargo test -p pulpit-discord` passes on Linux
- [ ] `cargo clippy -p pulpit-discord --all-targets` adds no warnings
- [ ] `git status` shows only `crates/discord/src/lib.rs` modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- A pipe dependency appears in a type that must stay shared (public API
  list above) in a way the cfg-split shapes cannot express.
- `pulpit-backend`/`pulpit-host`/`pulpit-server` still fail on Linux
  for reasons inside THEIR files (not the discord crate).
- The Windows cross-check fails (a cfg split broke the Windows build).
- A step's verification fails twice after a reasonable fix attempt.

## Maintenance notes

- Reviewer: the diff should be almost entirely `#[cfg(...)]` attributes
  and two small stub blocks; any logic edit is suspect.
- If the owner ever wants real Discord IPC on Linux (unix sockets),
  this split is the seam: implement `Conn::connect` for
  `cfg(not(windows))` and delete the stubs.
- `probe_status` on Linux now reports `NotRunning` after the
  configured/needs-auth checks — document nothing; it is a dev-only
  path.
