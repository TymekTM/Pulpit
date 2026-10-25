# 04 — desktop: catalog entry, slider live value + label, member picker

Status: resolved
Type: task
Blocked by: 03

## Goal

Desktop (editor + touch mode) can add and render the new tile.

## Tasks

- `apps/desktop/src/catalog.js`: new entries `discord-user-volume`
  (mode slider, default 0.5, options `{ userId: "" }`) and
  `discord-user-mute` (toggle), placed in the Discord group next to the
  existing voice/mute/deaf actions; register in the events map if the
  kind consumes custom values.
- `TileCell.vue`:
  - `sliderValue()` branch reading
    `customValues['discord-user-volume_<uid>']` (default 0.5);
  - title/label branch showing the live
    `discord-user-label_<uid>` text when present.
- Editor options: `userId` authored via options JSON in v1 (plan-013
  precedent). Stretch: a "discord-members" picker fed by a Tauri
  command listing live members from the discord actor — only if it
  falls out naturally; hand-edited JSON is acceptable for v1.
- Touch mode must behave identically to Android (same live keys).

## Acceptance

- `npx vite build` in `apps/desktop` clean.
- Manual: add the tile with a real user id, drag the slider → Discord
  client's right-click slider follows the perceptual curve (empirical
  check of the 50 dB / +6 dB mapping), drag in Discord → tile follows.

## Comments

- Implemented: catalog entries (user-volume slider + user-mute toggle with user_id field), STATE_BINDINGS user-mute, stateActive/sliderValue/boardTileTitle/sliderResetValue branches, live label wins over the static title. vitest green (ActionPicker failures pre-exist on HEAD), vite build green.
