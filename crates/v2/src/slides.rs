//! Per-tile ordering for slider values.
//!
//! A slider tile sends a `slide` event roughly every 120 ms while the
//! finger moves, plus a final one on release. Each event used to get its
//! own fire-and-forget `spawn_blocking`, and since some slider backends
//! are slow and synchronous (Spotify volume is one HTTPS request per
//! value; Discord is serialized behind a mutex), several tasks for the
//! same tile ran at once and could finish in any order - leaving the
//! volume on a stale value instead of where the finger stopped.
//!
//! [`SlideLanes`] keeps one lane per tile id: while a lane's worker is
//! busy, a newer job for that tile replaces any job still waiting
//! (intermediate values are obsolete for an absolute slider), the worker
//! runs jobs one at a time in submission order, and the last submitted
//! job always runs. Different tiles never block each other.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Default)]
pub(crate) struct SlideLanes {
    /// Present key = a worker is running for that tile; the value is the
    /// newest job waiting behind it, if any.
    lanes: Mutex<HashMap<i64, Option<Job>>>,
}

impl SlideLanes {
    /// Queues `job` on `tile`'s lane. If a worker already runs the lane,
    /// `job` replaces any older waiting job (latest wins); otherwise a
    /// worker starts and drains the lane. Returns immediately; never
    /// blocks on a running job.
    pub(crate) fn submit(self: &Arc<Self>, tile: i64, job: impl FnOnce() + Send + 'static) {
        let mut lanes = self.lanes.lock().expect("slide lanes poisoned");
        match lanes.entry(tile) {
            Entry::Occupied(mut waiting) => {
                waiting.insert(Some(Box::new(job)));
                return;
            }
            Entry::Vacant(lane) => {
                lane.insert(None);
            }
        }
        drop(lanes);
        let runner = self.clone();
        tokio::task::spawn_blocking(move || runner.run_lane(tile, Box::new(job)));
    }

    /// Runs `first`, then every waiting job in order, until the lane
    /// drains. The lock is only held to move jobs in and out - never
    /// while a job runs. `guard` removes the lane if a job panics, so a
    /// panicking backend call cannot wedge the tile forever.
    fn run_lane(self: &Arc<Self>, tile: i64, first: Job) {
        let mut guard = LaneGuard {
            lanes: self.clone(),
            tile,
            armed: true,
        };
        let mut job = first;
        loop {
            job();
            let next = {
                let mut lanes = self.lanes.lock().expect("slide lanes poisoned");
                match lanes.get_mut(&tile).and_then(|waiting| waiting.take()) {
                    Some(next) => Some(next),
                    None => {
                        lanes.remove(&tile);
                        None
                    }
                }
            };
            match next {
                Some(next) => job = next,
                None => {
                    guard.armed = false;
                    return;
                }
            }
        }
    }

    #[cfg(test)]
    fn idle(&self) -> bool {
        self.lanes.lock().expect("slide lanes poisoned").is_empty()
    }
}

/// Removes the lane when a job panics mid-run. Disarmed on the normal
/// exit path (which removes the lane itself under the lock): an armed
/// drop after that could delete a lane a newer worker already re-created
/// for the same tile.
struct LaneGuard {
    lanes: Arc<SlideLanes>,
    tile: i64,
    armed: bool,
}

impl Drop for LaneGuard {
    fn drop(&mut self) {
        if self.armed {
            self.lanes
                .lanes
                .lock()
                .expect("slide lanes poisoned")
                .remove(&self.tile);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Polls until every lane has drained (each worker exited) or fails
    /// the test once `deadline` passes.
    async fn wait_until_idle(lanes: &SlideLanes, deadline: Duration) {
        let end = tokio::time::Instant::now() + deadline;
        while !lanes.idle() {
            assert!(
                tokio::time::Instant::now() < end,
                "slide lanes did not drain within {deadline:?}"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_latest_value_wins_and_order_is_kept() {
        let lanes = Arc::new(SlideLanes::default());
        let record: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));
        // Job 1 parks inside the worker; `started` proves the worker is
        // busy with it, so the next submits can only land behind it.
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        {
            let record = record.clone();
            lanes.submit(7, move || {
                started_tx.send(()).expect("test listens for start");
                release_rx.recv().expect("test releases the job");
                record.lock().unwrap().push(1);
            });
        }
        started_rx.recv().expect("job 1 started");

        // 2 then 3 while the worker is busy: 3 replaces 2 (an absolute
        // slider's intermediate value is obsolete) and must run after 1.
        {
            let record = record.clone();
            lanes.submit(7, move || record.lock().unwrap().push(2));
        }
        {
            let record = record.clone();
            lanes.submit(7, move || record.lock().unwrap().push(3));
        }

        release_tx.send(()).expect("job 1 still parked");

        wait_until_idle(&lanes, Duration::from_secs(5)).await;
        assert_eq!(*record.lock().unwrap(), vec![1, 3]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tiles_do_not_block_each_other() {
        let lanes = Arc::new(SlideLanes::default());
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        lanes.submit(1, move || {
            started_tx.send(()).expect("test listens for start");
            release_rx.recv().expect("test releases the job");
        });
        started_rx.recv().expect("tile 1 job started");

        // tile 2's job must finish while tile 1's is still parked
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        lanes.submit(2, move || {
            let _ = done_tx.send(());
        });
        tokio::time::timeout(Duration::from_secs(5), done_rx)
            .await
            .expect("tile 2 must not queue behind tile 1")
            .expect("tile 2 job signaled");

        release_tx.send(()).expect("tile 1 job still parked");
        wait_until_idle(&lanes, Duration::from_secs(5)).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_panicking_job_does_not_wedge_the_lane() {
        let lanes = Arc::new(SlideLanes::default());
        lanes.submit(9, move || panic!("backend exploded"));
        // the drop guard removes the lane during the unwind
        wait_until_idle(&lanes, Duration::from_secs(5)).await;

        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        lanes.submit(9, move || {
            let _ = done_tx.send(());
        });
        tokio::time::timeout(Duration::from_secs(5), done_rx)
            .await
            .expect("a submit after a panic must still run")
            .expect("tile 9 job signaled");
        wait_until_idle(&lanes, Duration::from_secs(5)).await;
    }
}
