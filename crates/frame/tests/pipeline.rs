//! Frame pipelining (STRM-016, STRM-002, TEST-012): plans from a worker that equal the synchronous plans, frames in flight
//! counted against the bound, the stale-plan rule, the cover plan after a jump, and the time columns. Fake spawner, no GPU,
//! no clock: counts only (CON-20).

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::pipeline::*;
use planet_frame::{
    child_index_toward, cover_selection, cover_selection_masked, plan_terrain_field, quadrant_bit, scripted_views, Camera, LodParams,
    TerrainRequest, TerrainView, ALL_QUADRANTS,
};
use planet_generators::height::{HeightField, Reference};
use planet_streaming::{ManualSpawner, Spawner, ThreadPool};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);

fn lod() -> LodParams {
    LodParams {
        cells: 16,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 2.5,
        max_level: 26,
        min_level: 0,
        radius_m: R,
        max_height_m: LodParams::earth_1080p().max_height_m.max(Reference(7).max_abs_height_m()),
        ..LodParams::earth_1080p()
    }
}

/// Camera `height` above the sphere over a point of the scripted views, looking along `heading` pitched down; `lon` moves
/// the point (degrees of arc along the surface, to jump somewhere else).
fn camera_at(height: f64, lon_deg: f64) -> Camera {
    let (_, template) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), 7)[4];
    let dir = template.position.0.normalized();
    let axis = Vec3::new(0.0, 0.0, 1.0);
    let (s, c) = (libm::sin(lon_deg.to_radians()), libm::cos(lon_deg.to_radians()));
    let rotated = dir * c + axis.cross(dir) * s + axis * (axis.dot(dir) * (1.0 - c));
    Camera::at_surface_point(
        PlanetFixed(rotated * (R + height)),
        Vec3::new(0.0, 0.0, 1.0),
        20f64.to_radians(),
        template.fov_y_rad,
        template.aspect,
        0.1,
    )
}

fn request(camera: Camera) -> TerrainRequest {
    TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 7 }
}

/// Every tile resident, as it would be after the fill: records from the field itself (memoised).
struct Everything {
    memo: std::cell::RefCell<HashMap<TileId, ResidentTile>>,
}
impl Everything {
    fn new() -> Self {
        Self { memo: Default::default() }
    }
}
impl ResidentSet for Everything {
    fn resident(&self, tile: TileId) -> Option<ResidentTile> {
        Some(*self.memo.borrow_mut().entry(tile).or_insert_with(|| record(tile)))
    }
}

thread_local! {
    static RECORDS: ResidentRecords<'static> = ResidentRecords::new(&Reference(7), &TangentWarp, &lod());
}

fn record(tile: TileId) -> ResidentTile {
    RECORDS.with(|r| r.record(tile))
}

fn with_ancestors(tiles: impl IntoIterator<Item = TileId>) -> HashMap<TileId, ResidentTile> {
    let mut out = HashMap::new();
    for t in tiles {
        let mut up = Some(t);
        while let Some(a) = up {
            out.entry(a).or_insert_with(|| record(a));
            up = a.parent();
        }
    }
    out
}

fn roots() -> HashMap<TileId, ResidentTile> {
    with_ancestors(planet_core::Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).unwrap()))
}

fn worker(spawner: Arc<dyn Spawner>, threads: usize) -> PlanWorker {
    PlanWorker::new(spawner, Arc::new(Reference(7)), threads)
}

// spec: STRM-016
#[test]
fn plans_from_the_worker_equal_the_synchronous_plans_in_the_same_order() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(worker(spawner.clone(), 2));
    let everything = Everything::new();
    let cameras: Vec<TerrainRequest> = (0..14).map(|k| request(camera_at(3000.0 * libm::pow(0.9, f64::from(k)), 0.0))).collect();
    let mut delivered = Vec::new();
    for (k, req) in cameras.iter().enumerate() {
        let f = pipe.begin_frame(req, &everything).unwrap();

        assert!(f.draw.blank == 0 && !f.draw.plan.nodes.is_empty(), "frame {k} draws something");
        if let Some((seq, _)) = f.delivered {
            delivered.push(seq);
            let c = pipe.current().unwrap();
            let sync = plan_terrain_field(&cameras[seq as usize - 1], &Reference(7)).unwrap();
            assert_eq!(c.plan.plan_hash(), sync.plan_hash(), "the plan delivered for frame {seq} is the synchronous plan of its camera");
            assert_eq!(*c.plan, sync);
        }
        // The worker finishes one plan between two frames (a lag of one frame).
        spawner.run(1);
    }
    assert!(delivered.windows(2).all(|w| w[0] < w[1]), "plans arrive in request order: {delivered:?}");
    assert!(delivered.len() >= 6, "plans keep arriving while the camera moves: {delivered:?}");
    // Still camera: the newest completed plan is the synchronous frame's plan.
    let still = cameras.last().unwrap();
    for _ in 0..4 {
        pipe.begin_frame(still, &everything).unwrap();
        spawner.run(1);
    }
    let f = pipe.begin_frame(still, &everything).unwrap();
    assert_eq!(f.draw.kind, DrawKind::Exact);
    assert_eq!(f.draw.plan, plan_terrain_field(still, &Reference(7)).unwrap());
}

// spec: STRM-016, CON-14
#[test]
fn the_plan_made_for_a_camera_is_the_same_on_1_2_and_8_threads() {
    let cameras: Vec<TerrainRequest> = [(5.0e6, 0.0), (4000.0, 0.0), (1500.0, 0.3), (200.0, 0.3), (1.0e7, 40.0)]
        .iter()
        .map(|&(h, lon)| request(camera_at(h, lon)))
        .collect();
    let hashes = |threads: usize| -> Vec<u64> {
        // The spawner has `threads` workers too; one plan at a time, waited for by polling the worker.
        let w = worker(Arc::new(ThreadPool::new(threads)), threads);
        cameras
            .iter()
            .enumerate()
            .map(|(i, req)| {
                w.submit(i as u64 + 1, req.clone());
                loop {
                    if let Some(done) = w.take() {
                        let o = done.unwrap();
                        assert_eq!(o.seq, i as u64 + 1);
                        break o.plan.plan_hash();
                    }
                    std::thread::yield_now();
                }
            })
            .collect()
    };
    let one = hashes(1);
    assert_eq!(one, hashes(2));
    assert_eq!(one, hashes(8));
    let sync: Vec<u64> = cameras.iter().map(|r| plan_terrain_field(r, &Reference(7)).unwrap().plan_hash()).collect();
    assert_eq!(one, sync, "and equal to the synchronous plans");
}

// spec: STRM-016
#[test]
fn a_newer_request_replaces_one_that_waits_and_every_plan_is_the_plan_of_its_camera() {
    let spawner = Arc::new(ManualSpawner::default());
    let w = worker(spawner.clone(), 1);
    let reqs: Vec<TerrainRequest> = (0..4).map(|k| request(camera_at(2000.0 - 100.0 * f64::from(k), 0.0))).collect();
    for (i, r) in reqs.iter().enumerate() {
        w.submit(i as u64 + 1, r.clone());
    }
    assert_eq!(spawner.pending(), 1, "one plan at a time");
    assert_eq!(w.counts().superseded, 2, "requests 2 and 3 were replaced before they started");
    spawner.run(1);
    let first = w.take().unwrap().unwrap();
    assert_eq!(first.seq, 1);
    spawner.run(1);
    let last = w.take().unwrap().unwrap();
    assert_eq!(last.seq, 4, "the waiting slot held the newest request");
    assert_eq!(last.plan.plan_hash(), plan_terrain_field(&reqs[3], &Reference(7)).unwrap().plan_hash());
    assert!(!w.busy());
    assert_eq!(w.counts().completed, 2);
}

// spec: STRM-016
#[test]
fn frames_in_flight_are_counted_and_never_exceed_the_bound() {
    let mut fi = FramesInFlight::new(MAX_FRAMES_IN_FLIGHT);
    assert_eq!(fi.slot_retire_frames(), u64::from(MAX_FRAMES_IN_FLIGHT));
    // A GPU that finishes a frame every third submission: the loop must wait when the bound is reached, never exceed it.
    let mut gpu_done = 0u64;
    for k in 0..90u64 {
        fi.retire_to(gpu_done);
        while fi.is_full() {
            gpu_done += 1;
            fi.note_wait();
            fi.retire_to(gpu_done);
        }
        fi.submit();
        assert!(fi.in_flight() <= MAX_FRAMES_IN_FLIGHT, "frame {k}");
        if k % 3 == 2 {
            gpu_done += 1;
        }
    }
    assert_eq!(fi.peak(), MAX_FRAMES_IN_FLIGHT);
    assert!(fi.waits() > 0);
    // A GPU that keeps up never makes the loop wait and never has more than one frame in flight.
    let mut fast = FramesInFlight::new(MAX_FRAMES_IN_FLIGHT);
    for k in 1..=50u64 {
        fast.submit();
        fast.retire_to(k);
    }
    assert_eq!((fast.peak(), fast.waits()), (1, 0));
    // The renderer's pools retire a released slot after exactly the bound.
    let pool = planet_frame::pool::tier_compact_pool(planet_streaming::Tier::Mid, 16, fi.slot_retire_frames()).unwrap();
    assert_eq!(pool.frames_in_flight, u64::from(MAX_FRAMES_IN_FLIGHT));
}

#[should_panic(expected = "frames in flight")]
// spec: STRM-016
#[test]
fn submitting_past_the_bound_is_a_loop_bug() {
    let mut fi = FramesInFlight::new(2);
    fi.submit();
    fi.submit();
    fi.submit();
}

/// The nearest resident ancestor-or-self by walking up: the rule the cover and the stand-ins follow.
fn deepest_resident(set: &HashMap<TileId, ResidentTile>, t: TileId) -> Option<TileId> {
    let mut up = Some(t);
    while let Some(a) = up {
        if set.contains_key(&a) {
            return Some(a);
        }
        up = a.parent();
    }
    None
}

// spec: STRM-016, STRM-002
#[test]
fn a_plan_node_that_is_not_resident_is_drawn_by_its_deepest_resident_ancestor() {
    let (near, far) = (request(camera_at(300.0, 0.0)), request(camera_at(300.0, 0.002)));
    let plan_near = plan_terrain_field(&near, &Reference(7)).unwrap();
    // Resident: the near view's tiles with their ancestors, but only every other leaf of the view itself.
    let mut set = with_ancestors(plan_near.nodes.iter().step_by(2).map(|n| n.tile));
    for (i, n) in plan_near.nodes.iter().enumerate() {
        if i % 2 == 1 {
            // The sibling parents are resident already through the ancestors of the other leaves or not at all: drop the leaf.
            set.remove(&n.tile);
        }
    }
    let plan_far = plan_terrain_field(&far, &Reference(7)).unwrap();
    let d = draw_plan(&far, Some((&far, &plan_far)), &set).unwrap();
    assert_eq!(d.blank, 0, "roots are resident, so nothing is blank");
    let drawn: BTreeMap<TileId, u8> = d.plan.nodes.iter().map(|n| (n.tile, n.quadrants)).collect();
    // Oracle (STRM-016): a node that is resident with its key is drawn whole; one that is not is drawn by its deepest resident ancestor
    // over the quadrant that holds it; a tile under a quadrant an ancestor draws is not drawn.
    let mut expect: BTreeMap<TileId, u8> = BTreeMap::new();
    for n in &plan_far.nodes {
        let resident_exact = set.get(&n.tile).is_some_and(|r| r.mesh_key == n.mesh_key);
        if resident_exact {
            *expect.entry(n.tile).or_insert(0) |= ALL_QUADRANTS;
        } else {
            let a = deepest_resident(&set, n.tile).unwrap();
            *expect.entry(a).or_insert(0) |= quadrant_bit(child_index_toward(a, n.tile));
        }
    }
    let covered = |t: TileId, m: &BTreeMap<TileId, u8>| {
        std::iter::successors(t.parent(), |a| a.parent())
            .any(|a| m.get(&a).is_some_and(|mask| mask & quadrant_bit(child_index_toward(a, t)) != 0))
    };
    let expect: BTreeMap<TileId, u8> = expect.iter().filter(|(t, _)| !covered(**t, &expect)).map(|(t, m)| (*t, *m)).collect();
    assert_eq!(drawn, expect);
    assert!(d.substituted > 0 && d.kind == DrawKind::Stale);
    assert!(d.plan.nodes.windows(2).all(|w| w[0].tile < w[1].tile), "ascending tile-id order like every plan");
    // The camera of the frame is in the plan: origins and matrix follow `far`, not the plan's camera.
    assert_eq!(d.plan.camera, far.camera);
    for n in &d.plan.nodes {
        let rel = n.origin.0 - far.camera.position.0;
        assert_eq!(n.camera_relative_origin, [rel.x as f32, rel.y as f32, rel.z as f32]);
    }
}

// spec: STRM-016, STRM-002
#[test]
fn the_cover_is_the_deepest_resident_tiles_of_the_view_and_never_blank() {
    let req = request(camera_at(800.0, 0.0));
    let full = plan_terrain_field(&req, &Reference(7)).unwrap();
    let map = TangentWarp;
    // Only the roots resident: the cover is the visible roots.
    let only_roots = roots();
    let d = draw_plan(&req, None, &only_roots).unwrap();
    assert_eq!(d.kind, DrawKind::Cover);
    assert!(!d.plan.nodes.is_empty() && d.plan.nodes.iter().all(|n| n.level == 0));
    // Half of the full plan resident (with ancestors): the cover reaches down to resident tiles, no further, never blank.
    let set = with_ancestors(full.nodes.iter().step_by(3).map(|n| n.tile));
    let cover = cover_selection(&map, &req.camera, &req.lod, &|t| set.get(&t).map(|r| r.origin));
    assert!(!cover.is_empty());
    assert!(cover.iter().all(|t| set.contains_key(t)), "only resident tiles");
    assert!(cover.windows(2).all(|w| w[0] < w[1]));
    let cover_set: BTreeSet<TileId> = cover.iter().copied().collect();
    // No part is drawn twice: a tile lies under another only beside the quadrants the other draws (STRM-016).
    let masks: BTreeMap<TileId, u8> =
        cover_selection_masked(&map, &req.camera, &req.lod, &|t| set.get(&t).map(|r| r.origin), &Default::default()).into_iter().collect();
    assert_eq!(masks.keys().copied().collect::<Vec<_>>(), cover);
    for t in &cover {
        let covered = std::iter::successors(t.parent(), |a| a.parent())
            .any(|a| masks.get(&a).is_some_and(|m| m & quadrant_bit(child_index_toward(a, *t)) != 0));
        assert!(!covered, "{t:?} lies under a quadrant another cover tile draws");
    }
    // Every node of the full plan is served: by a cover tile above it (or itself) or by cover tiles below it.
    for n in &full.nodes {
        let above = std::iter::successors(Some(n.tile), |a| a.parent()).any(|a| cover_set.contains(&a));
        let below = cover.iter().any(|c| std::iter::successors(Some(*c), |a| a.parent()).any(|a| a == n.tile));
        assert!(above || below, "{:?} (level {}) has nothing in the cover", n.tile, n.level);
    }
    // The cover is the deepest-resident-ancestor rule applied to the view: where the whole subtree is resident it goes down
    // to the LOD criterion's depth, elsewhere it stops at the deepest resident tile that has a missing child in view.
    let complete = with_ancestors(full.nodes.iter().map(|n| n.tile));
    let deep = cover_selection(&map, &req.camera, &req.lod, &|t| complete.get(&t).map(|r| r.origin));
    for n in &full.nodes {
        assert!(
            deep.iter().any(|c| std::iter::successors(Some(*c), |a| a.parent()).any(|a| a == n.tile)
                || std::iter::successors(Some(n.tile), |a| a.parent()).any(|a| a == *c)),
            "{:?}",
            n.tile
        );
    }
    // Pure: same inputs, same cover.
    assert_eq!(cover, cover_selection(&map, &req.camera, &req.lod, &|t| set.get(&t).map(|r| r.origin)));
}

/// A field that counts how often it is asked, to prove what evaluates it.
struct Counting {
    calls: Arc<AtomicU64>,
}
impl HeightField for Counting {
    fn height(&self, dir: Vec3) -> f64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Reference(7).height(dir)
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        Reference(7).implementation_id()
    }
    fn version(&self) -> u32 {
        Reference(7).version()
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m()
    }
    fn height_bounds(&self, c: Vec3, a: f64) -> (f64, f64) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Reference(7).height_bounds(c, a)
    }
}

// spec: STRM-016, STRM-002
#[test]
fn a_jump_draws_a_cover_at_frame_zero_and_the_full_plan_arrives_later() {
    let calls = Arc::new(AtomicU64::new(0));
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(PlanWorker::new(spawner.clone(), Arc::new(Counting { calls: calls.clone() }), 4));
    // Settled at A (2 km, everything of the view resident).
    let a = request(camera_at(2000.0, 0.0));
    let plan_a = plan_terrain_field(&a, &Reference(7)).unwrap();
    // The six roots are always resident (CON-22), the rest is what the view at A needed.
    let mut resident = roots();
    resident.extend(with_ancestors(plan_a.nodes.iter().map(|n| n.tile)));
    for _ in 0..3 {
        pipe.begin_frame(&a, &resident).unwrap();
        spawner.run(1);
    }
    assert_eq!(pipe.begin_frame(&a, &resident).unwrap().draw.kind, DrawKind::Exact);
    // Jump to the other side of the planet, 1 m up.
    let b = request(camera_at(1.0, 180.0));
    let before = calls.load(Ordering::SeqCst);
    let f0 = pipe.begin_frame(&b, &resident).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), before, "the frame evaluates no field: the plan is on the worker");
    assert_eq!(f0.draw.kind, DrawKind::Cover, "the plan of A is no use at B");
    assert!(f0.requested && f0.base_seq.is_none());
    assert!(!f0.draw.plan.nodes.is_empty(), "never blank");
    assert!(f0.draw.plan.nodes.iter().all(|n| resident.contains_key(&n.tile)));
    // The worker has not run: more frames keep drawing the cover and ask for nothing new.
    let f1 = pipe.begin_frame(&b, &resident).unwrap();
    assert!(f1.draw.kind == DrawKind::Cover && !f1.requested);
    assert_eq!(f1.draw.plan, f0.draw.plan);
    // The worker finishes: the plan of B arrives; its tiles are not resident yet, so they are stood in for.
    assert_eq!(spawner.run(1), 1);
    assert!(calls.load(Ordering::SeqCst) > before, "the worker evaluated the field");
    let f2 = pipe.begin_frame(&b, &resident).unwrap();
    assert_eq!(f2.delivered.map(|d| d.0), Some(f0.frame));
    assert_eq!(f2.draw.kind, DrawKind::Stale);
    assert!(f2.draw.substituted > 0 && f2.draw.blank == 0);
    assert!(!f2.wanted.is_empty(), "the streamer is asked for the new place now");
    // Streaming completes: the frame is the synchronous frame.
    let plan_b = plan_terrain_field(&b, &Reference(7)).unwrap();
    resident.extend(with_ancestors(plan_b.nodes.iter().map(|n| n.tile)));
    let f3 = pipe.begin_frame(&b, &resident).unwrap();
    assert_eq!(f3.draw.kind, DrawKind::Exact);
    assert_eq!(f3.draw.plan, plan_b);
}

// spec: STRM-016, VIEW-014, VIEW-032
#[test]
fn a_plan_refusal_for_other_lod_parameters_than_the_frames_is_dropped_and_counted_not_returned() {
    // SPIKE-19: the coarse view's plan is refused while the worker is busy; by the time the refusal arrives the frame asks for the base
    // parameters again (F4 pressed twice). That refusal is of no use to the frame: it must not end the window.
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(worker(spawner.clone(), 1));
    let everything = Everything::new();
    let good = request(camera_at(300.0, 0.0));
    let mut bad = good.clone();
    // Far too coarse for 16 cells: the morph interval of some level is empty and the plan is refused.
    bad.lod.tau_px = 200.0;
    assert!(bad.lod.validate().is_err() && good.lod.validate().is_ok());
    assert!(plan_terrain_field(&bad, &Reference(7)).is_err(), "the plan refuses the coarse parameters");

    // A plan of the good parameters is in hand, then the frame asks for the bad ones and the worker refuses them.
    pipe.begin_frame(&good, &everything).unwrap();
    spawner.run(2);
    pipe.begin_frame(&good, &everything).unwrap();
    pipe.begin_frame(&bad, &everything).unwrap();
    spawner.run(2);
    // The refusal arrives at a frame that asks for the good parameters again: dropped, counted, the frame draws.
    let f = pipe.begin_frame(&good, &everything).expect("a refusal for other parameters does not end the frame");
    assert!(!f.draw.plan.nodes.is_empty());
    assert_eq!(pipe.dropped_plan_errors(), 1);
    // Another one, the same way: counted again.
    pipe.begin_frame(&bad, &everything).unwrap();
    spawner.run(2);
    pipe.begin_frame(&good, &everything).expect("a second refusal for other parameters does not end the frame either");
    assert_eq!(pipe.dropped_plan_errors(), 2);

    // A refusal for the frame's own parameters degrades the frame: it keeps the plan in hand, is counted and reported once, never an error.
    pipe.begin_frame(&bad, &everything).unwrap();
    spawner.run(2);
    let f = pipe.begin_frame(&bad, &everything).expect("the frame's own refusal does not end the window");
    assert!(!f.draw.plan.nodes.is_empty(), "the frame still draws");
    let said = pipe.take_refusal().expect("the refusal is reported");
    assert!(said.starts_with("plan for frame") && said.contains("invalid LOD parameters"), "{said}");
    assert_eq!((pipe.refused_plans(), pipe.dropped_plan_errors()), (1, 2), "counted as refused, not dropped");
    assert!(pipe.take_refusal().is_none(), "reported once");
}

// spec: VIEW-032
#[test]
fn a_refusal_with_no_plan_in_hand_still_draws_and_the_same_refusal_is_reported_once() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(worker(spawner.clone(), 1));
    let everything = Everything::new();
    let mut bad = request(camera_at(300.0, 0.0));
    bad.lod.tau_px = 200.0;
    pipe.begin_frame(&bad, &everything).unwrap();
    spawner.run(2);
    for _ in 0..3 {
        pipe.begin_frame(&bad, &everything).expect("a refusal with nothing in hand does not end the window");
    }
    assert!(pipe.refused_plans() >= 1);
    assert!(pipe.take_refusal().is_some());
    assert!(pipe.take_refusal().is_none());
}

// spec: VIEW-040
#[test]
fn a_hundred_refused_frames_print_a_few_lines_one_per_cause_and_period_and_the_count_is_all_of_them() {
    use planet_frame::pipeline::{refusal_cause, RefusalReporter, REFUSAL_REPORT_PERIOD_MS};
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(worker(spawner.clone(), 1));
    let everything = Everything::new();
    let mut reporter = RefusalReporter::default();
    let mut lines = Vec::new();
    // A flight over ground too steep for the parameters: the camera moves every frame, so the worker refuses a different tile at a
    // different distance each time, 16 ms apart.
    for k in 0..100u64 {
        let mut bad = request(camera_at(300.0 + k as f64, 0.0));
        bad.lod.tau_px = 200.0;
        pipe.begin_frame(&bad, &everything).expect("a refused frame draws");
        spawner.run(2);
        pipe.begin_frame(&bad, &everything).expect("a refused frame draws");
        if let Some(why) = pipe.take_refusal() {
            lines.extend(reporter.observe(&why, k * 16));
        }
    }
    assert!(pipe.refused_plans() >= 90, "nearly every frame was refused: {}", pipe.refused_plans());
    assert!(lines.len() <= 3, "{} lines for {} refusals: {lines:?}", lines.len(), pipe.refused_plans());
    // The same cause is due again after the period, with the number that came in between.
    let why = "plan for frame 9: invalid LOD parameters: tile TileId(7) (level 15): morph interval [1.0, 0.5] m is empty";
    let mut r = RefusalReporter::default();
    assert!(r.observe(why, 0).is_some() && r.observe(why, 100).is_none() && r.observe(why, 200).is_none());
    let again = r.observe(&why.replace("TileId(7)", "TileId(8)"), REFUSAL_REPORT_PERIOD_MS + 1).expect("due again");
    assert!(again.contains("2 more refusals"), "{again}");
    // Another level is another cause.
    assert!(r.observe(&why.replace("level 15", "level 16"), 300).is_some());
    assert_eq!(refusal_cause(why), "invalid LOD parameters / level 15");
}

// spec: STRM-016
#[test]
fn the_stale_plan_rule_follows_translation_rotation_and_the_frame_setup() {
    let base = request(camera_at(100.0, 0.0));
    let plan = plan_terrain_field(&base, &Reference(7)).unwrap();
    let finest = plan.nodes.iter().map(|n| n.level).max().unwrap();
    let limit = STALE_MOVE_FRACTION * base.lod.split_distance(finest);
    let nudge = |metres: f64| {
        let mut r = base.clone();
        let up = r.camera.position.0.normalized();
        let side = up.cross(r.camera.forward).normalized();
        r.camera.position = PlanetFixed(r.camera.position.0 + side * metres);
        r
    };
    assert!(plan_usable(&plan, &base));
    assert!(plan_usable(&plan, &nudge(0.9 * limit)));
    assert!(!plan_usable(&plan, &nudge(1.1 * limit)));
    let turned = |deg: f64| {
        let mut r = base.clone();
        let up = r.camera.up.normalized();
        let (s, c) = (libm::sin(deg.to_radians()), libm::cos(deg.to_radians()));
        let f = r.camera.forward;
        r.camera.forward = f * c + up.cross(f) * s + up * (up.dot(f) * (1.0 - c));
        r
    };
    assert!(plan_usable(&plan, &turned(0.4)));
    assert!(!plan_usable(&plan, &turned(0.6)));
    let mut other_size = base.clone();
    other_size.size = (64, 48);
    assert!(!plan_usable(&plan, &other_size));
    let mut other_view = base.clone();
    other_view.view = TerrainView::TileId;
    assert!(!plan_usable(&plan, &other_view));
}

// spec: TEST-012
#[test]
fn the_time_columns_are_named_and_a_slow_frame_is_attributed_to_its_largest_part() {
    assert_eq!(FrameColumns::NAMES, ["plan_ms", "scheduler_ms", "upload_ms", "record_ms", "gpu_ms", "wall_ms"]);
    let c = FrameColumns { plan_ms: 400.0, scheduler_ms: 0.4, upload_ms: 2.5, record_ms: 6.0, gpu_ms: Some(3.0), wall_ms: 12.0 };
    assert_eq!(c.largest(), "record_ms", "the plan runs on a worker and does not compete");
    assert_eq!(c.values()[0], 400.0);
    let slow_upload = FrameColumns { upload_ms: 30.0, ..c };
    assert_eq!(slow_upload.largest(), "upload_ms");
    assert!(FrameColumns { gpu_ms: None, ..c }.values()[4].is_nan());
}

/// A field that remembers which threads evaluated a height.
struct Threads {
    seen: std::sync::Mutex<BTreeSet<String>>,
    heights: AtomicU64,
}
impl HeightField for Threads {
    fn height(&self, dir: Vec3) -> f64 {
        self.heights.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().insert(format!("{:?}", std::thread::current().id()));
        Reference(7).height(dir)
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        Reference(7).implementation_id()
    }
    fn version(&self) -> u32 {
        Reference(7).version()
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m()
    }
    fn height_bounds(&self, c: Vec3, a: f64) -> (f64, f64) {
        Reference(7).height_bounds(c, a)
    }
}

// spec: STRM-016, LOD-009
#[test]
fn origin_heights_of_new_nodes_are_evaluated_on_worker_threads_once_per_tile() {
    let req = request(camera_at(400.0, 0.0));
    let field = Threads { seen: Default::default(), heights: AtomicU64::new(0) };
    let mut cache = planet_frame::SelectionCache::new();
    let plan = planet_frame::plan_terrain_threaded(&req, &field, &mut cache, 4).unwrap();
    let seen = field.seen.lock().unwrap().len();
    assert!(seen >= 2, "a cold plan spreads its origin heights over threads, saw {seen}");
    assert_eq!(field.heights.load(Ordering::SeqCst), plan.nodes.len() as u64, "one evaluation per new node");
    let again = planet_frame::plan_terrain_threaded(&req, &field, &mut cache, 4).unwrap();
    assert_eq!(field.heights.load(Ordering::SeqCst), plan.nodes.len() as u64, "a warm plan evaluates none");
    assert_eq!(plan, again);
    assert_eq!(plan, plan_terrain_field(&req, &Reference(7)).unwrap(), "and equals the single-threaded plan");
}

/// Integer PRNG written for the test (splitmix64): identical on every run and platform.
struct Prng(u64);
impl Prng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// A whole number of degrees in `-range..=range`.
    fn degrees(&mut self, range: u64) -> f64 {
        (self.next() % (2 * range + 1)) as f64 - range as f64
    }
}

/// `camera` turned by `yaw` degrees about its up axis, then `pitch` degrees about its right axis.
fn turned_by(camera: Camera, yaw: f64, pitch: f64) -> Camera {
    let rotate = |v: Vec3, axis: Vec3, deg: f64| {
        let (s, c) = (libm::sin(deg.to_radians()), libm::cos(deg.to_radians()));
        v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
    };
    let up = camera.up.normalized();
    let forward = rotate(camera.forward, up, yaw);
    let right = forward.cross(up).normalized();
    Camera { forward: rotate(forward, right, pitch).normalized(), ..camera }
}

// spec: STRM-002, STRM-016
#[test]
fn a_seeded_mouse_look_script_never_leaves_a_visible_node_blank() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(worker(spawner.clone(), 2));
    let home = camera_at(300.0, 0.0);
    // Resident: roots plus the tiles of the views in every direction a stride of 20 degrees apart (the position's residency set
    // is a superset of the draw set of every view from there, ADR 0031); the script's views fall in between.
    let mut resident = roots();
    for yaw in (0..360).step_by(20) {
        let plan = plan_terrain_field(&request(turned_by(home, f64::from(yaw), 0.0)), &Reference(7)).unwrap();
        resident.extend(with_ancestors(plan.nodes.iter().map(|n| n.tile)));
    }
    let mut prng = Prng(0x6c6f_6f6b);
    let (mut yaw, mut pitch) = (0.0f64, 0.0f64);
    let mut kinds = BTreeSet::new();
    for step in 0..60 {
        // Reversals and a 180 degree turn at the end.
        yaw += if step == 59 { 180.0 } else { prng.degrees(25) };
        pitch = (pitch + prng.degrees(6)).clamp(-30.0, 30.0);
        let req = request(turned_by(home, yaw, pitch));
        let f = pipe.begin_frame(&req, &resident).unwrap();
        kinds.insert(format!("{:?}", f.draw.kind));
        assert_eq!(f.draw.blank, 0, "step {step}");
        assert!(!f.draw.plan.nodes.is_empty(), "step {step}");
        // Every node of the view's own plan is served by a drawn tile above it or below it.
        let sync = plan_terrain_field(&req, &Reference(7)).unwrap();
        let drawn: BTreeSet<TileId> = f.draw.plan.nodes.iter().map(|n| n.tile).collect();
        for n in &sync.nodes {
            let above = std::iter::successors(Some(n.tile), |a| a.parent()).any(|a| drawn.contains(&a));
            let below = drawn.iter().any(|d| std::iter::successors(Some(*d), |a| a.parent()).any(|a| a == n.tile));
            assert!(above || below, "step {step}: {:?} (level {}) is blank", n.tile, n.level);
        }
        // The worker finishes a plan every second frame.
        if step % 2 == 1 {
            spawner.run(1);
        }
    }
    assert!(kinds.contains("Cover"), "turning makes plans unusable, so the cover serves: {kinds:?}");
}

/// Every node of `sync` is served by a tile of `drawn` above it, itself, or below it.
fn assert_served(sync: &planet_frame::TerrainPlan, drawn: &planet_frame::TerrainPlan, what: &str) {
    let tiles: BTreeSet<TileId> = drawn.nodes.iter().map(|n| n.tile).collect();
    for n in &sync.nodes {
        let above = std::iter::successors(Some(n.tile), |a| a.parent()).any(|a| tiles.contains(&a));
        let below = tiles.iter().any(|d| std::iter::successors(Some(*d), |a| a.parent()).any(|a| a == n.tile));
        assert!(above || below, "{what}: {:?} is blank", n.tile);
    }
}

// spec: STRM-002, STRM-016
#[test]
fn every_frame_around_a_jump_serves_every_node_of_the_new_view() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(worker(spawner.clone(), 2));
    let mut resident = roots();
    let b = request(camera_at(1.0, 180.0));
    let plan_b = plan_terrain_field(&b, &Reference(7)).unwrap();
    for k in 0..3 {
        let f = pipe.begin_frame(&b, &resident).unwrap();
        assert_served(&plan_b, &f.draw.plan, &format!("frame {k}"));
        spawner.run(1);
    }
    resident.extend(with_ancestors(plan_b.nodes.iter().map(|n| n.tile)));
    let f = pipe.begin_frame(&b, &resident).unwrap();
    assert_served(&plan_b, &f.draw.plan, "settled");
}

// spec: VIEW-044
#[test]
fn every_drawn_node_has_the_state_it_stands_in_against_the_current_definition() {
    use planet_frame::preview::PreviewStamp;
    use planet_frame::{TileBounds, TileState};
    let req = request(camera_at(300.0, 0.0));
    let full = plan_terrain_field(&req, &Reference(7)).unwrap();
    assert!(full.nodes.len() > 12, "the view must be non-trivial: {}", full.nodes.len());
    assert!(full.nodes.iter().all(|n| n.state == TileState::Exact), "a plan from the planner is exact");
    let set = with_ancestors(full.nodes.iter().map(|n| n.tile));
    let (bounds, none) = (TileBounds::default(), BTreeSet::new());
    let draw = |resident: &dyn ResidentSet, edited: &BTreeSet<TileId>, preview: &[PreviewStamp]| {
        draw_plan_transient(&req, Some((&req, &full)), resident, &bounds, preview, edited, &[]).unwrap()
    };
    let states = |d: &DrawPlan| d.plan.nodes.iter().map(|n| (n.tile, n.state)).collect::<BTreeMap<_, _>>();
    // Everything resident with the plan's keys: every node exact.
    let d = draw(&set, &none, &[]);
    assert!(d.plan.nodes.iter().all(|n| n.state == TileState::Exact) && d.kind == DrawKind::Exact);

    // An edit names three tiles: they are drawn as they are, stale; the rest stay exact.
    let edited: BTreeSet<TileId> = full.nodes.iter().take(3).map(|n| n.tile).collect();
    let s = states(&draw(&set, &edited, &[]));
    for (t, state) in &s {
        assert_eq!(*state, if edited.contains(t) { TileState::Stale } else { TileState::Exact }, "{t:?}");
    }
    // A resident tile of another version (its key is not the plan's) is stale too.
    let victim = full.nodes[5].tile;
    let mut other_key = set.clone();
    other_key.get_mut(&victim).unwrap().mesh_key ^= 1;
    let s = states(&draw(&other_key, &none, &[]));
    assert_eq!(s[&victim], TileState::Stale);
    assert_eq!(s.values().filter(|st| **st == TileState::Stale).count(), 1);

    // A node whose tile is not resident yet is covered by the nearest resident ancestor, a stand-in; the missing tile is not drawn.
    let missing = full.nodes.iter().find(|n| n.level >= 2).unwrap().tile;
    let mut without = set.clone();
    without.remove(&missing);
    let d = draw(&without, &none, &[]);
    let s = states(&d);
    assert!(!s.contains_key(&missing));
    let parent = missing.parent().unwrap();
    assert_eq!(s.get(&parent).copied(), Some(TileState::StandIn), "the ancestor stands in");
    assert!(s.iter().filter(|(t, _)| **t != parent).all(|(_, st)| *st == TileState::Exact));

    // A preview stamp over the camera's place: the nodes it touches are previewed, the others keep their state.
    let at = full.camera.position.0.normalized();
    let stamp = PreviewStamp {
        sign: 1,
        kind: planet_world_def::stamp::StampKind::Hill,
        amount_m: 50.0,
        dir: at,
        reach_m: 2_000.0,
        max_delta_m: 60.0,
    };
    // The edit's delta shows over the stale tiles it names (an exact tile already holds the edit) ...
    let under: BTreeSet<TileId> = full.nodes.iter().map(|n| n.tile).collect();
    let d = draw(&set, &under, &[stamp]);
    let previewed = d.plan.nodes.iter().filter(|n| n.state == TileState::Previewed).count();
    assert!(previewed > 0, "the delta displaces the stale tiles under it");
    assert!(d.plan.nodes.iter().all(|n| (n.state == TileState::Previewed) == n.preview), "previewed exactly where the node is flagged");
    assert!(d.plan.nodes.iter().any(|n| n.state == TileState::Stale), "stale tiles away from the stamp stay stale");
    // ... and a transient stamp (a brush stroke under the pointer) displaces exact tiles too.
    let d = draw_plan_transient(&req, Some((&req, &full)), &set, &bounds, &[], &none, &[stamp]).unwrap();
    assert!(d.plan.nodes.iter().any(|n| n.state == TileState::Previewed), "the transient stamp displaces the tiles under it");
    assert!(d.plan.nodes.iter().any(|n| n.state == TileState::Exact), "tiles away from the stamp stay exact");
    assert!(d.plan.nodes.iter().all(|n| n.state != TileState::Capped), "the capped colour is reserved (ADR 0074)");

    // The cover (no plan in hand) is a stand-in for the plan it cannot serve.
    let c = draw_plan(&req, None, &roots()).unwrap();
    assert!(c.plan.nodes.iter().all(|n| n.state == TileState::StandIn));
}
