# Plan 019: Full Polish/English i18n in the desktop UI

> Executor instructions: follow step by step; run every verification and
> confirm the expected result; STOP conditions are literal. The reviewer
> maintains `plans/README.md` — do not touch it. Do not push or open PRs.
>
> Drift check: `git diff --stat 97f96f5b..HEAD -- apps/desktop` (expected:
> possibly only the version bump from the release PR; a real mismatch
> with the excerpts below is a STOP).

## Status
- Priority: P1 | Effort: L | Risk: MED | Depends on: none | Category: feature/i18n
- Planned at: commit `97f96f5b` (main), 2026-10-29

## Why
The desktop UI is mixed-language today: English chrome with Polish
patches (e.g. `Aplikacja (opcjonalnie)` in EditTileModal, Polish strings
in SettingsOverlay sections, ConfirmDialog, OperatorAskModal,
GridEditor, ActionPicker — 10 files contain Polish diacritics). The
owner wants full pl/en with system-locale detection and a manual
override persisted in `settings.json`.

## Design decisions (binding)
1. **vue-i18n@9** (standard Vue 3 runtime). Install with npm; commit
   package.json + package-lock.json.
2. **English-as-key (gettext-style)**: normalize every user-visible
   literal to ENGLISH as the source, wrap in `t('English text')`.
   `locales/en.json` stays effectively empty (identity fallback),
   `locales/pl.json` maps EN→PL. Missing PL → EN shows (graceful).
   Rationale: `catalog.js` has ~198 `label:` entries — data stays
   untouched, only the RENDER sites call `t(label)`.
3. **Locale resolution**: stored `settings.json` `lang` (`"auto"|"pl"|"en"`)
   → else `navigator.language` startsWith `pl` → `pl`, else `en`.
   A language select in SettingsOverlay persists the choice through the
   EXISTING settings save path (find it: SettingsOverlay writes via the
   settings API — grep `saveSettings|settings_save|writeSettings` in
   src/api.js and src-tauri).
4. Polish text lives ONLY in `locales/pl.json` after this plan (grep
   gate below) — except `apps/desktop/src/locales/pl.json` itself.
5. Tile payload strings (e.g. the media idle text) are NOT part of this
   plan — they come from the backend and are handled by plan 021.

## Current state
- Components (apps/desktop/src/components): ActionPicker.vue,
  BoardModal.vue, ConfirmDialog.vue, EditTileModal.vue (has
  `<span class="sel-label">Aplikacja (opcjonalnie)</span>`),
  GridEditor.vue, OperatorAskModal.vue, SelectField.vue,
  SettingsOverlay.vue, TileCell.vue (mostly renderer; check aria/
  tooltips only), settings/ (DiscordSettings, ObsSettings,
  VoicemeeterSettings + any siblings in that dir), tiles/ToolTile.vue.
- App.vue (~2143 lines) — menus, tray labels, dialogs.
- catalog.js — ~198 `label:` entries, consumed for display via
  typeMeta/ActionPicker (render sites wrap with `t()`).
- Tests asserting literals: tests/SettingsOverlay.test.js (53),
  EditTileModal.test.js (38), modals.test.js, ActionPicker.test.js (16),
  GridEditor.test.js (28), App.test.js (41). With EN-as-key and test
  locale pinned to `en`, assertions on today's ENGLISH strings keep
  passing; assertions on today's POLISH strings must be updated to the
  EN source (or set locale pl explicitly when the test targets Polish).
- main.js mounts the app; wire createI18n there.

## Steps
1. `npm install vue-i18n@9` in apps/desktop. Create
   `src/locales/en.json` (`{}`), `src/locales/pl.json` (`{}` initially),
   `src/i18n.js` exporting the i18n instance (locale resolution per
   design 3), a `setUiLang(lang)` helper that persists `lang` through
   the existing settings API and switches `i18n.global.locale`, plus a
   test helper `__setLocaleForTests(lang)`. Mount plugin in main.js.
   In `tests/setup.js` pin locale `en` before every test.
   Verify: `npx vitest run` → all pass still (no strings migrated yet).
2. SettingsOverlay + settings/* components: migrate literals to
   `t(...)`, add the language select (auto/pl/en) that calls
   `setUiLang`. Fill pl.json entries for everything in these files.
   Verify: `npx vitest run tests/SettingsOverlay.test.js` → green
   (update PL-asserting tests to EN source or pl-locale).
3. Modals: EditTileModal (incl. `Aplikacja (opcjonalnie)` →
   `t('App (optional)')`), ActionPicker (+ catalog `label` rendering
   through `t`), ConfirmDialog, OperatorAskModal, BoardModal,
   SelectField. Verify: vitest for those files.
4. App.vue + GridEditor + tiles/ToolTile + TileCell (user-visible
   chrome only; payload strings untouched). Verify: full
   `npx vitest run`.
5. Complete pl.json sweep: every `t('...')` key has a pl entry.
   Verify gates:
   - `grep -rn "[ąćęłńóśźż]" apps/desktop/src --include="*.vue" --include="*.js" | grep -v locales`
     → NO matches (Polish only in pl.json)
   - `npx vitest run` → 436+ pass
   - `npx vite build` → success
   - `npx vue-tsc --noEmit` NOT configured — skip (no ts in project).
6. New test: switching locale to pl changes a rendered string (e.g.
   settings header) — model after an existing SettingsOverlay test.

## Scope
In: apps/desktop (src/**, tests/**, package.json, package-lock.json).
Out: backend crates, apps/mobile, docs/protocol (no wire change), tile
payload strings (plan 021), README.

## Done criteria
- vitest all green; vite build green
- Polish diacritics grep gate clean
- vue-i18n wired; language select persists via settings.json `lang`
- git status shows only apps/desktop files

## STOP conditions
- The settings persistence API cannot be found/used for a `lang` key.
- A test cannot be made deterministic under a pinned locale.
- vue-i18n@9 conflicts with the Vue/Vite versions in package.json.
- Verification fails twice on one step.

## Maintenance notes
- Reviewer checks: no literals bypass t(); pl.json coverage complete
  (grep every `t('` key); the language select round-trips through
  settings.json; tests pinned to en stay deterministic.
