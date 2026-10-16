//! Dispatch follows the generation pool (STRM-020, STRM-001, STRM-005, TEST-018), tier A: a spawner with a generation-rate model
//! (`TimedSpawner`: N workers, a duration per job, time moving by frames of 1/60 s) under the real scheduler. The scheduler sees
//! completions at a frame boundary only; before STRM-020 a worker that finished mid-frame idled until the next frame, so 8 workers
//! on jobs of 18 ms did 8 jobs per 2 frames (240 tiles/s, not 444) and the queue of a dive grew.

use planet_core::{Face, TileId};
use planet_streaming::{FakeClock, GenWorkers, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource, TimedSpawner};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const FRAME_US: u64 = 16_667;
/// A tile job on one worker (the measured lock-free rate of 1,217 tiles/s on 20 threads is 16 ms a job).
const JOB_US: u64 = 18_000;

#[derive(Default)]
struct Source(AtomicU64);

impl TileSource<u64> for Source {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(tile.raw())
    }
}

/// Every tile of face 0 down to `level`, parents first.
fn face(level: u8) -> Vec<(TileId, f64)> {
    let mut out = Vec::new();
    for l in 0..=level {
        let n = 1u32 << l;
        for x in 0..n {
            for y in 0..n {
                out.push((TileId::new(Face(0), l, x, y).unwrap(), f64::from(l) * 1000.0 + f64::from(x + y) * 0.001));
            }
        }
    }
    out
}

struct Frame {
    in_flight: usize,
    queued: usize,
    busy: usize,
    dispatched: u64,
    completed: u64,
}

fn config(gen: GenWorkers) -> StreamConfig {
    gen.apply(StreamConfig { capacity: 100_000, retry_after_ms: 100, ..Default::default() })
}

/// `frames` frames of a held draw set bigger than the pool can ever finish, on `workers` model workers.
fn run(gen: GenWorkers, frames: usize) -> Vec<Frame> {
    let spawner = Arc::new(TimedSpawner::new(gen.workers, JOB_US));
    let mut s =
        Streamer::new(Arc::new(Source::default()), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config(gen));
    s.load_roots_blocking().unwrap();
    let wants = face(6);
    s.set_draw(&wants);
    let mut out = Vec::new();
    let mut before = s.stats();
    for _ in 0..frames {
        spawner.advance(FRAME_US);
        s.step();
        let st = s.stats();
        out.push(Frame {
            in_flight: st.in_flight,
            queued: st.queued,
            busy: spawner.busy(),
            dispatched: st.dispatched - before.dispatched,
            completed: st.completed - before.completed,
        });
        before = st;
    }
    out
}

/// Completions a second over frames `from..`.
fn rate(frames: &[Frame], from: usize) -> f64 {
    let done: u64 = frames[from..].iter().map(|f| f.completed).sum();
    done as f64 / ((frames.len() - from) as f64 * FRAME_US as f64 / 1e6)
}

// spec: STRM-020, STRM-001, TEST-018
#[test]
fn in_flight_reaches_the_pool_size_and_stays_there_while_the_queue_is_not_empty() {
    for n in [4, 8, 20] {
        let gen = GenWorkers::new(n);
        let frames = run(gen, 120);
        assert_eq!(frames[0].in_flight, gen.workers + gen.ahead, "N={n}: the first frame fills the pool and its queue");
        for (k, f) in frames.iter().enumerate() {
            assert!(f.queued > 0, "N={n} frame {k}: the demand must outlast the run");
            assert_eq!(f.in_flight, gen.workers + gen.ahead, "N={n} frame {k}: in flight below the limit with requests waiting");
            assert_eq!(f.busy, n, "N={n} frame {k}: a worker idles with requests waiting");
        }
    }
}

// spec: STRM-020, STRM-005
#[test]
fn each_frame_dispatches_exactly_the_jobs_that_completed() {
    for n in [4, 8, 20] {
        let frames = run(GenWorkers::new(n), 120);
        for (k, f) in frames.iter().enumerate().skip(1) {
            assert_eq!(f.dispatched, f.completed, "N={n} frame {k}: the free places are not all refilled");
        }
    }
}

// spec: STRM-020, TEST-018
#[test]
fn throughput_follows_the_generation_model_and_the_old_frame_quantised_refill_does_not() {
    for n in [4, 8, 20] {
        let model = n as f64 / (JOB_US as f64 / 1e6);
        let now = rate(&run(GenWorkers::new(n), 180), 30);
        assert!(now >= 0.8 * model, "N={n}: {now:.0} tiles/s of the model's {model:.0}");
        // The control, the scheduler as it was (jobs handed over only up to the worker count, refilled at frame boundaries): a
        // job of 18 ms occupies its worker for two frames, so the rate is N / 2 frames, 54 % of the model.
        let old = rate(&run(GenWorkers::new(n).with_ahead(0), 180), 30);
        eprintln!("N={n}: model {model:.0} tiles/s, refill at frame boundaries only {old:.0}, with the queue ahead {now:.0}");
        assert!(old < 0.7 * model, "N={n}: the control reached {old:.0} of {model:.0} tiles/s: the model no longer shows the limit");
        assert!(now > 1.3 * old, "N={n}: {now:.0} against {old:.0}");
    }
}

// spec: STRM-001, STRM-005, CON-14
#[test]
fn the_request_order_depends_on_the_job_limit_not_on_how_it_is_split_between_running_and_queued() {
    let order = |max_in_flight: usize, queue_ahead: usize| {
        let spawner = Arc::new(ManualSpawner::default());
        let config = StreamConfig { max_in_flight, queue_ahead, capacity: 100_000, record_dispatches: true, ..Default::default() };
        let mut s = Streamer::new(Arc::new(Source::default()), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
        s.load_roots_blocking().unwrap();
        let wants = face(5);
        for k in 0..60 {
            // The wants move: the first half of the tiles, then all of them, then the second half.
            let part: Vec<_> = match k / 20 {
                0 => wants[..wants.len() / 2].to_vec(),
                1 => wants.clone(),
                _ => wants[wants.len() / 2..].to_vec(),
            };
            s.begin_frame(&part);
            spawner.run(5);
        }
        s.dispatch_log.clone()
    };
    let (a, b, c) = (order(8, 0), order(4, 4), order(2, 6));
    assert!(a.len() > 100);
    assert_eq!(a, b);
    assert_eq!(a, c);
}

// spec: STRM-020
#[test]
fn the_config_is_derived_from_the_pool_size() {
    let g = GenWorkers::new(12);
    let c = g.apply(StreamConfig::default());
    assert_eq!((c.max_in_flight, c.queue_ahead, c.job_limit()), (12, 12, 24));
    assert_eq!(g.with_ahead(0).apply(StreamConfig::default()).job_limit(), 12);
    assert_eq!(GenWorkers::new(0).workers, 1);
}

// spec: STRM-020, STRM-005
#[test]
fn dropping_the_streamer_with_the_pool_full_runs_no_job() {
    for n in [4, 8, 20] {
        let gen = GenWorkers::new(n);
        let source = Arc::new(Source::default());
        let spawner = Arc::new(TimedSpawner::new(n, JOB_US));
        let mut s = Streamer::new(source.clone(), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config(gen));
        s.load_roots_blocking().unwrap();
        let roots = source.0.load(Ordering::SeqCst);
        s.set_draw(&face(5));
        spawner.advance(FRAME_US);
        s.step();
        assert_eq!((spawner.busy(), spawner.waiting()), (n, gen.ahead), "N={n}");
        drop(s);
        spawner.advance(10_000_000);
        assert_eq!(source.0.load(Ordering::SeqCst), roots, "N={n}: a cancelled job loaded a tile");
        assert_eq!((spawner.busy(), spawner.waiting()), (0, 0), "N={n}");
    }
}
