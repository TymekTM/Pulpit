# 040 - Start-minimized launches build no WebView; pending operator asks survive a missing window

> **Executor instructions.** Follow the steps in order, touch only in-scope
> files, run each Verify. STOP conditions are binding. Do not edit `plans/`.
> **Drift check first:** confirm the excerpts below still match.

## Status

- Priority: P1 | Effort: M | Risk: medium (window lifecycle; the pairing prompt is a security gate and must stay deny-safe) | Depends on: none
- Category: performance (memory, startup CPU)
- Planned at: commit `7227027`, 2026-11-11

## Why this matters

The desktop app (`apps/desktop`, Tauri 2 + Vue 3) usually launches at login
with `start_minimized` on: tray icon only, no visible window. Today that
launch still builds the whole main window with its WebView - hidden - loads
the Vue app into it, and keeps it for 10 minutes until an idle sweep
destroys it. A WebView process tree costs roughly 60-150 MB of memory (the
code's own comment), plus the CPU to start it and boot the UI nobody sees.

The app already runs fine without a window: the idle sweep destroys it, and
every path that shows the window rebuilds it on demand. So a start-minimized
launch should simply not build the window.

One thing currently depends on the hidden window existing: the **operator
ask** (the in-app popup that asks the PC user to approve a tablet's pairing
request). The backend emits an `operator-ask` event; a hidden window
receives it and shows the popup when the user opens the window. With no
window the event reaches nobody and the user sees nothing when they open
the window later. (This gap already exists today once the idle sweep has
run; this plan closes it for both cases.)

## Current state

All Rust excerpts are from `apps/desktop/src-tauri/src/lib.rs`. Line numbers
are approximate (`~`); locate by the quoted text.

### The field doc (~:32)

```rust
    /// Start with the main window hidden in the tray (`start_minimized`
    /// in editor.json). The server and the tray are unaffected - only
    /// the window skips its show at launch, once, in `setup`.
    start_minimized: bool,
```

### `.setup` (~:205)

```rust
        .setup(|app| {
            let state = setup_core(app.handle().clone());
            let start_minimized = state.start_minimized;
            app.manage(state);

            build_tray(app.handle())?;
            create_main_window(app.handle(), !start_minimized)?;
            if start_minimized {
                // count the tray-hidden launch from now, so the idle
                // WebView teardown treats it like a manual hide
                HIDDEN_SINCE.store(
                    pulpit_v2::unix_millis(),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
            spawn_webview_teardown(app.handle().clone());
            Ok(())
        })
```

### The run loop already tolerates a window-less process (~:290)

```rust
            // The idle sweep destroys the hidden WebView, which leaves the
            // process window-less for a while; the default reaction to that
            // (exit) would kill the server and the tray. Only an explicit
            // exit carries a code - the tray's Quit.
            match event {
                tauri::RunEvent::ExitRequested {
                    code: None, api, ..
                } => {
                    api.prevent_exit();
                }
```

### Showing rebuilds on demand (~:898)

```rust
fn show_main_window(app: &AppHandle) {
    HIDDEN_SINCE.store(0, std::sync::atomic::Ordering::Relaxed);
    match app.get_webview_window("main") {
        Some(window) => { /* unminimize, show, focus */ }
        // torn down by the idle sweep: rebuild; the fresh Vue app
        // refetches boards, settings and live state on mount
        None => {
            tracing::info!("rebuilding the main window WebView");
            if let Err(e) = create_main_window(app, true) {
                tracing::error!("could not rebuild the main window: {e}");
            }
        }
    }
}
```

`show_main_window` is called by the single-instance plugin callback (a
second launch) and by `toggle_main_window` (tray click, tray "Show / Hide").
`toggle_touch_mode` (~:1091) also rebuilds with `create_main_window(app, true)`.

### `create_main_window` (~:943)

```rust
/// The main window is built here instead of `tauri.conf.json` because the
/// WebView2 argument list is only reachable through the builder API, and a
/// custom list replaces the default one - so the stock feature disables are
/// repeated alongside the memory-oriented flags (no background networking,
/// no component updates, capped HTTP disk cache). `visible` is false for a
/// start-minimized launch: the window (and its WebView) is created hidden
/// and only the tray can surface it. Rebuilds after a teardown always pass
/// true - whoever rebuilds the window is about to show it.
fn create_main_window(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
        .title("Pulpit")
        .visible(visible)
        .inner_size(1280.0, 800.0)
```

It has exactly three callers: `.setup`, `show_main_window`, `toggle_touch_mode`.

### Operator asks (~:1428-1493)

```rust
fn pending_asks() -> &'static std::sync::Mutex<HashMap<String, std::sync::mpsc::Sender<bool>>> {
    static PENDING: OnceLock<std::sync::Mutex<HashMap<String, std::sync::mpsc::Sender<bool>>>> =
        OnceLock::new();
    PENDING.get_or_init(Default::default)
}
```

```rust
fn ask_operator(app: &AppHandle, kind: &str, name: &str, code: Option<String>) -> bool {
    let id = next_ask_id();
    let (tx, rx) = std::sync::mpsc::channel();
    pending_asks()
        .lock()
        .expect("asks poisoned")
        .insert(id.clone(), tx);
    let payload = serde_json::json!({ "id": id, "kind": kind, "name": name, "code": code });
    if let Err(e) = app.emit("operator-ask", payload) {
        tracing::warn!(error = %e, "operator ask: webview emit failed - denying");
        pending_asks().lock().expect("asks poisoned").remove(&id);
        return false;
    }
    tracing::info!(kind, name, "operator ask shown in the editor");
    let answer = match rx.recv_timeout(OPERATOR_ASK_TIMEOUT) { /* Ok(approved) => approved, Err(_) => false */ };
    pending_asks().lock().expect("asks poisoned").remove(&id);
    answer
}
```

```rust
#[tauri::command]
fn resolve_operator_ask(id: String, approved: bool) -> Result<(), String> {
    let sender = pending_asks().lock().expect("asks poisoned").remove(&id);
    match sender {
        Some(tx) => tx
            .send(approved)
            .map_err(|_| "ask already gone".to_string()),
        None => Err("no such ask (already resolved or expired)".to_string()),
    }
}
```

The existing exemplar for "state parked for a WebView that does not exist
yet" is the touch toggle - follow its style:

```rust
static PENDING_TOUCH_TOGGLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Take (and clear) a touch-mode toggle that was parked while the
/// WebView was torn down (DESK-07). Called by the fresh app after its
/// listeners and boards are up.
#[tauri::command]
fn take_pending_touch_toggle() -> bool {
    PENDING_TOUCH_TOGGLE.swap(false, std::sync::atomic::Ordering::Relaxed)
}
```

Commands are registered in `tauri::generate_handler![ ... ]` (~:222-290),
where `resolve_operator_ask,` and `take_pending_touch_toggle,` sit on
adjacent lines.

### Frontend

`apps/desktop/src/api.js` (~:70):

```js
  resolveOperatorAsk: (id, approved) => invoke("resolve_operator_ask", { id, approved }),
  takePendingTouchToggle: () => invoke("take_pending_touch_toggle"),
```

`apps/desktop/src/components/OperatorAskModal.vue` (script, ~:11-22):

```js
const ask = ref(null); // { id, kind, name, code? }
const busy = ref(false);
let unlisten = null;

onMounted(async () => {
  unlisten = await listen("operator-ask", (e) => {
    // one ask at a time (the backend enforces it for pair-requests; a
    // manual pairing ask while one is up replaces it - the stale worker
    // times out and denies on its own)
    ask.value = e.payload;
  });
});
```

Tests: `apps/desktop/tests/modals.test.js` (`describe("OperatorAskModal")`,
~:132) and the command table in `apps/desktop/tests/api.test.js` (~:39).
The Tauri bridge mock is `apps/desktop/tests/tauri.js`: `handle(cmd, fn)`
registers a command handler, an unhandled `invoke` resolves to `null`, every
invoke is appended to `calls`.

## Scope

In scope (the ONLY files you may change):

- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src/api.js`
- `apps/desktop/src/components/OperatorAskModal.vue` (script block only)
- `apps/desktop/tests/modals.test.js`
- `apps/desktop/tests/api.test.js`

Out of scope: `tauri.conf.json`; the idle-teardown threshold and sweep
logic (`webview_teardown_after`, `spawn_webview_teardown`); the tray menu;
`toggle_main_window`; anything under `crates/`; `apps/server`; `apps/mobile`;
`plans/`. Do NOT make a pairing ask open or focus the window by itself -
that is a product decision this plan does not take.

In `lib.rs` change only what the steps name. If `cargo fmt` wants to
reformat unrelated lines of that file, revert those lines and say so.

## Steps

### Step 1 - no window on a start-minimized launch

1. `create_main_window`: drop the `visible` parameter and the
   `.visible(visible)` line (a new window is visible by default). Update all
   three call sites to `create_main_window(app)` / `create_main_window(app.handle())`.
   Rewrite the last two sentences of its doc comment (from "`visible` is
   false ..." on) to say: the window is built on demand - at launch unless
   `start_minimized` is on, otherwise by whatever first shows it (tray,
   second launch, touch toggle) - and every caller is about to show it.
2. `.setup`: replace the `create_main_window(...)` call and the
   `if start_minimized { HIDDEN_SINCE.store(...) }` block with:

   ```rust
            if start_minimized {
                // no window, no WebView: the tray, a second launch or the
                // touch toggle builds it on first use (show_main_window)
                tracing::info!("start minimized - the main window is built on first show");
            } else {
                create_main_window(app.handle())?;
            }
   ```

   `HIDDEN_SINCE` stays 0 on that path (there is nothing to tear down).
3. Update the `start_minimized` field doc (~:32) so it says the launch builds
   no window at all (and therefore no WebView) until something shows it.

Verify: `grep -n "create_main_window(" apps/desktop/src-tauri/src/lib.rs`
shows the definition plus exactly three calls, none with a second argument;
`grep -n "\.visible(" apps/desktop/src-tauri/src/lib.rs` prints nothing.

### Step 2 - pending asks remember their payload

Replace the bare `HashMap<String, Sender<bool>>` behind `pending_asks()` with
a small struct so the latest unanswered ask can be replayed to a WebView
that mounts later. Put it next to `pending_asks()`:

```rust
/// Unanswered operator asks: the parked workers' senders plus the payload
/// of the most recent ask, kept so a WebView that did not exist when the
/// ask was emitted (start-minimized launch, idle teardown) can still show
/// it on mount.
#[derive(Default)]
struct PendingAsks {
    senders: HashMap<String, std::sync::mpsc::Sender<bool>>,
    latest: Option<(String, serde_json::Value)>,
}

impl PendingAsks {
    /// Park a new ask; it becomes the latest.
    fn park(&mut self, id: &str, tx: std::sync::mpsc::Sender<bool>, payload: serde_json::Value) { ... }
    /// Remove an ask (answered, timed out, emit failed). Clears `latest`
    /// when it is this ask.
    fn take(&mut self, id: &str) -> Option<std::sync::mpsc::Sender<bool>> { ... }
    /// The latest ask's payload, only while that ask is still parked.
    fn latest(&self) -> Option<serde_json::Value> { ... }
}
```

- `pending_asks()` returns `&'static std::sync::Mutex<PendingAsks>`.
- `ask_operator`: build `payload` first, `park(&id, tx, payload.clone())`,
  then emit; every place that did `.remove(&id)` now does `.take(&id)`.
  Behaviour is otherwise unchanged: emit failure denies, timeout denies.
- `resolve_operator_ask`: `.take(&id)` instead of `.remove(&id)`; same
  errors as today.
- Update the doc comment above `pending_asks()`: a missing WebView no longer
  loses the ask - it is replayed through `pending_operator_ask` when the
  window is built; an unanswered ask still denies at the timeout.

Deny-safety rule: nothing in this step may send `true` on a channel. Only
`resolve_operator_ask` sends, exactly as before.

### Step 3 - the replay command

Next to `take_pending_touch_toggle`:

```rust
/// The latest operator ask that is still waiting for an answer, for a
/// WebView that mounted after the ask was emitted. Does not consume it -
/// the answer still goes through [`resolve_operator_ask`].
#[tauri::command]
fn pending_operator_ask() -> Option<serde_json::Value> {
    pending_asks().lock().expect("asks poisoned").latest()
}
```

Register `pending_operator_ask,` in `generate_handler![...]` on the line
after `resolve_operator_ask,`.

### Step 4 - Rust unit tests

In the existing `#[cfg(test)] mod tests` of `lib.rs`, add tests that use a
LOCAL `PendingAsks::default()` (never the global one - tests run in
parallel):

1. `latest_ask_is_replayable_until_taken`: park `a`; `latest()` returns its
   payload; `take("a")` returns a sender; `latest()` is `None`.
2. `a_newer_ask_replaces_the_latest_and_taking_the_older_keeps_it`: park
   `a`, park `b`; `latest()` is `b`'s payload; `take("a")`; `latest()` is
   still `b`'s payload; `take("b")`; `latest()` is `None`.
3. `taking_an_unknown_ask_returns_none`: `take("nope")` is `None` and leaves
   a parked ask's `latest()` untouched.

Verify: `CARGO_BUILD_JOBS=2 timeout 2400 cargo test -p pulpit-desktop --locked`
passes, including the three new tests.

### Step 5 - frontend

1. `api.js`: after the `resolveOperatorAsk` line add
   `pendingOperatorAsk: () => invoke("pending_operator_ask"),`
2. `OperatorAskModal.vue`, inside `onMounted`, AFTER the `listen` call:

   ```js
  // an ask emitted before this WebView existed (start-minimized launch,
  // idle teardown): the backend replays the one still waiting
  try {
    const pending = await api.pendingOperatorAsk();
    if (pending && !ask.value) ask.value = pending;
  } catch (e) {
    console.error("pending operator ask", e);
  }
   ```

   (`!ask.value`: a live event that arrived during the await wins.)
3. `tests/api.test.js`: add the row
   `["pendingOperatorAsk", [], "pending_operator_ask", undefined],`
   next to the `resolveOperatorAsk` row.
4. `tests/modals.test.js`, in `describe("OperatorAskModal")`:
   - New test "shows an ask that was already waiting when the window was
     built": `handle("pending_operator_ask", () => ({ id: 5, kind: "pair-request", name: "Late", code: "424242" }))`,
     mount, flush, expect the name `Late` and the code `424242` to render;
     click Trust; expect an invoke `resolve_operator_ask` with
     `{ id: 5, approved: true }`.
   - New test "stays hidden when nothing is waiting": no handler (the mock
     returns `null`), mount, flush, `.veil` does not exist.
   - Two existing tests count invokes (`expect(calls).toEqual([{ cmd: "resolve_operator_ask", ... }])`
     and `expect(calls).toHaveLength(1)`). The mount now also invokes
     `pending_operator_ask`, so narrow exactly those two assertions to
     `calls.filter((c) => c.cmd === "resolve_operator_ask")`. Do not change
     what they assert about `resolve_operator_ask`.

Verify (in `apps/desktop`; run `npm ci` first if `node_modules` is missing):
`npx vitest run` passes; `npx vite build` passes.

## Done criteria (all must hold)

| Command (repo root unless noted) | Expected |
| --- | --- |
| `cargo fmt --check` | exit 0 |
| `CARGO_BUILD_JOBS=2 timeout 2400 cargo test -p pulpit-desktop --locked` | exit 0, 0 failed, the 3 new tests listed |
| `CARGO_BUILD_JOBS=2 timeout 2400 cargo clippy -p pulpit-desktop --all-targets --locked -- -D warnings` | exit 0 |
| `npx vitest run` in `apps/desktop` | exit 0; test count grew by at least 3 |
| `npx vite build` in `apps/desktop` | exit 0 |
| `grep -c "\.visible(" apps/desktop/src-tauri/src/lib.rs` | 0 |
| `grep -c "pending_operator_ask" apps/desktop/src-tauri/src/lib.rs` | at least 2 (definition + handler list) |
| `git status --short` | only the five in-scope files |

Commits (two):

1. `Build no WebView on a start-minimized launch` (step 1)
2. `Replay a waiting operator ask to a WebView built later` (steps 2-5)

Do NOT launch the desktop app to try it: it would open the real data
directory and port. The reviewer does the live check.

## STOP conditions

- `create_main_window` has a caller other than the three named, or `.setup`
  no longer matches the excerpt - STOP and report what you found.
- Making step 2 work seems to require sending on an ask channel from
  anywhere other than `resolve_operator_ask` - STOP.
- An existing test other than the two named in step 5.4 needs changing to
  pass - STOP and report which and why.
- A test fails that also fails on an untouched checkout of `7227027` - not
  yours; name it in the report and continue.

## Maintenance notes

- Anything added later that needs the WebView at launch (a startup toast,
  an auto-update prompt) must work when the window is first built minutes
  or days after the process started - the same rule the idle teardown
  already imposes.
- `pending_operator_ask` replays only the latest ask, matching the modal's
  "one ask at a time" behaviour. If asks ever queue in the UI, replay all.
- Review focus: no new path approves a pairing; `latest` is cleared on every
  exit path of `ask_operator`.
