# 033 - Backend messages shown in the UI follow the UI language

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Drift check first:** confirm the excerpts below still match.

## Status

- Priority: P2 | Effort: M | Risk: low-medium (many small edits in one 3900-line file) | Depends on: 030, 031, 032 (same file; run after them in the same checkout)
- Category: correctness / UX (i18n)
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

The desktop UI is bilingual (English / Polish, user-selectable), but
messages produced by the Rust backend ignore that choice: about 48 string
literals in the desktop backend and 12 in the OBS client are Polish only, and
Spotify's three hint messages are English only. An English-speaking user
gets Polish error toasts; a Polish user gets English Spotify hints.

## Current state

### The locale plumbing that already exists

`crates/i18n/src/lib.rs` keeps a process-global locale (`locale()`,
`set_locale`, `reset_locale`) and a **private** picker (~:44):

```rust
/// Polish when the locale says so, English otherwise (any unknown
/// locale included).
fn pick(en: &'static str, pl: &'static str) -> &'static str {
    if locale() == "pl" {
        pl
    } else {
        en
    }
}
```

Public helpers such as `media_idle_title()` are built on it. The frontend
reports its language at startup and on every change through the `set_lang`
Tauri command (`apps/desktop/src-tauri/src/lib.rs` ~:2978 calls
`pulpit_i18n::set_locale(locale)`), so the backend always knows the UI
language. Tests in the i18n crate serialize on a `LOCALE_GUARD` mutex because
the locale is process-global.

### The untranslated strings

- `apps/desktop/src-tauri/src/lib.rs`: Polish literals returned to the UI in
  `Err(...)`, `format!(...)` and result lines, e.g.
  `"adb nie znaleziony w PATH: {e}"`,
  `"adb install nie powiodł się (brak wyjścia)"`,
  `"{serial}: już v{v}"`, `"{serial}: zainstalowano v{}"`,
  `"{serial}: błąd - {e}"`, plus update, Discord-auth, import/export and
  extension messages (around lines 1565, 1658-1674, 1708-1809, 1857-1941,
  1996-2017, 3093-3099, 3200, 3348-3428, 3575, 3764), plus whatever plans
  031/032 added.
- `crates/obs/src/client.rs`: 12 Polish literals (around :301, :485-:509,
  :554-:560, :596, :965). `crates/obs/Cargo.toml` does **not** depend on
  `pulpit-i18n` yet.
- `crates/spotify/src/error.rs`: `user_message(err)` (~:51) returns
  `err.to_string()` - English only. Its tests assert
  `"Log in to Spotify in Pulpit settings"`, `"Spotify Premium required"`,
  `"Open Spotify on a device first"`. `crates/spotify/Cargo.toml` already
  depends on `pulpit-i18n`.

Find candidates with:

```
grep -nP '"[^"]*[ąćęłńóśźżĄĆĘŁŃÓŚŹŻ][^"]*"' apps/desktop/src-tauri/src/lib.rs crates/obs/src/client.rs
grep -nP '"[^"]*\b(nie|brak|błąd|już|zainstalowano|nie udało|wybierz|plik)\b[^"]*"' apps/desktop/src-tauri/src/lib.rs crates/obs/src/client.rs
```

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test touched crates | `CARGO_BUILD_JOBS=3 timeout 1500 cargo test -p pulpit-i18n -p pulpit-obs -p pulpit-spotify -p pulpit-desktop --locked` |
| Lint | `CARGO_BUILD_JOBS=3 cargo clippy -p pulpit-i18n -p pulpit-obs -p pulpit-spotify -p pulpit-desktop --all-targets --locked` |

## Scope

In scope:
- `crates/i18n/src/lib.rs`
- `apps/desktop/src-tauri/src/lib.rs` (string literals that reach the UI, and tests that assert on them)
- `crates/obs/src/client.rs`, `crates/obs/Cargo.toml`
- `crates/spotify/src/error.rs`
- `Cargo.lock` only if cargo rewrites it for the new path dependency

Out of scope - do not touch:
- `tracing::…` log messages and code comments (logs stay as they are).
- The Vue frontend and its translation files.
- `crates/discord`, `crates/backend`, `crates/v2`, `crates/legacy`.
- Wire payloads sent to tablets.

## Git workflow

One commit: `Backend UI messages follow the UI language`. No push.

## Steps

### Step 1 - Public API in `pulpit-i18n`

1. Add `pub fn tr(en: &'static str, pl: &'static str) -> &'static str` with
   the body of `pick`, and make `pick` call it (or replace `pick` by `tr`
   everywhere in the crate - your choice, keep the diff small).
2. Add a formatting macro for messages with arguments:
   ```rust
   /// `format!` in the UI language: `trf!("{n} tiles", "{n} kafelków")`.
   #[macro_export]
   macro_rules! trf {
       ($en:literal, $pl:literal $(, $($arg:tt)*)?) => {
           if $crate::locale() == "pl" {
               format!($pl $(, $($arg)*)?)
           } else {
               format!($en $(, $($arg)*)?)
           }
       };
   }
   ```
   If inline captures (`{serial}`) do not compile through the macro, pass
   the arguments explicitly (`trf!("{}: x", "{}: y", serial)`).
3. Add a test for `tr` and `trf!` in the crate's test module, taking
   `LOCALE_GUARD` exactly like the neighbouring tests and calling
   `reset_locale()` at the end.

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-i18n --locked` passes.

### Step 2 - Desktop backend

For every UI-reaching Polish literal in `apps/desktop/src-tauri/src/lib.rs`:
- no arguments → `pulpit_i18n::tr("English text", "Polish text")` (add
  `.to_string()` / `.into()` where a `String` is needed);
- with arguments → `pulpit_i18n::trf!("English {x}", "Polish {x}")`.

Rules:
- The Polish text stays **byte-identical** to today's literal.
- English: faithful, sentence case, same punctuation and placeholders, same
  tone as the frontend (short, no exclamation marks). Product terms: "tile",
  "board", "tablet", "extension", "pairing".
- Do not translate strings that are identifiers, file names, event names,
  JSON keys or log messages.
- Tests in this file that assert on a Polish message: make them
  locale-independent (assert on a stable fragment such as the serial, the
  version or an error code) rather than calling `set_locale` - the locale is
  process-global and tests run in parallel.

Verify: `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` passes.

### Step 3 - OBS client

Add `pulpit-i18n = { path = "../i18n" }` to `crates/obs/Cargo.toml`
`[dependencies]` and convert the 12 literals the same way. If an existing
OBS test asserts a Polish message, make it locale-independent as above.

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-obs --locked` passes.

### Step 4 - Spotify hints

Open `crates/spotify/src/error.rs` and find where the three hint texts are
defined. Make `user_message` return the UI-language text for those three
cases (English text unchanged; Polish: `"Zaloguj się do Spotify w ustawieniach Pulpit"`,
`"Wymagane Spotify Premium"`, `"Najpierw otwórz Spotify na jakimś urządzeniu"`).
Leave the `Display`/`thiserror` messages (used in logs) English. Existing
tests assert the English text with the default locale: keep them passing; if
this crate's tests ever call `set_locale`, check
(`grep -n "set_locale" -r crates/spotify`) and follow that file's existing
serialization pattern for any new Polish assertion.

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-spotify --locked` passes.

## Test plan

- `tr` / `trf!` unit test (step 1).
- One test in `crates/spotify/src/error.rs` for the Polish variant only if a
  locale serialization pattern already exists in that crate; otherwise skip
  and say so in the report.
- All existing tests of the four crates stay green.

## Done criteria

- [ ] The Test command exits 0; Lint shows no new warnings in touched files.
- [ ] `grep -nP '"[^"]*[ąćęłńóśźżĄĆĘŁŃÓŚŹŻ][^"]*"' apps/desktop/src-tauri/src/lib.rs crates/obs/src/client.rs | grep -v "tr(" | grep -v "trf!" | grep -v "^\S*:\s*//"` - every remaining line is either a continuation line of a `tr(`/`trf!` call, a test, or a log message; list the remaining lines in the report with the reason.
- [ ] `git diff --stat` for this commit lists only in-scope files.
- [ ] Report includes the count of converted literals per file.

## STOP conditions

- A message is matched by the frontend on its exact text
  (`grep -rn "<polish text>" apps/desktop/src` finds it): leave that literal
  untouched and list it in the report.
- A Polish literal is part of a wire payload or stored data: leave it and
  report.

## Maintenance notes

New backend messages must use `tr`/`trf!`. `crates/discord` user-facing
messages were not audited and may need the same treatment later.
