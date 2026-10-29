# Plan 018: Share one art LRU across the two media producers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in the "STOP conditions" section occurs,
> stop and report — do not improvise. When done, update the status row
> for this plan in `plans/README.md` — unless a reviewer dispatched you
> and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 6dcddf3..HEAD -- crates/host/src/media.rs crates/host/src/spotify.rs crates/host/src/lib.rs apps/desktop/src-tauri/src/lib.rs apps/server/src/main.rs`
> Files other than `crates/host/src/media.rs` may have drifted from the
> 2026-10-29 cover-fix merge; compare the "Current state" excerpts
> against the live code before proceeding; on a mismatch, treat it as a
> STOP condition.

## Status

- **Priority**: P3
- **Effort**: S-M
- **Risk**: LOW
- **Depends on**: plans/017-discord-linux-build.md (only for running
  `cargo test -p pulpit-host` on Linux; before 017 lands, verify this
  plan's host tests via `cargo check --target x86_64-pc-windows-msvc
  -p pulpit-host --all-targets` + CI)
- **Category**: bug
- **Planned at**: commit `6dcddf3`, 2026-10-29

## Why this matters

Two producers import cover art into the single content-addressed
`AssetStore` and each keeps its OWN 32-entry eviction LRU:

- the SMTC lane (`crates/host/src/media.rs`) keyed by sha256(bytes),
- the Spotify Web API lane (`crates/host/src/spotify.rs`) keyed by CDN
  URL.

An LRU eviction only checks "is this hash still referenced within MY
deque" before deleting the file from the store. Producer A can
therefore delete a file that producer B's last payload still
references: the desktop tile then fails its `asset_data_url` lookup
(the WebView retries after 10 s since PR #25; mobile has its own retry
ladder) and tablets briefly render the card without art. One shared,
hash-keyed LRU (cap 64) makes every eviction decision see both
producers' live covers, which kills the cross-delete class outright.

## Current state

- `crates/host/src/media.rs` — SMTC pump. `resolve_art` (lines ~75-110)
  hashes the thumbnail bytes, consults a private
  `Mutex<VecDeque<(String, String)>>` (bytes-hash -> asset hash,
  `ART_LRU_CAP = 32`), imports via `assets.import_bytes`, and
  `lru_remember` (lines ~56-73) evicts + deletes:

  ```rust
  while entries.len() > ART_LRU_CAP {
      let (_, evicted) = entries.pop_front().expect("len above cap");
      let still_referenced = entries.iter().any(|(_, h)| h == &evicted);
      if !still_referenced {
          remove(&evicted);
      }
  }
  ```

  Pump entry: `pub async fn forward_media<F: ClientFeed>(feed, rx, assets: Option<Arc<AssetStore>>)`
  (line ~113) creates `let lru = Mutex::new(VecDeque::new());` locally.

- `crates/host/src/spotify.rs` — Spotify Web API pump. Same shape keyed
  by URL: `lru_lookup`/`lru_remember` (lines ~69-98, `ART_LRU_CAP = 32`),
  `resolve_art` downloads via ureq then imports; pump entry
  `pub async fn forward_spotify<F: ClientFeed>(feed, rx, assets, art_fetch)`
  (line ~198).

- `AssetStore::remove` (`crates/v2/src/assets.rs`) already refuses
  hashes a board image resolved to (`pinned`) — that protection exists
  and must not be duplicated in the new LRU.

- Wiring (call sites to update):
  - `apps/desktop/src-tauri/src/lib.rs:671` —
    `pulpit_host::spotify::forward_spotify(...)`;
  - `apps/desktop/src-tauri/src/lib.rs:704` —
    `pulpit_host::media::forward_media(...)`;
  - `apps/server/src/main.rs:290` —
    `pulpit_host::media::forward_media(...)` (the server runs no
    Spotify lane).

- Repo conventions: pure helper + in-file unit tests (see the LRU tests
  in `crates/host/src/media.rs` / `spotify.rs` test modules), pumps are
  `pub async fn` generics over `F: ClientFeed`, hosts construct shared
  state and pass it in (`Arc<AssetStore>` is the example directly
  above).

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Host tests (Linux, needs 017) | `cargo test -p pulpit-host` | all pass incl. new tests |
| Host tests (any OS, before 017) | `cargo check -p pulpit-host --target x86_64-pc-windows-msvc --all-targets` | exit 0 |
| Dependent check | `cargo check -p pulpit-desktop 2>/dev/null \|\| cargo check --workspace --exclude pulpit-discord` | exit 0 for host+server (desktop may legitimately fail on Linux pre-017; report) |
| Lint | `cargo clippy -p pulpit-host --all-targets` | no new warnings |

## Scope

**In scope**:
- `crates/host/src/lib.rs` (or a new `crates/host/src/art.rs` if you
  prefer a dedicated module — either is fine, pick one)
- `crates/host/src/media.rs`, `crates/host/src/spotify.rs`
- `apps/desktop/src-tauri/src/lib.rs`, `apps/server/src/main.rs`
  (constructor + the three call sites only)

**Out of scope**:
- `crates/v2/src/assets.rs` — the `pinned` mechanism and store stay
  untouched.
- The per-producer key->hash memos (bytes-hash lookup, URL lookup):
  keep them; they exist to skip re-imports, which the shared LRU does
  not know about.
- Any wire/payload change: the `image` hash semantics are unchanged.
- The WebView/mobile retry ladders (already landed).

## Git workflow

- Branch: `advisor/018-shared-art-lru`
- Commits per step; message style: `Share one art LRU across the media producers`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Add the shared LRU type

In `crates/host` (lib.rs or new art.rs):

```rust
/// Cross-producer cover-art LRU, keyed by AssetStore hash: the SMTC and
/// Spotify lanes import into one AssetStore, so eviction must see both
/// producers' live covers before deleting a file (a private LRU could
/// delete a hash the other lane's payload still references).
pub struct SharedArtLru {
    entries: std::sync::Mutex<std::collections::VecDeque<String>>,
}
const SHARED_ART_LRU_CAP: usize = 64;

impl SharedArtLru {
    pub fn new() -> Self { ... }
    /// Record a hash as live. Returns the hashes that fell out of the
    /// cap and are referenced by NO remaining entry: the caller deletes
    /// those from the AssetStore (which itself refuses board pins).
    pub fn touch(&self, hash: &str) -> Vec<String> { ... }
}
```

Semantics: dedupe on insert, push to the back (newest), evict from the
front while `len > 64`, and only return an evicted hash when it no
longer appears anywhere in the deque (the same `still_referenced` check
the two private LRUs do today). `Default` impl too (clippy likes it).

**Verify**: `cargo check -p pulpit-host` (pre-017 Linux: use the
windows-msvc target check from the commands table) → exit 0.

### Step 2: Unit-test the LRU

In the same module's `#[cfg(test)]`, following the style of the
existing `lru` tests in `crates/host/src/media.rs`:
- touch beyond the cap evicts the oldest;
- a hash touched twice stays newest and is never returned twice;
- an evicted hash still referenced by a later entry is NOT returned.

**Verify**: `cargo test -p pulpit-host shared_art` (post-017) or the
windows-msvc check (pre-017) → pass.

### Step 3: Thread it through both pumps

- `forward_media(feed, rx, assets)` → `forward_media(feed, rx, assets,
  art: std::sync::Arc<SharedArtLru>)`. In `resolve_art`, after the
  existing private-LRU hit OR a fresh `import_bytes`, call
  `art.touch(hash)` and delete every returned hash via the same
  `store.remove` closure used today. Remove the private
  delete-from-store eviction (`lru_remember` keeps maintaining the
  key->hash memo but its `remove` callback becomes a no-op — or inline
  the memo and drop the callback; pick the smaller diff).
- Same for `forward_spotify` and its `resolve_art`.
- Do NOT change the public `image`-attachment logic.

**Verify**: compile gate from the commands table → exit 0.

### Step 4: Wire the hosts

- Desktop (`apps/desktop/src-tauri/src/lib.rs`): construct one
  `Arc<SharedArtLru>` next to where the `AssetStore` is created and
  pass the same instance to `forward_spotify` (line ~671) and
  `forward_media` (line ~704).
- Server (`apps/server/src/main.rs:290`): construct one and pass it to
  `forward_media`.
- Update the in-file host tests that call `forward_media` /
  `forward_spotify` directly (they construct the args today; add
  `Arc::new(SharedArtLru::new())`).

**Verify**: `cargo test -p pulpit-host` (post-017) → all pass,
including `forwarded_snapshots_carry_the_image_hash_never_the_art_data`.

### Step 5: Cross-producer regression test

New test in `crates/host/src/media.rs`'s test module (or next to the
shared LRU): two different art byte sets driven through the shared LRU
(the SMTC side and — minimally — the LRU type directly, since driving
the full spotify pump needs its scripted `ArtFetch`): assert that after
interleaving 33+ touches from "producer A", a hash "producer B" kept
touching is never returned for deletion. Model the plumbing on
`forwarded_snapshots_carry_the_image_hash_never_the_art_data`.

**Verify**: `cargo test -p pulpit-host` → all pass.

## Test plan

- New: shared-LRU unit tests (cap eviction, dedupe/promotion,
  still-referenced guard) + the cross-producer regression above.
- Existing: `forwarded_snapshots_carry_the_image_hash_never_the_art_data`
  and the spotify lane's art tests must keep passing unchanged in
  behavior (signature updates only).

## Done criteria

- [ ] `cargo test -p pulpit-host` exits 0 (or, pre-017, the
      windows-msvc `--all-targets` check exits 0 and CI's test job is
      noted as the executor)
- [ ] `cargo clippy -p pulpit-host --all-targets` adds no warnings
- [ ] `grep -n "ART_LRU_CAP" crates/host/src/media.rs crates/host/src/spotify.rs`
      shows the private caps still exist as memos but no
      `store.remove` eviction outside `SharedArtLru::touch`'s callers
- [ ] Only in-scope files modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The pump signatures are used in more places than the three call sites
  listed (unexpected wiring).
- Making the spotify lane testable requires inventing more than the
  existing scripted `ArtFetch` seam.
- The desktop crate fails to compile on Linux for reasons unrelated to
  this change (expected pre-017 for other crates — report, don't fix).
- A verification fails twice after a reasonable fix attempt.

## Maintenance notes

- The cap moved from 32/private to 64/shared: worst-case disk for
  covers is ~64 files of ≤5 MiB (SMTC) — bounded and fine; note it in
  the PR description.
- Reviewer: the ONLY behavioral change must be "fewer deletions";
  imports, hashes, payloads and wire shapes are untouched.
- If a third art producer ever appears, it plugs into the same
  `Arc<SharedArtLru>` — no new eviction logic.
