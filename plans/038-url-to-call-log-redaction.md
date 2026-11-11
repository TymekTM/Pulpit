# 038 - `url-to-call` tiles: log scheme+host only (follow-up to 029)

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Base:** this plan builds on commit `066dcc5` (branch
> `t3/linux-cargo-tests-redact-http-logs`), not on `0da95cd`. Your worktree
> starts at `0da95cd`; first run `git merge --ff-only 066dcc5` in it. If
> that does not fast-forward cleanly, STOP and report.
> **Drift check:** after the fast-forward, confirm the excerpts below match.

## Status

- Priority: P2 | Effort: S | Risk: low | Depends on: 029 (same helper)
- Category: security (secrets in logs)
- Planned at: commit `066dcc5`, 2026-11-10

## Why this matters

Plan 029 stopped `http-request` tiles from writing full URLs to the log
file, because webhook URLs carry their secret in the path or query. The
older `url-to-call` tile kind (a plain fire-and-forget GET, used for the
same kind of webhook) still logs the full URL when the call fails. Logs
live in `~/pulpitApp/logs/` and are the first thing a user attaches to a
bug report.

## Current state

`crates/backend/src/http_request.rs` (added by plan 029):

```rust
/// What a log line may show of a request URL: scheme and authority only.
/// Webhook services carry their secret in the path or query, so both are
/// dropped; userinfo (`user:pw@`) is dropped too.
fn log_safe_url(url: &str) -> String {
```

and, in the failure branch of `execute_http_action` in the same file, the
pattern for the error text (ureq can embed the URL in its error message):

```rust
            let safe_error = e
                .to_string()
                .replace(substituted_url.as_str(), &log_safe_url(&substituted_url));
```

`crates/backend/src/lib.rs` (~:657), `mod http_request;` is declared at the
top of this file:

```rust
    fn exec_callurl(&self, cmd: &pulpit_actions::Command) -> bool {
        …
        match url {
            Some(url) => {
                if let Err(e) = self.http_agent.get(&url).call() {
                    tracing::warn!(url = %url, error = %e, "url-to-call failed");
                }
            }
            None => tracing::warn!(kind = "url-to-call", "tile has no urlToCall configured"),
        }
        true
    }
```

Existing tests for this tile kind are in the same file's `mod tests`
(`url_to_call_without_a_url_is_claimed`,
`url_to_call_fires_a_get_at_the_configured_url`).

## Commands (repo root)

| Purpose | Command |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=2 timeout 1200 cargo test -p pulpit-backend --locked` |
| Lint | `CARGO_BUILD_JOBS=2 timeout 1200 cargo clippy -p pulpit-backend --all-targets --locked -- -D warnings` |
| Format | `cargo fmt --check` |

After the fast-forward all backend tests pass on Linux with no skip flags.

## Scope

In scope: `crates/backend/src/http_request.rs` (visibility of one function,
one doc-comment sentence), `crates/backend/src/lib.rs` (the one `warn!` in
`exec_callurl`).

Out of scope: every other log line; `crates/host/src/spotify.rs` (album-art
CDN URLs, not secret); `crates/obs` (its URL is `ws://host:port`, no
secret); the value passed to `self.http_agent.get(&url)` (the request must
still use the full URL); `set_last_http_error`.

## Steps

### Step 1 - Share the helper
In `http_request.rs` change `fn log_safe_url` to `pub(crate) fn log_safe_url`.

### Step 2 - Use it in `exec_callurl`
Replace the `warn!` with the same shape plan 029 used:

```rust
                if let Err(e) = self.http_agent.get(&url).call() {
                    // same rule as http-request tiles: the path/query of
                    // a webhook URL is its secret, and ureq may repeat
                    // the URL inside its error text
                    let safe_url = http_request::log_safe_url(&url);
                    let safe_error = e.to_string().replace(url.as_str(), &safe_url);
                    tracing::warn!(url = %safe_url, error = %safe_error, "url-to-call failed");
                }
```

Verify: `grep -n 'url = %url' crates/backend/src/lib.rs` prints nothing.

### Step 3 - Gates
Run Test, Lint and Format. No new test is required: `log_safe_url` already
has its own table test from plan 029, and this change only routes one more
call site through it. Do not weaken or delete any existing test.

## Git workflow

One commit on top of `066dcc5`:
`Log only scheme and host for url-to-call tiles`. No push.

## Done criteria

- [ ] `git log --oneline 0da95cd..HEAD` shows exactly three commits: `680e59e`, `066dcc5`, and yours.
- [ ] `grep -rn 'url = %url' crates/backend/src` prints nothing.
- [ ] Test exits 0 with no `--skip` flags; Lint shows 0 warnings; Format exits 0.
- [ ] `git diff --stat 066dcc5..HEAD` lists only the two in-scope files.

## STOP conditions

- The fast-forward fails, or `log_safe_url` is not in `http_request.rs`.
- You find another log line that prints a user-configured URL in full:
  report its `file:line`; do not edit it.

## Maintenance notes

- Any future tile kind that takes a user-supplied URL must log through
  `log_safe_url`.
- The resulting branch contains plan 028 and 029's commits; it supersedes
  `t3/linux-cargo-tests-redact-http-logs` for merging.
