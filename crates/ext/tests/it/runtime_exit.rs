//! Runtime threads retire when nothing is left to do: one-shot-only
//! initializers do not pin a resident runtime, an executed stateless
//! package gives its thread (and heap) back, and a drained pinned runtime
//! parks without waking. Retirement must never lose a request: the manager
//! respawns on demand.

use std::time::{Duration, Instant};

use serde_json::json;

fn write_ext(dir: &std::path::Path, name: &str, js: &str) {
    std::fs::create_dir(dir.join(name)).expect("mkdir extension");
    std::fs::write(dir.join(name).join("index.js"), js).expect("write index.js");
}

fn eventually(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ok() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// OS threads of this process whose name is `ext-<package>` (so keep
/// package names unique within their first 11 bytes).
#[cfg(target_os = "linux")]
fn ext_threads(package: &str) -> Vec<std::path::PathBuf> {
    // the kernel truncates thread names to 15 bytes
    let mut want = format!("ext-{package}");
    want.truncate(15);
    std::fs::read_dir("/proc/self/task")
        .expect("read /proc/self/task")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| std::fs::read_to_string(p.join("comm")).is_ok_and(|c| c.trim_end() == want))
        .collect()
}

#[test]
fn one_shot_only_initializer_leaves_no_resident_runtime() {
    const JS: &str = r#"
    module.exports = {
        name: "OneShot",
        inputs: [{ value: "oneshot-act", label: "Act" }],
        initExtension: function () { setTimeout(function () {}, 0); },
        execute: function () {},
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-onesho", JS);
    let (manager, _events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    assert_eq!(
        manager.resident_count(),
        0,
        "a setTimeout is not an interval"
    );
    #[cfg(target_os = "linux")]
    eventually("probe thread to end", || {
        ext_threads("exit-onesho").is_empty()
    });
}

#[test]
fn one_shot_that_registers_an_interval_stays_resident() {
    const JS: &str = r#"
    module.exports = {
        name: "OneShotInterval",
        inputs: [{ value: "oneshot-iv-act", label: "Act" }],
        initExtension: function () {
            setTimeout(function () { setInterval(function () {}, 60000); }, 0);
        },
        execute: function () {},
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-onesh-iv", JS);
    let (manager, _events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    assert_eq!(manager.resident_count(), 1);
}

#[test]
fn nexttick_user_stays_resident_without_any_interval() {
    const JS: &str = r#"
    module.exports = {
        name: "Pinned",
        inputs: [{ value: "pinned-act", label: "Act" }],
        initExtension: function () { process.nextTick(function () {}); },
        execute: function () {},
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-pinned", JS);
    let (manager, _events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    assert_eq!(manager.resident_count(), 1);
    // let the nextTick fire and the timers drain: still resident
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        manager.resident_count(),
        1,
        "pinned runtime must not retire"
    );
    manager.execute("pinned-act", None).expect("execute");
    assert_eq!(manager.resident_count(), 1);
}

#[test]
fn drained_pinned_runtime_does_not_wake() {
    const JS: &str = r#"
    module.exports = {
        name: "Quiet",
        inputs: [{ value: "quiet-act", label: "Act" }],
        initExtension: function () { process.nextTick(function () {}); },
        execute: function () {},
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-quiet", JS);
    let (manager, _events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    assert_eq!(manager.resident_count(), 1);

    #[cfg(target_os = "linux")]
    {
        let switches = |threads: &[std::path::PathBuf]| -> u64 {
            threads
                .iter()
                .map(|t| {
                    std::fs::read_to_string(t.join("status"))
                        .unwrap_or_default()
                        .lines()
                        .find_map(|l| l.strip_prefix("voluntary_ctxt_switches:"))
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .unwrap_or(0)
                })
                .sum()
        };
        std::thread::sleep(Duration::from_millis(500));
        let threads = ext_threads("exit-quiet");
        assert_eq!(threads.len(), 1, "exactly one resident thread");
        let before = switches(&threads);
        std::thread::sleep(Duration::from_millis(3500));
        let woke = switches(&threads) - before;
        // a 1 s poll would show ~3 wakes; a parked thread shows none
        assert!(woke == 0, "drained runtime woke {woke} times while idle");
    }
}

#[test]
fn live_interval_keeps_exactly_one_thread() {
    const JS: &str = r#"
    module.exports = {
        name: "LiveOne",
        inputs: [{ value: "liveone-act", label: "Act" }],
        initExtension: function () { setInterval(function () {}, 60000); },
        execute: function () {},
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-liveone", JS);
    let (manager, _events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    assert_eq!(manager.resident_count(), 1);
    #[cfg(target_os = "linux")]
    assert_eq!(ext_threads("exit-liveone").len(), 1);
    manager.execute("liveone-act", None).expect("execute");
    assert_eq!(manager.resident_count(), 1);
    #[cfg(target_os = "linux")]
    assert_eq!(ext_threads("exit-liveone").len(), 1);
}

#[test]
fn executed_stateless_runtime_retires_and_respawns_on_demand() {
    const JS: &str = r#"
    module.exports = {
        name: "Stateless",
        inputs: [{ value: "exit-stateless-act", label: "Act" }],
        execute: function () { __pending_set_values.push({ ran: true }); },
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-stateless", JS);
    let (manager, mut events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    assert_eq!(manager.resident_count(), 0);

    manager
        .execute("exit-stateless-act", None)
        .expect("execute");
    #[cfg(target_os = "linux")]
    eventually("thread to exit", || {
        ext_threads("exit-stateless").is_empty()
    });
    eventually("registration to retire", || manager.resident_count() == 0);
    assert!(
        events.try_recv().is_ok(),
        "the action's value was forwarded"
    );

    // the handle in the entry is dead now; the next press respawns
    manager
        .execute("exit-stateless-act", None)
        .expect("execute after retire");
}

/// Hammer execute while threads retire right after each reply: no request
/// may be lost to the retire race.
#[test]
fn rapid_executes_never_lose_a_request_to_retirement() {
    const JS: &str = r#"
    module.exports = {
        name: "Race",
        inputs: [{ value: "exit-race-act", label: "Act" }],
        execute: function () {},
    };
    "#;
    let dir = tempfile::tempdir().expect("tempdir");
    write_ext(dir.path(), "exit-race", JS);
    let (manager, _events) = pulpit_ext::ExtManager::load(dir.path(), &json!(null), &[]);
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let manager = std::sync::Arc::clone(&manager);
            std::thread::spawn(move || {
                for i in 0..15 {
                    manager
                        .execute("exit-race-act", None)
                        .unwrap_or_else(|e| panic!("execute {i} failed: {e}"));
                    if i % 7 == 0 {
                        std::thread::sleep(Duration::from_millis(3));
                    }
                }
            })
        })
        .collect();
    for w in workers {
        w.join().expect("worker panicked");
    }
}
