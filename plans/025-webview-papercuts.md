# Plan 025: WebView papercuts sweep — tool-tile ticking, media-tile effects, clock formatters, art-cache LRU

Round 7, from audit B (PERF-02/03/04, MEM-01). All S/LOW risk. One sweep PR.
Files: apps/desktop/src/components/tiles/ToolTile.vue, apps/desktop/src/components/TileCell.vue, apps/desktop/src/statusMedia.js, tests as needed.

## Changes (each independent, verifiable by existing suite)
1. **ToolTile tick** (`ToolTile.vue:37-39`): timers/stopwatches tick at 200 ms for a display that changes 1×/s (`mmss` rounds up). Change to 1000 ms — better: schedule each tick at the next whole-second boundary of the DISPLAYED value (setTimeout chain keyed on the value's phase). Gate the interval on `windowVisible` from statusMedia.js exactly like TileCell's media tick (`TileCell.vue:283-292`), tearing down when hidden. Cap the finished-timer flash animation (~6 cycles then static highlighted state).
2. **TileCell effects re-run per push** (`TileCell.vue:266-275, 283-292`): both watchEffects read through `statusData.value?.…` — the payload object is replaced ~1×/s while playing, so both effects re-run and the interval restarts each push. Add `const statusImage = computed(() => statusData.value?.image)` and `const progressPlaying = computed(() => statusData.value?.progress?.playing === true)`; make the effects read ONLY those primitives (unchanged primitive → no re-run).
3. **Clock formatters** (`ToolTile.vue:137-156`, called twice per render at `:250/:252`): cache the two Intl.DateTimeFormat objects as computeds keyed on (locale, hour12, seconds, timeZone); compute parts once per tick.
4. **Art cache FIFO→LRU** (`statusMedia.js:52-58`): a hit returns without re-inserting, so the CURRENTLY PLAYING cover is evicted once 32 others passed (self-heals but costs an IPC + re-encode + art-less frame). On hit: `delete` + `set` to refresh recency (matches the comment's own claim and the host's LRU).

## Verification
npx vitest run (all green; add a test: tool tick does not run while windowVisible=false — model after the TileCell ticker test); npx vite build. No behavior change beyond timing.

## STOP
Any test that depends on the 200 ms cadence beyond timing tolerance.

## Reviewer note
Small wins individually (~ms/s each) but they also remove 4 wasted renders/s per running tool tile and per-second timer churn — predictability more than CPU.
