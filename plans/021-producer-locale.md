# Plan 021: Locale-aware media/Spotify payload texts (host locale)

> Executor instructions: follow step by step; run every verification;
> STOP conditions are literal. The reviewer maintains `plans/README.md`.
> Do not push or open PRs.
>
> Depends on plan 019 (the settings save path and SettingsOverlay exist
> there; this plan only ADDS one call + one Tauri command).
>
> Drift check: `git diff --stat <019-merge>..HEAD -- crates/os/src/media.rs crates/spotify/src/snapshot.rs`

## Status
- Priority: P2 | Effort: M | Risk: MED | Depends on: 019 | Category: feature/i18n
- Planned at: commit `97f96f5b` (main), 2026-10-29

## Why
Status payloads carry display-ready strings (the stock Deckboard client
renders values verbatim — the wire cannot carry keys). The media idle
text is Polish ("Nic nie jest odtwarzane", unified in PR #26) and the
Spotify login hint is English, regardless of the user's language. Full
i18n (owner decision 2026-10-29): these producer texts localize by the
HOST's locale — desktop passes its UI language, the headless server
reads a config key. Scope: ONLY the media/Spotify user-facing texts;
sysinfo/aidev/tools titles stay English (owner decision).

## Design (binding)
- New tiny crate `crates/i18n`: process-global locale in a
  `RwLock<String>` (NOT OnceLock — tests must reset it),
  `set_locale(&str)` / `locale() -> String` (default `"en"`), plus
  display-string helpers `media_idle()`, `media_idle_title()`,
  `spotify_idle()`, `spotify_login_hint()` returning per-locale strings
  (en + pl; unknown locale → en).
- `crates/os/src/media.rs build_snapshot`: idle rows/compact/summary
  from `pulpit_i18n::media_idle()` (replaces the hardcoded Polish; the
  "Multimedia" title from `media_idle_title()`).
- `crates/spotify/src/snapshot.rs`: idle "Nothing playing"/"Nic nie
  jest odtwarzane" and "Log in via Pulpit settings" texts from
  `pulpit_i18n::spotify_idle()` / `spotify_login_hint()`.
- Desktop (`apps/desktop/src-tauri/src/lib.rs`): new Tauri command
  `set_host_lang(locale)` calling `pulpit_i18n::set_locale`; called at
  startup (settings `lang`, resolved the same way the frontend resolves
  it: not-"auto" → that value, else system-locale detection server-side
  is unreliable — the FRONTEND sends its resolved locale on startup and
  on change; keep Rust dumb) and from the frontend's `setUiLang` (the
  019 helper gains one `api.setHostLang(resolved)` call).
- Server (`apps/server/src/main.rs`): read `"lang"` from the
  pulpitApp `settings.json` at startup (`"auto"`/missing → `"en"` on a
  headless box) and `pulpit_i18n::set_locale` before spawning producers.
- Producers read the global per snapshot build — a live language change
  reaches the next push without restart.

## Steps
1. `crates/i18n` (lib + tests: default en, set/get, reset helper,
   all five helpers per locale, unknown-locale fallback).
   Verify: `cargo test -p pulpit-i18n`.
2. media.rs: swap idle strings; update tests — set locale pl for the
   existing Polish assertions (`build_snapshot_idle_shape`) + add en
   variants. Verify: `cargo test -p pulpit-os`.
3. spotify snapshot.rs: same treatment (idle + login hint; the
   needs-login payload). Verify: `cargo test -p pulpit-spotify`.
4. Desktop: command + startup call + frontend hook in `setUiLang`
   (api.js gains `setHostLang`). Verify: `cargo check -p pulpit-desktop`
   fails on Linux by design — use
   `cargo check -p pulpit-desktop --target x86_64-pc-windows-msvc`
   with the fakecc stubs (see 017/018 reports) + `npx vitest run` +
   `npx vite build` in apps/desktop.
5. Server: config read + set before producers. Verify:
   `cargo check -p pulpit-server`.
6. Full: `cargo test -p pulpit-i18n -p pulpit-os -p pulpit-spotify`,
   clippy on the three crates, `cargo fmt --check -p pulpit-i18n
   -p pulpit-os -p pulpit-spotify`.

## Scope
In: crates/i18n/** (new), crates/os/src/media.rs,
crates/spotify/src/snapshot.rs (+ Cargo.toml workspace members list +
both crates' Cargo.toml deps), apps/desktop/src-tauri/src/lib.rs (one
command + one startup call), apps/desktop/src/i18n.js (one api call in
setUiLang), apps/desktop/src/api.js (one method), apps/server/src/main.rs
(config read + set).
Out: docs/protocol-v2.md (values are display-ready strings; the wire
shape is unchanged), mobile (renders payloads verbatim), every other
producer's strings.

## Done criteria
- cargo test green for the three crates; vitest + vite build green
- A lang switch on the desktop changes the NEXT idle payload text
  (unit-test the producer side with locale set; the end-to-end behavior
  follows from the same code path)
- git status shows only in-scope files

## STOP conditions
- The 019 settings save path does not expose a hookable setUiLang.
- Workspace member registration for a new crate differs from the
  pattern in the root Cargo.toml beyond adding one line.
- Any verification fails twice.

## Maintenance notes
- Reviewer: locale is RwLock (not OnceLock) precisely for testability;
  keep `set_locale` cheap and infrequent.
- Future producers that want localized texts add helpers to
  crates/i18n — do not sprinkle match-locale code through producers.
