# Plan 016: Correct the stale multi-range doc in `assets.rs`

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in the "STOP conditions" section occurs,
> stop and report — do not improvise. When done, update the status row
> for this plan in `plans/README.md` — unless a reviewer dispatched you
> and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 6dcddf3..HEAD -- crates/v2/src/assets.rs`
> If the file changed since this plan was written, compare the
> "Current state" excerpt against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `6dcddf3`, 2026-10-29

## Why this matters

`plans/README.md` (Deferred section, "Round: findings 9/11/12/13")
records a known doc drift: multi-range `Range` requests are refused
with HTTP 416, but the doc comment on `parse_byte_range` claims callers
"pass `None` and serve the full body". The next person touching asset
serving would trust the comment and mishandle multi-range. Pure
comment fix, zero behavior change.

## Current state

`crates/v2/src/assets.rs:226-234` today:

```rust
/// Parses a single-range `Range: bytes=...` header against a body of
/// `total` bytes. Returns inclusive `(start, end)`. Multi-range requests
/// are not supported (callers pass `None` and serve the full body); a
/// syntactically valid but out-of-bounds range yields `None` (HTTP 416).
pub fn parse_byte_range(spec: &str, total: u64) -> Option<(u64, u64)> {
    let rest = spec.trim().strip_prefix("bytes=")?;
    if rest.contains(',') {
        return None; // multi-range: ignored by design
    }
```

Actual behavior: `parse_byte_range` returns `None` for BOTH multi-range
and out-of-bounds specs, and the caller `read_for_serving`
(`assets.rs:180`) turns `None` into `AssetBody::Unsatisfiable`, which
the HTTP layer serves as 416. Browsers practically never send
multi-range, so refusing (not full-serving) is the accepted design; the
comment is the only thing wrong.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile+tests | `cargo test -p pulpit-v2` | all pass |

## Scope

**In scope**:
- `crates/v2/src/assets.rs` — the doc comment on `parse_byte_range`
  only.

**Out of scope**:
- Any code change to `parse_byte_range` / `read_for_serving` /
  `AssetBody` — behavior is correct and covered by
  `byte_range_parser_covers_the_header_forms` (`assets.rs:443+`).

## Git workflow

- Branch: `advisor/016-assets-doc` (or fold into the same PR as plan
  015 if the operator batched them — it is a one-comment change).
- Commit message: `Fix the multi-range doc on parse_byte_range (416, not full body)`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Rewrite the doc sentence

Replace the parenthetical "(callers pass `None` and serve the full
body)" so the comment reads:

```rust
/// Parses a single-range `Range: bytes=...` header against a body of
/// `total` bytes. Returns inclusive `(start, end)`. Multi-range
/// requests are refused: like an out-of-bounds range they yield `None`,
/// which `read_for_serving` maps to `AssetBody::Unsatisfiable` (HTTP
/// 416).
```

**Verify**: `cargo test -p pulpit-v2` → all pass (docs compile).

## Test plan

- None; the existing range tests already pin the behavior.

## Done criteria

- [ ] `cargo test -p pulpit-v2` exits 0
- [ ] `grep -n "serve the full body" crates/v2/src/assets.rs` returns
      no matches
- [ ] `git status` shows only `crates/v2/src/assets.rs` modified
- [ ] `plans/README.md` status row updated

## STOP conditions

- The excerpt above does not match the live file (drift since
  `6dcddf3`).
- Any test fails (should be impossible for a comment-only change).

## Maintenance notes

- None; this closes the deferred nit recorded in `plans/README.md`.
