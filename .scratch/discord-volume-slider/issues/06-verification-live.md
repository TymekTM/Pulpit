# 06 — verification: three surfaces + live Discord read-back

Status: open
Type: task
Blocked by: 02, 03, 04, 05

## Goal

Close the loop per AGENTS.md: no surface passes alone.

## Tasks

- `cargo test --workspace` and `cargo clippy --workspace --all-targets`
  from the repo root.
- `npx vite build` in `apps/desktop`; exercise touch mode with the new
  tile.
- `gradle :app:testDebugUnitTest` in `apps/mobile` (JDK 17) if Kotlin
  moved (it will: V2.kt, Tile.kt).
- Live Discord empirical checks (owner's machine, real client):
  1. Volume-curve read-back: set amplitude 6 via RPC, confirm the
     Discord UI slider shows 50% (validates the perceptual mapping
     from `api-docs.md` §3.1 before it ships).
  2. Slider drag in Discord UI → tile label/value updates
     (`VOICE_STATE_UPDATE` fan-out).
  3. Bot leaves the channel → label falls back, slider inert, no error
     spam; bot rejoins → tile recovers.
  4. With a second RPC voice controller running (Stream Deck plugin),
     confirm the error surfaces in the label suffix and Pulpit
     recovers when the other app disconnects.

## Acceptance

- All checks above pass and are noted in this file's Comments.

## Comments

- 2026-10-25: everything runnable in this environment is green - cargo check + clippy clean for the whole workspace on the windows-gnu target (cross-check, no wine here so test binaries do not run locally), fmt clean, 255 legacy/v2/proto tests pass on Linux (with a stub libxdo for linking), vitest 417/433 (ActionPicker failures pre-exist on HEAD), vite build ok. Remaining: gradle unit tests + the live Discord checks below on the owner's Windows machine.
