# 039 - Smaller release binaries: size-optimized profile, JS engine kept fast, unused engine features off

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Drift check first:** confirm the excerpts below still match.

## Status

- Priority: P2 | Effort: S | Risk: low (build configuration only; no source code changes) | Depends on: none
- Category: performance / binary size
- Planned at: commit `7227027`, 2026-11-11

## Why this matters

The release binaries (`pulpit-server`, and `pulpit-desktop.exe` which embeds
the same server core) are built at `opt-level = 3`. Measured on Linux at
commit `0da95cd`, stripped `pulpit-server`:

| Profile | Size | JS execution time (synthetic extension workload) |
| --- | --- | --- |
| current (thin LTO, opt 3) | 17.77 MB | 4.9 s |
| `opt-level = "s"` everywhere | 13.10 MB | 7.4 s (50% slower) |
| `"s"` everywhere except the JS engine crates | 14.72 MB | 4.9 s (unchanged) |
| the row above + `boa_engine` without default features | **13.78 MB (-22%)** | 4.9 s (unchanged) |

Idle CPU and idle memory do not change between these variants (measured:
about 12 MB resident, about 0.03% of one core).

The JavaScript engine (Boa) runs the user's extensions; it is the only part
whose speed measurably depends on `opt-level`, so it stays at 3. Everything
else is I/O-bound glue.

`boa_engine`'s default features are `float16`, `xsum` and `temporal`. The
`temporal` feature pulls in `temporal_rs`, a time-zone database provider and
ICU calendar data (about 1 MB). Extensions are Deckboard plugins written for
an old Electron runtime, where `Temporal`, `Float16Array` and
`Math.sumPrecise` never existed, and nothing under `crates/ext/src` uses
them. `Date` is NOT affected: it gets the local UTC offset through the
`time` crate, which is a non-optional dependency of `boa_engine`.

## Current state

### `Cargo.toml` (workspace root), the end of the file

```toml
# smaller image = fewer resident pages; LTO + one CGU deduplicate Boa and
# the stack of async helpers, strip drops symbols nobody loads at runtime
[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"
```

### `crates/ext/Cargo.toml`, line 8

```toml
boa_engine = "0.22"
```

`crates/ext` is the only crate in the workspace that depends on Boa.

## Scope

In scope (the ONLY files you may change):

- `Cargo.toml` (workspace root) - the `[profile.release]` block and the
  comment above it; new `[profile.release.package.*]` blocks after it.
- `crates/ext/Cargo.toml` - the `boa_engine` line only.
- `Cargo.lock` - only as rewritten by cargo itself in step 2.

Out of scope (do not touch): every `.rs`, `.js`, `.vue`, `.kt` file; any
other `Cargo.toml`; `lto`, `codegen-units`, `strip`, `panic`; the dev and
test profiles; CI workflows; `plans/`. Do NOT run `cargo update`.

## Steps

All cargo commands: prefix with `CARGO_BUILD_JOBS=2` and wrap in `timeout`
as shown. Other agents share this machine.

### Step 0 - baseline size

```
CARGO_BUILD_JOBS=2 timeout 2400 cargo build --release --locked -p pulpit-server
stat -c %s target/release/pulpit-server
```

Write the number down as `BEFORE`. (Expect roughly 17.5-18.5 million bytes.)

### Step 1 - profile

Replace the comment and the `[profile.release]` block at the end of the root
`Cargo.toml` with exactly this:

```toml
# smaller image = fewer resident pages; LTO + one CGU deduplicate Boa and
# the stack of async helpers, strip drops symbols nobody loads at runtime.
# opt-level "s" for everything except the JS engine: measured on the Linux
# server at 0da95cd, "s" everywhere shrinks the binary 26% but makes
# extension JavaScript ~50% slower; keeping the engine crates at 3 gives
# 17% with unchanged JS speed (22% together with the trimmed Boa features
# in crates/ext/Cargo.toml). Idle CPU and memory are the same either way.
[profile.release]
opt-level = "s"
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"

# the JS engine and its regex engine stay speed-optimized (see above)
[profile.release.package.boa_engine]
opt-level = 3
[profile.release.package.boa_parser]
opt-level = 3
[profile.release.package.boa_ast]
opt-level = 3
[profile.release.package.boa_gc]
opt-level = 3
[profile.release.package.boa_interner]
opt-level = 3
[profile.release.package.boa_string]
opt-level = 3
[profile.release.package.regress]
opt-level = 3
```

Verify: `cargo metadata --locked --format-version 1 >/dev/null` exits 0.
In every later cargo build, watch for a warning like "profile package spec
`X` in profile `release` did not match any packages": it means a crate name
above is wrong - STOP and report it.

### Step 2 - Boa features

In `crates/ext/Cargo.toml` change line 8 to:

```toml
# default features (temporal, float16, xsum) are off: extensions target the
# old Deckboard/Electron runtime that never had them, and Temporal alone
# drags in a time-zone database and ICU calendars (~1 MB)
boa_engine = { version = "0.22", default-features = false }
```

Then let cargo prune the lock file (no `--locked` on this one command):

```
CARGO_BUILD_JOBS=2 timeout 1800 cargo check -p pulpit-ext --offline
```

If `--offline` fails because something is not cached, run the same command
without `--offline`.

Verify the lock file only lost packages:

```
git diff Cargo.lock | grep -c '^+name = '      # must print 0
git diff Cargo.lock | grep -c '^+version = '   # must print 0
git diff Cargo.lock | grep -c '^-name = '      # must print a number > 0 (about 12)
cargo tree -p pulpit-server -e normal --locked | grep -c -E 'temporal_rs|timezone_provider'   # must print 0
```

If any package was ADDED or any version CHANGED, STOP and report the diff.

### Step 3 - prove it

```
CARGO_BUILD_JOBS=2 timeout 2400 cargo build --release --locked -p pulpit-server
stat -c %s target/release/pulpit-server
```

Write the number down as `AFTER`.

## Done criteria (all must hold)

| Command | Expected |
| --- | --- |
| `cargo fmt --check` | exit 0 |
| `CARGO_BUILD_JOBS=2 timeout 2400 cargo test --workspace --locked` | exit 0, 0 failed (in particular every `pulpit-ext` test passes) |
| `CARGO_BUILD_JOBS=2 timeout 2400 cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 |
| `AFTER` vs `BEFORE` | `AFTER` is at most 82% of `BEFORE` |
| `git status --short` | only `Cargo.toml`, `crates/ext/Cargo.toml`, `Cargo.lock` |
| `git diff --stat` | no `.rs` file |

Commit as one commit:
`Build release binaries for size; keep the JS engine at opt-level 3`

## STOP conditions

- Any `pulpit-ext` test fails after step 2 (an extension feature may depend
  on a default Boa feature after all) - STOP, report the failing test and
  its output; do not re-enable features on your own.
- `crates/ext` fails to compile with `default-features = false` - STOP and
  report the first error.
- `AFTER` is more than 82% of `BEFORE` - do not tune further; report both
  numbers.
- The lock file gains a package or changes a version - STOP.
- A test fails that also fails on an untouched checkout of `7227027` - not
  yours; name it in the report and continue.

## Maintenance notes

- A future Boa upgrade may add or rename crates (`boa_*`); a renamed crate
  silently falls back to `"s"`. Cargo prints a "did not match any packages"
  warning for stale names - treat that warning as a to-do.
- If an extension author reports a missing `Temporal` or `Float16Array`,
  the fix is one feature flag in `crates/ext/Cargo.toml`.
- The Windows desktop exe uses the same profile; its size change can only be
  measured on a Windows build (`npx tauri build --no-bundle`).
