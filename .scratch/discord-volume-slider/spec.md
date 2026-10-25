# Discord voice member volume (music bots)

Status: spec ready — research delivered in `api-docs.md` (Gemini
`gemini-discord-volume-docs-r1`), key claims verified against the code.
Implementation issues filed under `issues/`.

## Feature

Control the volume of a voice-channel member from a Pulpit tile, targeted
at music bots. "Volume of a member" means the **local per-user volume
override** the Discord desktop client applies to that member's stream
(each bot is just a member; its perceived loudness is that override).

## Decisions (2026-10-25, with owner)

- **One slider tile per bot** — tile shows the bot's display name plus a
  volume slider. No on-tile switching; swipe-based switching was rejected
  because slider/knob tiles never receive custom gestures (the drag
  surface belongs to the value, `docs/protocol-v2.md` §6). Gestures
  (swipe-left/right) ARE confirmed supported end-to-end (M5, `1bc074d`)
  for button-like tiles if we ever revisit switching.
- **Bots only** — the member list is filtered to bot accounts (bot badge);
  no per-human sliders.

## Known constraints

- `crates/discord` today speaks only `GET_VOICE_SETTINGS`,
  `SET_VOICE_SETTINGS`, `SELECT_VOICE_CHANNEL` over the local RPC pipe.
  Whether the documented RPC can set a per-user volume override is the
  core open question — see `api-docs.md`.
- Member enumeration must come from voice-state events/payloads
  (`VOICE_STATE_CREATE/UPDATE/DELETE`) with the bot flag.
- Live volume feedback would ride the v2 live-state fan-out
  (`discord.*` channels, `docs/protocol-v2.md` §7/M7).

## Surfaces checklist (AGENTS.md "one change, three surfaces")

- [ ] `crates/discord`: new RPC command(s) in `plan()`, member tracking.
- [ ] Action kind + params in `apps/desktop/src/catalog.js`; editor UI.
- [ ] Wire fields in BOTH builders: `crates/legacy/src/mapping.rs` and
      `crates/v2/src/boards.rs`; `crates/proto` types; hand-mirror in
      `apps/mobile/.../proto/V2.kt`; golden fixture update.
- [ ] Mobile: slider tile rendering with member name; live state.
- [ ] Verification: `cargo test --workspace` + clippy, `npx vite build`,
      `gradle :app:testDebugUnitTest` when Kotlin moves.

## Open questions

1. ~~Exact mechanism to set per-user volume~~ RESOLVED: official,
   documented `SET_USER_VOICE_SETTINGS` (`user_id`, `volume` 0..=200,
   `mute`, `pan`), scope `rpc.voice.write` — already in `SCOPES`.
   Members come from `GET_SELECTED_VOICE_CHANNEL` + channel-scoped
   `VOICE_STATE_CREATE/UPDATE/DELETE` subscriptions (needs a new
   top-level `evt` field on SUBSCRIBE frames — `Conn::request_until`
   today cannot send it).
2. ~~Which voice-state payload fields carry the bot flag~~ RESOLVED:
   `data.user.bot` (bool), display name = `nick` → `user.global_name` →
   `user.username`; per-user local volume/mute ride on every voice state.
3. Slider value mapping — DECIDED: wire carries the usual 0..1 fraction;
   Discord's native 0–200% range maps linearly onto it, so 0.5 = 100%
   (unity). Double-tap reset = 0.5. Conversion between the fraction and
   the RPC's amplitude units goes through the perceptual curve
   (`api-docs.md` §3.1) in `crates/discord`, unit-tested.
4. Bot leaves channel / absent from VC — tile label falls back to
   "offline"/"No bot" state via the label live key; slider becomes inert
   (no error spam).
5. Single-modifier voice lock (e.g. Elgato Stream Deck plugin running
   concurrently): log the RPC error and append it to the label key;
   `DiscordClient`'s persistent pipe keeps overrides alive otherwise.
   Note: RPC-set user volumes revert when the pipe closes — expected.

## Research results (2026-10-25, `api-docs.md`, verified)

- `SET_USER_VOICE_SETTINGS` is the documented per-user volume command;
  Pulpit's BYO-app OAuth already holds the needed scopes, so no
  re-authorization. Per-user receive volume is client-side attenuation
  (SFU forwards one Opus stream to everyone) — a music bot cannot do
  this per-listener itself, and bot `/volume` commands are mostly
  paywalled and change loudness for everyone.
- `VOICE_STATE_UPDATE` payloads carry fractional `volume` (amplitude
  units, e.g. `5.62`), while the Discord UI shows perceptual %: the
  `@discordapp/perceptual` curve (50 dB span below 100%, +6 dB boost
  above). The exact mapping must be confirmed once against a live
  client before shipping (read-back test), but the evidence is strong.
- Caveats: only one RPC app may mutate voice settings at a time;
  RPC-set volumes revert when the pipe disconnects (Pulpit's connection
  is long-lived, so this only matters on exit).
- v1 UX follows the owner's call: pinned per-bot sliders only
  (`{"userId": "<snowflake>"}` options, bots-only lists), the dynamic
  "selected target" tile with swipe switching is deferred. Tile
  authoring via options JSON follows the plan-013 precedent (editor UI
  deliberately out of scope for v1).
