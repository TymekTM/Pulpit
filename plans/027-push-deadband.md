# Plan 027: Position dead-band + follow-up re-poll gated to real property events

Round 7 (audit A PERF-02), depends on 022 (landed: digest compare, art-on-change). Priority P2, Effort S-M, Risk MED.
Files: crates/os/src/media/win_media.rs, crates/os/src/media.rs (+tests in media.rs).

## Why
While playing, EVERY 1 s poll counts as a change (position moves) → full fan-out: ~7 engine_set calls (25 allocs each), one legacy broadcast per client, one v2 patch per client, one WebView IPC + Vue update — for a position the clients already interpolate locally (design §4). Worse, the 400 ms follow-up (PR #31) arms after ANY event: the single shared waker cannot tell `MediaPropertiesChanged` from `TimelinePropertiesChanged`, and players that fire timeline events at ~1 Hz get DOUBLE pushes.

## Binding design
1. **Event typing**: three `Arc<AtomicBool>` flags (props / playback / timeline) + the existing wake, set by the corresponding handlers in `spawn_push` (`win_media.rs` event registration). The loop reads-and-clears them each iteration. Session-change (current_id != last) counts as props for follow-up purposes.
2. **Follow-up gating**: `do_follow_up = (props_flag || session_changed) && changed`. Timeline/playback-only events do NOT arm the follow-up (their data is in the periodic poll anyway).
3. **Position dead-band** (pure helper in media.rs, unit-tested there, used by the loop):
   - `identity_string(snapshot)`: serialized snapshot with the volatile position keys normalized out — `media-progress` and `media-now-playing.progress.position_ms` removed (keep `duration_ms`, `playing`, the art digest, titles — everything semantic).
   - Decision: send when `identity != last_identity` (any semantic change — art, title, play state, shuffle…) OR `!is_playing` (paused positions are user-visible seeks) OR `(now - last_send) >= 10 s` (resync heartbeat). Otherwise skip the send (the dead-band window).
   - On send: update `last_identity` and `last_send`.
   - The 10 s heartbeat bounds extrapolation drift on long tracks; clients clamp to duration anyway.
4. Poll cadence stays 1 s (the read is cheap now, post-022); only the PUSH is dead-banded. The host-side ProducerGate/WebView work disappears for dead-banded polls.

## Verification
cargo test -p pulpit-os (new pure tests: identity ignores position but sees art/title/playing changes; decision sends on identity change, on pause, on heartbeat, skips inside the window); clippy/fmt; msvc cross-check for the loop.

## STOP
If the identity normalization cannot express a semantic change you find in build_snapshot (a field that changes with position but is semantic — e.g. media-progress as a 3-decimal fraction MUST be normalized away; it is position-derived); if Windows CI reveals the follow-up gating breaks the late-cover catch (it must not: props events still arm it).

## Reviewer note
The late-cover mechanism from #31 must survive: follow-up still armed on MediaPropertiesChanged (which is how track changes announce themselves).
