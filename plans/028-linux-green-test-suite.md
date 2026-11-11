# 028 - Make `cargo test --workspace` green (and non-hanging) off Windows

> **Executor instructions.** Follow the steps in order. Touch only the files
> listed under "In scope". Run each step's Verify command before moving on.
> If a STOP condition triggers, stop and report instead of improvising.
> Do not edit anything under `plans/` (the reviewer maintains the index).
> **Drift check first:** `git log --oneline -1` should show `0da95cd` (or a
> descendant); open every file cited in "Current state" and confirm the
> excerpts still match. If they do not, STOP and report what changed.

## Status

- Priority: P1 | Effort: S | Risk: low | Depends on: none
- Category: tests / DX / correctness
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

CI runs the Rust tests on `windows-latest` only, so nobody noticed that on
Linux the suite has two failing tests, one test that hangs forever, and one
test that leaves a stray `Z:` directory inside the repository. The hang also
exposes a real product bug: when the keyboard backend cannot start (headless
server, no display), *every* built-in action is silently dropped - including
steps that need no keyboard at all (HTTP request, delay, open URL, run
program). After this plan `cargo test --workspace --locked` passes on Linux
with no skip flags, and a headless server still runs non-keyboard steps.

## Current state

### A. DB test assumes a Windows drive letter - `crates/db/src/lib.rs` (~:1495)

```rust
#[test]
fn open_read_write_still_errors_on_an_unwritable_path() {
    // creation cannot rescue a path whose parent cannot exist
    let err = Db::open_read_write(Some(Path::new("Z:/nope/pulpit.db"))).unwrap_err();
    assert!(matches!(err, DbError::Sqlite(_)));
}
```

`open_read_write` (~:295) runs `let _ = std::fs::create_dir_all(parent);`
and then opens with `SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_CREATE`. On Linux
`Z:/nope` is a perfectly valid relative path, so the open succeeds, the test
panics on `unwrap_err`, and `crates/db/Z:/nope/pulpit.db` is left on disk.
The neighbouring test just above already uses `tempfile::tempdir()`.

### B. Flood test with a fixed iteration count - `crates/v2/tests/integration.rs` (~:1431)

```rust
    for _ in 0..600 {
        state.hub.broadcast_frame(&fat);
        if state.hub.count() == 0 {
            break;
        }
    }
    assert_eq!(state.hub.count(), 0, "wedged session must be torn down");
    wait_for_exec(&backend, (23, false)).await;
```

Each frame is ~8 KiB, so 600 frames are ~4.9 MiB. The hub tears a session
down when its bounded outbound queue is full. On Linux the kernel socket
buffers on loopback absorb more than 4.9 MiB (`tcp_wmem` max is 4 MiB plus
the receiver's buffer), the writer task keeps draining the queue, the queue
never fills, and the assertion fails. The production behaviour is correct;
only the test's bound is too small.

### C. Built-in actions dropped when the keyboard backend cannot start - `crates/backend/src/lib.rs` (~:1185)

```rust
    fn with_input(&self, f: impl FnOnce(&mut dyn pulpit_actions::Input)) {
        let mut guard = self.input.lock().unwrap();
        if guard.is_none() {
            match EnigoInput::new() {
                Ok(i) => *guard = Some(Box::new(i)),
                Err(e) => {
                    tracing::error!("input backend init failed: {e}");
                    return;
                }
            }
        }
        if let Some(input) = guard.as_deref_mut() {
            f(input);
        }
    }
```

`EnigoInput` lives in `crates/actions/src/lib.rs` (~:567):

```rust
/// Real OS backend.
pub struct EnigoInput {
    enigo: enigo::Enigo,
}

impl EnigoInput {
    pub fn new() -> Result<Self> {
        let enigo = enigo::Enigo::new(&enigo::Settings::default())
            .map_err(|e| ActionError::Input(e.to_string()))?;
        Ok(EnigoInput { enigo })
    }
```

In its `impl Input for EnigoInput` only `key_down`, `key_up`, `key_tap`,
`text`, `mouse_move`, `mouse_click` and `media` use `self.enigo`; `open_url`
(`open::that`), `spawn` (`std::process::Command`) and `sleep`
(`thread::sleep`) do not. `ActionError` (~:15) is
`Unsupported(String) | BadPayload(String, String) | Input(String)`.

### D. The hanging test - `crates/backend/src/http_request.rs` (~:890)

`http_step_inside_a_multiaction_runs_the_native_chain` runs a `multiaction`
tile whose only step is an HTTP request against a `LocalServer::serve_once`
helper (~:608). On a machine with no display `with_input` returns early (C),
the request is never sent, and `server.request()` joins a thread that is
blocked forever in `listener.accept().unwrap()`.

## Commands

| Purpose | Command (run from repo root) |
| --- | --- |
| Build + test one crate | `CARGO_BUILD_JOBS=3 cargo test -p <crate> --locked` |
| Full suite (final gate) | `CARGO_BUILD_JOBS=3 timeout 1800 cargo test --workspace --locked` |
| Lint | `CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets --locked` |

Crate names: `pulpit-db`, `pulpit-v2`, `pulpit-backend`, `pulpit-actions`.
Other agents build on this machine at the same time: always keep
`CARGO_BUILD_JOBS=3`. Always wrap test runs in `timeout` - one test is known
to hang until step 4 is done.

## Scope

In scope:
- `crates/db/src/lib.rs` (the one test only)
- `crates/v2/tests/integration.rs` (the one test only)
- `crates/actions/src/lib.rs` (`EnigoInput` struct, `new`, a new `detached`
  constructor, the seven enigo-using methods, new unit tests)
- `crates/backend/src/lib.rs` (`with_input` only)
- `crates/backend/src/http_request.rs` (test helper `LocalServer` only)

Out of scope - do not touch:
- Any production code in `crates/v2/src` or `crates/db/src` outside tests.
- `paste_text` / `screenshot` in `crates/actions/src/lib.rs` (another plan
  edits them concurrently). Do not reformat the file.
- `.github/workflows/*` (adding a Linux CI job is a separate decision).

## Git workflow

Work on the branch your checkout is already on. One commit at the end,
message style of this repo (imperative sentence), e.g.
`Make the Rust test suite pass on Linux; run non-keyboard steps without an input backend`.
Do not push.

## Steps

### Step 1 - DB test uses a real unopenable path

Replace the test body so the parent of the DB path is a regular *file*:

```rust
#[test]
fn open_read_write_still_errors_on_an_unwritable_path() {
    // creation cannot rescue a path whose parent is a regular file
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-dir");
    std::fs::write(&blocker, b"x").unwrap();
    let err = Db::open_read_write(Some(&blocker.join("pulpit.db"))).unwrap_err();
    assert!(matches!(err, DbError::Sqlite(_)));
}
```

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-db --locked` passes and
`ls crates/db | grep -c "Z:"` prints `0`.

### Step 2 - Flood test is bounded by time, not by frame count

Replace the `for _ in 0..600` loop with a loop that keeps broadcasting until
`state.hub.count() == 0` or a deadline (20 s) passes. Keep the
`assert_eq!(state.hub.count(), 0, ...)` and the `wait_for_exec` line exactly
as they are. Sketch:

```rust
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while state.hub.count() != 0 && std::time::Instant::now() < deadline {
        state.hub.broadcast_frame(&fat);
    }
```

Verify: `CARGO_BUILD_JOBS=3 timeout 600 cargo test -p pulpit-v2 --locked --test integration hub_teardown`
passes, and passes 5 times in a row.

### Step 3 - `EnigoInput` can exist without a keyboard backend

In `crates/actions/src/lib.rs`:

1. Change the struct to hold the backend optionally, with the reason it is
   missing:
   ```rust
   pub struct EnigoInput {
       enigo: Option<enigo::Enigo>,
       /// Why `enigo` is `None` (the init error), for the per-step error.
       unavailable: String,
   }
   ```
2. `new()` keeps its signature and behaviour (returns `Err` when enigo
   cannot start).
3. Add `pub fn detached(reason: impl Into<String>) -> Self` that builds an
   `EnigoInput` with `enigo: None`.
4. Add a private helper
   `fn enigo(&mut self) -> Result<&mut enigo::Enigo>` returning
   `Err(ActionError::Input(format!("keyboard/mouse backend unavailable: {}", self.unavailable)))`
   when `None`. Route every `self.enigo.` use in the seven methods through
   it. Do not change `open_url`, `spawn`, `sleep`, `paste_text`,
   `screenshot`.
5. Add unit tests next to the existing tests in this file:
   - `detached_input_fails_key_steps_but_runs_the_rest`: `key_tap`, `text`
     and `media` on `EnigoInput::detached("no display")` return
     `Err(ActionError::Input(_))` whose message contains `no display`;
     `sleep(0)` returns `Ok(())`.

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-actions --locked` passes.

### Step 4 - `with_input` falls back instead of dropping the action

In `crates/backend/src/lib.rs` change only the `Err(e)` arm: keep the
`tracing::error!` line, then run `f` against a temporary detached input and
return. Do **not** store the detached input in `guard` - the next call must
retry `EnigoInput::new()` (a display may appear later).

```rust
                Err(e) => {
                    tracing::error!("input backend init failed: {e}");
                    // keyboard/mouse steps fail one by one; everything
                    // else in the chain (http, delay, url, run) still runs
                    let mut detached = EnigoInput::detached(e.to_string());
                    f(&mut detached);
                    return;
                }
```

Verify: `CARGO_BUILD_JOBS=3 timeout 900 cargo test -p pulpit-backend --locked`
finishes (no hang) and passes, including
`http_step_inside_a_multiaction_runs_the_native_chain`.

### Step 5 - `LocalServer` can never hang a test run again

In the `#[cfg(test)]` helper `LocalServer::serve_once`
(`crates/backend/src/http_request.rs`): make the accept bounded. Set the
listener non-blocking, poll `accept()` every 10 ms for up to 10 s, and
`panic!("LocalServer: no request arrived within 10 s")` on expiry. After a
successful accept call `sock.set_nonblocking(false)` and
`sock.set_read_timeout(Some(Duration::from_secs(5)))`. Nothing else in the
helper changes.

Verify: same command as step 4.

## Test plan

- New: `detached_input_fails_key_steps_but_runs_the_rest` (step 3).
- Existing tests fixed: the three named above.
- Final gate (must be run, with output quoted in the report):
  `CARGO_BUILD_JOBS=3 timeout 1800 cargo test --workspace --locked`.

## Done criteria

- [ ] `CARGO_BUILD_JOBS=3 timeout 1800 cargo test --workspace --locked` exits 0 with no `--skip` flags.
- [ ] `git status --porcelain --ignored | grep -c "Z:"` prints `0`.
- [ ] `CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets --locked` reports no warning in the five in-scope files that was not there before your change (run it once before step 1 and compare).
- [ ] `git diff --stat` lists only the five in-scope files.

## STOP conditions

- Step 2 still fails after the deadline change: the hub is not tearing the
  session down on Linux at all - that is a production bug, report it, do not
  edit `crates/v2/src`.
- After step 4 the full suite shows a *different* failing test that also
  fails on a clean checkout of `0da95cd`: report it (name, output), do not
  fix it.
- Any change would be needed outside the in-scope files.

## Maintenance notes

- A Linux job in `.github/workflows/ci.yml` would keep this green; left to
  the owner because it costs CI minutes.
- Plan 037 (Linux platform support) edits `paste_text`/`screenshot` in the
  same `impl`; it is written to call trait methods (`self.key_tap`) rather
  than `self.enigo`, so the two changes merge cleanly.
