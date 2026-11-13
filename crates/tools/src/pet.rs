//! The desk pet store: the simulation runs in the desktop webview
//! (`apps/desktop/src/tamagotchi.js`); the server only persists the
//! compact snapshot to `~/pulpitApp/pet.json` and relays it to every
//! client through the same producer lane as the tools. Care gestures
//! coming back from any surface are forwarded to the webview, which
//! applies them to the simulation and pushes the next snapshot. The
//! snapshot is an opaque JSON object - the rules never cross into Rust.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

/// The pet tile kind (the tile carries no per-id state: the pet is
/// global, so every pet tile on any board renders the same animal).
pub fn is_pet_action(kind: &str) -> bool {
    kind == "pet"
}

/// Shared pet store: persistence + relay + gesture forwarding.
pub struct PetManager {
    store_path: PathBuf,
    state: Mutex<Option<Value>>,
    change_tx: UnboundedSender<Value>,
    gesture_tx: Mutex<Option<UnboundedSender<String>>>,
}

impl PetManager {
    pub fn new(store_path: PathBuf, change_tx: UnboundedSender<Value>) -> Self {
        let state = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok());
        Self {
            store_path,
            state: Mutex::new(state),
            change_tx,
            gesture_tx: Mutex::new(None),
        }
    }

    /// The last pushed snapshot, for the connecting webview to adopt.
    pub fn get(&self) -> Option<Value> {
        self.state.lock().unwrap().clone()
    }

    /// Store a new snapshot from the simulation: persist atomically and
    /// relay on the `pet` channel (v2 `ext.pet`, legacy APP_CUSTOM_VALUE).
    pub fn set(&self, state: Value) {
        *self.state.lock().unwrap() = Some(state.clone());
        let _ = self.persist();
        let _ = self.change_tx.send(json!({ "pet": state }));
    }

    /// Forward a care gesture from any surface to the webview
    /// (`tap` | `double-tap` | `long-press`). No-op without a webview:
    /// the pet simply pauses, like it does while the app is closed.
    pub fn gesture(&self, gesture: &str) {
        if let Some(tx) = self.gesture_tx.lock().unwrap().as_ref() {
            let _ = tx.send(gesture.to_string());
        }
    }

    pub fn set_gesture_sink(&self, tx: UnboundedSender<String>) {
        *self.gesture_tx.lock().unwrap() = Some(tx);
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        let state = self.state.lock().unwrap();
        let Some(state) = state.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = self.store_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        pulpit_db::write_atomic(&self.store_path, &bytes)
    }
}

pub type SharedPetManager = Arc<PetManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tokio::sync::mpsc::unbounded_channel;

    fn manager(
        dir: &Path,
    ) -> (
        PetManager,
        tokio::sync::mpsc::UnboundedReceiver<Value>,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let (change_tx, change_rx) = unbounded_channel();
        let (gesture_tx, gesture_rx) = unbounded_channel();
        let mgr = PetManager::new(dir.join("pet.json"), change_tx);
        mgr.set_gesture_sink(gesture_tx);
        // no runtime here: the tests drain the receivers with
        // blocking_recv
        (mgr, change_rx, gesture_rx)
    }

    #[test]
    fn set_persists_and_relays_on_the_pet_channel() {
        let dir = std::env::temp_dir().join(format!("pulpit-pet-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mgr, mut change_rx, _gestures) = manager(&dir);

        assert!(mgr.get().is_none());
        mgr.set(json!({ "species": "cat", "stage": "baby", "label": "Kot · Zadowolone" }));

        let stored = mgr.get().expect("state kept in memory");
        assert_eq!(stored["species"], "cat");

        let pushed = change_rx.blocking_recv().expect("change pushed");
        assert_eq!(pushed["pet"]["species"], "cat");
        assert_eq!(pushed["pet"]["label"], "Kot · Zadowolone");

        // persisted atomically to pet.json
        let raw = std::fs::read_to_string(dir.join("pet.json")).expect("pet.json written");
        assert!(raw.contains("\"species\": \"cat\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_a_previous_snapshot_from_disk() {
        let dir = std::env::temp_dir().join(format!("pulpit-pet-reload-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pet.json"),
            r#"{ "species": "frog", "stage": "adult", "food": 12 }"#,
        )
        .unwrap();

        let (change_tx, _change_rx) = unbounded_channel();
        let mgr = PetManager::new(dir.join("pet.json"), change_tx);
        let stored = mgr.get().expect("loaded");
        assert_eq!(stored["species"], "frog");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gestures_forward_to_the_webview_sink() {
        let dir = std::env::temp_dir().join(format!("pulpit-pet-gest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mgr, _change_rx, mut gesture_rx) = manager(&dir);

        mgr.gesture("tap");
        assert_eq!(gesture_rx.blocking_recv().unwrap(), "tap");
        mgr.gesture("long-press");
        assert_eq!(gesture_rx.blocking_recv().unwrap(), "long-press");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
