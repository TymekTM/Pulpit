# Plan 026: Kill the tools 100 ms tick; keep Spotify's last values on transient errors

Round 7, from audit C (SRV-1: 11 of the headless server's 20.2 idle wakeups/s come from crates/tools' unconditional 100 ms tick + 1 s push loop) and audit A's side-finding (correctness: a transient Spotify error flips tiles to idle against the code's own contract). Priority P2, Effort S-M, Risk LOW.
Files: crates/tools/src/lib.rs, crates/spotify/src/poller.rs (+tests).

## Part 1 — tools tick (crates/tools/src/lib.rs:310-316)
The loop wakes every 100 ms to catch timer finishes promptly. Replace with event-driven scheduling: when a timer is created/started, compute the next finish instant and schedule a single `tokio::time::sleep_until` (a JoinHandle or `tokio::select!` in the loop over a wake channel + the next-deadline sleep); no periodic wake when no timer runs. The 1 s push loop for running-timer display pushes: keep the push, but only while timers actually run (and it may tick aligned to whole seconds — the clients' tiles interpolate their own progress since design §4; check `tick_timers`'s push shape: if it pushes position, clients already extrapolate; then a 1 s push only needs the `playing` state change moments + a coarse heartbeat).

## Part 2 — Spotify transient error (crates/spotify/src/poller.rs:90-94)
The `Err(e)` arm logs "transient (offline, 5xx): keep the clients' last values" but falls through to `build_snapshot(player=None)` → pushes the idle payload (tiles blank) and the next poll waits the paused cadence. Add the missing `continue` (keeping the last player's values in the loop state so the NEXT successful poll's compare dedupes correctly — verify the loop's `player` variable lifetime; if the snapshot builds from a local, keep the previous snapshot string untouched so nothing is pushed).

## Verification
cargo test -p pulpit-tools -p pulpit-spotify (new tests: tools — creating a timer schedules exactly one wake at its deadline, none while empty; spotify — a transient error keeps the last pushed snapshot (script the HTTP seam as the existing tests do)); clippy + fmt both.

## STOP
If the tools push shape requires sub-second display updates that clients cannot interpolate (check what the timer tile renders: if it renders the producer-pushed remaining time WITHOUT local extrapolation, keep a 1 s aligned push while running — that is still 10× fewer wakes than 100 ms).

## Reviewer note
After this lands, the 10 Hz v2 flusher is the only sub-second idle wake left — the documented condition for reopening it.
