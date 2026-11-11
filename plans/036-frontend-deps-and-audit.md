# 036 - Clear frontend dependency advisories and add automated dependency checks

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Drift check first:** `npm audit --package-lock-only` in `apps/desktop`
> should report advisories on `happy-dom`, `vitest`, `vue-i18n`, `vite`; if
> the picture is very different, report what you see and continue with what
> still applies.

## Status

- Priority: P2 | Effort: S (tooling) + M (major bumps) | Risk: medium (three major-version bumps) | Depends on: none
- Category: dependencies / security / DX
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

`npm audit` on `apps/desktop` reports 15 advisories (3 critical, 6 high).
Most are in dev tooling (`happy-dom`, `vitest`), but 5 high ones are in what
ships to users (`vue-i18n` 9 and its `@intlify/*` packages - a release line
that no longer receives fixes). Nothing in the repo watches dependencies:
no Dependabot config, no audit step in CI.

## Current state

`apps/desktop/package.json`: deps `vue ^3.5`, `vue-i18n ^9.14.5`,
`@tauri-apps/api ^2.9`, …; devDeps `vite ^6.3`, `vitest ^3.2.7`,
`happy-dom ^18.0.1`, `@vue/test-utils ^2.5.1`, `@vitejs/plugin-vue ^5.2`.

`npm audit` fix targets at planning time: `vue-i18n` → 11.4.18 (major),
`vitest` → 5.1.0 (major), `happy-dom` → 21.0.4 (major), `vite` and
`source-map-js` → non-breaking. The 6 moderate advisories reached through
`@vue/test-utils` are "fixed" only by a *downgrade* to 2.4.11 - leave them.

`apps/desktop/src/i18n.js` uses vue-i18n in composition mode:
`createI18n({ legacy: false, fallbackFormat: true, pluralRules: { pl: plPlural }, messages … })`,
English sentences as keys, `export const t = i18n.global.t`.

`.github/workflows/ci.yml`: a frontend job on ubuntu (node 22: `npm ci`,
`npx vite build`, `npx vitest run`); Rust jobs on `windows-latest`. There is
no `.github/dependabot.yml`.

## Commands (run in `apps/desktop`)

| Purpose | Command |
| --- | --- |
| Install | `npm ci` |
| Build | `npx vite build` |
| Test | `npx vitest run` |
| Audit (all) | `npm audit` |
| Audit (shipped) | `npm audit --omit=dev --audit-level=high` |

Run Install, Build and Test **before changing anything** and record the
baseline (number of test files / tests passed). Every bump below must end
with Build and Test at least as green as the baseline.

## Scope

In scope: `apps/desktop/package.json`, `apps/desktop/package-lock.json`,
`apps/desktop/src/i18n.js` and test setup/config files **only as far as a
bump requires**, `apps/desktop/vite.config.*` / `vitest.config.*` only as
far as a bump requires, new `.github/dependabot.yml`,
`.github/workflows/ci.yml` (one added step).

Out of scope: `@vue/test-utils` version, Rust and Android dependencies,
refactoring components, changing translation files' content.

## Git workflow

One commit **per step** (so a problematic bump can be dropped on its own),
e.g. `desktop: npm audit fix (non-breaking)`, `desktop: vue-i18n 11`,
`desktop: vitest 5`, `desktop: happy-dom 21`,
`ci: dependabot and production npm audit`. No push.

## Steps

### Step 1 - Non-breaking fixes
`npm audit fix` (never `--force`). Build + Test. Commit.

### Step 2 - vue-i18n 9 → 11
`npm install vue-i18n@^11.4.18`. Read the official migration notes for v10
and v11 (`https://vue-i18n.intlify.dev/guide/migration/breaking10` and
`.../breaking11`) before touching code - do not rely on memory. The app uses
composition mode (`legacy: false`), which is the supported path. Check in
particular that `fallbackFormat`, custom `pluralRules`, and messages
containing the special characters `@`, `{`, `}`, `|` still behave: the
existing vitest suite covers plural and fallback behaviour - it must pass
unchanged. Build + Test. Commit.

### Step 3 - vitest 3 → 5
Check peers first: `npm view vitest@5.1.0 peerDependencies`. If it needs a
newer `vite` major, bump `vite` and (only if required) `@vitejs/plugin-vue`
to the lowest versions that satisfy the peers. Adjust config/test-setup API
renames only. Build + Test. Commit.

### Step 4 - happy-dom 18 → 21
`npm install -D happy-dom@^21.0.4`. Build + Test. Commit.

### Step 5 - Automation
- `.github/dependabot.yml`, version 2, weekly, with four ecosystems:
  `npm` in `/apps/desktop`, `cargo` in `/`, `gradle` in `/apps/mobile`,
  `github-actions` in `/`. Group minor+patch updates per ecosystem
  (`groups:` with `update-types: [minor, patch]`), `open-pull-requests-limit: 5`.
- In the frontend job of `ci.yml`, after `npm ci`, add a step
  `npm audit --omit=dev --audit-level=high`. Only add it if that command
  exits 0 locally after steps 1-4; otherwise do not add it and report what
  still fails.
Commit.

## Rule for failed bumps

If Build or Test is not green after **two** honest fix attempts for a bump:
`git checkout -- .` back to the previous commit state for that step (or
`git revert` if already committed), reinstall with `npm ci`, and record in
the report: the package, the failing test/build output (first 30 lines), and
what you tried. Then continue with the next step. Never delete, skip or
weaken a test to make a bump pass.

## Done criteria

- [ ] `npx vite build` and `npx vitest run` exit 0 at the final commit, with at least the baseline number of tests.
- [ ] `npm audit --omit=dev --audit-level=high` exits 0, **or** the report lists exactly what remains and why.
- [ ] `npm audit` output (summary line) before and after is quoted in the report.
- [ ] `.github/dependabot.yml` exists and is valid YAML (`node -e "require('fs').readFileSync('.github/dependabot.yml','utf8')"` plus a visual check of indentation).
- [ ] `git diff --stat <start>..HEAD` lists only in-scope files.

## STOP conditions

- No network access to the npm registry: STOP at once and report.
- A bump would require changing more than ~30 lines of application code
  outside `i18n.js` / config / test setup: revert that bump per the rule
  above and report.

## Maintenance notes

- vue-i18n's message compiler behaviour is the thing to watch in review:
  click through both languages in the running app once after merging (the
  executor cannot run the Tauri window).
- The `@vue/test-utils` moderate advisories remain by decision; Dependabot
  will propose the real fix when one exists.
