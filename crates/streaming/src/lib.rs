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
mod layer;
pub mod manager;
pub mod priority;
mod queue;
pub mod slots;

pub use budget::{PoolConfig, Tier};
pub use layer::SchedWork;
pub use manager::{degrade, DegradationReport, Degraded, ManagerFrame, ResidencyManager};
pub use priority::Priority;
pub use slots::{PoolKind, SlotPool, SlotPools, SlotRef, VERTEX_POOL_MAX_LEVEL};

use layer::{normalized, Layer};
use planet_core::{TileId, Vec3};
use queue::RequestQueue;
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// A fixed, fast hasher for the scheduler's maps keyed by tile ids (internal; no map's iteration order decides anything, so the
/// choice affects speed only): one multiply per word and a final mix so that the low bits of structured ids spread.
#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher(u64);

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(u64::from(*b));
        }
    }

    #[inline]
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    #[inline]
    fn finish(&self) -> u64 {
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

pub(crate) type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub(crate) type FxSet<K> = HashSet<K, BuildHasherDefault<FxHasher>>;

/// A unit of work handed to a [`Spawner`].
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// One tile job that ran to its end, as the worker saw it (STRM-022). Wall-clock readings of the worker threads, kept for the
/// perf log: no decision of the streamer reads them, so no order or result depends on them (CON-14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobSample {
    /// Microseconds from the dispatch to the job's start: the time it waited in the spawner's queue.
    pub wait_us: u64,
    /// Microseconds the source took to load the tile (a cancelled or failed load included).
    pub run_us: u64,
    /// Jobs inside their load when this one started, itself included: the concurrency the load ran under.
    pub running: u32,
    /// The load produced a tile (not cancelled, not failed).
    pub done: bool,
    /// The level of the tile.
    pub level: u8,
}

/// Most samples a log keeps (about 25 MB); later jobs run unlogged.
const MAX_JOB_SAMPLES: usize = 1 << 20;

/// The jobs that ran, in the order they ended, and how many run now (STRM-022): recorded only.
#[derive(Default)]
pub struct JobLog {
    running: std::sync::atomic::AtomicU32,
    samples: Mutex<Vec<JobSample>>,
}

impl JobLog {
    /// Jobs recorded so far.
    pub fn len(&self) -> usize {
        self.samples.lock().expect("job log").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The samples from index `from` on (a case reads the part after its settling).
    pub fn since(&self, from: usize) -> Vec<JobSample> {
        self.samples.lock().expect("job log").get(from..).unwrap_or_default().to_vec()
    }

    /// Run `load`, recording its duration, how long the job waited since `queued` and how many loads ran beside it.
    fn timed<R>(&self, level: u8, queued: std::time::Instant, load: impl FnOnce() -> R, done: impl Fn(&R) -> bool) -> R {
        let started = std::time::Instant::now();
        let running = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        let out = load();
        let run_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.running.fetch_sub(1, Ordering::SeqCst);
        let wait_us = u64::try_from(started.saturating_duration_since(queued).as_micros()).unwrap_or(u64::MAX);
        let sample = JobSample { wait_us, run_us, running, done: done(&out), level };
        let mut samples = self.samples.lock().expect("job log");
        if samples.len() < MAX_JOB_SAMPLES {
            samples.push(sample);
        }
        out
    }
}

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
        Self::with_priority(threads, Priority::Normal)
    }

    /// `threads` workers, each setting its own thread's `priority` before it takes its first job (STRM-020). A platform that
    /// refuses the request leaves the worker at normal priority; the pool still works.
    pub fn with_priority(threads: usize, priority: Priority) -> Self {
        let (tx, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let workers = (0..threads.max(1))
            .map(|_| {
                let rx = Arc::clone(&rx);
                std::thread::spawn(move || {
                    if priority != Priority::Normal {
                        priority::set_current(priority);
                    }
                    loop {
                        let job = rx.lock().expect("pool receiver").recv();
                        match job {
                            Ok(j) => j(),
                            Err(_) => break,
                        }
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

/// Deterministic spawner with a generation-rate model (TEST-018, STRM-020): `workers` machines, each running one job at a time
/// for `duration_us(n)` microseconds of model time (`n` counts jobs from 0), jobs waiting in submission order for a free worker,
/// like a thread pool. Time moves only by [`advance`](TimedSpawner::advance), which runs the jobs that finish on the way, in
/// finish order (ties in submission order), and starts a waiting job on the worker that freed up at that moment, so a worker is
/// never idle while a job waits (the pool's behaviour), while the streamer, which sees completions only at a frame boundary,
/// refills only then. A job runs when it finishes; the model has no threads.
pub struct TimedSpawner {
    state: Mutex<TimedState>,
}

struct TimedState {
    workers: usize,
    duration_us: Box<dyn Fn(u64) -> u64 + Send>,
    now_us: u64,
    started: u64,
    waiting: VecDeque<Job>,
    /// `(finish_us, submission number, job)` of the jobs on a worker.
    running: Vec<(u64, u64, Job)>,
    finished: u64,
}

impl TimedSpawner {
    /// `workers` machines, every job taking `duration_us` microseconds.
    pub fn new(workers: usize, duration_us: u64) -> Self {
        Self::with_durations(workers, move |_| duration_us)
    }

    /// As [`new`](TimedSpawner::new) with the duration of job number `n` a pure function of `n`.
    pub fn with_durations(workers: usize, duration_us: impl Fn(u64) -> u64 + Send + 'static) -> Self {
        Self {
            state: Mutex::new(TimedState {
                workers: workers.max(1),
                duration_us: Box::new(duration_us),
                now_us: 0,
                started: 0,
                waiting: VecDeque::new(),
                running: Vec::new(),
                finished: 0,
            }),
        }
    }

    /// Jobs on a worker now.
    pub fn busy(&self) -> usize {
        self.state.lock().expect("timed").running.len()
    }

    /// Jobs waiting for a worker.
    pub fn waiting(&self) -> usize {
        self.state.lock().expect("timed").waiting.len()
    }

    /// Jobs finished (run) so far.
    pub fn finished(&self) -> u64 {
        self.state.lock().expect("timed").finished
    }

    /// Move model time forward by `dt_us` and run what finishes on the way.
    pub fn advance(&self, dt_us: u64) {
        let target = self.state.lock().expect("timed").now_us + dt_us;
        self.run_until(target);
    }

    fn run_until(&self, target: u64) {
        loop {
            let job = {
                let mut st = self.state.lock().expect("timed");
                let next = st.running.iter().enumerate().min_by_key(|(_, (f, n, _))| (*f, *n)).map(|(i, (f, _, _))| (i, *f));
                match next {
                    Some((i, f)) if f <= target => {
                        let (_, _, job) = st.running.swap_remove(i);
                        st.now_us = st.now_us.max(f);
                        st.finished += 1;
                        // The worker that freed up takes the next waiting job at once.
                        if let Some(next) = st.waiting.pop_front() {
                            st.start(next);
                        }
                        job
                    }
                    _ => {
                        st.now_us = st.now_us.max(target);
                        return;
                    }
                }
            };
            job();
        }
    }
}

impl TimedState {
    fn start(&mut self, job: Job) {
        let n = self.started;
        self.started += 1;
        let finish = self.now_us + (self.duration_us)(n);
        self.running.push((finish, n, job));
    }
}

impl Spawner for TimedSpawner {
    fn spawn(&self, job: Job) {
        let mut st = self.state.lock().expect("timed");
        if st.running.len() < st.workers {
            st.start(job);
        } else {
            st.waiting.push_back(job);
        }
    }

    /// Let model time pass until the next job finishes (a blocking caller such as `load_roots_blocking`).
    fn help(&self) -> bool {
        let next = self.state.lock().expect("timed").running.iter().map(|(f, _, _)| *f).min();
        match next {
            Some(f) => {
                self.run_until(f);
                true
            }
            None => false,
        }
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
    /// Prefetch set: R at the predicted position (translation only). Empty while the prefetch source is off (STRM-018).
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
    /// Jobs running at once: the generation pool's worker count (STRM-020; [`GenWorkers`] derives it from the pool).
    pub max_in_flight: usize,
    /// Jobs handed to the spawner beyond `max_in_flight`, waiting in its queue for the worker that frees up between two frames
    /// (STRM-020): the scheduler sees completions at a frame boundary only, so without them a worker that finishes mid-frame idles
    /// until the next. They are dispatched in queue order like any job and cancelled like one (a queued job stops at its start);
    /// 0: none.
    pub queue_ahead: usize,
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
            queue_ahead: 0,
            capacity: 4096,
            vertex_pool_slots: 0,
            retire_frames: 0,
            retry_after_ms: 1000,
            record_dispatches: false,
            record_changes: false,
        }
    }
}

impl StreamConfig {
    /// Jobs handed to the spawner at once: the running ones and those queued behind them.
    pub fn job_limit(&self) -> usize {
        self.max_in_flight + self.queue_ahead
    }
}

/// Hardware threads the default tile generation pool leaves free (STRM-020, perf window 2): the frame thread, the plan worker and
/// the driver's threads need cores of their own, or a frame's upload and scheduler steps wait for the OS to schedule them behind
/// the generation threads (their time then varies by milliseconds with the load, not with the bytes). A named default, not a
/// measured optimum: `--gen-reserve N` changes it, `--gen-workers N` sets the pool size outright.
pub const GEN_RESERVE: usize = 2;

/// The size of the tile generation pool and how far the scheduler queues ahead of it (STRM-020): one place that builds the pool
/// and sets the streamer's limits from it, so the number of jobs running at once is the number of threads that can run them.
/// The plan worker's threads are separate (its own pool and scoped threads), not part of this count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenWorkers {
    pub workers: usize,
    pub ahead: usize,
    /// The priority the workers run at (ADR 0046; below normal unless `--gen-priority normal`): changes when they run, never what
    /// they compute.
    pub priority: Priority,
}

impl GenWorkers {
    /// `workers` threads and as many jobs queued behind them.
    pub fn new(workers: usize) -> Self {
        let workers = workers.max(1);
        Self { workers, ahead: workers, priority: Priority::BelowNormal }
    }

    /// The default: one worker per hardware thread of the machine less [`GEN_RESERVE`] cores kept for the frame thread, the plan
    /// worker and the driver's threads.
    pub fn machine() -> Self {
        Self::machine_reserving(GEN_RESERVE)
    }

    /// One worker per hardware thread of the machine less `reserve` cores (`--gen-reserve N`).
    pub fn machine_reserving(reserve: usize) -> Self {
        Self::new(Self::workers_for(std::thread::available_parallelism().map_or(4, |n| n.get()), reserve))
    }

    /// The pool size for a machine with `cores` hardware threads of which `reserve` stay free: `cores - reserve`, never below one
    /// and never above `cores` (a pure function, so the default is the same wherever the arithmetic is read).
    pub fn workers_for(cores: usize, reserve: usize) -> usize {
        cores.saturating_sub(reserve).max(1).min(cores.max(1))
    }

    pub fn with_ahead(self, ahead: usize) -> Self {
        Self { ahead, ..self }
    }

    pub fn with_priority(self, priority: Priority) -> Self {
        Self { priority, ..self }
    }

    /// The pool: exactly `workers` threads.
    pub fn pool(&self) -> Arc<ThreadPool> {
        Arc::new(ThreadPool::with_priority(self.workers, self.priority))
    }

    /// `config` with the in-flight limits of this pool.
    pub fn apply(&self, config: StreamConfig) -> StreamConfig {
        StreamConfig { max_in_flight: self.workers, queue_ahead: self.ahead, ..config }
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
    /// The frame the tile became resident.
    inserted: u64,
    /// The last frame the tile was in the draw or residency class, while it is in neither (the key of its place among the
    /// eviction candidates). A tile in one of them is "in use now" and its value is not read.
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
    resident: FxMap<TileId, Resident<T>>,
    /// Queued requests in start order (class, rank, priority key, tile), evaluated lazily.
    queue: RequestQueue,
    in_flight: FxMap<TileId, InFlight>,
    failed_at: FxMap<TileId, u64>,
    /// `(failure time, tile)` of the failures still waiting out the retry delay.
    retry_due: BTreeSet<(u64, TileId)>,
    last_error: FxMap<TileId, String>,
    tx: Sender<Completion<T>>,
    rx: Receiver<Completion<T>>,
    frame: u64,
    stats: StreamStats,
    pools: SlotPools,
    changes: Vec<ResidentChange>,
    /// The wants as persistent sets (STRM-019): draw, residency, prefetch. A request class is a tile's best layer.
    layers: [Layer; 3],
    /// Lists handed over since the last frame, applied at its start (after the completions).
    pending: [Option<Vec<(TileId, f64)>>; 3],
    /// Tiles that may need a request: wanted, and not resident, running or waiting out a failure when last looked at.
    missing: FxSet<TileId>,
    /// Resident tiles in no class or in prefetch only (level 0 excluded), per pool, least recently used first: the victims
    /// of [`Streamer::make_room`]'s first tier.
    victims: [BTreeSet<(u64, TileId)>; 2],
    /// The queue was cleared by [`Streamer::cancel_all`]: every wanted tile is looked at again at the next frame.
    rescan: bool,
    work: SchedWork,
    /// Tiles in the draw or residency class that count against the pool, and counted tiles of the draw class's closure.
    active_counted: usize,
    draw_counted: usize,
    /// Order in which tiles were handed to the spawner; filled only with `StreamConfig::record_dispatches`.
    pub dispatch_log: Vec<TileId>,
    /// The jobs that ran, as their workers saw them (STRM-022; recorded only).
    job_log: Arc<JobLog>,
    /// Jobs that have sent their result (or their cancellation) to the channel; the difference to `stats.dispatched` is the jobs
    /// still running or waiting, which a test mode waits out ([`Streamer::unfinished_jobs`]).
    sent: Arc<std::sync::atomic::AtomicU64>,
}

/// The best class among the layers a tile is in: draw, residency, prefetch.
fn class_from(member: [bool; 3]) -> Option<RequestClass> {
    if member[0] {
        Some(RequestClass::Draw)
    } else if member[1] {
        Some(RequestClass::Residency)
    } else if member[2] {
        Some(RequestClass::Prefetch)
    } else {
        None
    }
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
            resident: FxMap::default(),
            queue: RequestQueue::default(),
            in_flight: FxMap::default(),
            failed_at: FxMap::default(),
            retry_due: BTreeSet::new(),
            last_error: FxMap::default(),
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
            layers: [Layer::new(true), Layer::new(false), Layer::new(false)],
            pending: [None, None, None],
            missing: FxSet::default(),
            victims: [BTreeSet::new(), BTreeSet::new()],
            rescan: false,
            work: SchedWork::default(),
            active_counted: 0,
            draw_counted: 0,
            dispatch_log: Vec::new(),
            job_log: Arc::default(),
            sent: Arc::default(),
        }
    }

    /// SPIKE-13 ONLY (never merged): request the resident tiles of `tiles` again without dropping them (stale while revalidating): the
    /// new tile takes a second slot, replaces the old one when it completes and gives the old slot back. Returns the requests queued.
    pub fn refresh(&mut self, tiles: &[TileId]) -> usize {
        let mut n = 0;
        for &t in tiles {
            if self.resident.contains_key(&t) && !self.in_flight.contains_key(&t) {
                if self.class_of(t).is_none() {
                    // Held only until the share needs the slot: not worth a request, drop the old version.
                    self.evict(t);
                    continue;
                }
                self.request(t);
                n += 1;
            }
        }
        n
    }

    /// SPIKE-13 ONLY (never merged): drop the resident tiles of `tiles` and cancel their running jobs, so the next frames request them
    /// again (they are still in their classes). Returns how many resident tiles were dropped.
    pub fn invalidate(&mut self, tiles: &[TileId]) -> usize {
        let mut dropped = 0;
        for &t in tiles {
            if let Some(f) = self.in_flight.get(&t) {
                f.token.store(true, Ordering::SeqCst);
            }
            if self.resident.contains_key(&t) {
                self.evict(t);
                dropped += 1;
            }
        }
        self.rescan = true;
        dropped
    }

    /// Cancel everything that is not resident: the queued requests are dropped and never reach the spawner, the dispatched
    /// ones are told to stop (a job still waiting in the spawner ends at once, a running one at its next stage boundary,
    /// STRM-005) and none of them becomes resident. Returns how many requests were cancelled. Dropping the streamer does this.
    pub fn cancel_all(&mut self) -> usize {
        let mut n = self.queue.len();
        self.queue.clear();
        self.rescan = true;
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
            self.resident.insert(id, Resident { data: Arc::new(data), inserted: self.frame, last_used: self.frame, slot });
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
        StreamStats {
            resident: self.resident.len(),
            queued: self.queue.len(),
            in_flight: self.in_flight.len(),
            queued_by_class: self.queue.by_class(),
            main_in_use: self.pools.main.in_use() as usize,
            main_retiring: self.pools.main.retiring() as usize,
            main_free: self.pools.main.free() as usize,
            ..self.stats
        }
    }

    /// Jobs handed to the spawner whose result has not reached the channel yet (running or waiting for a worker). Results that
    /// did arrive but were not collected by [`Streamer::step`] are not counted: when this is 0 the next step collects every
    /// job dispatched so far. For harnesses that make a loop independent of thread timing (`test-render --lockstep`).
    pub fn unfinished_jobs(&self) -> u64 {
        self.stats.dispatched.saturating_sub(self.sent.load(Ordering::SeqCst))
    }

    /// The log of the jobs that ran (STRM-022): recorded only, shared with the workers.
    pub fn job_log(&self) -> Arc<JobLog> {
        Arc::clone(&self.job_log)
    }

    /// What the scheduler's frames touched so far (STRM-019, CON-20): the difference of two readings is a frame's work.
    pub fn work(&self) -> SchedWork {
        self.work
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

    /// One frame with these wants for all three classes: [`set_draw`](Streamer::set_draw), [`set_residency`](Streamer::set_residency)
    /// and [`set_prefetch`](Streamer::set_prefetch) with the lists, [`step`](Streamer::step), and an answer for each tile of
    /// `wants.draw`: the resident tile to show. A frame that changed nothing from the last costs a pass over the lists; a
    /// caller that holds its wants between frames hands over only what changed and calls `step`.
    pub fn begin_frame_wants(&mut self, wants: &Wants) -> Vec<Resolved> {
        self.set_draw(&wants.draw);
        self.set_residency(&wants.residency);
        self.set_prefetch(&wants.prefetch);
        self.step();
        wants.draw.iter().map(|&(t, _)| Resolved { wanted: t, shown: self.nearest_resident(t) }).collect()
    }

    /// The draw set D for the next frame, until replaced (the ancestors of every tile are implied). Priority: lower first.
    pub fn set_draw(&mut self, draw: &[(TileId, f64)]) {
        self.set_layer(RequestClass::Draw, draw);
    }

    /// The residency set R for the next frame, until replaced.
    pub fn set_residency(&mut self, residency: &[(TileId, f64)]) {
        self.set_layer(RequestClass::Residency, residency);
    }

    /// The prefetch set for the next frame, until replaced.
    pub fn set_prefetch(&mut self, prefetch: &[(TileId, f64)]) {
        self.set_layer(RequestClass::Prefetch, prefetch);
    }

    fn set_layer(&mut self, class: RequestClass, list: &[(TileId, f64)]) {
        self.work.scanned += list.len() as u64;
        self.pending[class as usize] = Some(normalized(list).into_owned());
    }

    /// Two numbers of a degradation report that the layers know without a pass over the wants: the tiles in the draw or
    /// residency class, with the six roots, that count against the pool; and the counted tiles of the draw set's closure with
    /// the roots.
    pub fn closure_numbers(&self) -> (usize, usize) {
        let counted = |t: TileId| !self.has_vertex_pool() || t.level() > VERTEX_POOL_MAX_LEVEL;
        let roots: Vec<TileId> = planet_core::Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).expect("root")).collect();
        let root_outside_draw = roots.iter().filter(|&&r| counted(r) && !self.layers[0].has(r)).count();
        let root_outside_active = roots.iter().filter(|&&r| counted(r) && !self.is_active(r)).count();
        (self.active_counted + root_outside_active, self.draw_counted + root_outside_draw)
    }

    fn is_active(&self, t: TileId) -> bool {
        self.layers[0].has(t) || self.layers[1].has(t)
    }

    fn class_of(&self, t: TileId) -> Option<RequestClass> {
        class_from([self.layers[0].has(t), self.layers[1].has(t), self.layers[2].has(t)])
    }

    /// One frame: collect finished jobs, apply the wants that changed, request what is newly wanted by class (ancestors
    /// first), cancel what left every class, evict to make room, dispatch within the in-flight limit. Never blocks. The
    /// order of requests is a pure function of the wants, the resident set and the failure times: it does not depend on the
    /// spawner or the thread count (CON-14). The work of a frame follows what changed since the last (STRM-019): completions,
    /// the tiles added to or removed from a class, the requests that start, the evictions they need.
    pub fn step(&mut self) {
        self.frame += 1;
        let now = self.clock.now_ms();
        self.collect(now);
        self.apply_wants();
        self.request_missing(now);
        self.make_room();
        self.dispatch();
    }

    /// Step 1, completions. The result of a job that was cancelled meanwhile is dropped: a source that finished anyway may have
    /// cached it, nothing is made resident (and so nothing is uploaded).
    fn collect(&mut self, now: u64) {
        while let Ok((tile, result)) = self.rx.try_recv() {
            let Some(flight) = self.in_flight.remove(&tile) else { continue };
            self.work.scanned += 1;
            let was_cancelled = flight.token.load(Ordering::SeqCst);
            match result {
                Outcome::Done(_) if was_cancelled => {
                    self.stats.dropped += 1;
                    self.pools.release(flight.slot, self.frame);
                    self.missing.insert(tile);
                }
                Outcome::Done(data) => {
                    self.stats.completed += 1;
                    if let Some(at) = self.failed_at.remove(&tile) {
                        self.retry_due.remove(&(at, tile));
                    }
                    self.last_error.remove(&tile);
                    if let Some(old) = self.resident.insert(tile, Resident { data: Arc::new(data), inserted: self.frame, last_used: self.frame, slot: flight.slot }) {
                        // SPIKE-13: a refreshed tile: its old slot goes back (reusable after the frames in flight retire).
                        self.pools.release(old.slot, self.frame);
                        if tile.level() > 0 {
                            self.victims[self.pools.kind_of(tile) as usize].remove(&(old.last_used, tile));
                        }
                    }
                    if tile.level() > 0 && !self.is_active(tile) {
                        self.victims[self.pools.kind_of(tile) as usize].insert((self.frame, tile));
                        self.work.updated += 1;
                    }
                    self.note(ResidentChange::Added(tile));
                }
                Outcome::Cancelled => {
                    self.pools.release(flight.slot, self.frame);
                    self.missing.insert(tile);
                }
                Outcome::Failed(message) => {
                    self.last_error.insert(tile, message);
                    self.stats.failed += 1;
                    if let Some(old) = self.failed_at.insert(tile, now) {
                        self.retry_due.remove(&(old, tile));
                    }
                    self.retry_due.insert((now, tile));
                    self.pools.release(flight.slot, self.frame);
                    self.missing.insert(tile);
                }
            }
            self.work.updated += 1;
        }
    }

    /// Step 2, the wants. The lists handed over since the last frame replace the layers; what changed class is handled once per tile, after all
    /// three layers changed (a tile that moves from one class to another is not cancelled on the way).
    fn apply_wants(&mut self) {
        let mut touched: FxMap<TileId, [Option<bool>; 3]> = FxMap::default();
        for class in 0..3 {
            let Some(list) = self.pending[class].take() else { continue };
            let diff = self.layers[class].replace(list, &mut self.work, class);
            if diff.changed {
                self.queue.mark_stale(class as u8);
            }
            for (t, was) in diff.flipped {
                if class == 0 && (!self.has_vertex_pool() || t.level() > VERTEX_POOL_MAX_LEVEL) {
                    if was {
                        self.draw_counted -= 1;
                    } else {
                        self.draw_counted += 1;
                    }
                }
                touched.entry(t).or_default()[class] = Some(was);
            }
        }
        for (t, was) in touched {
            let member = |l: usize| was[l].unwrap_or_else(|| self.layers[l].has(t));
            let old = class_from([member(0), member(1), member(2)]);
            let new = self.class_of(t);
            if old != new {
                self.class_changed(t, old, new);
            }
        }
    }

    /// A tile's best class changed from `old` to `new` (`None`: in no class).
    fn class_changed(&mut self, t: TileId, old: Option<RequestClass>, new: Option<RequestClass>) {
        let active = |c: Option<RequestClass>| matches!(c, Some(RequestClass::Draw | RequestClass::Residency));
        if active(old) != active(new) && (!self.has_vertex_pool() || t.level() > VERTEX_POOL_MAX_LEVEL) {
            if active(new) {
                self.active_counted += 1;
            } else {
                self.active_counted -= 1;
            }
        }
        self.work.updated += 1;
        if let Some(r) = self.resident.get_mut(&t) {
            if t.level() > 0 {
                let kind = self.pools.kind_of(t) as usize;
                if active(old) && !active(new) {
                    // Last in use the frame before (or the frame it arrived, if that is now).
                    r.last_used = r.inserted.max(self.frame - 1);
                    self.victims[kind].insert((r.last_used, t));
                } else if !active(old) && active(new) {
                    self.victims[kind].remove(&(r.last_used, t));
                }
            }
            return;
        }
        if let Some(f) = self.in_flight.get(&t) {
            // A cancelled job is never revived (whether it had reached a stage boundary would make the resident set
            // timing-dependent, CON-14): it ends, its result is dropped, and the tile is requested again if needed.
            if new.is_none() && !f.token.swap(true, Ordering::SeqCst) {
                self.stats.cancelled += 1;
                self.stats.cancelled_prefetch += u64::from(f.class == RequestClass::Prefetch);
            }
            return;
        }
        match new {
            None => {
                if let Some(class) = self.queue.remove(t) {
                    self.stats.cancelled += 1;
                    self.stats.cancelled_prefetch += u64::from(class == RequestClass::Prefetch as u8);
                }
            }
            Some(_) => {
                self.missing.insert(t);
            }
        }
    }

    /// Queue `t` under its class and rank (draw: 0; residency and prefetch: level, coarse first); its place among the requests of
    /// that bucket is the layer's priority key, found when the bucket is read.
    fn request(&mut self, t: TileId) {
        let Some(class) = self.class_of(t) else { return };
        let rank = if class == RequestClass::Draw { 0 } else { u64::from(t.level()) };
        if self.queue.insert(t, class as u8, rank, self.pools.kind_of(t) as usize) {
            self.work.updated += 1;
        }
    }

    /// Step 3, requests. Enqueue the tiles that may need a request: retry failures that waited long enough, and look at each tile that became
    /// missing or changed class since the last frame.
    fn request_missing(&mut self, now: u64) {
        if self.rescan {
            self.rescan = false;
            for l in 0..3 {
                let tiles: Vec<TileId> = self.layers[l].member_tiles().collect();
                self.missing.extend(tiles);
            }
        }
        while let Some(&(at, tile)) = self.retry_due.iter().next() {
            if now.saturating_sub(at) < self.config.retry_after_ms {
                break;
            }
            self.retry_due.remove(&(at, tile));
            self.missing.insert(tile);
        }
        if self.missing.is_empty() {
            return;
        }
        let missing = std::mem::take(&mut self.missing);
        self.work.scanned += missing.len() as u64;
        for t in missing {
            if self.resident.contains_key(&t) || self.in_flight.contains_key(&t) {
                continue;
            }
            if self.failed_at.get(&t).is_some_and(|at| now.saturating_sub(*at) < self.config.retry_after_ms) {
                continue;
            }
            self.request(t);
        }
    }

    /// Step 5, dispatch in queue order. Prefetch is first in the queue only when nothing of the other classes waits; it goes
    /// out only into free slots and never evicts. A request without a slot blocks its pool for this frame.
    fn dispatch(&mut self) {
        let mut blocked = [false; 2];
        // Entries of each pool passed over: when every pool is blocked or has nothing left behind the cursor, nothing can start.
        let mut passed = [0usize; 2];
        let mut cursor: Option<QueueKey> = None;
        while self.in_flight.len() < self.config.job_limit() {
            let queued = self.queue.by_kind();
            if (0..2).all(|k| blocked[k] || passed[k] >= queued[k]) {
                break;
            }
            let Some(key) = self.next_request(cursor) else { break };
            cursor = Some(key);
            self.work.scanned += 1;
            let tile = key.3;
            let kind = self.pools.kind_of(tile);
            if blocked[kind as usize] {
                passed[kind as usize] += 1;
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
                    passed[kind as usize] += 1;
                    continue;
                }
            }
            let Some(slot) = self.pools.acquire(kind, self.frame) else {
                self.stats.blocked += 1;
                blocked[kind as usize] = true;
                passed[kind as usize] += 1;
                continue;
            };
            self.queue.remove(tile);
            let token = Arc::new(AtomicBool::new(false));
            let class = match key.0 {
                0 => RequestClass::Draw,
                1 => RequestClass::Residency,
                _ => RequestClass::Prefetch,
            };
            self.in_flight.insert(tile, InFlight { token: Arc::clone(&token), class, slot });
            self.stats.dispatched += 1;
            self.work.updated += 1;
            if self.config.record_dispatches {
                self.dispatch_log.push(tile);
            }
            let (source, tx, log) = (Arc::clone(&self.source), self.tx.clone(), Arc::clone(&self.job_log));
            let sent = Arc::clone(&self.sent);
            let queued = std::time::Instant::now();
            self.spawner.spawn(Box::new(move || {
                let cancel = CancelToken(token);
                if cancel.is_cancelled() {
                    let _ = tx.send((tile, Outcome::Cancelled));
                    sent.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                // A panicking source must not wedge the tile: it becomes a failure with the usual retry delay.
                let loaded = log.timed(
                    tile.level(),
                    queued,
                    || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.load_staged(tile, &cancel))),
                    |r| matches!(r, Ok(Ok(Some(_)))),
                );
                let outcome = match loaded {
                    Ok(Ok(Some(data))) => Outcome::Done(data),
                    Ok(Ok(None)) => Outcome::Cancelled,
                    Ok(Err(e)) => Outcome::Failed(e),
                    Err(_) => Outcome::Failed(format!("source panicked while loading {tile:?}")),
                };
                let _ = tx.send((tile, outcome));
                sent.fetch_add(1, Ordering::SeqCst);
            }));
        }
    }

    /// The queued request after `after` in start order (the first when `None`): the buckets it reads are put in order first.
    fn next_request(&mut self, after: Option<QueueKey>) -> Option<QueueKey> {
        let layers = &mut self.layers;
        let mut keyed = 0u64;
        let next = self.queue.next_after(after, &mut |class, t| layers[usize::from(class)].key(t, &mut self.work), &mut keyed);
        self.work.updated += keyed;
        next
    }

    /// Step 4, make room. Evict so that the requests about to be dispatched find slots (STRM-007): for each pool, as many slots as the first
    /// requests of the queue need (within the in-flight limit) minus those free or already retiring. Prefetch needs none
    /// (it only takes free slots). Victims: tiles in no class or prefetch only, least recently used first (kept in order as
    /// the classes change); then, for draw requests only, R tiles farthest first and finest first. The draw set with its
    /// ancestors and the roots are never victims.
    fn make_room(&mut self) {
        // Look ahead as far as the slots take to come back: a slot given back now is usable `retire_frames` frames later, so
        // the requests of the next frames need their room made now or the pool runs dry in a stop-and-go rhythm.
        let room =
            self.config.job_limit().saturating_sub(self.in_flight.len()) + self.config.job_limit() * self.config.retire_frames as usize;
        // [pool][0 = draw requests, 1 = residency requests]
        let mut demand = [[0usize; 2]; 2];
        let mut cursor = None;
        for _ in 0..room {
            let Some(k) = self.next_request(cursor) else { break };
            if k.0 == RequestClass::Prefetch as u8 {
                break;
            }
            demand[self.pools.kind_of(k.3) as usize][usize::from(k.0 == RequestClass::Residency as u8)] += 1;
            cursor = Some(k);
        }
        for kind in [PoolKind::Main, PoolKind::Vertex] {
            let [draw, res] = demand[kind as usize];
            if draw + res == 0 {
                continue;
            }
            // Slots that are free or will be reusable within the retire delay.
            let coming = |s: &Self| s.pools.pool(kind).free() as usize + s.pools.pool(kind).retiring() as usize;
            while draw + res > coming(self) {
                let Some(&(_, t)) = self.victims[kind as usize].iter().next() else { break };
                self.evict(t);
            }
            if draw > coming(self) {
                // Tier 1: R only (farthest first, then finest). Found by a pass over the resident tiles, which only a pool
                // that is full of tiles in use gets to.
                let candidates: Vec<TileId> = self
                    .resident
                    .keys()
                    .copied()
                    .filter(|&t| t.level() != 0 && self.pools.kind_of(t) == kind && self.class_of(t) == Some(RequestClass::Residency))
                    .collect();
                self.work.scanned += self.resident.len() as u64;
                let mut tier1: Vec<(Reverse<u64>, Reverse<u8>, TileId)> =
                    candidates.into_iter().map(|t| (Reverse(self.layers[1].key(t, &mut self.work)), Reverse(t.level()), t)).collect();
                tier1.sort();
                let mut victims = tier1.into_iter().map(|(_, _, t)| t);
                while draw > coming(self) {
                    let Some(t) = victims.next() else { break };
                    self.evict(t);
                }
            }
        }
    }

    fn evict(&mut self, tile: TileId) {
        if let Some(r) = self.resident.remove(&tile) {
            self.pools.release(r.slot, self.frame);
            self.stats.evicted += 1;
            self.work.updated += 1;
            if tile.level() != 0 {
                self.victims[self.pools.kind_of(tile) as usize].remove(&(r.last_used, tile));
            }
            self.note(ResidentChange::Evicted(tile));
            if self.class_of(tile).is_some() {
                // Still wanted: requested again from the next frame on.
                self.missing.insert(tile);
            }
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
