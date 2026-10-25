# 01 — crates/discord: member tracking + SET_USER_VOICE_SETTINGS

Status: resolved
Type: task
Blocked by: —

## Goal

Extend `crates/discord` from a request-driven command pipe into an eager
voice-state actor (background actor like `pulpit_obs` /
`pulpit_spotify::spawn_push`) that tracks the selected voice channel's
members and can set per-user local volume/mute.

## Tasks

- `Conn`: add `subscribe_until(deadline, subscribe: bool, evt, args)`
  emitting the top-level `"evt"` field `SUBSCRIBE`/`UNSUBSCRIBE`
  require (`request_until` cannot express it today).
- `classify()`/`Inbound`: handle `DISPATCH` for `VOICE_CHANNEL_SELECT`,
  `VOICE_STATE_CREATE/UPDATE/DELETE`, `SPEAKING_START/STOP`.
- Member state machine (see `api-docs.md` §2.4):
  - on connect: subscribe `VOICE_CHANNEL_SELECT`, then
    `GET_SELECTED_VOICE_CHANNEL` for the initial snapshot;
  - on channel select: resubscribe channel-scoped events, reseed
    members from the fresh snapshot (upsert by `user.id`, skip self);
  - keep an ordered `BTreeMap<user_id, MemberVoice>` with display name
    resolution `nick` → `global_name` → `username` and `user.bot`.
- New operations: set user volume (perceptual fraction 0..1 → RPC
  amplitude via the documented curve, coalesced latest-wins per
  `user_id` so slider drags don't queue stale frames), toggle user
  mute, list members (id, display name, bot flag, volume fraction).
- Perceptual conversion helpers from `api-docs.md` §3.1 as pure
  functions + round-trip unit tests.
- Snowflakes stay `String` end to end (never parsed to f64/i64).
- Handle pipe death/reconnect: members reset, overrides revert on
  Discord side — state must resync from a fresh snapshot.

## Acceptance

- `cargo test -p pulpit-discord` covers: classify of the new DISPATCH
  events, member upsert/delete ordering, conversion round-trip,
  coalescing. Pipe-dependent paths stay behind the existing live-test
  gate; everything else passes on Linux CI.
- `cargo clippy -p pulpit-discord --all-targets` clean.

## Comments

- Implemented: Conn tracks the selected voice channel (VOICE_CHANNEL_SELECT + channel-scoped VOICE_STATE_* subscriptions with the new top-level `evt` field), MemberVoice map with nick/global_name/username resolution, SET_USER_VOICE_SETTINGS for volume+mute, perceptual curve helpers, snapshot pushes, reconnect resync. Cross-checked compile+clippy (windows-gnu target); unit tests run in CI (windows-latest) - this box has no wine.

- Review round (Sonnet, needs-changes -> fixed): (1) VOICE_CHANNEL_SELECT handling moved out of absorb into a resync flag flushed outside pending requests (nested round trips were swallowing nonce replies); (2) tracking setup is best-effort - a failed subscribe/snapshot no longer breaks toggles/joins; (3) new DiscordError::Rpc splits semantic rejections from transport errors, so a benign 'member not in channel' no longer tears the session down; (4) idle reconnect (15 s cadence) + Job::Warm restore the session without a click; minors: channel_id only moves after channel-scoped subscribes, user_id validated as an all-digit snowflake, user-volume requires a slider value (no silent 100% slam), MemberVoice.sent keeps the exact slider fraction over the quantized read-back, unit tests for apply_voice_state/adopt/sent.
