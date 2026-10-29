# Plan 022: Take SMTC cover art off the per-second path (+ per-session app-name cache)

Round 7, from audits A (PERF-01, PERF-03) and B's handoff. Priority P1, Effort M, Risk MED.
Files: crates/os/src/media/win_media.rs, crates/os/src/media.rs, crates/host/src/media.rs (+tests in each).

## Why
Every 1 s poll — PAUSED sessions included — reads the whole thumbnail stream, base64-encodes it into the snapshot, serializes the full string for the change-compare (keeping a permanent 133 KB+ copy), and on change clones + decodes + SHA-256s it in the pump (which SHA-256s again on import). ~0.75 MB alloc/s at 100 KB covers, ~30 MB/s at the 5 MiB cap; also `aumid_to_friendly_name` enumerates the HKCU registry on every poll for apps outside the hardcoded list (~100-600 allocs/s).

## Binding design
1. **Art key compare**: on the OS thread, SHA-256 the raw thumbnail bytes; build_snapshot gains an art digest. The per-poll compare string must NOT contain base64 art anymore — compare `media-art-key` (the digest) instead. The snapshot still carries ART_DATA_KEY bytes, but the loop only SENDS art bytes when the digest changed since the last SENT snapshot (track last_sent_art_digest; when unchanged, strip ART_DATA_KEY/ART_EXT_KEY from the outgoing snapshot).
2. **Host memo by key**: crates/host/src/media.rs resolve_art gets the digest (a new internal key `media-art-key`, stripped like the others). Memo hit by digest → no decode, no sha256, no import. Decode+import happen only when bytes actually arrive (new cover). The follow-up re-poll (400 ms) keeps working: its snapshot re-sends art only if the digest changed — which is exactly the late-thumbnail case.
3. **Paused sessions**: with the digest unchanged and no other change, the poll sends nothing (today it sends nothing too — the dedupe holds; the win is no per-second read/encode/hash work: still read the thumbnail? NO — skip `read_thumbnail` when the previous poll's properties (title/artist/album/track) are unchanged AND the session id is unchanged; the thumbnail cannot change without a properties event waking us. Keep one read per session start and per properties-change event.)
4. **App-name cache**: compute `aumid_to_friendly_name` only when `current_id != last_session_id`; cache the result alongside. The control path may keep its own call.
5. Tests: media.rs build_snapshot digest presence/absence; host/media.rs memo-by-digest (same digest twice → one import, second snapshot without art bytes keeps image); win_media is Windows-only — verify via msvc cross-check + CI windows job.

## Verification
cargo test -p pulpit-os -p pulpit-host; cargo clippy + fmt on both; cargo check -p pulpit-os --target x86_64-pc-windows-msvc --all-targets; host tests via -p pulpit-host (Linux OK).

## STOP
If the stuck-cover regression guard (the compare must still see art CHANGES via the digest) cannot be preserved; if skipping read_thumbnail on unchanged properties risks missing art-only updates for some player (then keep the read but keep digest-compare — S fallback, still kills encode/serialize/hash work).

## Reviewer note
The digest MUST reach the host lane as an internal stripped key, never a client-visible field.
