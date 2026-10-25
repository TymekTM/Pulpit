# 03 — wire: proto + v2 builder + legacy mapper + V2.kt + fixture

Status: resolved
Type: task
Blocked by: 02

## Goal

Carry the new tile fields through BOTH wire builders and the proto
mirror, per AGENTS.md. Legacy field names are contractual (stock
Deckboard client renders exactly those fields).

## Tasks

- `crates/proto`: tile params/options for `discord-user-volume`
  (`user_id`), slider `text_state` hookup (field already exists in
  `TileSpec`), nothing new unless a field is missing.
- `crates/v2/src/boards.rs`:
  - `state_ref()` maps the kind to `ext_channel("discord-user-volume_<uid>")`
    (uid from options);
  - `text_state` → the matching `discord-user-label_<uid>` channel;
  - `slider_reset_value()` → `Some(0.5)` for this kind (100% unity on
    double-tap);
  - interactions: `slide` + `double-tap` (never custom gestures —
    sliders are excluded by `widget_kind_for`).
- `crates/legacy/src/mapping.rs`: same fields under the legacy names
  the original extension model used for sliders (`value`, text/live
  text equivalent — check how obs/spotify sliders carry live labels).
- `apps/mobile/.../proto/V2.kt`: mirror any new/changed fields by hand.
- Extend a golden fixture in `crates/proto/tests/fixtures` so
  `ProtoFixturesTest` proves the client parses the new tile shape.

## Acceptance

- `cargo test --workspace` green including the extended fixture and the
  `boards.rs` kind tests (reset value, state refs, interactions).

## Comments

- Implemented: WidgetManifest.text_state (proto + regenerated TS binding), text_state_ref + slider_reset_value 0.5 + discord_listener in the mappers, V2.kt textState, boards.sync fixture extended with a user-volume tile, Rust+Kotlin fixture tests extended. All Linux-runnable tests green.

- Review round: text_state now also emitted for user-mute (desktop/Android parity for the name cue), label channel registered with the StateEngine (shows up in welcome), user_id trimmed in the v2 mapper like the actor trims it.
