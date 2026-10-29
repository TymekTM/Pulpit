//! The cover-art LRU shared by every art-importing producer lane.
//!
//! The SMTC lane ([`crate::media`]) and the Spotify lane
//! ([`crate::spotify`]) both import covers into the one content-addressed
//! [`pulpit_v2::AssetStore`]. Each used to keep a private eviction LRU
//! and consulted only its own deque before deleting a file, so one lane
//! could delete a hash the other lane's last payload still referenced
//! (byte-identical covers - the same album on both lanes - collide on
//! one content-addressed hash). One shared, hash-keyed LRU makes every
//! eviction decision see both lanes' live covers: a hash is deleted only
//! once no lane has touched it within the last [`SHARED_ART_LRU_CAP`]
//! distinct covers.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Combined cap across both lanes (the two former 32-entry per-lane
/// deques, merged): a hash is deleted only after this many distinct
/// covers were touched without it.
pub(crate) const SHARED_ART_LRU_CAP: usize = 64;

/// The live cover hashes, oldest first. The deque IS the reference set:
/// a hash inside it must not be deleted from the store.
pub struct SharedArtLru {
    entries: Mutex<VecDeque<String>>,
}

impl SharedArtLru {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
        }
    }

    /// Record `hash` as live (re-touching promotes it to newest).
    /// Returns the hashes that fell out of the cap and are referenced by
    /// NO remaining entry: the caller deletes those from the AssetStore,
    /// which itself still refuses any hash a board image resolved to.
    pub fn touch(&self, hash: &str) -> Vec<String> {
        touch_entries(&mut self.entries.lock().unwrap(), hash)
    }
}

impl Default for SharedArtLru {
    fn default() -> Self {
        Self::new()
    }
}

/// Pure body of [`SharedArtLru::touch`] so the eviction contract is
/// unit-testable (the same shape as the lane-private `lru_lookup` /
/// `lru_remember` pair).
fn touch_entries(entries: &mut VecDeque<String>, hash: &str) -> Vec<String> {
    if let Some(index) = entries.iter().position(|h| h == hash) {
        let hash = entries.remove(index).expect("position just found");
        entries.push_back(hash); // re-touch promotes to newest
        return Vec::new();
    }
    entries.push_back(hash.to_string());
    let mut evicted = Vec::new();
    while entries.len() > SHARED_ART_LRU_CAP {
        let evicted_hash = entries.pop_front().expect("len above cap");
        let still_referenced = entries.iter().any(|h| h == &evicted_hash);
        if !still_referenced {
            evicted.push(evicted_hash);
        }
    }
    evicted
}

/// Delete the hashes [`SharedArtLru::touch`] returned: they fell out of
/// the combined cap and no remaining entry references them. Both
/// art-importing lanes call this on every resolve, memo hit or not, so
/// an eviction decision always sees the other lane's live covers.
pub(crate) fn delete_evicted(hashes: Vec<String>, assets: &pulpit_v2::AssetStore, kind: &str) {
    for hash in hashes {
        if assets.remove(&hash) {
            tracing::debug!(hash, "evicted {kind} from the asset store");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_evicts_the_oldest_past_the_cap() {
        let lru = SharedArtLru::new();
        for i in 0..SHARED_ART_LRU_CAP {
            assert!(lru.touch(&format!("h{i}")).is_empty());
        }
        // one more distinct hash evicts the oldest, exactly once
        assert_eq!(lru.touch("h-new"), ["h0"]);
        // the evicted hash is gone: touching it again is a fresh insert
        // (a still-live hash would promote silently instead), which
        // itself evicts the next-oldest
        assert_eq!(lru.touch("h0"), ["h1"]);
    }

    #[test]
    fn re_touch_promotes_to_newest_and_never_double_returns() {
        let lru = SharedArtLru::new();
        for i in 0..SHARED_ART_LRU_CAP {
            lru.touch(&format!("h{i}"));
        }
        // re-touching the oldest promotes it: the next eviction takes
        // h1 instead, and h0 is never returned for deletion
        assert!(lru.touch("h0").is_empty());
        assert_eq!(lru.touch("h-new"), ["h1"]);
        // repeated touches stay silent - a live hash is never returned
        for _ in 0..3 {
            assert!(lru.touch("h-new").is_empty());
        }
    }

    #[test]
    fn an_evicted_hash_still_referenced_by_a_remaining_entry_is_not_returned() {
        // the public API dedupes, but the eviction contract must also
        // hold if the same hash is in the deque more than once (the
        // shape the old per-lane deques produced): a hash only leaves
        // with its LAST reference
        let lru = SharedArtLru::new();
        {
            let mut entries = lru.entries.lock().unwrap();
            for _ in 0..SHARED_ART_LRU_CAP {
                entries.push_back("shared".to_string());
            }
        }
        let mut returned = Vec::new();
        for i in 0..=SHARED_ART_LRU_CAP {
            returned.extend(lru.touch(&format!("n{i}")));
        }
        // every "shared" copy except the last was evicted while another
        // remained and was withheld; the last one is returned exactly
        // once, followed by the ordinary eviction of n0
        assert_eq!(returned, ["shared", "n0"]);
    }
}
