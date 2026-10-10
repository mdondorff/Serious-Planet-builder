//! `streaming`: a minimal asynchronous tile path (CON-04, CON-22). Time, tile loading and task spawning are
//! injected, so the same code runs on a thread pool in the application and deterministically in tests.
//!
//! Each frame the caller passes the tiles it wants (with priorities). The streamer answers, for every wanted tile,
//! which resident tile to show: the tile itself, or its nearest resident ancestor while the finer one is generated.
//! Missing ancestors are requested before descendants (parent-first), requests for tiles that are no longer wanted are
//! cancelled, and resident tiles beyond the capacity are evicted least-recently-used. Nothing here ever waits for a job.

use planet_core::TileId;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// A unit of work handed to a [`Spawner`].
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// Monotonic milliseconds. Injected so retry back-off is testable.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock(std::time::Instant);

impl SystemClock {
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

/// Test clock advanced by hand.
#[derive(Default)]
pub struct FakeClock(AtomicU64);

impl FakeClock {
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Runs jobs somewhere else (thread pool) or on demand (tests).
pub trait Spawner: Send + Sync {
    fn spawn(&self, job: Job);
}

/// Source of tile data: generation, disk or network. May be slow and may fail.
pub trait TileSource<T>: Send + Sync {
    fn load(&self, tile: TileId) -> Result<T, String>;
}

/// Fixed-size worker pool; jobs run in submission order.
pub struct ThreadPool {
    tx: Mutex<Option<Sender<Job>>>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl ThreadPool {
    pub fn new(threads: usize) -> Self {
        let (tx, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let workers = (0..threads.max(1))
            .map(|_| {
                let rx = Arc::clone(&rx);
                std::thread::spawn(move || loop {
                    let job = rx.lock().expect("pool receiver").recv();
                    match job {
                        Ok(j) => j(),
                        Err(_) => break,
                    }
                })
            })
            .collect();
        Self { tx: Mutex::new(Some(tx)), workers: Mutex::new(workers) }
    }
}

impl Spawner for ThreadPool {
    fn spawn(&self, job: Job) {
        if let Some(tx) = self.tx.lock().expect("pool sender").as_ref() {
            let _ = tx.send(job);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.tx.lock().expect("pool sender").take();
        for w in self.workers.lock().expect("pool workers").drain(..) {
            let _ = w.join();
        }
    }
}

/// Deterministic spawner: jobs wait in a queue until the test runs them.
#[derive(Default)]
pub struct ManualSpawner {
    queue: Mutex<VecDeque<Job>>,
}

impl ManualSpawner {
    pub fn pending(&self) -> usize {
        self.queue.lock().expect("queue").len()
    }

    /// Run the oldest job; false when none was waiting.
    pub fn run_one(&self) -> bool {
        let job = self.queue.lock().expect("queue").pop_front();
        job.map(|j| j()).is_some()
    }

    pub fn run(&self, n: usize) -> usize {
        (0..n).take_while(|_| self.run_one()).count()
    }
}

impl Spawner for ManualSpawner {
    fn spawn(&self, job: Job) {
        self.queue.lock().expect("queue").push_back(job);
    }
}

/// What to show for one wanted tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub wanted: TileId,
    /// The resident tile to draw: `wanted` itself or its nearest resident ancestor; `None` only before any ancestor
    /// (including the root) is resident.
    pub shown: Option<TileId>,
}

impl Resolved {
    pub fn exact(&self) -> bool {
        self.shown == Some(self.wanted)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamStats {
    pub resident: usize,
    pub queued: usize,
    pub in_flight: usize,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub evicted: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct StreamConfig {
    /// Jobs running at once.
    pub max_in_flight: usize,
    /// Resident tiles kept at most (roots and tiles used this frame are never evicted).
    pub capacity: usize,
    /// A failed tile is retried after this many milliseconds.
    pub retry_after_ms: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self { max_in_flight: 4, capacity: 4096, retry_after_ms: 1000 }
    }
}

struct Resident<T> {
    data: Arc<T>,
    last_used: u64,
}

/// Marker error of a job that was cancelled before it started; not a failure.
const CANCELLED: &str = "cancelled";

type Completion<T> = (TileId, Result<T, String>);

pub struct Streamer<T: Send + Sync + 'static> {
    source: Arc<dyn TileSource<T>>,
    spawner: Arc<dyn Spawner>,
    clock: Arc<dyn Clock>,
    config: StreamConfig,
    resident: HashMap<TileId, Resident<T>>,
    /// Queued requests ordered by `(priority key, tile)`; lower runs first.
    queue: BTreeMap<(u64, TileId), ()>,
    queued_key: HashMap<TileId, u64>,
    in_flight: HashMap<TileId, Arc<AtomicBool>>,
    failed_at: HashMap<TileId, u64>,
    tx: Sender<Completion<T>>,
    rx: Receiver<Completion<T>>,
    frame: u64,
    stats: StreamStats,
    /// Order in which tiles were handed to the spawner (for tests and diagnostics).
    pub dispatch_log: Vec<TileId>,
}

/// Total order on f64 priorities that survives `u64` keys (negative and NaN safe).
fn priority_key(p: f64) -> u64 {
    let b = if p.is_nan() { f64::MAX.to_bits() } else { p.to_bits() };
    if b >> 63 == 1 {
        !b
    } else {
        b | (1 << 63)
    }
}

impl<T: Send + Sync + 'static> Streamer<T> {
    pub fn new(source: Arc<dyn TileSource<T>>, spawner: Arc<dyn Spawner>, clock: Arc<dyn Clock>, config: StreamConfig) -> Self {
        let (tx, rx) = channel();
        Self {
            source,
            spawner,
            clock,
            config,
            resident: HashMap::new(),
            queue: BTreeMap::new(),
            queued_key: HashMap::new(),
            in_flight: HashMap::new(),
            failed_at: HashMap::new(),
            tx,
            rx,
            frame: 0,
            stats: StreamStats::default(),
            dispatch_log: Vec::new(),
        }
    }

    /// Load the six level-0 tiles synchronously, once at start-up, so that some level of detail always exists
    /// (CON-22). This is the only place that waits.
    pub fn load_roots_blocking(&mut self) -> Result<(), String> {
        for f in planet_core::Face::ALL {
            let id = TileId::new(f, 0, 0, 0).expect("root");
            let data = self.source.load(id)?;
            self.resident.insert(id, Resident { data: Arc::new(data), last_used: self.frame });
        }
        Ok(())
    }

    pub fn get(&self, tile: TileId) -> Option<Arc<T>> {
        self.resident.get(&tile).map(|r| Arc::clone(&r.data))
    }

    pub fn is_resident(&self, tile: TileId) -> bool {
        self.resident.contains_key(&tile)
    }

    pub fn stats(&self) -> StreamStats {
        StreamStats { resident: self.resident.len(), queued: self.queue.len(), in_flight: self.in_flight.len(), ..self.stats }
    }

    fn nearest_resident(&self, mut t: TileId) -> Option<TileId> {
        loop {
            if self.resident.contains_key(&t) {
                return Some(t);
            }
            t = t.parent()?;
        }
    }

    /// One frame: collect finished jobs, request what is wanted (ancestors first), cancel what is not, dispatch within
    /// the in-flight limit, evict beyond capacity, and say what to draw for each wanted tile. Never blocks.
    pub fn begin_frame(&mut self, wanted: &[(TileId, f64)]) -> Vec<Resolved> {
        self.frame += 1;
        let now = self.clock.now_ms();
        // 1. Completions.
        while let Ok((tile, result)) = self.rx.try_recv() {
            self.in_flight.remove(&tile);
            match result {
                Ok(data) => {
                    self.stats.completed += 1;
                    self.resident.insert(tile, Resident { data: Arc::new(data), last_used: self.frame });
                }
                Err(e) if e == CANCELLED => {}
                Err(_) => {
                    self.stats.failed += 1;
                    self.failed_at.insert(tile, now);
                }
            }
        }
        // 2. What is needed: wanted tiles and all their ancestors; the base priority is the wanted tile's.
        let mut needed: HashMap<TileId, f64> = HashMap::new();
        for &(tile, priority) in wanted {
            let mut t = Some(tile);
            let mut depth = 0.0;
            while let Some(x) = t {
                // Ancestors sort before descendants: parent-first.
                let p = priority - depth * 1.0e12;
                needed.entry(x).and_modify(|e| *e = e.min(p)).or_insert(p);
                depth += 1.0;
                t = x.parent();
            }
        }
        for t in needed.keys() {
            if let Some(r) = self.resident.get_mut(t) {
                r.last_used = self.frame;
            }
        }
        // 3. Cancel queued and in-flight requests that nobody needs any more.
        let stale_queued: Vec<TileId> = self.queued_key.keys().filter(|t| !needed.contains_key(t)).copied().collect();
        for t in stale_queued {
            if let Some(k) = self.queued_key.remove(&t) {
                self.queue.remove(&(k, t));
                self.stats.cancelled += 1;
            }
        }
        for (t, token) in &self.in_flight {
            if !needed.contains_key(t) && !token.swap(true, Ordering::SeqCst) {
                self.stats.cancelled += 1;
            }
        }
        // 4. Enqueue missing tiles (ancestors sort first) unless recently failed.
        let mut missing: Vec<(TileId, f64)> = needed
            .iter()
            .filter(|(t, _)| !self.resident.contains_key(*t) && !self.in_flight.contains_key(*t))
            .filter(|(t, _)| self.failed_at.get(*t).is_none_or(|at| now.saturating_sub(*at) >= self.config.retry_after_ms))
            .map(|(t, p)| (*t, *p))
            .collect();
        missing.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        for (t, p) in missing {
            let key = priority_key(p);
            if let Some(old) = self.queued_key.insert(t, key) {
                self.queue.remove(&(old, t));
            }
            self.queue.insert((key, t), ());
        }
        // 5. Dispatch.
        while self.in_flight.len() < self.config.max_in_flight {
            let Some((&(key, tile), _)) = self.queue.iter().next() else { break };
            self.queue.remove(&(key, tile));
            self.queued_key.remove(&tile);
            let token = Arc::new(AtomicBool::new(false));
            self.in_flight.insert(tile, Arc::clone(&token));
            self.dispatch_log.push(tile);
            let (source, tx) = (Arc::clone(&self.source), self.tx.clone());
            self.spawner.spawn(Box::new(move || {
                if token.load(Ordering::SeqCst) {
                    let _ = tx.send((tile, Err(CANCELLED.to_string())));
                    return;
                }
                let result = source.load(tile);
                // A cancelled job's result is dropped by the receiver side only if nobody needs it; keep it, it is valid.
                let _ = tx.send((tile, result));
            }));
        }
        // 6. Evict least recently used tiles beyond capacity (never roots or tiles used this frame).
        if self.resident.len() > self.config.capacity {
            let mut victims: Vec<(u64, TileId)> =
                self.resident.iter().filter(|(t, r)| t.level() > 0 && r.last_used < self.frame).map(|(t, r)| (r.last_used, *t)).collect();
            victims.sort();
            let excess = self.resident.len() - self.config.capacity;
            for (_, t) in victims.into_iter().take(excess) {
                self.resident.remove(&t);
                self.stats.evicted += 1;
            }
        }
        wanted.iter().map(|&(t, _)| Resolved { wanted: t, shown: self.nearest_resident(t) }).collect()
    }

    /// Tiles whose requests ended in failure and are waiting for the retry delay.
    pub fn failed_tiles(&self) -> HashSet<TileId> {
        self.failed_at.keys().copied().collect()
    }
}
