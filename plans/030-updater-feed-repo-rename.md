# 030 - Point the update feed and README at the renamed repository

> **Executor instructions.** Follow the steps in order, touch only the
> in-scope files, run each Verify. STOP conditions are binding. Do not edit
> `plans/`. **Drift check first:** confirm the excerpts below still match.

## Status

- Priority: P1 | Effort: S | Risk: low | Depends on: none
- Category: correctness / docs
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

The GitHub repository is `TymekTM/Pulpit` (`git remote -v`), and
`latest.json` (the update feed) lives at its root on `main`. The desktop app
still polls the *old* repository name `TymekTM/deckboard`. That only works
while GitHub keeps redirecting the old name; the day anything is created
under the old name, every installed copy silently stops seeing updates. The
README badges and download link have the same problem.

## Current state

`apps/desktop/src-tauri/src/lib.rs` (~:1605):

```rust
const DEFAULT_UPDATE_URL: &str =
    "https://raw.githubusercontent.com/TymekTM/deckboard/main/latest.json";
```

`resolve_update_url` (~:1610) returns this constant unless an override is
configured; tests near :2274 reference the constant by name (not by value).

`README.md` lines 9, 10 and 115 contain `github.com/TymekTM/deckboard` /
`img.shields.io/github/v/release/TymekTM/deckboard`.

Other occurrences of the word "Deckboard" in the README (the original
third-party app, `~/deckboard` data directory, `.boardjson`) are correct and
must stay.

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` |
| Find leftovers | `grep -rn "TymekTM/deckboard" --include=*.rs --include=*.md --include=*.json --include=*.yml --include=*.toml --include=*.js --include=*.vue --include=*.kt . --exclude-dir=target --exclude-dir=node_modules --exclude-dir=plans` |

## Scope

In scope: `apps/desktop/src-tauri/src/lib.rs` (the constant and one new
test), `README.md` (three lines).

Out of scope: `latest.json`, release workflows, any other mention of
"Deckboard".

## Git workflow

One commit: `Point the update feed and README at TymekTM/Pulpit`. No push.

## Steps

1. Change the constant's value to
   `https://raw.githubusercontent.com/TymekTM/Pulpit/main/latest.json`.
2. In `README.md` replace `TymekTM/deckboard` with `TymekTM/Pulpit` on the
   three lines found by the "Find leftovers" command. No other README edits.
3. In the `#[cfg(test)] mod tests` of `lib.rs` (it starts around :2098, in
   the middle of the file), next to the existing `resolve_update_url` tests,
   add:
   ```rust
   #[test]
   fn default_update_feed_points_at_the_pulpit_repo() {
       assert!(DEFAULT_UPDATE_URL.contains("/TymekTM/Pulpit/"));
   }
   ```

Verify: the "Find leftovers" command prints nothing; the Test command passes.

## Done criteria

- [ ] "Find leftovers" prints nothing.
- [ ] `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-desktop --locked` exits 0.
- [ ] `git diff --stat` for this commit lists only the two in-scope files.

## STOP conditions

- `pulpit-desktop` does not build on this machine for a reason unrelated to
  your change (missing system library): report the exact error; still make
  the edit and say the test could not be run.

## Maintenance notes

Already-installed copies keep polling the old URL until they update once, so
the old repository name must not be reused for anything.
