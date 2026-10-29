# Plan 024: Asset GC across restarts + fast, low-RSS startup

Round 7, from audits A (MEM-01) and C (SRV-2: 390 MB of accumulated assets → 1.8 s startup and 160 MB peak RSS at open vs 17 ms / 7.7 MB empty). Priority P2, Effort S-M, Risk LOW-MED.
Files: crates/v2/src/assets.rs, crates/host/src/{media,spotify}.rs (+tests); optionally apps/server/src/main.rs + desktop lib.rs call sites unchanged.

## Binding design
1. **Producer covers live in a subdirectory** (`assets/covers/`), board images stay in the root (existing behavior). AssetStore::open rebuilds the board-image pin set (existing `pinned` logic — verify it is derivable at open: every board image goes through import_data_url, which pins; but at REOPEN the pins must be re-derived from boards, not memory — check how `pinned` survives restarts today; if it does not, re-pin by importing boards' data-URLs at open is NOT viable — instead: root = board images (only they are written by board builds), covers = subdir).
2. **Wipe covers at open**: the Spotify lane's module doc says "temporary caching only" — covers are disposable by contract. At open, delete the covers subdir contents (or skip loading them into `exts` entirely and delete lazily on next import of the same hash). Clients re-fetch on demand; the desktop WebView has its own 10 s retry anyway.
3. **No hashing of covers at open**: only root files (board images) are verified+indexed; startup cost returns to board-image volume only.
4. **/assets/<hash> serving** must look in both root and covers dir (hash → file map built from root at open; covers resolve on demand by path convention `<dir>/covers/<hash>.<ext>` — extension discovery: try known exts or keep a tiny on-import index file; simplest: keep the covers subdir files' exts in a small JSON index written on import, loaded lazily).
   Simpler alternative if clean: keep covers in the SAME dir but with a `cover-` filename prefix that open() skips+deletes. Pick the smaller diff; document the choice.
5. Host lanes import covers through the store unchanged (the store routes by caller kind: add `import_cover_bytes` or a flag).

## Verification
cargo test -p pulpit-v2 -p pulpit-host (new tests: cover files wiped at reopen; board image survives; serving a cover after re-import works; startup does not read cover bytes); clippy+fmt; msvc cross-check for pulpit-v2.

## STOP
If `pinned` pins do not survive restarts today AND board images cannot be distinguished from covers by import path — report the two options instead of picking silently.

## Reviewer note
Unbounded disk growth across launches is the real bug; the 1.8 s/160 MB startup is its symptom.
