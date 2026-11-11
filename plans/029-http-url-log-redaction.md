# 029 - Stop logging full URLs of HTTP request tiles

> **Executor instructions.** Follow the steps in order. Touch only the file
> listed under "In scope". Run each Verify command before moving on. If a
> STOP condition triggers, stop and report. Do not edit anything under
> `plans/`. **Drift check first:** open the file cited below and confirm the
> excerpts match; if not, STOP and report.

## Status

- Priority: P2 | Effort: S | Risk: low | Depends on: 028 (same file; run after it in the same checkout)
- Category: security
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

HTTP request tiles are used for webhooks (Discord, Slack, Home Assistant,
IFTTT). Those services put the secret **in the URL path or query**. Pulpit
already redacts sensitive headers before logging, but writes the complete
URL to `~/pulpitApp/logs/` on every press - a log file the user is asked to
attach to bug reports. After this plan the log carries only
`scheme://host[:port]/…`.

## Current state

`crates/backend/src/http_request.rs`:

- `redact_headers` (~:128) already replaces sensitive header values with
  `"[REDACTED]"` - the convention to follow.
- Success log (~:327):
  ```rust
  tracing::info!(method = %method_upper, url = %substituted_url, status = status, duration_ms = ..., "HTTP request completed")
  ```
- Failure log (~:386):
  ```rust
  tracing::warn!(method = ..., url = %substituted_url, ..., error = %e, "HTTP request failed")
  ```
- `crates/backend/Cargo.toml` has `ureq = "3"` and **no** `url` crate. Do not
  add a dependency for this.

Find every log call that prints a URL with:
`grep -n "tracing::" -A4 crates/backend/src/http_request.rs | grep -n "url"`

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=3 timeout 900 cargo test -p pulpit-backend --locked` |
| Lint | `CARGO_BUILD_JOBS=3 cargo clippy -p pulpit-backend --all-targets --locked` |

If plan 028 has not been applied in this checkout, the test
`http_step_inside_a_multiaction_runs_the_native_chain` hangs on a machine
without a display: append
`-- --skip http_step_inside_a_multiaction_runs_the_native_chain`.

## Scope

In scope: `crates/backend/src/http_request.rs` only.

Out of scope: the error text returned to the caller / shown in the UI (the
user owns that URL and needs it to debug), header redaction, any other
crate.

## Git workflow

One commit, e.g. `Log only scheme and host for HTTP request tiles`. No push.

## Steps

### Step 1 - Pure helper

Add next to `redact_headers`:

```rust
/// What a log line may show of a request URL: scheme and authority only.
/// Webhook services carry their secret in the path or query, so both are
/// dropped; userinfo (`user:pw@`) is dropped too.
fn log_safe_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return "[url]".to_string();
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    if host.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return "[url]".to_string();
    }
    format!("{scheme}://{host}/…")
}
```

### Step 2 - Use it in every log call

Replace `url = %substituted_url` (and any other URL field found by the grep
above) with `url = %log_safe_url(&substituted_url)` in `tracing::` calls
only. If the `error = %e` value can contain the URL (check how `e` is built
in this function - e.g. a `format!` that embeds the URL), log a version
without it; do not change what the function returns.

Verify: `grep -n "url = %substituted_url" crates/backend/src/http_request.rs`
prints nothing.

### Step 3 - Unit tests

In the existing `#[cfg(test)] mod tests` of this file add
`log_safe_url_drops_path_query_and_userinfo`:

| Input | Expected |
| --- | --- |
| `https://hooks.example.com/services/T0/B0/s3cr3t?x=1#f` | `https://hooks.example.com/…` |
| `http://user:pw@host.local:8080/a/b` | `http://host.local:8080/…` |
| `http://127.0.0.1:9` | `http://127.0.0.1:9/…` |
| `not a url` | `[url]` |
| `://nohost` | `[url]` |

Also assert that none of the outputs contains `s3cr3t`, `pw` or `x=1`.

Verify: the Test command passes.

## Done criteria

- [ ] `CARGO_BUILD_JOBS=3 timeout 900 cargo test -p pulpit-backend --locked` exits 0 (with the skip flag only if 028 is absent).
- [ ] `grep -c "log_safe_url" crates/backend/src/http_request.rs` ≥ 4 (definition, two call sites, test).
- [ ] clippy for `pulpit-backend` shows no new warning in this file.
- [ ] `git diff --stat` for this commit lists only `crates/backend/src/http_request.rs`.

## STOP conditions

- The URL is also written to logs from another file (e.g. the step
  dispatcher in `crates/backend/src/lib.rs`): report the location, do not
  edit it.

## Maintenance notes

Any new log line in this module must use `log_safe_url`; reviewers should
grep for `url = %` in PRs touching it.
