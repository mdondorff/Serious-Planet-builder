//! Lock-wait probe (M8-6 measurement): how long each named lock made its callers wait. Recorded only; no decision reads it (CON-14).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

/// The locks that are probed.
#[derive(Clone, Copy, Debug)]
pub enum Site {
    ChunkState,
    PoolState,
    Landform,
    EpochMemo,
    RegionCache,
    RouteMemo,
}

const NAMES: [&str; 6] = ["chunk state", "pool state", "landform lattice", "epoch memo", "region cache", "route memo"];
/// Waits of at least this many microseconds are counted as waits.
const WAITED_US: u64 = 100;

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);
static CALLS: [AtomicU64; 6] = [ZERO; 6];
static WAITS: [AtomicU64; 6] = [ZERO; 6];
static TOTAL_US: [AtomicU64; 6] = [ZERO; 6];
static MAX_US: [AtomicU64; 6] = [ZERO; 6];
/// Microseconds spent building the landform lattice while holding its lock, and how many builds.
static BUILD_US: AtomicU64 = AtomicU64::new(0);
static BUILDS: AtomicU64 = AtomicU64::new(0);

/// Take `m`, recording the time the call waited for it.
pub fn lock<'a, T>(m: &'a Mutex<T>, site: Site) -> MutexGuard<'a, T> {
    let at = Instant::now();
    let g = m.lock().expect("a probed lock");
    let us = u64::try_from(at.elapsed().as_micros()).unwrap_or(u64::MAX);
    let i = site as usize;
    CALLS[i].fetch_add(1, Ordering::Relaxed);
    if us >= WAITED_US {
        WAITS[i].fetch_add(1, Ordering::Relaxed);
        TOTAL_US[i].fetch_add(us, Ordering::Relaxed);
        MAX_US[i].fetch_max(us, Ordering::Relaxed);
    }
    g
}

/// A landform lattice was built (under its lock) in `us` microseconds.
pub fn built(us: u64) {
    BUILDS.fetch_add(1, Ordering::Relaxed);
    BUILD_US.fetch_add(us, Ordering::Relaxed);
}

/// The waits since the last call, one line per lock that was waited for.
pub fn take() -> Vec<String> {
    let mut out = Vec::new();
    for (i, name) in NAMES.iter().enumerate() {
        let (calls, waits) = (CALLS[i].swap(0, Ordering::Relaxed), WAITS[i].swap(0, Ordering::Relaxed));
        let (total, max) = (TOTAL_US[i].swap(0, Ordering::Relaxed), MAX_US[i].swap(0, Ordering::Relaxed));
        if waits > 0 {
            out.push(format!("{name}: {waits} of {calls} takes waited >= {WAITED_US} us, {:.1} ms in all, longest {:.1} ms", total as f64 / 1000.0, max as f64 / 1000.0));
        }
    }
    let (builds, us) = (BUILDS.swap(0, Ordering::Relaxed), BUILD_US.swap(0, Ordering::Relaxed));
    if builds > 0 {
        out.push(format!("landform lattice built {builds} times under its lock, {:.1} ms in all", us as f64 / 1000.0));
    }
    out
}
