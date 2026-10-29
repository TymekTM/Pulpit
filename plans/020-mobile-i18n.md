# Plan 020: Polish/English string resources in the Android client

> Executor instructions: follow step by step; run every verification you
> can and confirm the expected result; STOP conditions are literal. The
> reviewer maintains `plans/README.md`. Do not push or open PRs.
>
> Drift check: `git diff --stat 97f96f5b..HEAD -- apps/mobile` (expected:
> empty or version-only; a mismatch with the excerpts is a STOP).

## Status
- Priority: P2 | Effort: M | Risk: LOW-MED | Depends on: none | Category: feature/i18n
- Planned at: commit `97f96f5b` (main), 2026-10-29

## Why
The Android client (Kotlin/Compose, target tablet SM-T561 / LineageOS
7.1) has ~80 `Text(...)` literals and zero localized resources: every
string is hardcoded English (plus any Polish leftovers). Full i18n =
standard Android resources: `res/values/strings.xml` (English default)
+ `res/values-pl/strings.xml` (Polish), consumed via
`stringResource(R.string.x)`. The tablet follows its system locale
automatically once `values-pl` exists — no in-app switcher (owner
decision 2026-10-29).

## Current state
- Files with UI strings (grep `Text(`, `Toast`, dialog titles):
  `apps/mobile/app/src/main/java/app/pulpit/mobile/` — MainActivity.kt
  (pairing/goodbye overlays, toasts), ui/BoardScreen.kt (board switcher
  label, empty states), ui/Tile.kt + ui/Widgets.kt (mostly renderers —
  payload text arrives pre-localized from the host; do NOT localize
  payload strings, only static chrome/labels/aria), ui/MainActivity
  routing dialogs, any settings/pairing screens under ui/.
- `res/values/` exists (drawable, font, values) — no strings.xml with
  UI text expected; check `res/values/strings.xml` for the app name.
- Resource system applies values-pl automatically on pl system locale.
- Gradle is NOT on PATH on this box (AGENTS.md). Try to enable local
  verification: `sudo pacman -S --noconfirm jdk17-openjdk gradle`
  (executor A used sudo pacman successfully today for xdotool). If that
  fails, rely on the grep gates + CI's mobile job as the compile gate.

## Steps
1. Inventory: `grep -rn 'Text("' apps/mobile/app/src/main/java` and
   every stringResource-able literal (Text, Toast, title/contentDescription,
   placeholder labels). Build the EN→PL table as you go.
2. Try `sudo pacman -S --noconfirm jdk17-openjdk gradle`; if installed,
   verify baseline: `cd apps/mobile && gradle :app:testDebugUnitTest`
   (JDK 17). If setup fails, note it and continue with gates below.
3. Create `res/values/strings.xml` (EN, `<string name="...">`) and
   `res/values-pl/strings.xml` (PL). Naming: lower_snake, grouped
   prefixes (main_, board_, pair_, tile_).
4. Replace literals with `stringResource(R.string.x)` (Compose) /
   `getString(R.string.x)` (non-composable contexts). Keep payload text
   and mDNS/protocol names as-is.
5. Gates:
   - `grep -rn 'Text("' apps/mobile/app/src/main/java | grep -v stringResource`
     → only payload-driven or numeric/format sites remain, each with a
     justifying comment
   - every `R.string.` reference has an entry in BOTH strings.xml files
     (`grep -c '<string name=' ` on each — equal counts)
   - if gradle works: `gradle :app:testDebugUnitTest` green (unit tests
     parse payloads; none assert UI literals — verify with a grep
     through app/src/test)
6. Polish translation quality: natural UI Polish (e.g. "Połącz",
   "Zapomnij parowanie", "Brak tablic"), consistent with the desktop's
   pl.json vocabulary where terms overlap (board/tablica, tile/kafel,
   pairing/parowanie).

## Scope
In: apps/mobile/app/src/main/{java, res}** only (plus nothing else).
Out: proto/, unit tests (unless a literal assertion exists — update
then), build.gradle.kts, desktop, backend.

## Done criteria
- Both resource files exist with equal entry counts; no literal Text
  sites left (gate 5)
- Unit tests green (locally if gradle available, else via CI on the PR
  — say which in the report)
- git status shows only apps/mobile files

## STOP conditions
- A string is used in a non-resource context that cannot take
  resources (protocol/mDNS) — leave it and note it.
- pacman/gradle setup breaks the worktree in any way.
- More than ~120 string sites found (scope doubled vs inventory) —
  report before continuing.

## Maintenance notes
- Reviewer (Polish speaker) checks PL quality + naming and that no
  payload string was localized.
- values-pl follows system locale on Android 7.1 — no code change for
  locale selection is wanted.
