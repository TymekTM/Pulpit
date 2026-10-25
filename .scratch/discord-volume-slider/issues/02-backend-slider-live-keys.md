# 02 — backend + host: slider/gesture dispatch and live keys

Status: resolved
Type: task
Blocked by: 01

## Goal

Route slider input for the new action kind to `crates/discord` and push
live member state to all three surfaces through the existing
`pulpit_host::forward_producer` fan-out (flat keys → `ext.<key>` in v2,
`APP_CUSTOM_VALUE` in legacy, `customValues` in Tauri).

## Tasks

- `crates/backend/src/lib.rs`: add `exec_discord(&cmd, Some(value),
  &tile.options)` to `SqlBackend::slider`'s dispatch chain (today the
  chain ends at `exec_media`; only the tap chain reaches discord).
- Action kinds (names final in 03): `discord-user-volume` (slider,
  options `{"userId": "<snowflake>"}`) and `discord-user-mute`
  (toggle, same options) — dispatch into the new discord operations.
- Producer keys (bots-only filtering happens at the picker/cycle layer,
  not here):
  - `discord-user-volume_<user_id>`: f64 0..1 (0 when member absent)
  - `discord-user-label_<user_id>`: `"Jockie Music · 100%"` /
    `"🔇 …"` / `"offline"`; RPC errors append a short suffix.
- Throttle/coalesce producer pushes (only on change, like obs/spotify).
- Live read-back: when a `VOICE_STATE_UPDATE` changes the tracked
  user's volume, the key updates (Discord-UI edits reflect on tiles).

## Acceptance

- Unit test: slider dispatch claims `discord-user-volume` and passes
  the fraction to the discord actor; non-discord sliders unaffected.
- Producer keys appear in the v2 `state.patch` stream (existing
  producer test pattern extended).

## Comments

- Implemented: exec_discord_slider in SqlBackend::slider, shared run_discord_action (config+reauth+actor), discord_push channel through with_discord, forward_producer pumps in both hosts (apps/server + desktop src-tauri). Slider drags ride the existing 150 ms client throttle; the actor pushes state after each SET (producer lanes change-gate).

- Review round: warm_discord() added to SqlBackend and called by both hosts at startup, so tiles are live without the first action; the push-channel wiring reviewed as correct by the reviewer.
