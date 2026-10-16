//! The scheduler decides as it did before it was made incremental (STRM-005, STRM-007, STRM-018, CON-14), tier A, fakes only.
//!
//! The request order, the cancellations, the evictions, the resident sets and the counts of every frame of scripted runs are
//! folded into one digest per script. The digests were taken from the implementation that rebuilt every derived set each frame
//! (the scheduler of a8d7fb0, before the change `scheduler-incremental`) and stay pinned: the incremental scheduler must
//! reproduce each of them bit for bit, on the streamer alone (wants handed over every frame, failures and retries, a pool too
//! small for R, a vertex pool, prefetch) and through the managed loop (plans and R from the worker, upload budget, prefetch,
//! eviction under a pool smaller than R). This file uses only the interfaces both implementations have.

#[path = "support/sched.rs"]
mod sched;

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId};
use planet_frame::prefetch::prefetch_set;
use planet_frame::upload::UploadBudget;
use planet_frame::{residency_set_field, select_nodes_in_view_field, LodParams, SelectionCache};
use planet_generators::height::{HeightField, Reference};
use planet_streaming::{FakeClock, ManualSpawner, Spawner, StreamConfig, Streamer, TileSource, Wants};
use sched::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ---- the streamer alone ----

/// Tiles the streamer asks for; some fail twice first (deterministically, by tile), to exercise retries.
struct FlakySource {
    attempts: Mutex<HashMap<TileId, u32>>,
    flaky_modulus: u64,
}

impl TileSource<u64> for FlakySource {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        if self.flaky_modulus > 0 && tile.level() > 0 && tile.raw().is_multiple_of(self.flaky_modulus) {
            let mut a = self.attempts.lock().unwrap();
            let n = a.entry(tile).or_insert(0);
            *n += 1;
            if *n <= 2 {
                return Err(format!("flaky {tile:?} attempt {n}"));
            }
        }
        Ok(tile.raw())
    }
}

struct StreamRig {
    s: Streamer<u64>,
    sp: Arc<ManualSpawner>,
    clock: Arc<FakeClock>,
}

fn stream_rig(capacity: usize, vertex_pool: usize, max_in_flight: usize, flaky_modulus: u64) -> StreamRig {
    let sp = Arc::new(ManualSpawner::default());
    let clock = Arc::new(FakeClock::default());
    let config = StreamConfig {
        max_in_flight,
        queue_ahead: 0,
        capacity,
        vertex_pool_slots: vertex_pool,
        retire_frames: 2,
        retry_after_ms: 100,
        record_dispatches: true,
        record_changes: true,
    };
    let src = FlakySource { attempts: Mutex::default(), flaky_modulus };
    let mut s = Streamer::new(Arc::new(src), sp.clone() as Arc<dyn Spawner>, clock.clone(), config);
    s.load_roots_blocking().unwrap();
    StreamRig { s, sp, clock }
}

struct Views {
    field: Reference,
    p: LodParams,
    cache: SelectionCache,
}

impl Views {
    fn new() -> Views {
        let field = Reference(1);
        let p = LodParams {
            cells: 16,
            viewport_h_px: 360.0,
            tau_px: 4.0,
            max_level: 16,
            max_height_m: field.max_abs_height_m(),
            ..LodParams::earth_1080p()
        };
        Views { field, p, cache: SelectionCache::new() }
    }

    fn wants(&mut self, pos: PlanetFixed, yaw: f64, pitch: f64, ahead: Option<PlanetFixed>) -> Wants {
        let d = select_nodes_in_view_field(&TangentWarp, &camera(pos, yaw, pitch), &self.field, &self.p);
        let r = residency_set_field(&TangentWarp, pos, &self.field, &self.p, &mut self.cache);
        let prefetch = ahead
            .map(|a| prefetch_set(&r, &residency_set_field(&TangentWarp, a, &self.field, &self.p, &mut self.cache)))
            .unwrap_or_default();
        Wants { draw: d.nodes.iter().map(|n| (n.tile, n.distance_m)).collect(), residency: r.wants(), prefetch }
    }
}

/// One frame of the streamer rig, folded into the digest: what it answered, what it counted, what it dispatched and changed.
fn stream_frame(rig: &mut StreamRig, w: &Wants, jobs: usize, d: &mut Digest) {
    rig.clock.advance(FRAME_MS);
    let log_before = rig.s.dispatch_log.len();
    let resolved = rig.s.begin_frame_wants(w);
    for r in &resolved {
        d.tile(r.wanted);
        d.word(r.shown.map_or(u64::MAX, TileId::raw));
    }
    let st = rig.s.stats();
    for v in [
        st.resident as u64,
        st.queued as u64,
        st.in_flight as u64,
        st.completed,
        st.failed,
        st.cancelled,
        st.cancelled_prefetch,
        st.dropped,
        st.evicted,
        st.dispatched,
        st.blocked,
        st.main_in_use as u64,
        st.main_retiring as u64,
        st.main_free as u64,
        st.queued_by_class[0] as u64,
        st.queued_by_class[1] as u64,
        st.queued_by_class[2] as u64,
    ] {
        d.word(v);
    }
    for t in &rig.s.dispatch_log[log_before..] {
        d.tile(*t);
    }
    d.word(u64::MAX - 1);
    for c in rig.s.take_changes() {
        match c {
            planet_streaming::ResidentChange::Added(t) => {
                d.word(1);
                d.tile(t);
            }
            planet_streaming::ResidentChange::Evicted(t) => {
                d.word(2);
                d.tile(t);
            }
        }
    }
    rig.sp.run(jobs);
}

/// The wants of the streamer script, one per frame: settle, rotate, travel (with prefetch), turn, teleport, rotate again. Built
/// once and replayed against every pool and source variant.
fn streamer_wants() -> Vec<Wants> {
    let mut views = Views::new();
    let mut out = vec![views.wants(here(), 0.0, -0.4, None); 12];
    for k in 0..12 {
        out.push(views.wants(here(), f64::from(k) * 0.5, [-1.0, -0.4, 0.2][k as usize % 3], None));
    }
    for k in 0..24 {
        let pos = track(k as f64 * 100.0, 800.0);
        let ahead = track(k as f64 * 100.0 + 400.0, 800.0);
        out.push(views.wants(pos, 0.3, -0.4, (k % 7 != 6).then_some(ahead)));
    }
    for k in 0..6 {
        out.push(views.wants(there(), 1.0 + f64::from(k) * 0.2, -0.3, None));
    }
    for k in 0..8 {
        out.push(views.wants(there(), f64::from(k) * 0.8, -0.2, None));
    }
    out
}

/// The script on the streamer alone; a few tiles fail twice.
fn streamer_script(wants: &[Wants], capacity: usize, vertex_pool: usize, max_in_flight: usize, flaky_modulus: u64, jobs: usize) -> u64 {
    let mut rig = stream_rig(capacity, vertex_pool, max_in_flight, flaky_modulus);
    let mut d = Digest::new();
    for w in wants {
        stream_frame(&mut rig, w, jobs, &mut d);
    }
    let st = rig.s.stats();
    eprintln!(
        "  streamer script cap {capacity}: resident {} completed {} failed {} cancelled {} (prefetch {}) dropped {} evicted {} dispatched {} blocked {}",
        st.resident, st.completed, st.failed, st.cancelled, st.cancelled_prefetch, st.dropped, st.evicted, st.dispatched, st.blocked
    );
    d.0
}

// ---- the managed loop ----

fn dive_position(k: usize) -> PlanetFixed {
    const DESCENT: usize = 45;
    if k < DESCENT {
        track(0.0, 20_000.0 * libm::pow(200.0 / 20_000.0, k as f64 / DESCENT as f64))
    } else {
        track((k - DESCENT) as f64 * 20.0, 200.0)
    }
}

/// Settle, dive (descent then ground segment), teleport, hold, look around.
fn loop_script(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget, jobs: usize) -> u64 {
    let mut rig = loop_rig(capacity, prefetch, tile_bytes, budget);
    let mut d = Digest::new();
    let req = request(dive_position(0), 0.3, -0.4);
    for _ in 0..20 {
        rig.step(&req, jobs, &mut d);
    }
    for k in 0..60 {
        rig.step(&request(dive_position(k), 0.3, -0.4), jobs, &mut d);
    }
    for _ in 0..24 {
        rig.step(&request(there(), 1.0, -0.3), jobs, &mut d);
    }
    for k in 0..18 {
        rig.step(&request(there(), f64::from(k) * 0.3, [-1.0, -0.4, 0.2][k as usize % 3]), jobs, &mut d);
    }
    rig.finish(&mut d);
    let st = rig.lp.manager().streamer().stats();
    eprintln!(
        "  loop script cap {capacity}: resident {} completed {} cancelled {} (prefetch {}) dropped {} evicted {} dispatched {} blocked {} uploads {} forgets {} pending {}",
        st.resident, st.completed, st.cancelled, st.cancelled_prefetch, st.dropped, st.evicted, st.dispatched, st.blocked, rig.sink.order.len(), rig.sink.forgets.len(), rig.lp.pending().len()
    );
    d.0
}

// spec: STRM-005, STRM-007, CON-14
#[test]
fn the_streamer_alone_requests_cancels_and_evicts_exactly_as_before() {
    // Pools smaller than R (eviction by class; degradation is the manager's), failures with retries, an in-flight limit that
    // leaves a queue, a vertex pool for levels 0-2, prefetch sets while travelling.
    let w = streamer_wants();
    let digests =
        [streamer_script(&w, 700, 0, 16, 0, 1000), streamer_script(&w, 300, 0, 8, 11, 1000), streamer_script(&w, 250, 64, 8, 0, 100)];
    eprintln!("streamer digests: {digests:?}");
    assert_eq!(digests, STREAMER_DIGESTS);
}

// spec: STRM-005, STRM-006, STRM-007, STRM-018, CON-14
#[test]
fn the_managed_loop_requests_cancels_evicts_and_uploads_exactly_as_before() {
    let budgeted = UploadBudget { tile_bytes_per_frame: 40_000, node_records_in_cap: false };
    let digests = [
        loop_script(1_000_000, true, 8_840, budgeted, 12),
        loop_script(500, true, 0, UploadBudget::default(), 24),
        loop_script(500, false, 8_840, budgeted, 7),
    ];
    eprintln!("loop digests: {digests:?}");
    assert_eq!(digests, LOOP_DIGESTS);
}

const STREAMER_DIGESTS: [u64; 3] = [15300534013828900970, 689671829587726302, 5616887668897300662];
const LOOP_DIGESTS: [u64; 3] = [210182232526776486, 1769320474311536167, 11894629634072874866];
