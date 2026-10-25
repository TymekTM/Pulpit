# 05 — android: slider caption + live value for the new kind

Status: resolved
Type: task
Blocked by: 03

## Goal

The Android client renders the member slider with its live name label
and value, mirroring desktop behavior.

## Tasks

- `proto/V2.kt` mirror lands with 03; here: `SliderTile` (Tile.kt
  ~489–592) currently renders bar + icon only — pass the tile's
  `liveText` (already resolved from `text_state` at the TileView
  level) into `SliderTile` and render it as a compact caption.
- Render the caption only when `text_state` is set, so existing
  speaker/spotify/VM sliders stay pixel-identical.
- Percent readout while dragging uses the same 0..1 → 0–200% scale as
  the label (server label is the source of truth when idle).
- `discord-user-mute` renders through the existing toggle path (verify
  the discordKind "OFF" muted styling applies to a plain toggle).

## Acceptance

- `gradle :app:testDebugUnitTest` green (JDK 17).
- Unit test for the caption-visible-only-with-text_state logic where
  testable; visual check on a device against the fixture board.

## Comments

- Implemented: V2.kt textState field, BoardScreen resolves the text_state channel, SliderTile renders the label caption only when a text_state label arrives (existing sliders pixel-identical). Gradle tests must run on the owner's machine (no JVM here).
