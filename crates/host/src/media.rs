//! The system media (SMTC) producer's host lane: strips the host-consumed
//! internal thumbnail bytes key, imports thumbnails into the v2
//! [pulpit_v2::AssetStore], and forwards the rest through the three
//! client lanes as a custom-value status push.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::art::{delete_evicted, SharedArtLru};
use crate::{forward_snapshot, ClientFeed, ProducerGate, StatusApp};

pub const ART_DATA_KEY: &str = pulpit_os::media::ART_DATA_KEY;
pub const ART_EXT_KEY: &str = pulpit_os::media::ART_EXT_KEY;
pub const ART_KEY_KEY: &str = pulpit_os::media::ART_KEY_KEY;

const ART_LRU_CAP: usize = 32;

/// The internal art keys lifted off a snapshot: the OS thread's digest
/// of the cover, and (only when a new cover arrived) the base64 bytes
/// and extension. Decoding is deferred so a memo hit by digest never
/// pays for it.
#[derive(Default)]
struct TakenArt {
    key: Option<String>,
    b64: Option<String>,
    ext: Option<String>,
}

fn take_art(snapshot: &mut Value) -> TakenArt {
    let Some(map) = snapshot.as_object_mut() else {
        return TakenArt::default();
    };
    let take = |map: &mut serde_json::Map<String, Value>, key: &str| {
        map.remove(key).and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        })
    };
    TakenArt {
        key: take(map, ART_KEY_KEY),
        b64: take(map, ART_DATA_KEY),
        ext: take(map, ART_EXT_KEY),
    }
}

/// Remove internal art keys from snapshot, returning the decoded bytes and extension.
pub fn take_art_data(snapshot: &mut Value) -> Option<(Vec<u8>, String)> {
    decode_art(take_art(snapshot))
}

fn decode_art(art: TakenArt) -> Option<(Vec<u8>, String)> {
    let b64 = art.b64?;
    let ext = art.ext.unwrap_or_else(|| "jpg".to_string());
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .ok()?;
    Some((bytes, ext))
}

fn attach_image(snapshot: &mut Value, hash: Option<&str>) {
    let Some(payload) = snapshot.get_mut("media-now-playing") else {
        return;
    };
    match hash {
        Some(hash) => payload["image"] = Value::String(hash.to_string()),
        None => {
            if let Some(map) = payload.as_object_mut() {
                map.remove("image");
            }
        }
    }
}

fn lru_lookup(entries: &mut VecDeque<(String, String)>, art_key: &str) -> Option<String> {
    let index = entries.iter().position(|(k, _)| k == art_key)?;
    let (_, hash) = entries.remove(index).expect("position just found");
    entries.push_back((art_key.to_string(), hash.clone()));
    Some(hash)
}

fn lru_remember(
    entries: &mut VecDeque<(String, String)>,
    art_key: String,
    hash: String,
    remove: &mut dyn FnMut(&str),
) {
    if let Some(index) = entries.iter().position(|(k, _)| *k == art_key) {
        entries.remove(index);
    }
    entries.push_back((art_key, hash));
    while entries.len() > ART_LRU_CAP {
        let (_, evicted) = entries.pop_front().expect("len above cap");
        let still_referenced = entries.iter().any(|(_, h)| h == &evicted);
        if !still_referenced {
            remove(&evicted);
        }
    }
}

fn resolve_art(
    lru: &Mutex<VecDeque<(String, String)>>,
    shared: &SharedArtLru,
    art: TakenArt,
    assets: Option<&Arc<pulpit_v2::AssetStore>>,
) -> Option<String> {
    let assets = assets?;

    // The OS thread digests the raw thumbnail and sends the bytes only
    // when the digest changed: a memo hit by digest needs no decode, no
    // SHA-256 and no import. A snapshot without a digest (no cover, or a
    // producer that predates the key) falls back to hashing the bytes.
    if let Some(key) = &art.key {
        // A memo hit still touches the shared LRU: the other lane's
        // eviction decision must see this cover as live.
        if let Some(hash) = lru_lookup(&mut lru.lock().unwrap(), key) {
            delete_evicted(shared.touch(&hash), assets, "smtc thumbnail");
            return Some(hash);
        }
    }

    let art_key = art.key.clone();
    let (bytes, ext) = decode_art(art)?;
    if bytes.is_empty() {
        return None;
    }
    let art_key = match art_key {
        Some(key) => key,
        None => {
            use sha2::Digest as _;
            let key = hex::encode(sha2::Sha256::digest(&bytes));
            if let Some(hash) = lru_lookup(&mut lru.lock().unwrap(), &key) {
                delete_evicted(shared.touch(&hash), assets, "smtc thumbnail");
                return Some(hash);
            }
            key
        }
    };
    let hash = assets.import_cover_bytes(&bytes, &ext).ok()?;
    // The memo only skips re-imports now; deletions belong to the
    // shared LRU, so its own eviction callback is a no-op.
    lru_remember(
        &mut lru.lock().unwrap(),
        art_key,
        hash.clone(),
        &mut |_evicted: &str| {},
    );
    delete_evicted(shared.touch(&hash), assets, "smtc thumbnail");
    Some(hash)
}

/// The SMTC media producer pump: receiver in, three client lanes out.
/// `art` is the host-wide cover-art LRU ([`crate::art`]) - the SAME
/// instance the Spotify pump got, so neither lane can delete the other
/// lane's live covers.
pub async fn forward_media<F: ClientFeed>(
    feed: Arc<F>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Value>,
    assets: Option<Arc<pulpit_v2::AssetStore>>,
    art: Arc<SharedArtLru>,
) {
    let lru = Mutex::new(VecDeque::new());
    let mut gate = ProducerGate::default();
    while let Some(mut snapshot) = rx.recv().await {
        let taken = take_art(&mut snapshot);
        let hash = resolve_art(&lru, &art, taken, assets.as_ref());
        attach_image(&mut snapshot, hash.as_deref());
        forward_snapshot(&*feed, StatusApp::CustomValue, &mut gate, &snapshot).await;
    }
}

/// Disabled / idle snapshot for SMTC when no provider is active.
pub fn disabled_snapshot() -> Value {
    pulpit_os::media::build_snapshot(None, None, None, "", None)
}

pub async fn forward_media_disabled<F: ClientFeed>(feed: Arc<F>) {
    let mut gate = ProducerGate::default();
    let mut snapshot = disabled_snapshot();
    let _ = take_art_data(&mut snapshot);
    forward_snapshot(&*feed, StatusApp::CustomValue, &mut gate, &snapshot).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulpit_v2::StateEngine;

    fn b64(bytes: &[u8]) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    struct FakeFeed {
        engine: StateEngine,
        broadcasts: Mutex<Vec<String>>,
        emits: Mutex<Vec<Value>>,
    }

    impl ClientFeed for FakeFeed {
        fn engine_set(&self, key: &str, value: Value) {
            self.engine.set(&pulpit_v2::ext_channel(key), value);
        }
        async fn broadcast_status(&self, payload: &str) {
            self.broadcasts.lock().unwrap().push(payload.to_string());
        }
        fn emit_status(&self, payload: &Value) -> bool {
            self.emits.lock().unwrap().push(payload.clone());
            true
        }
    }

    fn feed() -> Arc<FakeFeed> {
        Arc::new(FakeFeed {
            engine: StateEngine::new(64),
            broadcasts: Mutex::new(Vec::new()),
            emits: Mutex::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn forwarded_snapshots_carry_the_image_hash_never_the_art_data() {
        let dir = tempfile::tempdir().unwrap();
        let assets = Arc::new(pulpit_v2::AssetStore::open(dir.path().to_path_buf()).unwrap());
        let feed = feed();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let pump_feed = feed.clone();
        let pump_assets = Some(assets.clone());
        let art = Arc::new(SharedArtLru::new());
        let pump = tokio::spawn(async move {
            forward_media(pump_feed, rx, pump_assets, art).await;
        });

        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"cover-bytes-1");
        let snap = serde_json::json!({
            "media-playing": "ON",
            "media-now-playing": { "title": "Track", "rows": [] },
            "media-art-data": b64,
            "media-art-ext": "jpg",
        });

        tx.send(snap).unwrap();
        drop(tx);
        pump.await.unwrap();

        let emits = feed.emits.lock().unwrap();
        assert_eq!(emits.len(), 1);
        let first = &emits[0]["data"];
        assert!(first.get("media-art-data").is_none());
        assert!(first.get("media-art-ext").is_none());
        let hash = first["media-now-playing"]["image"]
            .as_str()
            .expect("hash attached");
        assert_eq!(hash.len(), 64);
        assert_eq!(assets.get(hash).unwrap(), b"cover-bytes-1");
    }

    #[tokio::test]
    async fn art_memo_by_digest_imports_once_and_keeps_the_image_without_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let assets = Arc::new(pulpit_v2::AssetStore::open(dir.path().to_path_buf()).unwrap());
        let feed = feed();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let pump_feed = feed.clone();
        let pump_assets = Some(assets.clone());
        let art = Arc::new(SharedArtLru::new());
        let pump = tokio::spawn(async move {
            forward_media(pump_feed, rx, pump_assets, art).await;
        });

        let digest = pulpit_os::media::art_digest(b"cover-bytes-1");
        // first snapshot: digest + bytes (a new cover)
        tx.send(serde_json::json!({
            "media-progress": "0.100",
            "media-now-playing": { "title": "Track", "rows": [] },
            "media-art-key": digest,
            "media-art-data": b64(b"cover-bytes-1"),
            "media-art-ext": "jpg",
        }))
        .unwrap();
        // second: another track off the same album - same digest, bytes
        // withheld by the OS thread (the title differs so the delta gate
        // forwards the now-playing payload again)
        tx.send(serde_json::json!({
            "media-progress": "0.200",
            "media-now-playing": { "title": "Track 2", "rows": [] },
            "media-art-key": digest,
        }))
        .unwrap();
        drop(tx);
        pump.await.unwrap();

        let emits = feed.emits.lock().unwrap();
        assert_eq!(emits.len(), 2);
        let first = emits[0]["data"]["media-now-playing"]["image"]
            .as_str()
            .expect("first image");
        let second = emits[1]["data"]["media-now-playing"]["image"]
            .as_str()
            .expect("second snapshot keeps the image");
        assert_eq!(first, second);
        assert_eq!(assets.get(first).unwrap(), b"cover-bytes-1");
        for emit in emits.iter() {
            let data = &emit["data"];
            assert!(data.get("media-art-key").is_none(), "digest is internal");
            assert!(data.get("media-art-data").is_none());
            assert!(data.get("media-art-ext").is_none());
        }
    }

    #[tokio::test]
    async fn a_new_digest_with_bytes_swaps_the_cover() {
        let dir = tempfile::tempdir().unwrap();
        let assets = Arc::new(pulpit_v2::AssetStore::open(dir.path().to_path_buf()).unwrap());
        let feed = feed();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let pump_feed = feed.clone();
        let pump_assets = Some(assets.clone());
        let art = Arc::new(SharedArtLru::new());
        let pump = tokio::spawn(async move {
            forward_media(pump_feed, rx, pump_assets, art).await;
        });

        for cover in [b"cover-a".as_slice(), b"cover-b".as_slice()] {
            tx.send(serde_json::json!({
                "media-now-playing": { "title": "Track", "rows": [] },
                "media-art-key": pulpit_os::media::art_digest(cover),
                "media-art-data": b64(cover),
                "media-art-ext": "jpg",
            }))
            .unwrap();
        }
        drop(tx);
        pump.await.unwrap();

        let emits = feed.emits.lock().unwrap();
        let a = emits[0]["data"]["media-now-playing"]["image"]
            .as_str()
            .unwrap();
        let b = emits[1]["data"]["media-now-playing"]["image"]
            .as_str()
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(assets.get(b).unwrap(), b"cover-b");
    }

    #[tokio::test]
    async fn disabled_push_marks_idle() {
        let feed = feed();
        forward_media_disabled(feed.clone()).await;
        let sync = feed.engine.snapshot();
        assert_eq!(sync.values["ext.media-playing"], serde_json::json!("OFF"));
    }
}
