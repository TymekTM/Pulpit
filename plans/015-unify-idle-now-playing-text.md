# Plan 015: Unify the idle now-playing text to Polish in both producers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in the "STOP conditions" section occurs,
> stop and report — do not improvise. When done, update the status row
> for this plan in `plans/README.md` — unless a reviewer dispatched you
> and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 6dcddf3..HEAD -- crates/spotify/src/snapshot.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug (UX consistency)
- **Planned at**: commit `6dcddf3`, 2026-10-29

## Why this matters

Two producers feed the same now-playing tile kind: the dedicated Spotify
Web API producer (`spotify-now-playing`) and the system-media SMTC
producer (`media-now-playing`). When nothing plays, the SMTC producer
says **"Nic nie jest odtwarzane"** (Polish) while the Spotify producer
says **"Nothing playing"** (English). The desktop app's UI is
Polish-facing (see the Polish editor strings, e.g. `apps/desktop/src/components/EditTileModal.vue`
`"Aplikacja (opcjonalnie)"`), and the owner decided (2026-10-29) to
unify on the Polish string so an idle tile reads the same no matter
which producer is behind it.

## Current state

- `crates/spotify/src/snapshot.rs` — builds the `spotify-now-playing`
  status payload. The English idle strings live in two places inside
  `now_playing_payload` (lines ~113-169):

  ```rust
  // line ~120: no player at all
  let Some(player) = player else {
      payload["rows"] = json!([{ "label": "Status", "value": "Nothing playing" }]);
      payload["compact"] = json!("Nothing playing");
      payload["summary"] = json!("Nothing playing");
      return payload;
  };
  ```

  ```rust
  // lines ~143-147: a player object without an item (ad breaks report no item)
  if rows.is_empty() {
      rows.push(json!({ "label": "Status", "value": "Nothing playing" }));
      payload["compact"] = json!("Nothing playing");
      payload["summary"] = json!("Nothing playing");
  }
  ```

  In-file tests assert the English strings (`nothing_playing_and_needs_login_shapes`,
  line ~385: `assert_eq!(payload["rows"][0]["value"], "Nothing playing");`).

- `crates/os/src/media.rs` — the SMTC producer already uses the target
  Polish string (`"Nic nie jest odtwarzane"`, lines ~302-304 and its
  in-file test ~428-431). **Do not touch this file**; it is the
  reference.

- Repo convention for display strings: they are display-ready values on
  the payload (`rows`/`compact`/`summary`), rendered verbatim by every
  client; only this crate's builder decides them.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Tests | `cargo test -p pulpit-spotify` | all pass, incl. the updated assertions |
| Grep gate | `grep -rn "Nothing playing" crates/` | no matches |

## Scope

**In scope** (the only files you should modify):
- `crates/spotify/src/snapshot.rs` (idle strings + the in-file test
  assertions that pin them)

**Out of scope** (do NOT touch, even though they look related):
- `crates/os/src/media.rs` — already Polish; it is the reference.
- `apps/mobile/app/src/test/java/app/pulpit/mobile/ui/StatusDataTest.kt`
  — uses `"Nothing playing"` as an arbitrary parser sample, not as the
  producer contract; the test stays valid either way.
- The `"Log in via Pulpit settings"` strings in the same file — a
  different message, deliberately left as-is this round.
- Any translation/locale machinery — none exists; strings are literals.

## Git workflow

- Branch: `advisor/015-unify-idle-text`
- One commit, message: `Unify the idle now-playing text on the Polish string`
  (repo style: short imperative subject, see `git log --oneline -5`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Replace the idle strings in `now_playing_payload`

In `crates/spotify/src/snapshot.rs`, replace all six occurrences of
`"Nothing playing"` (three in the `player else` branch, three in the
`rows.is_empty()` branch) with `"Nic nie jest odtwarzane"`. Keep the
`"Status"` row label unchanged — both producers use that label and it
stays consistent.

### Step 2: Update the in-file test assertions

In the same file's `#[cfg(test)] mod tests`, change every assertion that
pins `"Nothing playing"` (in `nothing_playing_and_needs_login_shapes`
and, if present, `now_playing_payload_shape`) to
`"Nic nie jest odtwarzane"`.

**Verify**: `cargo test -p pulpit-spotify` → all pass.

**Verify**: `grep -rn "Nothing playing" crates/` → no matches.

## Test plan

- No new tests: the existing `nothing_playing_and_needs_login_shapes`
  already pins the idle payload's rows/compact/summary; it becomes the
  regression guard for the Polish string.
- Verification: `cargo test -p pulpit-spotify` → all pass.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo test -p pulpit-spotify` exits 0
- [ ] `grep -rn "Nothing playing" crates/` returns no matches
- [ ] `grep -rn "Nic nie jest odtwarzane" crates/spotify/src/snapshot.rs`
      returns 6 payload occurrences (plus assertions)
- [ ] `git status` shows only `crates/spotify/src/snapshot.rs` modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the
  excerpts (the file drifted since `6dcddf3`).
- Any other crate/test asserts the English idle string in a way the
  grep gate trips outside `apps/mobile` tests.
- A step's verification fails twice after a reasonable fix attempt.

## Maintenance notes

- The needs-login hint (`"Log in via Pulpit settings"`) is still English
  by deliberate scope cut; if the owner wants full Polish, that is a
  one-string follow-up in the same file.
- Reviewer: confirm no client-side code matched on the literal English
  string (the grep gate in Done criteria covers this).
