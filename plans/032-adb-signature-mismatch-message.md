# 032 - Explain known `adb install` failures instead of dumping raw adb text

> **Executor instructions.** Follow the steps in order, touch only the
> in-scope file, run each Verify. STOP conditions are binding. Do not edit
> `plans/`. **Drift check first:** confirm the excerpt below still matches.

## Status

- Priority: P3 | Effort: S | Risk: low | Depends on: 031 (same function neighbourhood; run after it in the same checkout)
- Category: UX / correctness
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

Updating the tablet app over USB fails with a wall of adb output when the
tablet has a build signed with a different key (a developer build vs. a
release APK): `INSTALL_FAILED_UPDATE_INCOMPATIBLE`. The user cannot tell
what to do. The fix is always the same - uninstall the app on the tablet,
install again, pair again - so the app should say so.

## Current state

`apps/desktop/src-tauri/src/lib.rs`, `adb_install` (~:1555-1569): calls
`run_adb(&["-s", serial, "install", "-r", path])`; on failure it returns the
raw adb text, or `"adb install nie powiodł się (brak wyjścia)"` when adb
printed nothing. `install_tablet_update` (~:1964-2044) wraps the error as
`"{serial}: błąd - {e}"`. User-facing strings in this file are Polish for
now (plan 033 makes them bilingual).

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` |

## Scope

In scope: `apps/desktop/src-tauri/src/lib.rs` - a new pure helper, its use
in `adb_install`, tests in the existing test module (~:2098).

Out of scope: automatically uninstalling the app (it deletes the tablet's
pairing and settings - must stay a human decision), the frontend.

## Git workflow

One commit: `Explain signature-mismatch and downgrade failures of adb install`. No push.

## Steps

### Step 1 - Pure helper

```rust
/// A human explanation for the `adb install` failures a user can act on;
/// `None` for everything else (the raw adb text is shown then).
fn explain_adb_install_failure(output: &str) -> Option<&'static str> {
    if output.contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE") {
        return Some("Na tablecie jest Pulpit podpisany innym kluczem (np. wersja deweloperska). Odinstaluj aplikację na tablecie, zainstaluj ponownie i sparuj tablet od nowa.");
    }
    if output.contains("INSTALL_FAILED_VERSION_DOWNGRADE") {
        return Some("Na tablecie jest nowsza wersja niż instalowana. Odinstaluj aplikację na tablecie, jeśli chcesz wrócić do starszej.");
    }
    if output.contains("INSTALL_FAILED_INSUFFICIENT_STORAGE") {
        return Some("Na tablecie brakuje miejsca na instalację.");
    }
    None
}
```

### Step 2 - Use it

In the failure branch of `adb_install`: when the helper returns `Some(msg)`,
return `Err(format!("{msg}\n\n{raw}"))` where `raw` is the adb text returned
today (so the diagnostic detail is not lost). Otherwise behaviour is
unchanged.

### Step 3 - Tests

Add `adb_install_failures_are_explained` covering the three markers (use
realistic lines such as
`adb: failed to install app.apk: Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE: Existing package app.pulpit.mobile signatures do not match newer version; ignoring!]`)
and one unknown failure returning `None`. Assert on `is_some()` / `is_none()`
and on a stable keyword, not on whole sentences.

Verify: Test command passes.

## Done criteria

- [ ] `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` exits 0.
- [ ] `grep -c "INSTALL_FAILED_UPDATE_INCOMPATIBLE" apps/desktop/src-tauri/src/lib.rs` ≥ 2.
- [ ] `git diff --stat` for this commit lists only `apps/desktop/src-tauri/src/lib.rs`.

## STOP conditions

- `adb_install` no longer returns the raw adb text on failure (code drifted):
  report what it does now.

## Maintenance notes

Release APKs are signed with whatever key the release workflow uses; if that
key ever changes, every user hits this message once - mention it in release
notes.
