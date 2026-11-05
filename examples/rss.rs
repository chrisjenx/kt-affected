//! Peak RSS and wall time of library calls.
//!
//!   cargo run --release --example rss -- <root> [--base REF] [--head REF] [--cache-dir DIR] [--calls N] [--threads N] [--pause-ms MS]
//!
//! Calls `kt_affected::select` N times (default 1) in this process and prints, per call, its wall
//! time, the process's peak and current RSS after it, and the heap bytes still allocated after it
//! (counted by this example's allocator), which shows whether a call leaves anything behind.
//! `--pause-ms` sleeps before each call, so threads a call's pools released have exited.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Instant;

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_add(new as isize - l.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(
        args.first()
            .expect("usage: rss <root> [--base REF] [--calls N]"),
    );
    let flag = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let calls: usize = flag("--calls").map(|n| n.parse().unwrap()).unwrap_or(1);
    let mut opts = kt_affected::SelectOptions::new();
    if let Some(b) = flag("--base") {
        opts = opts.base(b);
    }
    if let Some(n) = flag("--threads") {
        opts = opts.threads(n.parse().unwrap());
    }
    if let Some(h) = flag("--head") {
        opts = opts.head(h);
    }
    if let Some(d) = flag("--cache-dir") {
        opts = opts.cache_dir(d);
    }
    let pause: u64 = flag("--pause-ms").map(|n| n.parse().unwrap()).unwrap_or(0);
    for i in 1..=calls {
        std::thread::sleep(std::time::Duration::from_millis(pause));
        let t = Instant::now();
        let classes = match kt_affected::select(&root, &opts) {
            Ok(s) => s
                .groups
                .iter()
                .map(|g| g.classes.len())
                .sum::<usize>()
                .to_string(),
            Err(e) => format!("error: {e}"),
        };
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let (peak, now) = rss_mb();
        let heap = LIVE.load(Ordering::Relaxed) as f64 / 1048576.0;
        println!(
            "call {i}: {ms:.0} ms, classes {classes}, peak RSS {peak:.1} MB, current RSS {now:.1} MB, heap live {heap:.3} MB"
        );
    }
}

/// (peak, current) resident set size in MB.
#[cfg(unix)]
fn rss_mb() -> (f64, f64) {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    // ru_maxrss is bytes on macOS, KiB on Linux.
    let peak = if cfg!(target_os = "macos") {
        ru.ru_maxrss as f64 / 1048576.0
    } else {
        ru.ru_maxrss as f64 / 1024.0
    };
    (peak, current_rss_mb().unwrap_or(f64::NAN))
}

#[cfg(not(unix))]
fn rss_mb() -> (f64, f64) {
    (f64::NAN, f64::NAN)
}

#[cfg(target_os = "linux")]
fn current_rss_mb() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: f64 = s.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096.0 / 1048576.0)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn current_rss_mb() -> Option<f64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let kib: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kib / 1024.0)
}
