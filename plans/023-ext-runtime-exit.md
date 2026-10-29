# Plan 023: Extension runtimes — thread exit when drained, one-shot laziness, no idle wake

Round 7, from audit C (measured on a Linux harness: a 490 KB stateless package grew 17→169 MB RSS after ONE execute because the parked thread keeps the freed heap; drained runtimes wake 1×/s each for nothing; a trivial package costs ~2.3 MB). Priority P1, Effort M, Risk MED.
Files: crates/ext/src/{manager,host,prelude}.rs (+ in-file tests).

## Binding design
1. **Thread exit when drained**: a runtime thread whose interval set drains (no live intervals, no pending one-shots) exits instead of parking forever. Exit must be safe mid-`execute` queue wait — the manager re-spawns on demand exactly like the cold start path. The interpreter is dropped with the thread, returning the heap.
2. **One-shot laziness**: `setTimeout` is emulated as a self-clearing interval, so `setTimeout(cb, 0)`-only initializers pin a resident runtime. Classify a package lazy when — after running its pending one-shots — no interval remains AND the package did not use `spawn`/`http`/`nextTick`/`setImmediate` (those legitimately keep it resident). The probe must run pending one-shots BEFORE deciding (a one-shot may register a real interval).
3. **No empty wake**: a drained-but-resident runtime (e.g. http-capable package that went quiet) must not wake every second; park on the job queue only.
4. **Stacks stay 64 MiB** — audit C measured ~91 kB resident per trivial thread (virtual reservation) and Boa needs >16 MiB for deep expressions; overflow aborts the process. Do NOT shrink. This confirms the prior round's condition; record as rejected in the index.
5. Preserve: behavior of packages with real intervals (timers, watchers) must be byte-identical; the desktop's activity/idle stretching (IDLE_TICK_FLOOR) unchanged.

## Measurements to reproduce (acceptance)
With the ext test harness style used by audit C (throwaway, /tmp): a one-shot-only package must leave NO resident thread after its callback; an executed-then-idle stateless package's thread exits and RSS returns near baseline; a package with a live interval keeps exactly one resident thread that does not wake while idle.

## Verification
cargo test -p pulpit-ext; cargo clippy -p pulpit-ext --all-targets; fmt. Linux builds fine.

## STOP
If thread exit cannot be made safe for a pending execute (race with an arriving action) without a redesign — report instead of forcing.

## Reviewer note
This is the single largest RAM item of the round (up to ~150 MB per heavy stateless package used once).
