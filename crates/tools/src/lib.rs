//! Native utility tiles (tools): clock, timer, stopwatch, counter.
//!
//! - State lives on the server so every tablet and desktop touch mode agree.
//! - State is persisted to `~/pulpitApp/tools.json` via [`pulpit_db::write_atomic`].
//! - Compact state (`{running, startedAtMs, elapsedMs, durationMs, count}`) is
//!   pushed through `pulpit_host::forward_producer` on change only.
//! - For the legacy client, formatted labels are pushed at 1 Hz only while a
//!   timer/stopwatch runs and a legacy client is connected.
//! - Nothing wakes periodically while no timer runs: the finish loop sleeps
//!   until the nearest running timer's deadline and the label loop parks on
//!   a [`Notify`]; any gesture re-arms both.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;

pub mod state;
#[cfg(test)]
mod tests;

pub use state::{
    parse_duration_ms, Clock, CounterState, FinishAction, StopwatchState, SystemClock, TimerState,
    ToolKind, ToolState,
};

/// Tool action gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGesture {
    Tap,
    DoubleTap,
    LongPress,
    Reset,
}

impl std::str::FromStr for ToolGesture {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "double-tap" => ToolGesture::DoubleTap,
            "long-press" => ToolGesture::LongPress,
            "reset" => ToolGesture::Reset,
            _ => ToolGesture::Tap,
        })
    }
}

/// The persistent tools store on disk: `~/pulpitApp/tools.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DiskStore {
    #[serde(default)]
    pub tools: HashMap<String, ToolState>,
}

impl DiskStore {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => DiskStore::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        pulpit_db::write_atomic(path, &bytes)
    }
}

/// Shared tools manager.
pub struct ToolManager {
    store_path: PathBuf,
    clock: Arc<dyn Clock>,
    state: Mutex<HashMap<i64, ToolState>>,
    change_tx: UnboundedSender<Value>,
    board_tx: Mutex<Option<UnboundedSender<i64>>>,
    /// Re-arms the finish loop (deadline changed).
    deadline_wake: Notify,
    /// Re-arms the legacy label loop (something may be running now).
    labels_wake: Notify,
}

impl ToolManager {
    pub fn new(
        store_path: PathBuf,
        clock: Arc<dyn Clock>,
        change_tx: UnboundedSender<Value>,
    ) -> Self {
        let loaded = DiskStore::load(&store_path);
        let mut map = HashMap::new();
        for (k, v) in loaded.tools {
            if let Ok(id) = k.parse::<i64>() {
                map.insert(id, v);
            }
        }
        Self {
            store_path,
            clock,
            state: Mutex::new(map),
            change_tx,
            board_tx: Mutex::new(None),
            deadline_wake: Notify::new(),
            labels_wake: Notify::new(),
        }
    }

    pub fn set_board_sink(&self, tx: UnboundedSender<i64>) {
        *self.board_tx.lock().unwrap() = Some(tx);
    }

    /// Persist current tools state to disk.
    pub fn persist(&self) {
        let snapshot = {
            let guard = self.state.lock().unwrap();
            let mut store = DiskStore::default();
            for (&id, tool) in guard.iter() {
                store.tools.insert(id.to_string(), tool.clone());
            }
            store
        };
        if let Err(e) = snapshot.save(&self.store_path) {
            tracing::warn!(path = %self.store_path.display(), error = %e, "failed to save tools.json");
        }
    }

    /// Lazily drop entries for deleted buttons.
    pub fn clean_deleted(&self, known_button_ids: &[i64]) {
        let mut changed = false;
        {
            let mut guard = self.state.lock().unwrap();
            let before = guard.len();
            guard.retain(|id, _| known_button_ids.contains(id));
            if guard.len() != before {
                changed = true;
            }
        }
        if changed {
            self.persist();
        }
    }

    /// Execute an interaction gesture on a tool tile.
    pub fn execute(
        &self,
        button_id: i64,
        kind: &str,
        command: Option<&str>,
        gesture: ToolGesture,
    ) -> bool {
        let now = self.clock.now_ms();
        let push_val: Option<Value>;

        {
            let mut guard = self.state.lock().unwrap();
            let tool = guard
                .entry(button_id)
                .or_insert_with(|| ToolState::from_config(kind, command));

            // update configuration if changed in editor
            tool.update_config(kind, command);

            match tool {
                ToolState::Timer(t) => {
                    match gesture {
                        ToolGesture::Tap => {
                            t.tap(now);
                        }
                        ToolGesture::DoubleTap | ToolGesture::LongPress | ToolGesture::Reset => {
                            t.reset();
                        }
                    }
                    push_val = Some(t.compact_state(now));
                }
                ToolState::Stopwatch(s) => {
                    match gesture {
                        ToolGesture::Tap => {
                            s.tap(now);
                        }
                        ToolGesture::DoubleTap | ToolGesture::LongPress | ToolGesture::Reset => {
                            s.reset();
                        }
                    }
                    push_val = Some(s.compact_state(now));
                }
                ToolState::Counter(c) => {
                    match gesture {
                        ToolGesture::Tap => {
                            c.tap();
                        }
                        ToolGesture::DoubleTap => {
                            c.double_tap();
                        }
                        ToolGesture::LongPress | ToolGesture::Reset => {
                            c.reset();
                        }
                    }
                    push_val = Some(c.compact_state());
                }
                ToolState::Clock => {
                    return true;
                }
            }
        }

        self.persist();

        if let Some(val) = push_val {
            let key = format!("tool-{button_id}");
            let _ = self.change_tx.send(json!({ key: val }));
        }

        // a timer may have started, paused or reset: recompute the deadline
        self.deadline_wake.notify_one();
        self.labels_wake.notify_one();

        true
    }

    /// How long until the nearest running timer finishes, or `None` when no
    /// timer runs (nothing to schedule).
    pub fn next_timer_wait(&self) -> Option<Duration> {
        let now = self.clock.now_ms();
        let guard = self.state.lock().unwrap();
        guard
            .values()
            .filter_map(|tool| match tool {
                ToolState::Timer(t) if t.running => Some(t.remaining_ms(now)),
                _ => None,
            })
            .min()
            .map(Duration::from_millis)
    }

    /// Finish loop: sleeps until the nearest running timer's deadline (or
    /// parks while none runs) and re-arms whenever a gesture changes it.
    async fn run_timer_loop(self: Arc<Self>) {
        loop {
            // `notify_one` stores a permit, so a gesture landing between
            // computing the wait and awaiting is not lost
            match self.next_timer_wait() {
                None => self.deadline_wake.notified().await,
                Some(wait) => {
                    tokio::select! {
                        () = tokio::time::sleep(wait) => self.tick_timers(),
                        () = self.deadline_wake.notified() => {}
                    }
                }
            }
        }
    }

    /// Legacy label loop: 1 Hz while a timer/stopwatch runs, parked otherwise.
    async fn run_legacy_label_loop(self: Arc<Self>, hub: Arc<pulpit_legacy::Hub>) {
        loop {
            if !self.has_running_tools() {
                self.labels_wake.notified().await;
                continue;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            // Only while a timer/stopwatch runs AND a legacy client is connected
            let legacy_connected = hub.try_len().unwrap_or(0) > 0;
            if legacy_connected && self.has_running_tools() {
                let labels = self.legacy_formatted_labels();
                if !labels.is_empty() {
                    let data = serde_json::to_value(&labels).unwrap_or(Value::Null);
                    let payload = json!({
                        "app": "APP_CUSTOM_VALUE",
                        "data": data,
                    })
                    .to_string();
                    hub.broadcast("app_status_update", Some(&payload)).await;
                }
            }
        }
    }

    /// Get current compact state for a button.
    pub fn get_state(&self, button_id: i64) -> Option<Value> {
        let now = self.clock.now_ms();
        let guard = self.state.lock().unwrap();
        guard.get(&button_id).map(|t| t.compact_state(now))
    }

    /// Background tick for checking running timers and finishing them.
    pub fn tick_timers(&self) {
        let now = self.clock.now_ms();
        let mut finished_actions = Vec::new();
        let mut finished_pushes = Vec::new();

        {
            let mut guard = self.state.lock().unwrap();
            for (&id, tool) in guard.iter_mut() {
                if let ToolState::Timer(t) = tool {
                    if let Some(action) = t.tick(now) {
                        finished_actions.push(action);
                        finished_pushes.push((id, t.compact_state(now)));
                    }
                }
            }
        }

        if !finished_pushes.is_empty() {
            self.persist();
            for (id, val) in finished_pushes {
                let key = format!("tool-{id}");
                let _ = self.change_tx.send(json!({ key: val }));
            }
        }

        for action in finished_actions {
            self.handle_finish_action(action);
        }
    }

    /// Check if any timer or stopwatch is currently running.
    pub fn has_running_tools(&self) -> bool {
        let guard = self.state.lock().unwrap();
        guard.values().any(|tool| match tool {
            ToolState::Timer(t) => t.running,
            ToolState::Stopwatch(s) => s.running,
            _ => false,
        })
    }

    /// Produce legacy 1 Hz formatted labels for all running timers/stopwatches.
    pub fn legacy_formatted_labels(&self) -> HashMap<String, String> {
        let now = self.clock.now_ms();
        let guard = self.state.lock().unwrap();
        let mut map = HashMap::new();
        for (&id, tool) in guard.iter() {
            match tool {
                ToolState::Timer(t) if t.running => {
                    map.insert(format!("tool-{id}"), t.format_legacy(now));
                }
                ToolState::Stopwatch(s) if s.running => {
                    map.insert(format!("tool-{id}"), s.format_legacy(now));
                }
                _ => {}
            }
        }
        map
    }

    fn handle_finish_action(&self, action: FinishAction) {
        match action {
            FinishAction::None => {}
            FinishAction::Play(path) => {
                if !path.is_empty() {
                    if let Err(e) = pulpit_os::play_audio(&path) {
                        tracing::warn!(path = %path, error = %e, "timer sound playback failed");
                    }
                }
            }
            FinishAction::Board(board_id) => {
                if let Some(sink) = self.board_tx.lock().unwrap().as_ref() {
                    let _ = sink.send(board_id);
                }
            }
        }
    }
}

/// Spawns the tool manager service and its background loops.
/// Returns `(Arc<ToolManager>, UnboundedReceiver<Value>)`.
pub fn spawn_tools(
    store_path: PathBuf,
    legacy_hub: Arc<pulpit_legacy::Hub>,
) -> (Arc<ToolManager>, UnboundedReceiver<Value>) {
    let (tx, rx) = unbounded_channel();
    let clock = Arc::new(SystemClock);
    let manager = Arc::new(ToolManager::new(store_path, clock, tx));

    // Finish loop: one sleep to the nearest deadline, none while idle
    tokio::spawn(manager.clone().run_timer_loop());

    // Legacy 1 Hz label push, only while something runs
    tokio::spawn(manager.clone().run_legacy_label_loop(legacy_hub));

    (manager, rx)
}

/// Checks if an action kind is a native tool kind.
pub fn is_tool_action(kind: &str) -> bool {
    matches!(
        kind,
        "tool-clock" | "tool-timer" | "tool-stopwatch" | "tool-counter"
    )
}

/// Declarations for legacy mapper inputs.
pub fn input_declarations() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    vec![
        ("tool-clock", "clock", "#34495e", Some("custom-value")),
        (
            "tool-timer",
            "hourglass-half",
            "#34495e",
            Some("custom-value"),
        ),
        (
            "tool-stopwatch",
            "stopwatch",
            "#34495e",
            Some("custom-value"),
        ),
        (
            "tool-counter",
            "calculator",
            "#34495e",
            Some("custom-value"),
        ),
    ]
}
