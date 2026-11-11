# 034 - Cancelling a pairing request frees the slot (and never leaves a stray code)

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Drift check first:** confirm the excerpts below still match.

## Status

- Priority: P1 | Effort: M | Risk: medium (pairing is a security boundary; Android part cannot be compiled on this machine) | Depends on: none
- Category: correctness / security hygiene
- Planned at: commit `0da95cd`, 2026-11-10

## Why this matters

"Bluetooth-style" pairing: the tablet `POST`s `/v2/pair-request`, the desktop
shows a dialog with a code, the tablet polls `GET /v2/pair-request/:id`. Only
one request may be live. Two defects:

1. **Cancel locks pairing for 5 minutes.** When the user taps Cancel on the
   tablet the app just stops polling. The server keeps the slot until the
   code TTL (5 min) passes, so the next attempt - from this or any tablet -
   gets HTTP 409 "busy". The slot is only released when a terminal decision
   is *polled*, which a cancelled tablet never does.
2. **A refused request leaves a valid pairing code behind.** The handler
   mints the code *before* it knows whether the slot is free; on 409 the code
   stays in the valid set for 5 minutes although nobody was told it.

## Current state

### `crates/v2/src/devices.rs`

- `Pairing` (~:238) holds `codes: Mutex<HashMap<String, Instant>>` and a
  `pre_approved` set. `new_code()` (~:275) inserts a code; `pre_approve`
  (~:400) / `take_pre_approved` (~:408) manage operator pre-approvals.
- `PairDecision { Pending, Approved, Rejected }`.
- `PairRequest` (~:429): `id`, `code`, `name`, `created: Instant`,
  `decision: Mutex<PairDecision>`, methods `decision()`, `set_decision()`,
  `age()`.
- `PairRequests` (~:451): `current: Mutex<Option<Arc<PairRequest>>>`, `ttl`,
  constructors `default()` / `with_ttl(ttl)`, and:
  ```rust
  pub fn begin(&self, id: String, code: String, name: String) -> Option<Arc<PairRequest>> {
      let mut current = self.current.lock().expect("pair requests poisoned");
      if let Some(existing) = current.as_ref() {
          if existing.age() < self.ttl {
              return None;
          }
      }
      …
  ```
  plus `get(id)` and `reset(id)`. Unit tests live at the bottom of the file
  (e.g. `pair_requests_are_one_at_a_time_until_resolved`).

### `crates/v2/src/service.rs`

- Routes (~:78): `.route("/v2/pair-request", post(pair_request_create))`,
  `.route("/v2/pair-request/:id", get(pair_request_status))`.
- `pair_request_create` (~:175): rejects loopback callers and disallowed
  origins, then
  ```rust
  let id = hex_string_16();
  let code = state.pairing.new_code();
  let Some(request) = state.pair_requests.begin(id.clone(), code.clone(), name.clone()) else {
      return (StatusCode::CONFLICT, Json(json!({"code": "request-in-flight"}))).into_response();
  };
  ```
  then spawns a task that runs the blocking operator gate
  (`pairing.request_gate()`), and on approval calls
  `pairing.pre_approve(&pairing_code)` and `request.set_decision(Approved)`,
  otherwise `set_decision(Rejected)`.
- `pair_request_status` (~:245): 404 `{"status":"unknown"}` for an unknown
  id; `expired` after the TTL; `pending`; `approved` / `rejected` (both call
  `state.pair_requests.reset(&id)`).

### Android - `apps/mobile/app/src/main/java/app/pulpit/mobile/`

- `net/PairRequest.kt`: `createPairRequest` (POST, 409 →
  `PairRequestRejected(resId = R.string.main_error_pair_busy)`) and
  `pairRequestStatus` (GET `/v2/pair-request/$id`).
- `state/PulpitViewModel.kt` (~:251):
  ```kotlin
  fun cancelPairRequest() {
      pairPollJob?.cancel()
      pairPollJob = null
      _pairRequest.value = null
  }
  ```

### Docs and tests

- `docs/protocol-v2.md` ~:153-161 documents the flow.
- Integration tests for the flow: `crates/v2/tests/integration.rs` ~:458-620
  (helpers `http_post_json`, `http_get`).

## Commands

| Purpose | Command (repo root) |
| --- | --- |
| Test | `CARGO_BUILD_JOBS=3 timeout 1200 cargo test -p pulpit-v2 --locked -- --skip hub_teardown_ends_the_session_and_releases_held_keys` |
| Lint | `CARGO_BUILD_JOBS=3 cargo clippy -p pulpit-v2 --all-targets --locked` |
| Android unit tests | `gradle :app:testDebugUnitTest` in `apps/mobile` - **needs the Android SDK, which this machine does not have.** Try once; if it fails for lack of an SDK, say so in the report. |

The skipped test is a known Linux-only failure fixed by another plan.

## Scope

In scope:
- `crates/v2/src/devices.rs`, `crates/v2/src/service.rs`
- `crates/v2/tests/integration.rs` - new tests only, placed directly after
  the existing pair-request tests (~:620). Do not touch the rest of the file.
- `apps/mobile/app/src/main/java/app/pulpit/mobile/net/PairRequest.kt`
- `apps/mobile/app/src/main/java/app/pulpit/mobile/state/PulpitViewModel.kt`
- `docs/protocol-v2.md` (the pair-request section)

Out of scope: the desktop dialog (`OperatorAskModal.vue`, `ask_operator` in
the desktop backend) - it cannot be dismissed from the server side today; the
`V2State` struct (do not add fields); the legacy protocol; `crates/proto`.

## Design (decided - do not redesign)

- New terminal decision `PairDecision::Cancelled`.
- `PairRequest` records *when* it became terminal:
  `decided_at: Mutex<Option<Instant>>`, set by `set_decision` for any
  non-`Pending` value.
- `PairRequests` gets a `decided_grace: Duration` (production value
  `PAIR_DECIDED_GRACE = 5 s`, a new `pub const` beside `PAIR_CODE_TTL`) and a
  builder `with_decided_grace(self, d: Duration) -> Self` for tests.
- `begin` refuses only while the existing request is live: **not** expired
  **and** (still `Pending` **or** terminal for less than `decided_grace`).
  The short grace lets the original tablet poll its `approved` result before
  the slot is reused, and bounds how fast a client can spam the operator
  dialog (each request holds a blocking gate worker for up to 6 minutes).
- `begin` returns the request it **replaced**, if any, so the caller can
  discard that request's code: change the return type to
  `Result<Arc<PairRequest>, ()>`-style refusal plus replaced value, e.g.
  `pub fn begin(...) -> Option<(Arc<PairRequest>, Option<Arc<PairRequest>>)>`.
- `Pairing::discard(&self, code: &str)`: removes the code from `codes` and
  from `pre_approved`.
- `DELETE /v2/pair-request/:id`: unknown id → 404 `{"status":"unknown"}`;
  otherwise `pairing.discard(&request.code)`,
  `request.set_decision(Cancelled)`, respond `204 No Content`. The slot is
  **not** reset - it frees itself after `decided_grace`. Apply the same
  `pulpit_legacy::origin_host_allowed(&headers)` check as the POST handler
  (403 on failure); no loopback restriction.
- `pair_request_status` for a `Cancelled` request answers 404
  `{"status":"unknown"}` (no new wire status).
- `pair_request_create`: when `begin` refuses, call
  `state.pairing.discard(&code)` before returning 409; when it replaced an
  older request, `discard` that request's code too.
- The spawned decision task must not resurrect a dead request: after the
  gate returns, apply the decision only if the request is still `Pending`
  (`request.decision() == PairDecision::Pending`); in particular never call
  `pre_approve` for a cancelled or replaced request. To detect "replaced",
  also check `state.pair_requests.get(&request.id).is_some()` (clone the
  `Arc<V2State>` into the task).

## Git workflow

One commit: `Pairing: tablet cancel frees the request slot; refused requests leave no code`. No push.

## Steps

### Step 1 - `devices.rs`
Implement `Cancelled`, `decided_at`, `PAIR_DECIDED_GRACE`, `decided_grace` +
`with_decided_grace`, the new `begin`, and `Pairing::discard`. Update the
existing unit tests for the new `begin` return type. Add unit tests:
- `a_decided_request_frees_the_slot_after_the_grace`: `with_ttl(60 s)` +
  `with_decided_grace(50 ms)`; begin A; begin B refused; `A.set_decision(Rejected)`;
  begin B still refused immediately; after sleeping 80 ms begin B succeeds
  and returns A as the replaced request.
- `a_pending_request_holds_the_slot_until_expiry` (regression: Pending is
  never replaced before the TTL).
- `discard_removes_the_code_and_its_pre_approval`: after
  `new_code` + `pre_approve` + `discard`, `take_pre_approved` is `false` and
  consuming the code fails (use the same `consume` call the existing tests
  use).

Verify: `CARGO_BUILD_JOBS=3 cargo test -p pulpit-v2 --locked --lib` passes.

### Step 2 - `service.rs`
Add the DELETE route and handler, change `pair_request_create` and the
decision task and `pair_request_status` per the design. Match the existing
handler style (extractors, `json!` bodies, `tracing::info!` without the
code).

### Step 3 - Integration tests
After the existing pair-request tests add:
- `cancelling_a_pair_request_frees_the_slot`: POST → 201; DELETE → 204; GET
  status → 404; the old code no longer pairs (reuse whatever helper the
  neighbouring tests use to open `/v2/ws?pair=<code>` or to check a code);
  a second POST succeeds once the grace has passed. Look at how
  `test_state(backend, |cfg| …)` lets a test shorten TTLs; if the grace
  cannot be shortened from an integration test without changing `V2State`,
  wait the real 5 s in the test (`tokio::time::sleep`) rather than adding
  fields.
- `a_refused_pair_request_leaves_no_code`: POST A → 201; POST B → 409; assert
  through the `Pairing` handle on the test state that only one code is
  outstanding (add a small `#[cfg(test)]`-free read-only accessor such as
  `Pairing::outstanding_codes(&self) -> usize` if none exists).

Verify: the Test command passes.

### Step 4 - Android client
- `PairRequest.kt`: add `suspend fun cancelPairRequest(host: String, port: Int, id: String)`
  mirroring `pairRequestStatus` (same HTTP client, dispatcher, timeouts),
  sending `DELETE http://$host:$port/v2/pair-request/$id`, swallowing every
  exception (best effort) and ignoring the status code.
- `PulpitViewModel.cancelPairRequest()`: before clearing `_pairRequest`,
  read the current request's host, port and id and launch the DELETE in
  `viewModelScope` (do not block the UI; the local state is cleared
  immediately as today). Update the doc comment - it no longer "leaves the
  desktop untouched".
- If a unit test for the view model's pair flow exists under
  `apps/mobile/app/src/test/.../state/`, extend it; otherwise add none.

Verify: try the Android command; report the result honestly. Re-read both
Kotlin files for compile errors (imports, named parameters, suspend context)
since the compiler may not be available.

### Step 5 - Docs
Update the pair-request section of `docs/protocol-v2.md`: the DELETE
endpoint, the 5 s grace, and that a refused/cancelled request's code is
discarded.

## Done criteria

- [ ] The Test command exits 0 with the two new integration tests and three new unit tests present (`grep -c` their names).
- [ ] Lint shows no new warnings in `devices.rs` / `service.rs`.
- [ ] `grep -n "delete(pair_request_cancel)\|\.delete(" crates/v2/src/service.rs` shows the new route.
- [ ] `git diff --stat` lists only in-scope files.
- [ ] Report states whether the Android unit tests ran, with output or the SDK error.

## STOP conditions

- The desktop or headless app constructs `PairRequests` in a way that the new
  field breaks (it should not: both use `Default`): report instead of editing
  files outside scope.
- The Android client parses the status endpoint in a way that a 404 after
  cancel would crash or loop: report the code path.
- You find that a code can be consumed without the operator gate after these
  changes: STOP immediately and report - that would be a pairing bypass.

## Maintenance notes

- The desktop dialog of a cancelled request stays open until the operator
  answers or it times out (6 min); a newer request replaces it on screen.
  Dismissing it needs a cancellable gate - future work.
- Reviewers: pairing invariants to re-check - a code is valid only while its
  request is live; `pre_approve` is only ever called for a `Pending`, current
  request.
