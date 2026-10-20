//! The streaming test cases of ADR 0032 as camera scripts and the detail-lag measure (TEST-018, GPU-free, CON-03): the dive
//! (`dive`, `dive-fast`), the teleport (`teleport-1m`, `teleport-2km`) and mouse look (`look-1m`, `look-2km`). One definition
//! serves the tier-A fake-clock cases and the perf scenes of `cargo xtask perf`: a script is a list of cameras, one per frame
//! of a fixed step ([`FRAME_STEP_S`]), a pure function of the scene, the height field and the seed. Time never enters a script
//! except as the frame index times the step (CON-04); the real clock is used by the harness only to measure and to pace.

use crate::camera::Camera;
use crate::lod::{LodParams, SelectionCache};
use crate::plan::PlanError;
use crate::terrain::{above_terrain, plan_terrain_cached, TerrainDraw, TerrainPlan, TerrainRequest};
use planet_core::{TileId, Vec3};
use planet_generators::height::HeightField;
use std::collections::HashSet;

/// Seconds of script time per frame: every case runs at 60 frames per second of script time.
pub const FRAME_STEP_S: f64 = 1.0 / 60.0;
/// Altitude at the start of the dive and at its end (above the terrain, metres): 7.3 decades.
pub const DIVE_START_M: f64 = 2.0e7;
pub const DIVE_END_M: f64 = 1.0;
/// Seconds of the descent and of the ground segment that follows it.
pub const DIVE_DESCENT_S: f64 = 30.0;
pub const DIVE_GROUND_S: f64 = 10.0;
/// Ground speeds of `dive` (walking) and `dive-fast` (vehicle), metres per second.
pub const WALK_SPEED_MPS: f64 = 1.5;
pub const VEHICLE_SPEED_MPS: f64 = 30.0;
/// Seconds a teleport or look case holds after its start (the measured part).
pub const HOLD_S: f64 = 10.0;
/// Altitudes of the teleport destinations and of the mouse-look positions.
pub const GROUND_M: f64 = 1.0;
pub const LOW_M: f64 = 2000.0;
/// Seconds the 180 degree turn that ends a mouse-look script takes.
pub const TURN_S: f64 = 0.2;
/// Seconds without a request, an in-flight job or a waiting upload that make a view settled.
pub const SETTLE_S: f64 = 1.0;
/// Latitude and longitude (degrees) of the dive, the teleport's start and the look position; the teleport lands on the
/// antipode of this point.
pub const SITE_LAT_DEG: f64 = 20.0;
pub const SITE_LON_DEG: f64 = 30.0;
/// The mouse-look seed the perf scenes and the tier-A cases use (ADR 0032 leaves the generator open).
pub const LOOK_SEED: u64 = 0x4C4F_4F4B_5345_4544;
const FOV_Y_DEG: f64 = 60.0;
const NEAR_M: f64 = 0.1;
/// Mouse-look pitch limits (degrees below the horizon).
const PITCH_MIN_DEG: f64 = -15.0;
const PITCH_MAX_DEG: f64 = 60.0;

/// The seven scenes: the six of ADR 0032 point 8 and `dive-cold`, the dive from a tile pack whose coarse tiles are ignored (ADR 0045 point 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamScene {
    Dive,
    DiveFast,
    /// The dive script, read through a reader that ignores the pack's pre-filled coarse tiles (tracked, report-only).
    DiveCold,
    Teleport1m,
    Teleport2km,
    Look1m,
    Look2km,
}

impl StreamScene {
    pub const ALL: [StreamScene; 7] = [
        StreamScene::Dive,
        StreamScene::DiveFast,
        StreamScene::Teleport1m,
        StreamScene::Teleport2km,
        StreamScene::Look1m,
        StreamScene::Look2km,
        StreamScene::DiveCold,
    ];

    /// The perf scene name (`cargo xtask perf`, the log file and the baseline are named after it).
    pub fn name(self) -> &'static str {
        match self {
            StreamScene::Dive => "dive",
            StreamScene::DiveFast => "dive-fast",
            StreamScene::Teleport1m => "teleport-1m",
            StreamScene::Teleport2km => "teleport-2km",
            StreamScene::Look1m => "look-1m",
            StreamScene::Look2km => "look-2km",
            StreamScene::DiveCold => "dive-cold",
        }
    }

    pub fn parse(s: &str) -> Option<StreamScene> {
        StreamScene::ALL.into_iter().find(|v| v.name() == s)
    }

    /// Altitude (metres above the terrain) of the destination or position of a teleport or look scene.
    fn altitude_m(self) -> f64 {
        match self {
            StreamScene::Teleport1m | StreamScene::Look1m => GROUND_M,
            StreamScene::Teleport2km | StreamScene::Look2km => LOW_M,
            StreamScene::Dive | StreamScene::DiveFast | StreamScene::DiveCold => DIVE_END_M,
        }
    }

    /// Ground speed of a dive scene, metres per second.
    pub fn ground_speed_mps(self) -> f64 {
        match self {
            StreamScene::DiveFast => VEHICLE_SPEED_MPS,
            _ => WALK_SPEED_MPS,
        }
    }
}

/// Altitude above the terrain at script time `t_s` of a dive: a constant factor per unit time from [`DIVE_START_M`] to
/// [`DIVE_END_M`] in [`DIVE_DESCENT_S`] (equal time per decade), then [`DIVE_END_M`] for the ground segment.
pub fn dive_altitude_m(t_s: f64) -> f64 {
    let u = (t_s / DIVE_DESCENT_S).clamp(0.0, 1.0);
    DIVE_START_M * libm::pow(DIVE_END_M / DIVE_START_M, u)
}

/// Metres travelled north along the surface by script time `t_s` at `speed_mps`: nothing during the descent, then a constant
/// speed on the ground segment.
pub fn dive_ground_distance_m(t_s: f64, speed_mps: f64) -> f64 {
    (t_s - DIVE_DESCENT_S).max(0.0) * speed_mps
}

/// A camera script: what to hold until the world is settled, then one camera per frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneScript {
    pub scene: StreamScene,
    /// The camera held (unmeasured) until the view is settled before frame 0 (teleport, look); `None`: frame 0 starts from an
    /// empty cache (dive).
    pub prelude: Option<Camera>,
    /// One camera per frame of [`FRAME_STEP_S`].
    pub cameras: Vec<Camera>,
    /// Frames of the dive's descent (the ground segment is the rest); 0 for the other scenes.
    pub descent_frames: usize,
}

impl SceneScript {
    /// Script time of frame `k` in seconds.
    pub fn time_s(k: usize) -> f64 {
        k as f64 * FRAME_STEP_S
    }
}

/// Build the script of `scene` for a planet of `radius_m` with `field` (cameras are placed above the terrain), a viewport of
/// `aspect`, and `seed` for the mouse-look sequence. Pure and deterministic.
pub fn build_script(scene: StreamScene, radius_m: f64, aspect: f64, field: &dyn HeightField, seed: u64) -> SceneScript {
    let fov = FOV_Y_DEG.to_radians();
    let north = Vec3::new(0.0, 0.0, 1.0);
    let cam = |lat: f64, lon: f64, h: f64, pitch_deg: f64| {
        Camera::at_surface_point(above_terrain(radius_m, field, lat, lon, h), north, pitch_deg.to_radians(), fov, aspect, NEAR_M)
    };
    match scene {
        StreamScene::Dive | StreamScene::DiveFast | StreamScene::DiveCold => {
            let frames = ((DIVE_DESCENT_S + DIVE_GROUND_S) / FRAME_STEP_S).round() as usize;
            let descent_frames = (DIVE_DESCENT_S / FRAME_STEP_S).round() as usize;
            let speed = scene.ground_speed_mps();
            let cameras = (0..frames)
                .map(|k| {
                    let t = SceneScript::time_s(k);
                    let h = dive_altitude_m(t);
                    // The view tilts from nearly straight down to a walker's eight degrees in proportion to the descent (the
                    // TEST-013 flight's law: a function of time only, no look input), then stays.
                    let u = (t / DIVE_DESCENT_S).clamp(0.0, 1.0);
                    let pitch = 85.0 - 77.0 * u;
                    let lat = SITE_LAT_DEG + (dive_ground_distance_m(t, speed) / radius_m).to_degrees();
                    cam(lat, SITE_LON_DEG, h, pitch)
                })
                .collect();
            SceneScript { scene, prelude: None, cameras, descent_frames }
        }
        StreamScene::Teleport1m | StreamScene::Teleport2km => {
            let frames = (HOLD_S / FRAME_STEP_S).round() as usize;
            let pitch = if scene == StreamScene::Teleport1m { 8.0 } else { 20.0 };
            // Start at rest at 2 km over the site; land on its antipode.
            let prelude = cam(SITE_LAT_DEG, SITE_LON_DEG, LOW_M, 20.0);
            let landing = cam(-SITE_LAT_DEG, SITE_LON_DEG + 180.0, scene.altitude_m(), pitch);
            SceneScript { scene, prelude: Some(prelude), cameras: vec![landing; frames], descent_frames: 0 }
        }
        StreamScene::Look1m | StreamScene::Look2km => {
            let position = above_terrain(radius_m, field, SITE_LAT_DEG, SITE_LON_DEG, scene.altitude_m());
            let up = position.0.normalized();
            let north_t = (north - up * north.dot(up)).normalized();
            let east = north_t.cross(up);
            let cameras: Vec<Camera> = look_angles(seed)
                .into_iter()
                .map(|(yaw, pitch)| {
                    let heading = north_t * libm::cos(yaw) + east * libm::sin(yaw);
                    Camera::at_surface_point(position, heading, pitch, fov, aspect, NEAR_M)
                })
                .collect();
            let prelude = cameras[0];
            SceneScript { scene, prelude: Some(prelude), cameras, descent_frames: 0 }
        }
    }
}

/// A self-contained SplitMix64 generator: identical on every platform and run, no `libm` or platform generator involved.
#[derive(Clone, Debug)]
pub struct ScriptRng(u64);

impl ScriptRng {
    pub fn new(seed: u64) -> Self {
        ScriptRng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
}

/// Yaw (radians from north towards east) and pitch (radians below the horizon) of every frame of a mouse-look script of
/// [`HOLD_S`] seconds: segments of 0.3 to 1.0 s with a yaw rate of 30 to 180 degrees per second whose sign reverses at every
/// segment, a pitch rate of 10 to 60 degrees per second with a random sign (reflected at the pitch limits), and the last
/// [`TURN_S`] seconds a 180 degree yaw turn. A pure function of `seed`.
pub fn look_angles(seed: u64) -> Vec<(f64, f64)> {
    let frames = (HOLD_S / FRAME_STEP_S).round() as usize;
    let turn_frames = (TURN_S / FRAME_STEP_S).round() as usize;
    let random_frames = frames - turn_frames;
    let mut rng = ScriptRng::new(seed);
    let (mut yaw, mut pitch) = (0.0f64, 8.0f64);
    let mut yaw_sign = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
    let mut out = Vec::with_capacity(frames);
    let mut k = 0;
    while k < random_frames {
        let len = (18 + (rng.next_u64() % 43) as usize).min(random_frames - k);
        let yaw_rate = yaw_sign * (30.0 + 150.0 * rng.unit());
        let mut pitch_rate = (10.0 + 50.0 * rng.unit()) * if rng.unit() < 0.5 { -1.0 } else { 1.0 };
        for _ in 0..len {
            yaw += yaw_rate * FRAME_STEP_S;
            pitch += pitch_rate * FRAME_STEP_S;
            if pitch > PITCH_MAX_DEG || pitch < PITCH_MIN_DEG {
                pitch = pitch.clamp(PITCH_MIN_DEG, PITCH_MAX_DEG);
                pitch_rate = -pitch_rate;
            }
            out.push((yaw.to_radians(), pitch.to_radians()));
        }
        yaw_sign = -yaw_sign;
        k += len;
    }
    let start = yaw;
    for i in 1..=turn_frames {
        out.push(((start + 180.0 * i as f64 / turn_frames as f64).to_radians(), pitch.to_radians()));
    }
    debug_assert_eq!(out.len(), frames);
    out
}

// ---- detail lag (ADR 0032 point 4) ----

/// Projected pixel area of the target node `d` seen from `camera` (its footprint on the screen, capped at the screen): the
/// weight of its detail lag. The tile edge is `cells * sample spacing`, its distance the distance of its origin (at least the
/// near plane and half the edge).
pub fn node_weight(camera: &Camera, lod: &LodParams, d: &TerrainDraw) -> f64 {
    let edge_m = lod.sample_spacing(d.level) * f64::from(lod.cells);
    let dist = (d.origin.0 - camera.position.0).length().max(0.5 * edge_m).max(camera.near_m);
    let px_per_rad = lod.viewport_h_px / (2.0 * libm::tan(camera.fov_y_rad / 2.0));
    let side = edge_m / dist * px_per_rad;
    let screen = lod.viewport_h_px * lod.viewport_h_px * camera.aspect;
    (side * side).min(screen)
}

/// What the drawn plan shows of the target plan.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameLag {
    /// Area-weighted mean over the target nodes of (target level minus the level drawn for it).
    pub mean: f64,
    /// The largest single node's lag.
    pub worst: u8,
    /// Target nodes with no drawn tile at, above or below them: counted as `level + 1` levels in the mean. Each one is a blank
    /// tile (ADR 0032's hard measure: a tile, root or ancestor, for every visible node); the frame's drawing rule (a plan of another
    /// camera is drawn only where it covers the frame's view, else the cover) leaves none.
    pub uncovered: Vec<TileId>,
}

/// The tiles a frame draws, with their proper ancestors: what [`detail_lag`] looks a target node up in.
#[derive(Clone, Debug, Default)]
pub struct DrawnSet {
    drawn: HashSet<TileId>,
    /// Every proper ancestor of a drawn tile (a node whose descendants are drawn is covered by finer tiles, never uncovered).
    above: HashSet<TileId>,
}

impl DrawnSet {
    pub fn new(tiles: impl IntoIterator<Item = TileId>) -> Self {
        let mut set = DrawnSet::default();
        for t in tiles {
            if set.drawn.insert(t) {
                let mut up = t.parent();
                while let Some(a) = up {
                    if !set.above.insert(a) {
                        break;
                    }
                    up = a.parent();
                }
            }
        }
        set
    }

    /// The tiles of a drawn plan.
    pub fn of_plan(plan: &TerrainPlan) -> Self {
        Self::new(plan.nodes.iter().map(|n| n.tile))
    }

    /// The level of the deepest drawn tile that is `tile` or one of its ancestors; `None` when none is (a tile with only
    /// descendants drawn is covered by finer tiles: see [`DrawnSet::covered_below`]).
    pub fn drawn_level_of(&self, tile: TileId) -> Option<u8> {
        let mut up = Some(tile);
        while let Some(t) = up {
            if self.drawn.contains(&t) {
                return Some(t.level());
            }
            up = t.parent();
        }
        None
    }

    /// Whether finer tiles than `tile` are drawn below it (it is a proper ancestor of a drawn tile).
    pub fn covered_below(&self, tile: TileId) -> bool {
        self.above.contains(&tile)
    }

    pub fn len(&self) -> usize {
        self.drawn.len()
    }

    pub fn is_empty(&self) -> bool {
        self.drawn.is_empty()
    }
}

/// Detail lag of a frame (ADR 0032 point 4, TEST-018/TEST-020): `fresh` is the plan the camera of the frame selects with every
/// tile resident (the nodes' levels are the target); the drawn level of a node is the level of the deepest drawn tile that is
/// the node or one of its ancestors, and the node's lag is the target level minus it. A node with a drawn tile below it but
/// none at or above it is covered by finer tiles (finer than the target is clamped to lag 0, never negative); a node with
/// nothing drawn at, above or below it is uncovered, counts as `level + 1` levels and is reported with its tile id. The mean is
/// weighted by the nodes' projected pixels ([`node_weight`]). Pure: no field, no clock, no residency state.
pub fn detail_lag(fresh: &TerrainPlan, drawn: &DrawnSet, lod: &LodParams) -> FrameLag {
    let (mut sum, mut total, mut worst) = (0.0f64, 0.0f64, 0u8);
    let mut uncovered = Vec::new();
    for n in &fresh.nodes {
        let mut up = Some(n.tile);
        let mut covered = None;
        while let Some(t) = up {
            if drawn.drawn.contains(&t) {
                covered = Some(t.level());
                break;
            }
            up = t.parent();
        }
        let lag = match covered {
            Some(l) => n.level.saturating_sub(l),
            None if drawn.above.contains(&n.tile) => 0,
            None => {
                uncovered.push(n.tile);
                n.level + 1
            }
        };
        worst = worst.max(lag);
        let w = node_weight(&fresh.camera, lod, n);
        sum += w * f64::from(lag);
        total += w;
    }
    FrameLag { mean: if total > 0.0 { sum / total } else { 0.0 }, worst, uncovered }
}

/// [`detail_lag`] of a target plan against a drawn plan (the tiles of its nodes).
pub fn frame_lag(target: &TerrainPlan, drawn: &TerrainPlan, lod: &LodParams) -> FrameLag {
    detail_lag(target, &DrawnSet::of_plan(drawn), lod)
}

/// The detail lag of the frame whose camera is in `req`: the selection with every tile resident is made afresh, over `field`
/// (the plan the synchronous path makes for that camera, the stale-plan rule of the drawing not involved), and judged against
/// `drawn`. The selection is deterministic, so this is exact whenever it is run, on any thread (CON-14).
pub fn fresh_detail_lag(
    req: &TerrainRequest,
    field: &dyn HeightField,
    cache: &mut SelectionCache,
    drawn: &DrawnSet,
) -> Result<FrameLag, PlanError> {
    let fresh = plan_terrain_cached(req, field, cache)?;
    Ok(detail_lag(&fresh, drawn, &req.lod))
}

/// What a case records of one frame for the lag post-pass: the f64 camera pose the frame was drawn for and the tiles of the plan
/// actually drawn (resident and drawn that frame). A few kilobytes a frame; the post-pass needs nothing else.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameDraw {
    /// `None` is missing data (a frame whose pose was not recorded): the post-pass fails naming the frame.
    pub camera: Option<Camera>,
    pub drawn: Vec<TileId>,
}

/// The detail lag of every frame of a recorded case, off the timed path: the fresh selection of each recorded camera against the
/// tiles drawn in that frame ([`fresh_detail_lag`]). Frames are split into contiguous chunks over `threads` scoped threads, each
/// with its own field (`make_field`) and selection cache; a selection is a pure function of the request, so the result is the
/// same for every thread count (CON-14) and equal, frame for frame, to the inline computation. `req_of` builds the request of a
/// camera (viewport, LOD parameters, seed of the case). A frame without a recorded pose is an error naming it.
pub fn lag_post_pass<'f>(
    frames: &[FrameDraw],
    req_of: &(dyn Fn(Camera) -> TerrainRequest + Sync),
    make_field: &(dyn Fn() -> Result<Box<dyn HeightField + 'f>, String> + Sync),
    threads: usize,
) -> Result<Vec<FrameLag>, String> {
    if let Some(k) = frames.iter().position(|f| f.camera.is_none()) {
        return Err(format!("detail lag: frame {k} has no recorded camera pose, so its fresh plan cannot be made (missing data)"));
    }
    let chunk = frames.len().div_ceil(threads.clamp(1, frames.len().max(1))).max(1);
    let run = |base: usize, part: &[FrameDraw]| -> Result<Vec<FrameLag>, String> {
        let field = make_field()?;
        let mut cache = SelectionCache::new();
        part.iter()
            .enumerate()
            .map(|(i, f)| {
                let camera = f.camera.expect("checked above");
                fresh_detail_lag(&req_of(camera), field.as_ref(), &mut cache, &DrawnSet::new(f.drawn.iter().copied()))
                    .map_err(|e| format!("detail lag: fresh plan of frame {}: {e}", base + i))
            })
            .collect()
    };
    let parts: Vec<Result<Vec<FrameLag>, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = frames.chunks(chunk).enumerate().map(|(c, part)| s.spawn(move || (c, run(c * chunk, part)))).collect();
        handles.into_iter().map(|h| h.join().expect("lag post-pass thread").1).collect()
    });
    let mut out = Vec::with_capacity(frames.len());
    for p in parts {
        out.extend(p?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_generators::height::Reference;

    const R: f64 = 6_371_000.0;

    fn script(scene: StreamScene) -> SceneScript {
        build_script(scene, R, 16.0 / 9.0, &Reference(1), LOOK_SEED)
    }

    // spec: TEST-018
    #[test]
    fn the_dive_falls_by_a_constant_factor_per_time_from_20000_km_to_1_m_in_30_s_then_holds() {
        assert!((dive_altitude_m(0.0) - 2.0e7).abs() < 1e-3 && (dive_altitude_m(30.0) - 1.0).abs() < 1e-9);
        let step = |t: f64| dive_altitude_m(t + FRAME_STEP_S) / dive_altitude_m(t);
        let f0 = step(0.0);
        for k in (0..1799).step_by(37) {
            let f = step(SceneScript::time_s(k));
            assert!(f < 1.0 && ((f - f0) / f0).abs() < 1e-9, "frame {k}: factor {f} against {f0}");
        }
        // Equal time per decade: 7.3 decades in 30 s.
        let decades = libm::log10(DIVE_START_M / DIVE_END_M);
        assert!((decades - 7.30).abs() < 0.01, "{decades}");
        let t_per_decade = DIVE_DESCENT_S / decades;
        assert!((dive_altitude_m(t_per_decade) / dive_altitude_m(0.0) - 0.1).abs() < 1e-9);
        // The ground segment: 1 m for 10 s, then moving at the scene's speed.
        assert_eq!(dive_altitude_m(35.0), 1.0);
        assert_eq!(dive_altitude_m(1.0e6), 1.0);
        assert_eq!(dive_ground_distance_m(30.0, 1.5), 0.0);
        assert!((dive_ground_distance_m(40.0, 1.5) - 15.0).abs() < 1e-12);
        assert!((dive_ground_distance_m(40.0, 30.0) - 300.0).abs() < 1e-12);
    }

    // spec: TEST-018
    #[test]
    fn the_dive_scripts_are_40_seconds_of_monotone_descent_then_ground_travel_at_the_scene_speed() {
        for scene in [StreamScene::Dive, StreamScene::DiveFast, StreamScene::DiveCold] {
            let s = script(scene);
            assert_eq!((s.cameras.len(), s.descent_frames), (2400, 1800), "{scene:?}: 40 s in all");
            assert!(s.prelude.is_none(), "a dive starts from an empty cache");
            let radius = |c: &Camera| c.position.0.length();
            // Altitude above the sphere falls monotonically over the descent (the field is a perturbation of metres).
            for w in s.cameras[..1800].windows(2) {
                assert!(radius(&w[1]) < radius(&w[0]), "{scene:?}: the descent must be monotone");
            }
            // Ground segment: constant speed along the surface.
            let step = (s.cameras[2300].position.0 - s.cameras[2301].position.0).length() / FRAME_STEP_S;
            let want = scene.ground_speed_mps();
            assert!((step - want).abs() / want < 0.02, "{scene:?}: ground speed {step} against {want}");
            // The view looks along the direction of travel (north), tilting to eight degrees by the end of the descent.
            let c = s.cameras[2399];
            let up = c.position.0.normalized();
            assert!((c.forward.dot(up) + libm::sin(8f64.to_radians())).abs() < 1e-9);
        }
        assert_eq!(script(StreamScene::Dive), script(StreamScene::Dive), "pure and deterministic");
        // The two dives share the descent exactly.
        assert_eq!(script(StreamScene::Dive).cameras[..1800], script(StreamScene::DiveFast).cameras[..1800]);
        // `dive-cold` is the dive script itself (ADR 0045 point 3): only the reader of the tile pack differs.
        assert_eq!(script(StreamScene::Dive).cameras, script(StreamScene::DiveCold).cameras);
        assert_eq!(script(StreamScene::Dive).descent_frames, script(StreamScene::DiveCold).descent_frames);
    }

    // spec: TEST-018
    #[test]
    fn a_teleport_holds_a_settled_2_km_view_and_jumps_to_the_antipode_for_600_frames() {
        for (scene, alt) in [(StreamScene::Teleport1m, 1.0), (StreamScene::Teleport2km, 2000.0)] {
            let s = script(scene);
            let from = s.prelude.expect("a teleport settles first");
            assert_eq!(s.cameras.len(), 600, "10 s");
            assert!(s.cameras.iter().all(|c| *c == s.cameras[0]), "the camera holds still after the jump");
            let (a, b) = (from.position.0, s.cameras[0].position.0);
            // Antipodal: the directions are opposite to within the latitude/terrain offsets of the exact negation.
            assert!(a.normalized().dot(b.normalized()) < -0.999_999, "{scene:?}");
            let h = |p: Vec3| p.length() - R;
            assert!((h(a) - 2000.0).abs() < 100.0 && (h(b) - alt).abs() < 100.0, "{} {}", h(a), h(b));
            assert_eq!(s, script(scene));
        }
    }

    // spec: TEST-018
    #[test]
    fn mouse_look_is_a_seeded_script_with_reversals_ending_in_a_180_degree_turn_within_0_2_s() {
        let a = look_angles(LOOK_SEED);
        assert_eq!(a.len(), 600);
        assert_eq!(a, look_angles(LOOK_SEED), "the same seed is the same script");
        assert_ne!(a, look_angles(LOOK_SEED + 1), "another seed is another script");
        // Known answer of the self-contained generator (SplitMix64, seed 0): the same on every platform.
        let mut r = ScriptRng::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        // Yaw reverses direction between segments.
        let rates: Vec<f64> = a.windows(2).map(|w| w[1].0 - w[0].0).collect();
        let reversals = rates[..587].windows(2).filter(|w| w[0].signum() != w[1].signum()).count();
        assert!(reversals >= 8, "{reversals} reversals in 9.8 s");
        // The last 12 frames turn by 180 degrees; the rest stay within the pitch limits.
        let turn = a[599].0 - a[587].0;
        assert!((turn - std::f64::consts::PI).abs() < 1e-9, "{turn}");
        assert!(a.iter().all(|&(_, p)| (PITCH_MIN_DEG.to_radians()..=PITCH_MAX_DEG.to_radians()).contains(&p)));
        // Frame-to-frame steps stay at or below the fastest segment rate (180 degrees per second), except the turn itself.
        assert!(rates[..587].iter().all(|r| r.abs() <= (180.0f64 * FRAME_STEP_S).to_radians() + 1e-12));
        // The cameras share one position; only the orientation moves.
        for scene in [StreamScene::Look1m, StreamScene::Look2km] {
            let s = script(scene);
            assert_eq!(s.cameras.len(), 600);
            assert!(s.cameras.iter().all(|c| c.position == s.cameras[0].position));
            assert!(s.cameras.windows(2).any(|w| w[0].forward != w[1].forward));
            assert_eq!(s.prelude, Some(s.cameras[0]), "settled at the first look direction");
        }
    }

    fn draw(tile: TileId, camera: &Camera) -> TerrainDraw {
        TerrainDraw {
            tile,
            level: tile.level(),
            origin: camera.position,
            camera_relative_origin: [0.0; 3],
            morph: 0.0,
            morph_start_m: 0.0,
            morph_end_m: 0.0,
            color: [0; 4],
            mesh_key: 0,
            preview: false,
            quadrants: 0b1111,
        }
    }

    fn plan(camera: Camera, tiles: &[TileId]) -> TerrainPlan {
        TerrainPlan {
            size: (64, 64),
            view: crate::terrain::TerrainView::Level,
            camera,
            view_projection: camera.view_projection(),
            clear_color: [0; 4],
            height_range: [0.0, 1.0],
            cells: 16,
            shore_ramp_per_m: 0.0,
            nodes: tiles.iter().map(|t| draw(*t, &camera)).collect(),
            lit: None,
            water: None,
        }
    }

    // spec: TEST-018
    #[test]
    fn detail_lag_is_the_area_weighted_level_gap_and_a_node_without_any_ancestor_is_uncovered() {
        let cam = script(StreamScene::Dive).cameras[2399];
        let lod = LodParams { cells: 16, viewport_h_px: 90.0, tau_px: 6.0, max_level: 22, ..LodParams::earth_1080p() };
        let root = TileId::new(planet_core::Face(0), 0, 0, 0).unwrap();
        let kids: Vec<TileId> = (0..4).map(|i| TileId::new(planet_core::Face(0), 1, i & 1, i >> 1).unwrap()).collect();
        let grand = TileId::new(planet_core::Face(0), 2, 0, 0).unwrap();
        // Everything drawn at its own level: no lag.
        let target = plan(cam, &[kids[0], kids[1], grand]);
        let same = frame_lag(&target, &target, &lod);
        assert_eq!((same.mean, same.worst, same.uncovered.len()), (0.0, 0, 0));
        // A grandchild drawn by its grandparent root: two levels; the two level-1 nodes are drawn exactly.
        let drawn = plan(cam, &[root]);
        let l = frame_lag(&plan(cam, &[grand]), &drawn, &lod);
        assert_eq!((l.mean, l.worst), (2.0, 2));
        // Equal weights (same origin and level): mean of the gaps of the nodes.
        let l = frame_lag(&plan(cam, &[kids[0], kids[1]]), &drawn, &lod);
        assert!((l.mean - 1.0).abs() < 1e-12 && l.uncovered.is_empty(), "{l:?}");
        // Nothing covers a node: uncovered, the tile id is reported and the node counts as level + 1.
        let other = TileId::new(planet_core::Face(3), 1, 0, 0).unwrap();
        let l = frame_lag(&plan(cam, &[other]), &drawn, &lod);
        assert_eq!((l.uncovered.clone(), l.mean), (vec![other], 2.0), "{l:?}");
        // A finer drawn tile is not a lag (never negative).
        assert_eq!(frame_lag(&plan(cam, &[root]), &plan(cam, &[root, kids[0]]), &lod).mean, 0.0);
    }

    // spec: TEST-020
    #[test]
    fn detail_lag_takes_the_deepest_drawn_ancestor_clamps_finer_tiles_and_a_missing_parent_falls_to_the_next_resident_one() {
        let cam = script(StreamScene::Dive).cameras[2399];
        let lod = LodParams { cells: 16, viewport_h_px: 90.0, tau_px: 6.0, max_level: 22, ..LodParams::earth_1080p() };
        let f = planet_core::Face(0);
        let root = TileId::new(f, 0, 0, 0).unwrap();
        let kid = TileId::new(f, 1, 0, 0).unwrap();
        let grand = TileId::new(f, 2, 0, 0).unwrap();
        let great = TileId::new(f, 3, 0, 0).unwrap();
        let fresh = |tiles: &[TileId]| plan(cam, tiles);
        let set = |tiles: &[TileId]| DrawnSet::new(tiles.iter().copied());
        // The deepest drawn ancestor counts: with the root and the level-1 tile both drawn, a level-3 node is 2 levels behind.
        let l = detail_lag(&fresh(&[great]), &set(&[root, kid]), &lod);
        assert_eq!((l.mean, l.worst, l.uncovered.len()), (2.0, 2, 0));
        // A missing parent: nothing drawn at levels 2 and 1, so the resident root is the drawn level (3 levels behind).
        let l = detail_lag(&fresh(&[great]), &set(&[root]), &lod);
        assert_eq!((l.mean, l.worst), (3.0, 3));
        // The node itself drawn is lag 0, whatever else is drawn.
        assert_eq!(detail_lag(&fresh(&[great]), &set(&[root, great]), &lod).mean, 0.0);
        // Finer than the target is clamped to 0, never negative: the drawn tiles lie below the node, none at or above it.
        let l = detail_lag(&fresh(&[kid]), &set(&[grand]), &lod);
        assert_eq!((l.mean, l.worst, l.uncovered.len()), (0.0, 0, 0), "{l:?}");
        // Nothing at, above or below the node: uncovered, counted as level + 1 and reported with its tile id.
        let other = TileId::new(planet_core::Face(2), 2, 1, 1).unwrap();
        let l = detail_lag(&fresh(&[other]), &set(&[root, kid]), &lod);
        assert_eq!((l.uncovered.clone(), l.mean, l.worst), (vec![other], 3.0, 3), "{l:?}");
        // An empty drawn set: every node is uncovered. An empty selection has lag 0.
        assert_eq!(detail_lag(&fresh(&[kid]), &DrawnSet::default(), &lod).uncovered, vec![kid]);
        assert_eq!(detail_lag(&fresh(&[]), &set(&[root]), &lod).mean, 0.0);
        // The mean is weighted by the projected pixel area ([`node_weight`]): a far node weighs less than a near one.
        let mut two = fresh(&[kid, great]);
        two.nodes[1].origin = planet_core::PlanetFixed(cam.position.0 + Vec3::new(0.0, 0.0, 1.0e9));
        let (wk, wg) = (node_weight(&cam, &lod, &two.nodes[0]), node_weight(&cam, &lod, &two.nodes[1]));
        assert!(wk > wg && wg > 0.0, "{wk} {wg}");
        let both = detail_lag(&two, &set(&[root]), &lod);
        let want = (wk * 1.0 + wg * 3.0) / (wk + wg);
        assert!((both.mean - want).abs() < 1e-12, "{} against {want}", both.mean);
    }

    // spec: TEST-018
    #[test]
    fn a_nearer_or_finer_node_weighs_more_and_the_weight_is_capped_at_the_screen() {
        let cam = script(StreamScene::Dive).cameras[2399];
        let lod = LodParams { cells: 16, viewport_h_px: 90.0, tau_px: 6.0, max_level: 22, ..LodParams::earth_1080p() };
        let t = TileId::new(planet_core::Face(0), 14, 0, 0).unwrap();
        let near = draw(t, &cam);
        let mut far = near;
        far.origin = planet_core::PlanetFixed(cam.position.0 + Vec3::new(0.0, 0.0, 1.0e5));
        assert!(node_weight(&cam, &lod, &near) > node_weight(&cam, &lod, &far));
        let screen = 90.0 * 90.0 * cam.aspect;
        assert!(node_weight(&cam, &lod, &near) <= screen);
    }
}
