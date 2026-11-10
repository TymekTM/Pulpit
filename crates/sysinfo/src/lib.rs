//! Native System Info source: replaces the `deckboard-system-info` JS
//! extension, whose Boa runtime was the heaviest one loaded (~13 MB of
//! committed heap, churned by per-second systeminformation sampling).
//!
//! Push parity contract with the JS package (`index.js`):
//! - a push every 10 s (`si.observe(..., 10000, ...)`) plus one right at
//!   startup, as one `setValue` object with the four `si-*` keys;
//! - RAM "percent" value is the raw `used / total` fraction with one
//!   decimal (upstream computes it that way; the client scales it);
//! - CPU temperature has no usable native source here, so it is `null`
//!   (systeminformation returns no real value on Windows either, and no
//!   board in the wild consumes this key).

use std::time::Duration;

use tokio::sync::mpsc as tokio_mpsc;

/// Tile inputs this source serves: (value, icon, fontIcon, color, mode).
pub fn input_declarations() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    vec![
        ("si-cpu", "headphones", "fas", "#8E44AD", "graph"),
        ("si-ram", "headphones", "fas", "#8E44AD", "graph"),
    ]
}

/// Does this action kind belong to the native system-info source?
pub fn is_sysinfo_action(kind: &str) -> bool {
    kind == "si-cpu" || kind == "si-ram"
}

/// The JS extension's `execute(action, args) {}` is empty; keep the same
/// no-op so tile presses succeed without changing anything.
pub fn execute(kind: &str) {
    tracing::debug!(kind, "system-info action accepted (no-op, JS parity)");
}

/// Spawn the sampling loop; values arrive as the `setValue` object (all
/// four keys, exactly like the JS package pushed).
pub fn spawn_push() -> tokio_mpsc::UnboundedReceiver<serde_json::Value> {
    let (tx, rx) = tokio_mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("sysinfo-push".into())
        .spawn(move || push_loop(&tx))
        .expect("spawn sysinfo thread");
    rx
}

fn push_loop(tx: &tokio_mpsc::UnboundedSender<serde_json::Value>) {
    let brand = cpu_brand();
    let interval = Duration::from_secs(10);
    let mut times = cpu_times();
    // first push has no delta yet; the JS observe loop warmed up the same way
    let mut load_pct = 0.0;
    loop {
        // a failed send means the receiver is gone (server shutting
        // down): end the thread instead of sampling into the void forever
        if !send(tx, &brand, load_pct, &mem_info()) {
            tracing::debug!("sysinfo push channel closed, stopping the sampling loop");
            return;
        }
        std::thread::sleep(interval);
        let next = cpu_times();
        load_pct = cpu_load_pct(times, next);
        times = next;
    }
}

fn send(
    tx: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    brand: &str,
    load_pct: f64,
    mem: &(u64, u64),
) -> bool {
    let (total, avail) = *mem;
    let used = total.saturating_sub(avail) as f64;
    let payload = serde_json::json!({
        "si-load-cpu": {
            "title": "CPU Load",
            "description": brand,
            "value": format!("{load_pct:.1}"),
            "suffix": "%",
        },
        "si-temperature-cpu": {
            "title": "CPU Temperature",
            "description": brand,
            "value": serde_json::Value::Null,
            "suffix": "°C",
        },
        "si-load-p-ram": {
            "title": "RAM Usage",
            "value": format!("{:.1}", if total == 0 { 0.0 } else { used / total as f64 }),
            "suffix": "%",
        },
        "si-load-gb-ram": {
            "title": "RAM Usage",
            "value": format!("{:.1}", used * 1e-9),
            "suffix": "GB",
        },
    });
    if tx.send(payload).is_err() {
        // receiver dropped: tell the loop to stop
        return false;
    }
    tracing::debug!(target: "pulpit_sysinfo", load = load_pct, "pushed system-info values");
    true
}

// ------------------------------------------------------------ CPU sampling

/// Total (idle, kernel, user) CPU times in 100 ns ticks since boot.
/// Same kernel32 read the JS host's os shim is built on.
#[cfg(windows)]
fn cpu_times() -> (u64, u64, u64) {
    #[repr(C)]
    #[derive(Default)]
    struct Filetime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut Filetime, kernel: *mut Filetime, user: *mut Filetime) -> i32;
    }
    let (mut idle, mut kernel, mut user) = (
        Filetime::default(),
        Filetime::default(),
        Filetime::default(),
    );
    // SAFETY: three distinct out-parameters of the documented struct size
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return (0, 0, 0);
    }
    let raw = |f: &Filetime| ((f.high as u64) << 32) | f.low as u64;
    (raw(&idle), raw(&kernel), raw(&user))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn cpu_times() -> (u64, u64, u64) {
    (0, 0, 0)
}

/// Aggregate one `cpu  user nice system idle …` line of `/proc/stat`
/// into the Windows GetSystemTimes shape this crate computes load
/// from: `(idle, kernel, user)` where **kernel includes idle** (the
/// contract `cpu_load_pct` below is written against). Linux counts
/// user+nice and irq/softirq/steal separately, so they fold in:
/// idle = idle + iowait, user = user + nice,
/// kernel = system + irq + softirq + steal + idle.
#[cfg(target_os = "linux")]
fn cpu_times() -> (u64, u64, u64) {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    parse_proc_stat(&stat).unwrap_or((0, 0, 0))
}

/// Parse `/proc/stat`'s aggregate `cpu` line; `None` when it is absent
/// or has no numbers. Counter overflows wrap (u64 since boot: no).
#[cfg(target_os = "linux")]
fn parse_proc_stat(stat: &str) -> Option<(u64, u64, u64)> {
    let line = stat.lines().next()?;
    // per-kernel-doc fields after the "cpu" label, in order
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map(|f| f.parse::<u64>())
        .collect::<Result<_, _>>()
        .ok()?;
    if fields.len() < 4 {
        return None;
    }
    let get = |i: usize| fields.get(i).copied().unwrap_or(0);
    let idle = get(3) + get(4); // idle + iowait
    let user = get(0) + get(1); // user + nice
    let kernel = get(2) + get(5) + get(6) + get(7) + idle;
    Some((idle, kernel, user))
}

/// (total, available) physical memory in bytes. Mirrors the JS host's
/// GlobalMemoryStatusEx read; systeminformation computes
/// `mem.used = total - available` from the same source.
#[cfg(windows)]
fn mem_info() -> (u64, u64) {
    // must mirror MEMORYSTATUSEX exactly: 64 bytes, dwLength = 64, or the
    // API rejects the call
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_virtual_extended: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buf: *mut MemoryStatus) -> i32;
    }
    let mut buf = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page: 0,
        avail_page: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_virtual_extended: 0,
    };
    debug_assert_eq!(buf.length, 64);
    // SAFETY: buf is initialized with the expected dwLength
    let ok = unsafe { GlobalMemoryStatusEx(&mut buf) };
    if ok == 0 {
        return (0, 0);
    }
    (buf.total_phys, buf.avail_phys)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn mem_info() -> (u64, u64) {
    (0, 0)
}

/// `(total, available)` physical memory from `/proc/meminfo` (values are
/// kB). The kernel has published MemAvailable since 3.14; MemFree is the
/// fallback for anything older.
#[cfg(target_os = "linux")]
fn mem_info() -> (u64, u64) {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    parse_meminfo(&info)
}

/// Parse `/proc/meminfo` into `(MemTotal, MemAvailable)` bytes.
#[cfg(target_os = "linux")]
fn parse_meminfo(info: &str) -> (u64, u64) {
    let mut total = None;
    let mut available = None;
    for line in info.lines() {
        let value = || {
            line.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb.saturating_mul(1024))
        };
        // MemAvailable wins wherever it appears; MemFree only fills in
        // (pre-3.14 kernels without MemAvailable)
        let available_key = line.starts_with("MemAvailable:")
            || (line.starts_with("MemFree:") && available.is_none());
        if line.starts_with("MemTotal:") {
            total = value();
        } else if available_key {
            available = value();
        }
    }
    (total.unwrap_or(0), available.unwrap_or(0))
}

/// Busy percentage between two time samples. The JS stack derived the same
/// number from these counters via the os shim: node's shape folds all
/// kernel time (idle included) into `sys`, so busy = user + kernel - idle
/// and total = user + kernel.
fn cpu_load_pct(prev: (u64, u64, u64), next: (u64, u64, u64)) -> f64 {
    let delta = |a: u64, b: u64| b.saturating_sub(a);
    let idle = delta(prev.0, next.0);
    let kernel = delta(prev.1, next.1);
    let user = delta(prev.2, next.2);
    let total = user + kernel;
    if total == 0 {
        return 0.0;
    }
    100.0 * (total - idle) as f64 / total as f64
}

/// `si.cpu().brand` equivalent: the processor name from the registry (the
/// same string WMI serves systeminformation on Windows).
#[cfg(windows)]
fn cpu_brand() -> String {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueA(
            key: usize,
            sub: *const u8,
            value: *const u8,
            flags: u32,
            r#type: *mut u32,
            data: *mut u8,
            len: *mut u32,
        ) -> i32;
    }
    const HKEY_LOCAL_MACHINE: usize = 0x8000_0002;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    let sub = b"HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0\0";
    let name = b"ProcessorNameString\0";
    let mut buf = [0u8; 256];
    let mut len = buf.len() as u32;
    // SAFETY: all pointers reference initialized buffers of the passed size
    let ok = unsafe {
        RegGetValueA(
            HKEY_LOCAL_MACHINE,
            sub.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            &mut len,
        )
    };
    if ok == 0 {
        let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        let text = String::from_utf8_lossy(&buf[..end]).trim().to_string();
        if !text.is_empty() {
            return text;
        }
    }
    "CPU".into()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn cpu_brand() -> String {
    "CPU".into()
}

/// `si.cpu().brand` equivalent: the first `model name` in `/proc/cpuinfo`
/// (`Intel(R) Core(TM) i7-8550U CPU @ 1.80GHz`), the same string the
/// DMI/WMI layer serves systeminformation on Windows.
#[cfg(target_os = "linux")]
fn cpu_brand() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    parse_cpu_brand(&info)
}

/// First `model name\t: …` value, trimmed; `"CPU"` when cpuinfo carries
/// none (some VMs and ARM boards name their cores differently).
#[cfg(target_os = "linux")]
fn parse_cpu_brand(info: &str) -> String {
    for line in info.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            if let Some(value) = rest.split_once(':') {
                let value = value.1.trim();
                if !value.is_empty() {
                    return value.to_string();
                }
            }
        }
    }
    "CPU".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_channel_ends_the_push_loop() {
        // the receiver is dropped immediately: send must report the
        // closed channel so push_loop exits instead of sampling forever
        let (tx, rx) = tokio_mpsc::unbounded_channel::<serde_json::Value>();
        drop(rx);
        assert!(!send(&tx, "CPU", 1.0, &(8, 4)));
        // an open channel keeps the loop alive
        let (tx, _rx) = tokio_mpsc::unbounded_channel::<serde_json::Value>();
        assert!(send(&tx, "CPU", 1.0, &(8, 4)));
    }

    fn one_payload(load: f64, mem: (u64, u64)) -> serde_json::Value {
        let (tx, mut rx) = tokio_mpsc::unbounded_channel();
        assert!(send(&tx, "Ryzen 7", load, &mem));
        rx.try_recv().unwrap()
    }

    #[test]
    fn payload_carries_the_four_js_keys() {
        let p = one_payload(12.345, (8_000_000_000, 2_000_000_000));
        let keys: Vec<&str> = p.as_object().unwrap().keys().map(String::as_str).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            vec![
                "si-load-cpu",
                "si-load-gb-ram",
                "si-load-p-ram",
                "si-temperature-cpu"
            ]
        );
        assert_eq!(p["si-load-cpu"]["title"], "CPU Load");
        assert_eq!(p["si-load-cpu"]["description"], "Ryzen 7");
        assert_eq!(p["si-load-cpu"]["value"], "12.3");
        assert_eq!(p["si-load-cpu"]["suffix"], "%");
    }

    #[test]
    fn temperature_is_null_with_the_celsius_suffix() {
        let p = one_payload(0.0, (1, 1));
        assert!(p["si-temperature-cpu"]["value"].is_null());
        assert_eq!(p["si-temperature-cpu"]["suffix"], "°C");
        assert_eq!(p["si-temperature-cpu"]["description"], "Ryzen 7");
    }

    #[test]
    fn ram_values_follow_the_js_formulas() {
        // 6 GB used of 8 GB: the "percent" key is the raw fraction (JS parity)
        let p = one_payload(0.0, (8_000_000_000, 2_000_000_000));
        assert_eq!(p["si-load-p-ram"]["value"], "0.8");
        assert_eq!(p["si-load-p-ram"]["suffix"], "%");
        assert_eq!(p["si-load-gb-ram"]["value"], "6.0");
        assert_eq!(p["si-load-gb-ram"]["suffix"], "GB");
    }

    #[test]
    fn ram_values_survive_missing_or_inconsistent_readings() {
        // a failed GlobalMemoryStatusEx reads as zero total
        let p = one_payload(0.0, (0, 0));
        assert_eq!(p["si-load-p-ram"]["value"], "0.0");
        assert_eq!(p["si-load-gb-ram"]["value"], "0.0");
        // available above total must not underflow
        let p = one_payload(0.0, (1_000, 5_000));
        assert_eq!(p["si-load-p-ram"]["value"], "0.0");
    }

    #[test]
    fn cpu_load_is_busy_over_user_plus_kernel() {
        // idle 50, kernel 60 (idle included), user 40 -> busy 50 of 100
        assert_eq!(cpu_load_pct((0, 0, 0), (50, 60, 40)), 50.0);
        // fully idle
        assert_eq!(cpu_load_pct((100, 100, 0), (200, 200, 0)), 0.0);
        // fully busy
        assert_eq!(cpu_load_pct((0, 0, 0), (0, 30, 70)), 100.0);
    }

    #[test]
    fn cpu_load_without_progress_or_with_counter_resets_is_zero() {
        assert_eq!(cpu_load_pct((5, 5, 5), (5, 5, 5)), 0.0);
        // counters going backwards saturate instead of wrapping
        assert_eq!(cpu_load_pct((100, 100, 100), (0, 0, 0)), 0.0);
    }

    #[test]
    fn declarations_and_actions_agree() {
        let decls = input_declarations();
        assert_eq!(decls.len(), 2);
        for (value, _, font, color, mode) in &decls {
            assert!(is_sysinfo_action(value), "{value}");
            assert_eq!(*font, "fas");
            assert!(color.starts_with('#') && color.len() == 7);
            assert_eq!(*mode, "graph");
        }
        assert!(!is_sysinfo_action("si-load-cpu"));
        assert!(!is_sysinfo_action(""));
        assert!(!is_sysinfo_action("SI-CPU"));
    }

    #[test]
    fn native_readings_are_sane() {
        let brand = cpu_brand();
        assert!(!brand.is_empty());
        assert_eq!(brand, brand.trim());
        let (total, avail) = mem_info();
        assert!(avail <= total || total == 0);
        let (idle, kernel, user) = cpu_times();
        assert!(idle <= kernel || (idle, kernel, user) == (0, 0, 0));
    }

    #[tokio::test]
    async fn spawn_push_delivers_a_first_sample_right_away() {
        let mut rx = spawn_push();
        let first = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("first push within the timeout")
            .expect("channel open");
        // no delta yet: the warm-up push reports zero load
        assert_eq!(first["si-load-cpu"]["value"], "0.0");
        assert!(first["si-load-gb-ram"]["value"].is_string());
    }

    // ---- Linux /proc parsers (fixture text; no live /proc needed) ----

    #[cfg(target_os = "linux")]
    const PROC_STAT: &str = "cpu  74608 2520 24433 1117073 6176 0 1292 0 0 0\n\
                             cpu0 37300 1260 12200 558000 3088 0 600 0 0 0\n\
                             intr 0\n";

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_folds_into_the_windows_counter_shape() {
        // idle = 1117073 + 6176, kernel = 24433 + 0 + 1292 + 0 + idle,
        // user = 74608 + 2520
        let (idle, kernel, user) = parse_proc_stat(PROC_STAT).expect("parse");
        assert_eq!(idle, 1_123_249);
        assert_eq!(kernel, 1_148_974);
        assert_eq!(user, 77_128);
        // kernel always includes idle: busy = user + kernel - idle
        assert!(kernel >= idle);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_without_numbers_is_none() {
        assert!(parse_proc_stat("").is_none());
        assert!(parse_proc_stat("cpu\n").is_none());
        assert!(parse_proc_stat("intr 0\ncpu 1 2 3 4\n").is_none()); // cpu is not first
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_without_optional_columns_still_parses() {
        // pre-2.6 kernels stop after idle
        let (idle, kernel, user) = parse_proc_stat("cpu  10 0 20 70\n").expect("parse");
        assert_eq!((idle, kernel, user), (70, 90, 10));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cpu_load_over_two_proc_stat_samples() {
        // 25% busy: busy delta 100, total delta 400
        let first = parse_proc_stat("cpu  100 0 0 300 0 0 0 0\n").expect("first");
        let second = parse_proc_stat("cpu  150 0 50 600 0 0 0 0\n").expect("second");
        // (idle 300, kernel 300, user 100) -> (idle 600, kernel 650, user 150)
        assert_eq!(first, (300, 300, 100));
        assert_eq!(second, (600, 650, 150));
        assert_eq!(cpu_load_pct(first, second), 25.0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn meminfo_kilobytes_become_bytes() {
        // a non-numeric value column breaks only that line's parse, not
        // the whole file: the key reads as missing and stays at zero
        const BAD: &str = "MemTotal:       not-a-number kB\n\
                            MemFree:         1000 kB\n\
                            MemAvailable:    9000 kB\n";
        assert_eq!(parse_meminfo(BAD), (0, 9000 * 1024));
        // no MemAvailable: MemFree is the fallback
        let no_avail = "MemTotal:       16090 kB\nMemFree:         1000 kB\n";
        assert_eq!(parse_meminfo(no_avail), (16_090 * 1024, 1000 * 1024));
        let real = "MemTotal:       16090432 kB\nMemFree:         6622312 kB\nMemAvailable:   10930200 kB\n";
        assert_eq!(parse_meminfo(real), (16_090_432 * 1024, 10_930_200 * 1024));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cpuinfo_model_name_is_trimmed_with_cpu_fallback() {
        const PROC_CPUINFO: &str = "processor\t: 0\n\
                                    vendor_id\t: GenuineIntel\n\
                                    model name\t: Intel(R) Core(TM) i7-8550U CPU @ 1.80GHz\n\
                                    processor\t: 1\n\
                                    model name\t: AMD Ryzen 9\n";
        assert_eq!(
            parse_cpu_brand(PROC_CPUINFO),
            "Intel(R) Core(TM) i7-8550U CPU @ 1.80GHz"
        );
        assert_eq!(parse_cpu_brand("processor: 0\n"), "CPU");
        assert_eq!(parse_cpu_brand(""), "CPU");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_proc_readings_are_sane() {
        let (idle, kernel, user) = cpu_times();
        assert!(idle <= kernel || (idle, kernel, user) == (0, 0, 0));
        let (total, available) = mem_info();
        assert!(available <= total || total == 0);
        assert_ne!(cpu_brand(), "");
    }
}
