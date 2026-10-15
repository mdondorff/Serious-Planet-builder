//! `streaming`: a minimal asynchronous tile path (CON-04, CON-22). Time, tile loading and task spawning are
//! injected, so the same code runs on a thread pool in the application and deterministically in tests.
//!
//! Each frame the caller passes the tiles it wants (with priorities). The streamer answers, for every wanted tile,
//! which resident tile to show: the tile itself, or its nearest resident ancestor while the finer one is generated.
//! Missing ancestors are requested before descendants (parent-first), requests for tiles that are no longer wanted are
//! cancelled, and resident tiles beyond the capacity are evicted least-recently-used. Nothing here ever waits for a job.
//!
//! The scheduler (STRM-005, ADR 0031): requests come in three classes, highest first: the missing draw set, the rest
//! of the residency set, prefetch. A request is cancelled only when its tile is in none of them (never for leaving the
//! view frustum, which is not an input); prefetch dispatches only while nothing of the other classes waits and only
//! into free capacity. [`Wants`] carries the three sets; [`Streamer::begin_frame`] is the draw-only special case.
//!
//! Capacity is counted in pooled slots (STRM-006, STRM-007, ADR 0033): a slot is reserved when a job is dispatched and given
//! back when the tile is evicted, cancelled or failed, and a given-back slot is reusable only after the frames in flight
//! retire. A request that finds no free slot makes room by evicting (prefetch and tiles outside R first, then R tiles
//! farthest and finest first, never the draw set with its ancestors or a root) or waits. [`manager`] fits the wants to
//! the pool before they get here.

pub mod budget;
pub mod manager;
pub mod slots;

pub use budget::{PoolConfig, Tier};
pub use manager::{degrade, DegradationReport, Degraded, ManagerFrame, ResidencyManager};
pub use slots::{PoolKind, SlotPool, SlotPools, SlotRef, VERTEX_POOL_MAX_LEVEL};

use planet_core::{TileId, Vec3};
use std::cmp::Reverse;
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

    /// Run one waiting job on the calling thread; false when this spawner runs its jobs elsewhere (a pool) or none waits.
    /// A caller that must wait for jobs it spawned ([`Streamer::load_roots_blocking`]) calls it so that a spawner which
    /// holds jobs until asked (the deterministic one of tests) cannot deadlock it.
    fn help(&self) -> bool {
        false
    }
}

/// Source of tile data: generation, disk or network. May be slow and may fail.
pub trait TileSource<T>: Send + Sync {
    fn load(&self, tile: TileId) -> Result<T, String>;

    /// Like [`load`](TileSource::load) for a source that works in stages: it polls `cancel` at each stage boundary and
    /// returns `Ok(None)` to stop (STRM-005). The default is one stage.
    fn load_staged(&self, tile: TileId, cancel: &CancelToken) -> Result<Option<T>, String> {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        self.load(tile).map(Some)
    }
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

    fn help(&self) -> bool {
        self.run_one()
    }
}

/// Set by the streamer when nothing needs a running job any more; a staged source polls it between stages
/// ([`TileSource::load_staged`]) and stops at the next boundary.
#[derive(Clone, Debug)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token nobody cancels: for calling a staged source outside a streamer.
    pub fn never() -> CancelToken {
        CancelToken(Arc::new(AtomicBool::new(false)))
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// A token that is already cancelled: for testing a source's stage boundaries.
    pub fn cancelled() -> CancelToken {
        CancelToken(Arc::new(AtomicBool::new(true)))
    }
}

/// The request class of a tile, highest priority first (STRM-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestClass {
    /// Missing nodes of the draw set D.
    Draw = 0,
    /// The rest of the residency set R.
    Residency = 1,
    /// R at the predicted position; free capacity only.
    Prefetch = 2,
}

/// What the caller wants this frame, as `(tile, priority)` pairs, lower priority numbers first within a class.
/// The ancestors of every tile are implied.
#[derive(Clone, Debug, Default)]
pub struct Wants {
    /// Draw set D: shown this frame (`begin_frame_wants` answers for each). Priority: screen-space error times visibility
    /// (as a number that sorts ascending).
    pub draw: Vec<(TileId, f64)>,
    /// Residency set R: everything worth keeping for the camera position (a superset of D). Priority: distance; the
    /// caller may order in-view tiles first. Level (coarse first) outranks priority.
    pub residency: Vec<(TileId, f64)>,
    /// Prefetch set: R at the predicted position (translation only). Empty until the prefetch source exists.
    pub prefetch: Vec<(TileId, f64)>,
}

/// The camera position `look_ahead_ms` ahead along its translation velocity (metres per second), the look-ahead capped
/// at `max_look_ahead_ms`. Translation only: yaw and pitch rates are never extrapolated (ADR 0031 point 2.3), so
/// there is no parameter for them.
pub fn predicted_position(position: Vec3, velocity: Vec3, look_ahead_ms: u64, max_look_ahead_ms: u64) -> Vec3 {
    position + velocity * (look_ahead_ms.min(max_look_ahead_ms) as f64 / 1000.0)
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
    /// Of `cancelled`, the requests of the prefetch class.
    pub cancelled_prefetch: u64,
    /// Jobs that finished after they were cancelled; their tiles were not made resident.
    pub dropped: u64,
    pub evicted: u64,
    /// Queued requests by class: draw, residency, prefetch.
    pub queued_by_class: [usize; 3],
    /// Jobs handed to the spawner.
    pub dispatched: u64,
    /// Dispatch attempts that found no slot (every slot in use or still retiring, nothing evictable for that class).
    pub blocked: u64,
    /// Slots of the main pool: in use (resident plus in flight), retiring, free.
    pub main_in_use: usize,
    pub main_retiring: usize,
    pub main_free: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct StreamConfig {
    /// Jobs running at once.
    pub max_in_flight: usize,
    /// Slots of the main pool (resident tiles plus jobs in flight never exceed it; roots and the draw set with its ancestors
    /// are never evicted, so a draw set that does not fit is a budget failure the manager reports).
    pub capacity: usize,
    /// Slots of the small vertex pool for levels 0-2 (ADR 0033 point 3); 0: one pool, every tile in `capacity`.
    pub vertex_pool_slots: usize,
    /// A given-back slot is reusable this many frames later (frames in flight; 0 for a renderer that finishes a frame before
    /// the next begins).
    pub retire_frames: u64,
    /// A failed tile is retried after this many milliseconds.
    pub retry_after_ms: u64,
    /// Keep the order in which tiles were dispatched in `dispatch_log` (tests and diagnostics; grows without bound).
    pub record_dispatches: bool,
    /// Record every tile that becomes resident or is evicted for [`Streamer::take_changes`] (a loop that mirrors the resident
    /// set into the renderer's slots drains it every frame; off, nothing is kept).
    pub record_changes: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_in_flight: 4,
            capacity: 4096,
            vertex_pool_slots: 0,
            retire_frames: 0,
            retry_after_ms: 1000,
            record_dispatches: false,
            record_changes: false,
        }
    }
}

struct InFlight {
    token: Arc<AtomicBool>,
    class: RequestClass,
    /// Reserved for the tile when the job was dispatched; given back when the job ends without a resident tile.
    slot: SlotRef,
}

/// Queue order: class, rank (draw: 0; residency and prefetch: level), priority key, tile.
type QueueKey = (u8, u64, u64, TileId);

struct Resident<T> {
    data: Arc<T>,
    last_used: u64,
    slot: SlotRef,
}

/// A change of the resident set, in the order it happened (STRM-007): what a loop that places tiles in the renderer's slots
/// must follow. A tile that became resident and was evicted before the loop looked is in the list twice; the loop acts on
/// the state at the time it drains (`Streamer::get`, `slot_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidentChange {
    /// The tile became resident in its slot.
    Added(TileId),
    /// The tile was evicted and its slot given back (reusable after the frames in flight retire).
    Evicted(TileId),
}

/// How a job ended.
enum Outcome<T> {
    Done(T),
    Failed(String),
    /// Skipped because it was cancelled before it started or at a stage boundary; not a failure.
    Cancelled,
}

type Completion<T> = (TileId, Outcome<T>);

pub struct Streamer<T: Send + Sync + 'static> {
    source: Arc<dyn TileSource<T>>,
    spawner: Arc<dyn Spawner>,
    clock: Arc<dyn Clock>,
    config: StreamConfig,
    resident: HashMap<TileId, Resident<T>>,
    /// Queued requests ordered by `(priority key, tile)`; lower runs first.
    queue: BTreeMap<QueueKey, ()>,
    queued_key: HashMap<TileId, QueueKey>,
    in_flight: HashMap<TileId, InFlight>,
    failed_at: HashMap<TileId, u64>,
    last_error: HashMap<TileId, String>,
    tx: Sender<Completion<T>>,
    rx: Receiver<Completion<T>>,
    frame: u64,
    stats: StreamStats,
    pools: SlotPools,
    changes: Vec<ResidentChange>,
    /// Order in which tiles were handed to the spawner; filled only with `StreamConfig::record_dispatches`.
    pub dispatch_log: Vec<TileId>,
}

/// Total order on f64 priorities that survives `u64` keys (negative and NaN safe).
pub(crate) fn priority_key(p: f64) -> u64 {
    let b = if p.is_nan() { f64::MAX.to_bits() } else { p.to_bits() };
    if b >> 63 == 1 {
        !b
    } else {
        b | (1 << 63)
    }
}

impl<T: Send + Sync + 'static> Drop for Streamer<T> {
    fn drop(&mut self) {
        self.cancel_all();
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
            last_error: HashMap::new(),
            tx,
            rx,
            frame: 0,
            stats: StreamStats::default(),
            pools: SlotPools::new(
                u32::try_from(config.capacity).expect("capacity in slots fits u32"),
                u32::try_from(config.vertex_pool_slots).expect("vertex pool slots fit u32"),
                config.retire_frames,
            ),
            changes: Vec::new(),
            dispatch_log: Vec::new(),
        }
    }

    /// Cancel everything that is not resident: the queued requests are dropped and never reach the spawner, the dispatched
    /// ones are told to stop (a job still waiting in the spawner ends at once, a running one at its next stage boundary,
    /// STRM-005) and none of them becomes resident. Returns how many requests were cancelled. Dropping the streamer does this.
    pub fn cancel_all(&mut self) -> usize {
        let mut n = self.queue.len();
        self.queue.clear();
        self.queued_key.clear();
        for f in self.in_flight.values() {
            if !f.token.swap(true, Ordering::SeqCst) {
                n += 1;
                self.stats.cancelled_prefetch += u64::from(f.class == RequestClass::Prefetch);
            }
        }
        self.stats.cancelled += n as u64;
        n
    }

    /// Load the six level-0 tiles once at start-up, so that some level of detail always exists (CON-22). This is the only
    /// place that waits. The six jobs are handed to the spawner together and the caller waits for all of them, so a pool
    /// loads them concurrently (STRM-001); the tiles are made resident in face order once all have arrived, so the
    /// result does not depend on the spawner, the thread count or the order the jobs finish (CON-14). If one fails, the
    /// others are told to stop, nothing becomes resident and the error names the failing tile (the lowest face if several).
    pub fn load_roots_blocking(&mut self) -> Result<(), String> {
        let ids: Vec<TileId> = planet_core::Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).expect("root")).collect();
        let mut slots = Vec::with_capacity(ids.len());
        for &id in &ids {
            let kind = self.pools.kind_of(id);
            match self.pools.acquire(kind, self.frame) {
                Some(slot) => slots.push(slot),
                None => {
                    for slot in slots {
                        self.pools.release(slot, self.frame);
                    }
                    return Err(format!("no slot for root {id:?}: capacity {} is below the six roots", self.config.capacity));
                }
            }
        }
        let tokens: Vec<Arc<AtomicBool>> = ids.iter().map(|_| Arc::new(AtomicBool::new(false))).collect();
        let (tx, rx) = channel::<Completion<T>>();
        for (&id, token) in ids.iter().zip(&tokens) {
            let (source, tx, token) = (Arc::clone(&self.source), tx.clone(), Arc::clone(token));
            self.spawner.spawn(Box::new(move || {
                let cancel = CancelToken(token);
                if cancel.is_cancelled() {
                    let _ = tx.send((id, Outcome::Cancelled));
                    return;
                }
                let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.load_staged(id, &cancel))) {
                    Ok(Ok(Some(data))) => Outcome::Done(data),
                    Ok(Ok(None)) => Outcome::Cancelled,
                    Ok(Err(e)) => Outcome::Failed(e),
                    Err(_) => Outcome::Failed(format!("source panicked while loading {id:?}")),
                };
                let _ = tx.send((id, outcome));
            }));
        }
        drop(tx);
        let mut results: Vec<Option<Outcome<T>>> = ids.iter().map(|_| None).collect();
        let mut missing = ids.len();
        while missing > 0 {
            let (id, outcome) = match rx.try_recv() {
                Ok(r) => r,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if self.spawner.help() {
                        continue;
                    }
                    match rx.recv() {
                        Ok(r) => r,
                        Err(_) => break, // the spawner dropped jobs unrun
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            };
            if matches!(outcome, Outcome::Failed(_)) {
                tokens.iter().for_each(|t| t.store(true, Ordering::SeqCst));
            }
            let i = ids.iter().position(|&x| x == id).expect("a root id");
            results[i] = Some(outcome);
            missing -= 1;
        }
        // A real failure is reported before the cancellations it caused (lowest face first within each).
        let mut error = results.iter().enumerate().find_map(|(i, r)| match r {
            Some(Outcome::Failed(e)) => Some(format!("root {:?} failed to load: {e}", ids[i])),
            _ => None,
        });
        let mut loaded = Vec::with_capacity(ids.len());
        for (i, r) in results.into_iter().enumerate() {
            match r {
                Some(Outcome::Done(d)) => loaded.push(d),
                Some(Outcome::Failed(_)) => {}
                Some(Outcome::Cancelled) | None => {
                    error.get_or_insert_with(|| format!("root {:?} was not loaded (cancelled or never run)", ids[i]));
                }
            }
        }
        if let Some(e) = error {
            for slot in slots {
                self.pools.release(slot, self.frame);
            }
            return Err(e);
        }
        for ((id, slot), data) in ids.into_iter().zip(slots).zip(loaded) {
            self.resident.insert(id, Resident { data: Arc::new(data), last_used: self.frame, slot });
            self.note(ResidentChange::Added(id));
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
        let mut queued_by_class = [0usize; 3];
        for k in self.queue.keys() {
            queued_by_class[usize::from(k.0)] += 1;
        }
        StreamStats {
            resident: self.resident.len(),
            queued: self.queue.len(),
            in_flight: self.in_flight.len(),
            queued_by_class,
            main_in_use: self.pools.main.in_use() as usize,
            main_retiring: self.pools.main.retiring() as usize,
            main_free: self.pools.main.free() as usize,
            ..self.stats
        }
    }

    /// Slots of the main pool: what the residency manager fits R to.
    pub fn main_capacity(&self) -> usize {
        self.config.capacity
    }

    /// Whether levels 0-2 have their own small pool (their tiles do not count against [`main_capacity`](Streamer::main_capacity)).
    pub fn has_vertex_pool(&self) -> bool {
        self.pools.vertex.is_some()
    }

    /// The slot a resident tile occupies: where the renderer finds (and put) its data.
    pub fn slot_of(&self, tile: TileId) -> Option<SlotRef> {
        self.resident.get(&tile).map(|r| r.slot)
    }

    pub fn pools(&self) -> &SlotPools {
        &self.pools
    }

    /// Frames begun so far (the clock of the slot retirement).
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// What to show for `tile`: itself or its nearest resident ancestor.
    pub fn resolve(&self, tile: TileId) -> Resolved {
        Resolved { wanted: tile, shown: self.nearest_resident(tile) }
    }

    fn nearest_resident(&self, mut t: TileId) -> Option<TileId> {
        loop {
            if self.resident.contains_key(&t) {
                return Some(t);
            }
            t = t.parent()?;
        }
    }

    /// One frame with only a draw class: [`begin_frame_wants`](Streamer::begin_frame_wants) with no residency set and
    /// no prefetch, so a tile is wanted exactly when it is drawn or an ancestor of one.
    pub fn begin_frame(&mut self, wanted: &[(TileId, f64)]) -> Vec<Resolved> {
        self.begin_frame_wants(&Wants { draw: wanted.to_vec(), ..Wants::default() })
    }

    /// One frame: collect finished jobs, request what is wanted by class (ancestors first), cancel what is in no class,
    /// dispatch within the in-flight limit, evict beyond capacity, and say what to draw for each tile of `wants.draw`.
    /// Never blocks. The order of requests is a pure function of `wants`, the resident set and the failure times: it
    /// does not depend on the spawner or the thread count (CON-14).
    pub fn begin_frame_wants(&mut self, wants: &Wants) -> Vec<Resolved> {
        self.frame += 1;
        let now = self.clock.now_ms();
        // 1. Completions. The result of a job that was cancelled meanwhile is dropped: a source that finished anyway may have cached it,
        // nothing is made resident (and so nothing is uploaded).
        while let Ok((tile, result)) = self.rx.try_recv() {
            let Some(flight) = self.in_flight.remove(&tile) else { continue };
            let was_cancelled = flight.token.load(Ordering::SeqCst);
            match result {
                Outcome::Done(_) if was_cancelled => {
                    self.stats.dropped += 1;
                    self.pools.release(flight.slot, self.frame);
                }
                Outcome::Done(data) => {
                    self.stats.completed += 1;
                    self.failed_at.remove(&tile);
                    self.last_error.remove(&tile);
                    self.resident.insert(tile, Resident { data: Arc::new(data), last_used: self.frame, slot: flight.slot });
                    self.note(ResidentChange::Added(tile));
                }
                Outcome::Cancelled => {
                    self.pools.release(flight.slot, self.frame);
                }
                Outcome::Failed(message) => {
                    self.last_error.insert(tile, message);
                    self.stats.failed += 1;
                    self.failed_at.insert(tile, now);
                    self.pools.release(flight.slot, self.frame);
                }
            }
        }
        // 2. What is needed, by class: a tile and all its ancestors take the best (class, rank, priority) of any
        // member of the sets they belong to. Draw: ancestors sort before descendants through the priority (parent-first).
        // Residency and prefetch: rank = level, so coarse comes before fine, then the caller's priority (distance).
        let mut needed: HashMap<TileId, (u8, u64, u64)> = HashMap::new();
        let mut want = |tile: TileId, class: RequestClass, rank: u64, priority: f64| {
            let key = (class as u8, rank, priority_key(priority));
            needed.entry(tile).and_modify(|e| *e = (*e).min(key)).or_insert(key);
        };
        for &(tile, priority) in &wants.draw {
            let (mut t, mut depth) = (Some(tile), 0.0);
            while let Some(x) = t {
                want(x, RequestClass::Draw, 0, priority - depth * 1.0e12);
                depth += 1.0;
                t = x.parent();
            }
        }
        for (set, class) in [(&wants.residency, RequestClass::Residency), (&wants.prefetch, RequestClass::Prefetch)] {
            for &(tile, priority) in set {
                let mut t = Some(tile);
                while let Some(x) = t {
                    want(x, class, u64::from(x.level()), priority);
                    t = x.parent();
                }
            }
        }
        // Tiles of D and R are in use (never evicted this frame); prefetch tiles are not, so eviction takes them first.
        for (t, key) in &needed {
            if key.0 < RequestClass::Prefetch as u8 {
                if let Some(r) = self.resident.get_mut(t) {
                    r.last_used = self.frame;
                }
            }
        }
        // 3. Cancel queued and in-flight requests that are in no class (prefetch requests count separately). A job
        // already cancelled is not revived (see below).
        let stale_queued: Vec<TileId> = self.queued_key.keys().filter(|t| !needed.contains_key(t)).copied().collect();
        for t in stale_queued {
            if let Some(k) = self.queued_key.remove(&t) {
                self.queue.remove(&k);
                self.stats.cancelled += 1;
                self.stats.cancelled_prefetch += u64::from(k.0 == RequestClass::Prefetch as u8);
            }
        }
        for (t, f) in &self.in_flight {
            // A cancelled job is never revived (whether it had reached a stage boundary would make the resident set
            // timing-dependent, CON-14): it ends, its result is dropped, and the tile is requested again if needed.
            if !needed.contains_key(t) && !f.token.swap(true, Ordering::SeqCst) {
                self.stats.cancelled += 1;
                self.stats.cancelled_prefetch += u64::from(f.class == RequestClass::Prefetch);
            }
        }
        // 4. Enqueue missing tiles unless recently failed.
        for (&t, &(class, rank, prio)) in &needed {
            if self.resident.contains_key(&t) || self.in_flight.contains_key(&t) {
                continue;
            }
            if self.failed_at.get(&t).is_some_and(|at| now.saturating_sub(*at) < self.config.retry_after_ms) {
                continue;
            }
            let key = (class, rank, prio, t);
            if let Some(old) = self.queued_key.insert(t, key) {
                self.queue.remove(&old);
            }
            self.queue.insert(key, ());
        }
        // 5. Make room for the requests about to start: evict (prefetch and tiles outside R first, then R tiles farthest and
        // finest first, never D with its ancestors or a root). A slot given back is reusable after `retire_frames`.
        self.make_room(&needed);
        // 6. Dispatch in queue order. Prefetch is first in the queue only when nothing of the other classes waits; it goes
        // out only into free slots and never evicts. A request without a slot blocks its pool for this frame.
        let mut blocked = [false; 2];
        let mut cursor: Option<QueueKey> = None;
        while self.in_flight.len() < self.config.max_in_flight {
            let next = match cursor {
                None => self.queue.keys().next().copied(),
                Some(c) => self.queue.range((std::ops::Bound::Excluded(c), std::ops::Bound::Unbounded)).next().map(|(k, _)| *k),
            };
            let Some(key) = next else { break };
            cursor = Some(key);
            let tile = key.3;
            let kind = self.pools.kind_of(tile);
            if blocked[kind as usize] {
                continue;
            }
            if key.0 == RequestClass::Prefetch as u8 {
                // Prefetch yields: it starts only while no draw or residency job is running either, and only into free capacity.
                let busy = self.in_flight.values().any(|f| f.class != RequestClass::Prefetch);
                if busy {
                    break;
                }
                if self.pools.pool(kind).free_at(self.frame) == 0 {
                    blocked[kind as usize] = true;
                    continue;
                }
            }
            let Some(slot) = self.pools.acquire(kind, self.frame) else {
                self.stats.blocked += 1;
                blocked[kind as usize] = true;
                continue;
            };
            self.queue.remove(&key);
            self.queued_key.remove(&tile);
            let token = Arc::new(AtomicBool::new(false));
            let class = match key.0 {
                0 => RequestClass::Draw,
                1 => RequestClass::Residency,
                _ => RequestClass::Prefetch,
            };
            self.in_flight.insert(tile, InFlight { token: Arc::clone(&token), class, slot });
            self.stats.dispatched += 1;
            if self.config.record_dispatches {
                self.dispatch_log.push(tile);
            }
            let (source, tx) = (Arc::clone(&self.source), self.tx.clone());
            self.spawner.spawn(Box::new(move || {
                let cancel = CancelToken(token);
                if cancel.is_cancelled() {
                    let _ = tx.send((tile, Outcome::Cancelled));
                    return;
                }
                // A panicking source must not wedge the tile: it becomes a failure with the usual retry delay.
                let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.load_staged(tile, &cancel))) {
                    Ok(Ok(Some(data))) => Outcome::Done(data),
                    Ok(Ok(None)) => Outcome::Cancelled,
                    Ok(Err(e)) => Outcome::Failed(e),
                    Err(_) => Outcome::Failed(format!("source panicked while loading {tile:?}")),
                };
                let _ = tx.send((tile, outcome));
            }));
        }
        wants.draw.iter().map(|&(t, _)| Resolved { wanted: t, shown: self.nearest_resident(t) }).collect()
    }

    /// Evict so that the requests about to be dispatched find slots (STRM-007): for each pool, as many slots as the first
    /// requests of the queue need (within the in-flight limit) minus those free or already retiring. Prefetch needs none
    /// (it only takes free slots). Victims: tiles in no class or prefetch only, least recently used first; then, for draw
    /// requests only, R tiles farthest first and finest first. The draw set with its ancestors and the roots are never victims.
    fn make_room(&mut self, needed: &HashMap<TileId, (u8, u64, u64)>) {
        // Look ahead as far as the slots take to come back: a slot given back now is usable `retire_frames` frames later, so
        // the requests of the next frames need their room made now or the pool runs dry in a stop-and-go rhythm.
        let room =
            self.config.max_in_flight.saturating_sub(self.in_flight.len()) + self.config.max_in_flight * self.config.retire_frames as usize;
        // [pool][0 = draw requests, 1 = residency requests]
        let mut demand = [[0usize; 2]; 2];
        for k in self.queue.keys().take(room) {
            if k.0 == RequestClass::Prefetch as u8 {
                break;
            }
            demand[self.pools.kind_of(k.3) as usize][usize::from(k.0 == RequestClass::Residency as u8)] += 1;
        }
        for kind in [PoolKind::Main, PoolKind::Vertex] {
            let [draw, res] = demand[kind as usize];
            if draw + res == 0 {
                continue;
            }
            // Slots that are free or will be reusable within the retire delay.
            let coming = |s: &Self| s.pools.pool(kind).free() as usize + s.pools.pool(kind).retiring() as usize;
            // Victims of the pool, best first. Tier 0: in no class, or prefetch only (least recently used first).
            // Tier 1: R only (farthest first, then finest).
            let mut tier0: Vec<(u64, TileId)> = Vec::new();
            let mut tier1: Vec<(Reverse<u64>, Reverse<u8>, TileId)> = Vec::new();
            for (&t, r) in &self.resident {
                if t.level() == 0 || self.pools.kind_of(t) != kind {
                    continue;
                }
                match needed.get(&t) {
                    None => tier0.push((r.last_used, t)),
                    Some(n) if n.0 == RequestClass::Prefetch as u8 => tier0.push((r.last_used, t)),
                    Some(n) if n.0 == RequestClass::Residency as u8 => tier1.push((Reverse(n.2), Reverse(t.level()), t)),
                    Some(_) => {}
                }
            }
            tier0.sort();
            tier1.sort();
            let mut victims = tier0.into_iter().map(|(_, t)| t);
            while draw + res > coming(self) {
                let Some(t) = victims.next() else { break };
                self.evict(t);
            }
            let mut victims = tier1.into_iter().map(|(_, _, t)| t);
            while draw > coming(self) {
                let Some(t) = victims.next() else { break };
                self.evict(t);
            }
        }
    }

    fn evict(&mut self, tile: TileId) {
        if let Some(r) = self.resident.remove(&tile) {
            self.pools.release(r.slot, self.frame);
            self.stats.evicted += 1;
            self.note(ResidentChange::Evicted(tile));
        }
    }

    fn note(&mut self, change: ResidentChange) {
        if self.config.record_changes {
            self.changes.push(change);
        }
    }

    /// Whether resident-set changes are being recorded (`StreamConfig::record_changes`).
    pub fn records_changes(&self) -> bool {
        self.config.record_changes
    }

    /// The changes of the resident set since the last call, oldest first (empty unless `StreamConfig::record_changes`).
    pub fn take_changes(&mut self) -> Vec<ResidentChange> {
        std::mem::take(&mut self.changes)
    }

    /// The message of the last failure of `tile`, if any.
    pub fn last_error(&self, tile: TileId) -> Option<&str> {
        self.last_error.get(&tile).map(String::as_str)
    }

    /// Tiles whose requests ended in failure and are waiting for the retry delay.
    pub fn failed_tiles(&self) -> HashSet<TileId> {
        self.failed_at.keys().copied().collect()
    }
}
