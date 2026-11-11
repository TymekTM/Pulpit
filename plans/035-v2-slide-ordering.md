# 035 - v2 slider events are applied in order, latest value wins

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Drift check first:** confirm the excerpts below still match.

## Status

- Priority: P2 | Effort: M | Risk: medium (concurrency) | Depends on: none
- Category: correctness
- Planned at: commit `0da95cd`, 2026-11-10
- Confidence in the bug: medium - found by reading the code, not reproduced.

## Why this matters

A slider tile on the tablet sends a `slide` interaction roughly every 120 ms
while the finger moves, and a final one on release. The v2 server handles
each one with a fire-and-forget blocking task. Some slider backends are slow
and synchronous (Spotify volume is one HTTPS request per value; Discord is
serialized behind a mutex), so several tasks for the same tile run at once
and may finish in any order: the volume can end on a stale value instead of
where the finger stopped. The legacy protocol does not have this problem - it
awaits each slider call.

## Current state

`crates/v2/src/session.rs` (~:515):

```rust
        Interaction::Slide => {
            ack_ok();
            let value = payload.args.value.unwrap_or(0.0);
            let backend = state.backend.clone();
            tokio::task::spawn_blocking(move || backend.slider(button, value));
        }
```

and the double-tap reset a few lines below (~:541):

```rust
                    let backend = state.backend.clone();
                    tokio::task::spawn_blocking(move || backend.slider(target_button, reset));
                    return;
```

`button` / `target_button` are `pulpit_db::ButtonRow` values (the tile's DB
row; it has an integer `id`). `state.hub` is an `Arc<V2Hub>`;
`crates/v2/src/hub.rs` (~:20):

```rust
#[derive(Default)]
pub struct V2Hub {
    sessions: Mutex<HashMap<u64, Arc<V2Session>>>,
    next_id: AtomicU64,
}
```

Modules are declared in `crates/v2/src/lib.rs` (`mod hub; mod service; mod session; mod state; …`).
The integration test backend (`MockBackend` in
`crates/v2/tests/integration.rs`) records slider calls in
`sliders: Arc<Mutex<Vec<(i64, f64)>>>`.

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-v2 --locked -- --skip hub_teardown_ends_the_session_and_releases_held_keys` |
| Lint | `CARGO_BUILD_JOBS=3 cargo clippy -p pulpit-v2 --all-targets --locked` |

The skipped test is a known Linux-only failure fixed by another plan.

## Scope

In scope: new file `crates/v2/src/slides.rs`; `crates/v2/src/lib.rs` (one
`mod` line); `crates/v2/src/hub.rs` (one field + accessor);
`crates/v2/src/session.rs` (the two call sites).

Out of scope: `V2State` (do not add fields - it is built by struct literal
in four places), `crates/v2/src/service.rs`, `crates/v2/src/devices.rs`,
`crates/v2/tests/integration.rs`, `crates/legacy`, the desktop's own
`exec_slider` command, the Android client.

## Design (decided)

`SlideLanes`: one lane per tile id. While a lane's worker is busy, a newer
job for the same tile **replaces** any job still waiting (intermediate
values are obsolete for an absolute slider); the worker runs jobs one at a
time; the last submitted job always runs. Different tiles never block each
other.

```rust
//! Per-tile ordering for slider values (see the module docs for why).
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Default)]
pub(crate) struct SlideLanes {
    /// Present key = a worker is running for that tile; the value is the
    /// newest job waiting behind it, if any.
    lanes: Mutex<HashMap<i64, Option<Job>>>,
}

impl SlideLanes {
    pub(crate) fn submit(self: &Arc<Self>, tile: i64, job: impl FnOnce() + Send + 'static) { … }
    #[cfg(test)]
    fn idle(&self) -> bool { self.lanes.lock().unwrap().is_empty() }
}
```

`submit`: lock; if the key exists → store `Some(Box::new(job))` (dropping an
older waiting job) and return. Otherwise insert `None` and
`tokio::task::spawn_blocking` a worker that runs `job`, then loops: lock,
`take()` the waiting job; if there is one, unlock and run it; if none,
remove the key and exit. **Panic safety:** the worker holds a drop guard that
removes the key if a job panics, so one panicking backend call cannot wedge
the tile forever. Never hold the mutex while running a job.

## Git workflow

One commit: `v2: apply slider values per tile in order, latest wins`. No push.

## Steps

### Step 1 - `slides.rs` + unit tests
Write the module as designed, with tests using
`#[tokio::test(flavor = "multi_thread")]`:
- `the_latest_value_wins_and_order_is_kept`: job 1 blocks on a
  `std::sync::mpsc` receiver and then records `1`; wait until it has started
  (second channel); submit jobs recording `2` and `3`; release job 1; wait
  (poll `idle()` with a 5 s deadline); assert the record is `[1, 3]`.
- `tiles_do_not_block_each_other`: a blocked job on tile 1 does not delay a
  job on tile 2.
- `a_panicking_job_does_not_wedge_the_lane`: first job panics; a later
  `submit` for the same tile still runs.

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-v2 --locked --lib slides` passes.

### Step 2 - Wire it
- `lib.rs`: `mod slides;`
- `hub.rs`: field `slides: Arc<crate::slides::SlideLanes>` (the struct keeps
  `#[derive(Default)]`) and `pub(crate) fn slides(&self) -> &Arc<crate::slides::SlideLanes>`.
- `session.rs`: both call sites become
  `state.hub.slides().submit(button.id, move || { backend.slider(button, value); });`
  (use the real id field name of `ButtonRow`; discard `slider`'s return
  value the same way the current code does).

Verify: the Test command passes (existing slider integration tests
included).

## Done criteria

- [ ] The Test command exits 0; the three new unit tests exist and pass.
- [ ] `grep -n "spawn_blocking(move || backend.slider" crates/v2/src/session.rs` prints nothing.
- [ ] Lint shows no new warnings in the four files.
- [ ] `git diff --stat` lists only the four in-scope files.

## STOP conditions

- `ButtonRow` has no stable integer id usable as a key (e.g. gesture targets
  are synthetic rows with id 0): report - do not invent a key.
- An existing integration test asserts that *every* intermediate slider value
  reaches the backend: report the test; do not change it.

## Maintenance notes

- The desktop touch mode's own `exec_slider` Tauri command has the same
  fire-and-forget shape and could reuse this type later.
- Relative sliders (if ever added) must not use latest-wins coalescing.
