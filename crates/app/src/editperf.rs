//! The edit-to-visible perf scene (SPIKE-13, the M6 exit criterion): `test-render --perf --scene edit-latency`. One stamp is put into the
//! definition of a settled, still view of the streamed world; the scene times the edit to the first frame that shows it as a preview
//! (`PreviewStamp`, REND-033, target <= 16 ms) and to the first frame whose tiles are all exact again (target <= 300 ms for a stamp up to
//! 150 m, <= 1 s up to 500 m and at 1 m altitude), then takes the edit back and settles again. It drives the pipelined managed loop on the
//! real renderer (`pipelined::Rig`) with the real `WorldTerrain::set_definition`, `ManagedLoop::edit` and `set_preview`, as the editor
//! does (order: swap the definition, name the edited tiles, set the preview).
//!
//! Timing starts at the call of `set_definition` and ends when the GPU has finished the frame (the loop waits for it), so the numbers
//! include the footprint find, the plan, the refresh of the tiles, their generation, upload and the frame. A smoke run exercises the path
//! (one altitude, one radius, one repeat, 768x432, no gate) and is never a measurement.

use crate::pipelined::{RendererSink, Rig};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::preview::{preview_delta_on, PreviewStamp};
use planet_frame::{Camera, LodParams, TerrainRequest, TerrainView};
use planet_render::GpuContext;
use planet_world_def::footprint::Cap;
use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The scene's name on the command line.
pub const SCENE: &str = "edit-latency";

/// A settle that has not happened within this long is an error, not a hang.
const SETTLE_LIMIT: Duration = Duration::from_secs(900);

/// Targets of the M6 exit criterion (SPIKE-13): the preview, and the exact tiles for a stamp up to 150 m, up to 500 m and at 1 m altitude.
pub const PREVIEW_BUDGET_MS: f64 = 16.0;
pub const EXACT_BUDGET_SMALL_MS: f64 = 300.0;
pub const EXACT_BUDGET_LARGE_MS: f64 = 1000.0;

/// The altitudes above the ground and the stamp radii of a measured run; a smoke run takes the first radius at 200 m.
pub const ALTITUDES_M: [f64; 3] = [1.0, 200.0, 2000.0];
pub const RADII_M: [f64; 3] = [50.0, 150.0, 500.0];
/// The brush cases (TEST-042, the revisit trigger of the parked coarse-first refinement): one raise stroke of [`BRUSH_POINTS`] points at these
/// radii and altitudes. A smoke run takes the first radius at 200 m.
pub const BRUSH_RADII_M: [f64; 2] = [150.0, 1000.0];
pub const BRUSH_ALTITUDES_M: [f64; 2] = [200.0, 2000.0];
pub const BRUSH_POINTS: usize = 20;
/// The numbers of active preview stamps the preview cost is measured with (TEST-042): none, a few and the cap.
pub const PREVIEW_COST_COUNTS: [usize; 3] = [0, 8, planet_frame::preview::MAX_PREVIEW_STAMPS];
/// The regional cases (TEST-044): a stamp of these radii seen from these altitudes. 50 km is the owner's regional scale, but an item reaches
/// one cell edge at most (`CELL_EDGE_M`, 40 km), so the largest stamp is the largest an item can be. A smoke run leaves them out.
pub const REGIONAL_RADII_M: [f64; 2] = [5_000.0, planet_world_def::cells::CELL_EDGE_M];
pub const REGIONAL_ALTITUDES_M: [f64; 2] = [2_000.0, 20_000.0];
/// The line and region cases (TEST-044): a ridge of this length and a region of this side, at these altitudes.
pub const LINE_LENGTH_M: f64 = 5_000.0;
pub const REGION_SIDE_M: f64 = 2_000.0;
pub const LINE_ALTITUDES_M: [f64; 2] = [200.0, 2_000.0];
/// The surface types of the region case: `from` is in the world before the edit, `to` is the edit (rock to forest).
const REGION_TYPE_FROM: f64 = 0.0;
const REGION_TYPE_TO: f64 = 4.0;
/// The layer of the brush cases' item.
pub const BRUSH_LAYER: &str = "brush";
/// Repeats of each case (the median is reported).
pub const REPEATS: usize = 3;

/// Whether `--scene NAME` is this scene.
pub fn is_edit_scene(name: &str) -> bool {
    name == SCENE
}

/// The budget of the exact tiles for a stamp of `radius_m` seen from `altitude_m`.
pub fn exact_budget_ms(radius_m: f64, altitude_m: f64) -> f64 {
    if radius_m <= 150.0 && altitude_m > 1.5 {
        EXACT_BUDGET_SMALL_MS
    } else {
        EXACT_BUDGET_LARGE_MS
    }
}

/// The stamp of a case: a hill whose height is a fifth of its radius up to 100 m, at the place the scripted views look at (latitude 20,
/// longitude 30), `reach_m` the radius.
pub fn case_stamp(radius_m: f64) -> ItemSpec {
    let (la, lo) = (20f64.to_radians(), 30f64.to_radians());
    let at = Vec3::from_lat_lon(la, lo);
    let height = (0.2 * radius_m).min(100.0);
    ItemSpec {
        id: 1,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: at,
        content_hash: 0,
        reach_m: radius_m,
        max_delta_m: height * 1.5,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [height, 0.0, 0.0]))
}

/// The place the scripted views look at (latitude 20, longitude 30).
pub(crate) fn case_centre() -> Vec3 {
    let (la, lo) = (20f64.to_radians(), 30f64.to_radians());
    Vec3::from_lat_lon(la, lo)
}

/// The brush case of radius `radius_m` on a planet of `planet_radius_m`: one raise stroke of [`BRUSH_POINTS`] points a quarter of the radius
/// apart (what a drag is resampled to), running east and centred on the place the views look at, as one brush item in [`BRUSH_LAYER`]: the
/// anchor is the first point, the reach holds the stroke and its radius, the bound is the strength (a fifth of the radius up to 100 m, the
/// height of the hill case). Returns the item and the centre of the stroke, which the camera looks at.
pub fn case_brush(radius_m: f64, planet_radius_m: f64) -> (ItemSpec, Vec3) {
    let at = case_centre();
    let north = Vec3::new(0.0, 0.0, 1.0);
    let east = north.cross(at).normalized();
    let spacing = 0.25 * radius_m;
    let points: Vec<Vec3> = (0..BRUSH_POINTS)
        .map(|k| {
            let along = (k as f64 - 0.5 * (BRUSH_POINTS - 1) as f64) * spacing / planet_radius_m;
            (at * libm::cos(along) + east * libm::sin(along)).normalized()
        })
        .collect();
    let strength = (0.2 * radius_m).min(100.0);
    let far = planet_radius_m * planet_frame_angle(points[0], points[BRUSH_POINTS - 1]);
    let stroke = Stroke { kind: BrushKind::Raise, radius_m, strength_m: strength, target_m: None, points: points.clone() };
    let spec = ItemSpec {
        id: 1,
        layer: BRUSH_LAYER.into(),
        priority: 0,
        dir: points[0],
        content_hash: 0,
        reach_m: far + radius_m + 2.0,
        max_delta_m: strength,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(&[stroke]));
    (spec, at)
}

/// What the preview costs a frame with `stamps` active stamps (TEST-042): the nodes they flag, the median GPU time of the frame from the
/// timestamp queries when the device has them, the median wall time until the GPU finished it, and how many renders were timed.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewCost {
    pub altitude_m: f64,
    pub stamps: usize,
    pub flagged_nodes: usize,
    pub nodes: usize,
    pub gpu_ms: Option<f64>,
    pub wall_ms: f64,
    pub renders: usize,
}

/// The preview cost table (markdown): per altitude and number of active stamps, the flagged nodes of the plan, the GPU time and the wall time.
pub fn preview_cost_table(rows: &[PreviewCost]) -> String {
    let mut s = String::from(
        "| altitude | active stamps | flagged nodes / nodes | GPU ms (median) | wall ms (median) | renders |\n|---|---|---|---|---|---|\n",
    );
    for r in rows {
        s += &format!(
            "| {} m | {} | {} / {} | {} | {:.2} | {} |\n",
            r.altitude_m,
            r.stamps,
            r.flagged_nodes,
            r.nodes,
            r.gpu_ms.map_or("-".to_string(), |g| format!("{g:.3}")),
            r.wall_ms,
            r.renders
        );
    }
    s
}

/// `count` hill preview stamps of reach `reach_m` in a row east of `at`, a third of a reach apart: a worst case for the vertex stage, which
/// evaluates every active stamp at every vertex of every flagged node.
pub fn cost_stamps(at: Vec3, count: usize, reach_m: f64, planet_radius_m: f64) -> Vec<planet_frame::preview::PreviewStamp> {
    let east = Vec3::new(0.0, 0.0, 1.0).cross(at).normalized();
    (0..count)
        .map(|k| {
            let along = (k as f64 - 0.5 * count as f64) * reach_m / 3.0 / planet_radius_m;
            planet_frame::preview::PreviewStamp {
                sign: 1,
                kind: StampKind::Hill,
                amount_m: 20.0,
                dir: (at * libm::cos(along) + east * libm::sin(along)).normalized(),
                reach_m,
                max_delta_m: 20.0,
            }
        })
        .collect()
}

/// The preview cost on the settled rig for the view `req` looking at `at`: for each of [`PREVIEW_COST_COUNTS`] stamps the transient preview
/// is set, the frame is planned and drawn, and the plan is rendered `renders` times with the GPU waited for (`TerrainRenderer::render`, timed).
fn measure_preview_cost(
    h: &mut Harness<'_>,
    req: &TerrainRequest,
    at: Vec3,
    planet_radius_m: f64,
    altitude_m: f64,
    renders: usize,
    smoke: bool,
) -> Result<Vec<PreviewCost>, String> {
    let mut rows = Vec::new();
    for &count in &PREVIEW_COST_COUNTS {
        h.rig.managed.set_transient_preview(cost_stamps(at, count, 400.0, planet_radius_m));
        let mut frame = h.step(req)?;
        for _ in 0..if smoke { 1 } else { 3 } {
            frame = h.step(req)?;
        }
        h.rig.fi.retire_to(h.rig.renderer.wait_oldest(h.ctx));
        let plan = frame.pipeline.draw.plan.clone();
        let (mut gpu, mut wall) = (Vec::new(), Vec::new());
        for _ in 0..renders {
            let (timing, _) = h.rig.renderer.render(h.ctx, &plan, true).map_err(|e| format!("preview cost render: {e:?}"))?;
            gpu.extend(timing.gpu_ms);
            wall.push(timing.total_ms);
        }
        rows.push(PreviewCost {
            altitude_m,
            stamps: count,
            flagged_nodes: plan.nodes.iter().filter(|n| n.preview).count(),
            nodes: plan.nodes.len(),
            gpu_ms: median(gpu),
            wall_ms: median(wall).unwrap_or(0.0),
            renders,
        });
    }
    h.rig.managed.set_transient_preview(Vec::new());
    h.settle(req, if smoke { 2 } else { 5 })?;
    Ok(rows)
}

/// An item of the case view for the scene: at `centre_deg` (latitude, longitude), in `layer`, with its points in degrees.
fn case_item(
    id: u128,
    layer: &str,
    kind: &str,
    params: &[(&str, f64)],
    points: Vec<(f64, f64)>,
    reach_m: f64,
) -> planet_world_def::item::Item {
    planet_world_def::item::Item {
        id,
        body: BodyId::PLANET,
        surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
        layer: layer.to_string(),
        priority: 0,
        kind: kind.to_string(),
        anchor: planet_world_def::item::Anchor { latitude_deg: points[0].0, longitude_deg: points[0].1, height_m: 0.0, heading_deg: 0.0 },
        reach_m,
        max_delta_m: params.iter().find(|(k, _)| *k == "amount_m").map_or(0.0, |(_, v)| 1.5 * v),
        params: params.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
        base_hash: 0,
        generator_version: 0,
        stage_version: 0,
        name: None,
        notes: None,
        points,
        group: None,
        part: None,
        mask_layer: None,
        strokes: Vec::new(),
    }
}

/// Degrees of longitude and latitude that are `east_m` and `north_m` metres at the view's place (latitude 20, longitude 30).
fn view_degrees(east_m: f64, north_m: f64, planet_radius_m: f64) -> (f64, f64) {
    let lat = 20.0f64;
    (lat + (north_m / planet_radius_m).to_degrees(), 30.0 + (east_m / (planet_radius_m * libm::cos(lat.to_radians()))).to_degrees())
}

/// The ridge case: one straight ridge of [`LINE_LENGTH_M`] running east, centred on the place the views look at (17 points, so a point
/// every 312.5 m), 30 m high and 200 m of half-width. Returns the item's spec and the centre the camera looks at.
pub fn case_ridge(planet_radius_m: f64) -> (ItemSpec, Vec3) {
    let points: Vec<(f64, f64)> =
        (0..17).map(|k| view_degrees((f64::from(k) - 8.0) * LINE_LENGTH_M / 16.0, 0.0, planet_radius_m)).collect();
    let item = case_item(3, "stamps", "ridge", &[("amount_m", 30.0), ("width_m", 200.0)], points, LINE_LENGTH_M + 200.0 + 2.0);
    (item.spec(), case_centre())
}

/// The region case: a square of [`REGION_SIDE_M`] centred on the place the views look at, in the layer `regions`, labelling only (mode 0) and
/// of surface type `surface_type`. The case puts the rock one in the world first and edits it to the forest one. Returns the spec and the centre.
pub fn case_region(surface_type: f64, planet_radius_m: f64) -> (ItemSpec, Vec3) {
    let h = REGION_SIDE_M / 2.0;
    let points: Vec<(f64, f64)> = [(-h, -h), (h, -h), (h, h), (-h, h)].iter().map(|(e, n)| view_degrees(*e, *n, planet_radius_m)).collect();
    let item = case_item(
        4,
        "regions",
        "region",
        &[("surface_type", surface_type), ("mask_mode", 0.0)],
        points,
        REGION_SIDE_M * std::f64::consts::SQRT_2 + 2.0,
    );
    (item.spec(), case_centre())
}

/// The angle in radians between two unit directions.
fn planet_frame_angle(a: Vec3, b: Vec3) -> f64 {
    a.angle_to(b)
}

/// A camera `altitude_m` above the ground at ground distance `rho_m` from `at`, looking at the stamp's centre.
pub fn camera_for(at: Vec3, ground: &dyn Fn(Vec3) -> f64, radius_m: f64, rho_m: f64, altitude_m: f64, aspect: f64) -> Camera {
    let north = Vec3::new(0.0, 0.0, 1.0);
    let t1 = (north - at * north.dot(at)).normalized();
    let a = rho_m / radius_m;
    let dir = (at * libm::cos(a) + t1 * libm::sin(a)).normalized();
    let position = PlanetFixed(dir * (radius_m + ground(dir) + altitude_m));
    let target = at * (radius_m + ground(at));
    Camera { position, forward: (target - position.0).normalized(), up: dir, fov_y_rad: 60f64.to_radians(), aspect, near_m: 0.1 }
}

/// What one run of a case measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaseRun {
    /// The edit to the first frame that shows the preview, GPU finished (`None`: no node was previewed).
    pub preview_ms: Option<f64>,
    /// The edit to the first all-exact frame, GPU finished (`None`: not within the limit).
    pub exact_ms: Option<f64>,
    pub resident: usize,
    pub refreshed: usize,
    pub previewed_nodes_max: usize,
    pub frames: usize,
    /// Where the time of the run went (the attribution of the preview and the exact time, M6 plan E1/E3).
    pub stages: Stages,
    /// The refreshed set by level: how many of the tiles the edit named the plan drew, and at how many levels the stamp's largest change
    /// is below the sample spacing (the change cannot show there).
    pub levels: Vec<LevelRow>,
    /// The first frames after the edit: (ms from set_definition, frame ms, tiles refreshed, jobs in flight, tiles waiting for an upload).
    pub trace: Vec<[f64; 5]>,
    /// The refresh jobs of the case that ended last, by tile (queued behind the streamer, waited for a worker, ran, ended at, jobs beside it).
    pub stragglers: Vec<String>,
}

/// The tiles the edit named at one level (the refreshed set's breakdown, M6 plan E1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelRow {
    pub level: u8,
    pub found: usize,
    /// Of `found`, the tiles the plan drawn before the edit contains (the rest were resident only).
    pub drawn: usize,
    /// The worst-case sample spacing of the level, metres (`LodParams::sample_spacing`).
    pub spacing_m: f64,
    /// Of `found`, the tiles at a level whose spacing is above the stamp's `max_delta_m`.
    pub below_spacing: usize,
}

/// The breakdown of the edit's find by level, against the tiles of the plan that was drawn and the stamp's largest change.
pub fn level_rows(found: &[TileId], drawn: &HashSet<TileId>, lod: &LodParams, max_delta_m: f64) -> Vec<LevelRow> {
    let mut by: BTreeMap<u8, LevelRow> = BTreeMap::new();
    for t in found {
        let level = t.level();
        let row = by.entry(level).or_insert_with(|| LevelRow { level, spacing_m: lod.sample_spacing(level), ..Default::default() });
        row.found += 1;
        row.drawn += usize::from(drawn.contains(t));
        row.below_spacing += usize::from(row.spacing_m > max_delta_m);
    }
    by.into_values().collect()
}

/// The refreshed set of the first run of each case: per case and level, the tiles named, drawn, resident only and below the spacing.
pub fn levels_table(rows: &[CaseRow]) -> String {
    let mut s = String::from(
        "| altitude | stamp radius | level | spacing m | named | drawn | resident only | change below spacing |\n|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let Some(run) = r.runs.first() else { continue };
        for l in &run.levels {
            s += &format!(
                "| {} m | {} | {} | {:.3} | {} | {} | {} | {} |\n",
                r.altitude_m,
                r.radius_label(),
                l.level,
                l.spacing_m,
                l.found,
                l.drawn,
                l.found - l.drawn,
                l.below_spacing
            );
        }
    }
    s
}

/// The stages of one run, each in milliseconds from the call of `set_definition` unless it says it is a duration: the instants are taken
/// at the end of the frame that first shows the stage (before the GPU wait), so they are exact to one frame. `None`: the stage did not
/// happen in the run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stages {
    /// Durations of the three calls of the edit, in the editor's order: `set_definition`, `ManagedLoop::edit`, `set_preview` with the
    /// `preview_delta` that makes its argument.
    pub set_definition_ms: f64,
    pub edit_ms: f64,
    pub preview_call_ms: f64,
    /// The first frame after the edit that delivered a plan: its instant, and the worker's own time for it (a duration).
    pub plan_delivered_ms: Option<f64>,
    pub plan_worker_ms: Option<f64>,
    /// The first frame after the edit that delivered the residency set R.
    pub residency_delivered_ms: Option<f64>,
    /// The first frame in which a tile job had been handed to the spawner since the edit.
    pub first_dispatch_ms: Option<f64>,
    /// The frame in which half of the refresh requests, and all of them, had completed (the streamer's `refreshed` counter), if the
    /// count reached them (a refresh whose job was cancelled and loaded again counts when it completes).
    pub half_refreshed_ms: Option<f64>,
    pub all_refreshed_ms: Option<f64>,
    /// The first frame in which nothing waited for an upload (the new tiles were written).
    pub uploaded_ms: Option<f64>,
    /// The median wall time of the frames of the run (a duration): the pacing and the frame cost inside every instant above.
    pub frame_median_ms: f64,
    /// The footprint find of the edit (`planet_editor::feed::edited_tiles`, what the editor runs), a duration inside `edit_ms`.
    pub find_ms: f64,
    /// The first frame after the edit whose draw plan is exact (`DrawKind::Exact`: every drawn tile has the plan's key), which may come
    /// before the idle frame `exact_ms` waits for (the refreshes of tiles nothing draws).
    pub drawn_exact_ms: Option<f64>,
    /// The refresh settle window of the run in milliseconds (ADR 0072) and the pre-emption counts of the run (STRM-031): refreshes that found
    /// theirs waiting, running jobs a refresh stopped, results dropped before they were made resident.
    pub settle_ms: u64,
    pub coalesced: u64,
    pub cancelled: u64,
    pub dropped: u64,
    /// The chunk side of the edit (a run with the chunk stage): `None` without one.
    pub chunks: Option<ChunkStages>,
    /// The horizon maps the edit touched (a run with the horizon jobs and no chunk stage): per map the chunk and the milliseconds from `set_definition` to the
    /// first landing after it (`None`: it had not landed when the run stopped waiting). `None` without the jobs.
    pub horizon: Option<Vec<(planet_generators::chunkfield::ChunkKey, Option<f64>, f64)>>,
}

/// What the chunk stage did after an edit (M8-7, criterion 6), in milliseconds from `set_definition`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkStages {
    /// The longest `chunk_frame` call of the frames up to the exact one: what the versions of the wanted chunks cost on the first frames
    /// after the edit (a duration).
    pub first_frame_ms: f64,
    /// Chunks that landed again after the edit, and when the one under the edit centre and the last one did (`None`: none did).
    pub resolved: u64,
    pub centre_ms: Option<f64>,
    pub last_ms: Option<f64>,
    /// When the tiles that read the last chunk were drawn again: the later of the last chunk landing and the end of the last refresh job of a tile
    /// the plan draws (ADR 0072 point 1), plus the upload frame; the eroded result of every touched chunk the view wants is then on the screen (the
    /// fade over about 1.5 s follows). The gated reading of the 10 s criterion.
    pub drawn_ms: Option<f64>,
    /// When nothing was left to do after the last chunk (the plan exact, every resident tile refreshed and uploaded, nothing in flight): the reading
    /// of windows before 24, recorded beside the drawn one.
    pub settled_ms: Option<f64>,
    /// The horizon jobs after the edit (0 without them): maps baked, of them re-bakes of a map that had one, and when the last was in flight.
    pub horizon_baked: u64,
    pub horizon_rebaked: u64,
    pub horizon_last_ms: Option<f64>,
}

/// What a case edits: a hill stamp, or a brush stroke (TEST-042).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseKind {
    Stamp,
    Brush,
    /// One ridge of [`LINE_LENGTH_M`] (TEST-044).
    Line,
    /// One region of [`REGION_SIDE_M`] whose surface type changes (TEST-044).
    Region,
}

/// One row of the report: a case with its repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseRow {
    pub kind: CaseKind,
    pub altitude_m: f64,
    pub radius_m: f64,
    pub runs: Vec<CaseRun>,
}

/// The median of the values that exist (`None` when none does).
pub fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

impl CaseRow {
    /// The radius as the tables print it: `150 m` for a stamp, `150 m brush` for a brush stroke.
    pub fn radius_label(&self) -> String {
        match self.kind {
            CaseKind::Stamp => format!("{} m", self.radius_m),
            CaseKind::Brush => format!("{} m brush", self.radius_m),
            CaseKind::Line => format!("{} m ridge", self.radius_m),
            CaseKind::Region => format!("{} m region", self.radius_m),
        }
    }

    /// Whether the verdict reads the budgets: the regional stamps (over 1 km), the ridge and the region are only recorded (the owner sets a
    /// number after measuring, TEST-044).
    pub fn gated(&self) -> bool {
        matches!(self.kind, CaseKind::Stamp | CaseKind::Brush) && self.radius_m <= 1_000.0
    }

    pub fn preview_median(&self) -> Option<f64> {
        median(self.runs.iter().filter_map(|r| r.preview_ms).collect())
    }

    pub fn exact_median(&self) -> Option<f64> {
        median(self.runs.iter().filter_map(|r| r.exact_ms).collect())
    }

    /// The edit to the first frame whose draw plan is exact: "exact" of the M6 criterion (ADR 0072: every tile the plan draws is exact).
    pub fn drawn_exact_median(&self) -> Option<f64> {
        median(self.runs.iter().filter_map(|r| r.stages.drawn_exact_ms).collect())
    }

    /// `ok` or `OVER` against the budgets; a row with no frame that showed the preview is judged on the exact time alone and says
    /// `(no preview)`; `no result` when a run produced no exact frame.
    pub fn verdict(&self) -> &'static str {
        if !self.gated() {
            return "recorded";
        }
        match (self.preview_median(), self.drawn_exact_median()) {
            (Some(p), Some(e)) if p <= PREVIEW_BUDGET_MS && e <= exact_budget_ms(self.radius_m, self.altitude_m) => "ok",
            (Some(_), Some(_)) => "OVER",
            (None, Some(e)) if e <= exact_budget_ms(self.radius_m, self.altitude_m) => "ok (no preview)",
            (None, Some(_)) => "OVER (no preview)",
            _ => "no result",
        }
    }
}

/// The report table (markdown): one row per case, medians over the repeats, budgets named.
pub fn table(rows: &[CaseRow]) -> String {
    let mut s = String::from(
        "| altitude | stamp radius | preview ms (budget 16) | drawn exact ms (budget) | tiles refreshed / resident | previewed nodes (max) | repeats | verdict |\n|---|---|---|---|---|---|---|---|\n",
    );
    let cell = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.1}"));
    for r in rows {
        let last = r.runs.last().cloned().unwrap_or_default();
        s += &format!(
            "| {} m | {} | {} | {} ({}) | {} / {} | {} | {} | {} |\n",
            r.altitude_m,
            r.radius_label(),
            cell(r.preview_median()),
            cell(r.drawn_exact_median()),
            if r.gated() { format!("{}", exact_budget_ms(r.radius_m, r.altitude_m)) } else { "recorded".to_string() },
            last.refreshed,
            last.resident,
            r.runs.iter().map(|x| x.previewed_nodes_max).max().unwrap_or(0),
            r.runs.len(),
            r.verdict()
        );
    }
    s
}

/// The attribution table (markdown): per case the median over the repeats of every stage, in milliseconds from `set_definition` (the
/// three calls and the frame median are durations).
pub fn stages_table(rows: &[CaseRow]) -> String {
    let mut s = String::from(
        "| altitude | stamp radius | set_definition call | edit call | preview call | plan delivered (worker ms) | R delivered | first job dispatched | half refreshed | all refreshed | uploaded | preview | all exact | frame median | find call | drawn exact | settle ms | coalesced | cancelled | dropped |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let cell = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.1}"));
    for r in rows {
        let pick = |f: &dyn Fn(&CaseRun) -> Option<f64>| median(r.runs.iter().filter_map(f).collect());
        let (plan, worker) = (pick(&|x| x.stages.plan_delivered_ms), pick(&|x| x.stages.plan_worker_ms));
        s += &format!(
            "| {} m | {} | {} | {} | {} | {} ({}) | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.altitude_m,
            r.radius_label(),
            cell(pick(&|x| Some(x.stages.set_definition_ms))),
            cell(pick(&|x| Some(x.stages.edit_ms))),
            cell(pick(&|x| Some(x.stages.preview_call_ms))),
            cell(plan),
            cell(worker),
            cell(pick(&|x| x.stages.residency_delivered_ms)),
            cell(pick(&|x| x.stages.first_dispatch_ms)),
            cell(pick(&|x| x.stages.half_refreshed_ms)),
            cell(pick(&|x| x.stages.all_refreshed_ms)),
            cell(pick(&|x| x.stages.uploaded_ms)),
            cell(r.preview_median()),
            cell(r.exact_median()),
            cell(pick(&|x| Some(x.stages.frame_median_ms))),
            cell(pick(&|x| Some(x.stages.find_ms))),
            cell(pick(&|x| x.stages.drawn_exact_ms)),
            pick(&|x| Some(x.stages.settle_ms as f64)).map_or("-".to_string(), |v| format!("{v:.0}")),
            pick(&|x| Some(x.stages.coalesced as f64)).map_or("-".to_string(), |v| format!("{v:.0}")),
            pick(&|x| Some(x.stages.cancelled as f64)).map_or("-".to_string(), |v| format!("{v:.0}")),
            pick(&|x| Some(x.stages.dropped as f64)).map_or("-".to_string(), |v| format!("{v:.0}")),
        );
    }
    s
}

struct Harness<'a> {
    ctx: &'a GpuContext,
    rig: Rig,
    iteration: u64,
    /// The tiles the last frame's plan drew.
    drawn: HashSet<TileId>,
    /// How long a settle may take before it is an error (a smoke run is short, so a hang names itself instead of waiting for the runner).
    settle_limit: Duration,
    /// The refresh settle window of the run (ADR 0072), for the report.
    settle_ms: u64,
    /// The world with a chunk stage the frames drive (`chunk_frame`, the landings into the managed loop), as the viewer does.
    chunk_world: Option<Arc<WorldTerrain>>,
    /// Whether the stale chunks wait for the edit's tiles (`ChunkManager::hold_stale`, the viewer's behaviour); `--no-stale-hold` runs without.
    /// The features of the chunk step this scene runs (`--no-stale-hold`, `--no-horizon-jobs` turn one off and the log says so) and what it counted.
    features: crate::chunkstage::FrameFeatures,
    counters: crate::chunkstage::FeatureCounters,
    /// The chunks that landed in the frames since the log was last taken, with the instant, and the time of the slowest `chunk_frame` call.
    landed_log: Vec<(Instant, planet_generators::chunkfield::ChunkKey)>,
    chunk_frame_max_ms: f64,
    /// The world whose horizon jobs run without a chunk stage (the default since the owner's fallback, STRM-064), and the maps that landed since the log was cleared.
    horizon_world: Option<Arc<WorldTerrain>>,
    horizon_landed: Vec<(Instant, planet_generators::chunkfield::ChunkKey)>,
    /// The chunks the horizon jobs were asked for in the last frame (the same tiles and truncation as the jobs see).
    horizon_wanted: Vec<(planet_generators::chunkfield::ChunkKey, f64)>,
}

impl Harness<'_> {
    /// One frame of the loop: the managed frame, the GPU wait at the frames-in-flight bound, the submit.
    fn step(&mut self, req: &TerrainRequest) -> Result<planet_frame::managed::ManagedFrame, String> {
        self.iteration += 1;
        let ctx = self.ctx;
        let rig = &mut self.rig;
        let frame = rig.managed.begin_frame_strict(req, &mut RendererSink { ctx, renderer: &mut rig.renderer })?;
        if let Some(world) = &self.chunk_world {
            // The chunks the plan's tiles read: the version of every wanted chunk is asked, the stale and the missing ones are solved, the
            // landed ones refresh the tiles that read them (the viewer's frame, STRM-040, STRM-056).
            let tiles: Vec<TileId> = frame.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
            let call = Instant::now();
            let update =
                crate::chunkstage::frame_step(world, &mut rig.managed, &tiles, req.camera.position, &self.features, &mut self.counters)
                    .update;
            self.chunk_frame_max_ms = self.chunk_frame_max_ms.max(call.elapsed().as_secs_f64() * 1000.0);
            let now = Instant::now();
            self.landed_log.extend(update.landed.iter().map(|k| (now, *k)));
        } else if let Some(world) = &self.horizon_world {
            let tiles: Vec<TileId> = frame.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
            self.horizon_wanted = world.horizon_wanted(&tiles, req.camera.position);
            let update = world.horizon_frame(&tiles, req.camera.position);
            let now = Instant::now();
            self.horizon_landed.extend(update.landed.iter().map(|k| (now, *k)));
        }
        rig.fi.retire_to(rig.renderer.retired_frames(ctx));
        while rig.fi.is_full() {
            rig.fi.note_wait();
            rig.fi.retire_to(rig.renderer.wait_oldest(ctx));
        }
        self.drawn = frame.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
        rig.renderer.set_preview_of(&frame.pipeline.draw);
        rig.renderer.submit(ctx, &frame.pipeline.draw.plan).map_err(|e| format!("frame {}: {e}", self.iteration))?;
        rig.fi.submit();
        Ok(frame)
    }

    /// Whether nothing is left to do: the plan is the exact one, nothing generates, nothing waits for an upload.
    fn idle(&self, frame: &planet_frame::managed::ManagedFrame) -> bool {
        crate::editwin::exact_and_idle(&self.rig.managed, frame)
    }

    /// Frames with the camera still until the chunks the view wants are all landed and nothing is in flight for `calm` frames, then the
    /// tiles that read them are drawn and idle (a no-op without a chunk stage).
    fn settle_chunks(&mut self, req: &TerrainRequest, calm: usize) -> Result<(), String> {
        let Some(manager) = self.chunk_world.as_ref().and_then(|w| w.chunk_manager()) else { return Ok(()) };
        let started = Instant::now();
        let mut quiet = 0;
        loop {
            let frame = self.step(req)?;
            let busy = manager.in_flight() > 0 || !self.idle(&frame);
            quiet = if busy { 0 } else { quiet + 1 };
            if quiet >= calm {
                self.landed_log.clear();
                return Ok(());
            }
            if started.elapsed() > Duration::from_secs(600) {
                return Err("the chunks did not settle in 600 s".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Frames with the camera still until `calm` consecutive idle frames.
    fn settle(&mut self, req: &TerrainRequest, calm: usize) -> Result<f64, String> {
        let limit = self.settle_limit;
        let started = Instant::now();
        let mut quiet = 0;
        loop {
            let frame = self.step(req)?;
            quiet = if self.idle(&frame) { quiet + 1 } else { 0 };
            if quiet >= calm {
                return Ok(started.elapsed().as_secs_f64());
            }
            if started.elapsed() > limit {
                return Err(format!(
                    "did not settle in {} s after {} frames: kind {:?}, substituted {}, blank {}, unfinished jobs {}, plan worker busy {}, pending {}",
                    limit.as_secs(),
                    self.iteration,
                    frame.pipeline.draw.kind,
                    frame.pipeline.draw.substituted,
                    frame.pipeline.draw.blank,
                    self.rig.managed.manager().streamer().unfinished_jobs(),
                    self.rig.managed.pipeline().worker().busy(),
                    self.rig.managed.pending().len()
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// The resident tiles the stamp's reach touches: the edit's find as the editor makes it (`planet_editor::feed::edited_tiles`), what
/// `ManagedLoop::edit` is given.
fn found_tiles(stamp: &ItemSpec, resident: &[TileId], radius_m: f64) -> Vec<TileId> {
    let affected = [planet_editor::commands::Affected {
        body: BodyId::PLANET,
        dir: stamp.dir,
        reach_m: stamp.reach_m,
        brush: stamp.stamp.clone().map(std::sync::Arc::new),
    }];
    planet_editor::feed::edited_tiles(&affected, BodyId::PLANET, resident, radius_m, &TangentWarp)
}

/// Items of `layer` near the stamp, for the preview delta (the whole body would be the same list: only stamps that differ count).
fn items_near(def: &WorldDefinition, layer: &str, at: Vec3) -> Vec<planet_world_def::footprint::FootprintItem> {
    def.footprint_items_cap(BodyId::PLANET, layer, Cap { centre: at, angle: core::f64::consts::PI }).unwrap_or_default()
}

/// The preview stamps the editor sets for the edit of `stamp` from `before` to `edited`, seen from `camera` on a body of `planet_radius_m`:
/// the footprint delta with the brushes (VIEW-036), so a brush stroke is previewed as its chain of stamps as in the editor.
fn case_preview(
    before: &WorldDefinition,
    edited: &WorldDefinition,
    stamp: &ItemSpec,
    camera: Vec3,
    planet_radius_m: f64,
) -> Vec<PreviewStamp> {
    preview_delta_on(&items_near(before, &stamp.layer, stamp.dir), &items_near(edited, &stamp.layer, stamp.dir), camera, planet_radius_m)
        .stamps
}

/// Writes the report as `edit-latency.md` under `out` (or at `out` when it names a `.json` file), creating the directory that is missing.
fn write_report(out: &std::path::Path, text: &str) -> Result<std::path::PathBuf, String> {
    let path = crate::streamperf::out_log_path(out, "edit-latency.md")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Tell the plan cache where the items changed from `old` to the world's definition now, as the viewer does after a swap (STRM-034), so the plan
/// after the edit keeps what the change cannot reach; `old_revision` is the item revision of the field before the swap.
fn announce_item_change(h: &mut Harness<'_>, world: &Arc<WorldTerrain>, old: &planet_world_def::WorldDefinition, old_revision: u64) {
    crate::chunkstage::announce_item_change(&mut h.rig.managed, world, old, old_revision, &mut h.counters);
}

/// One run of a case on a settled harness: edit, time the preview and the exact frame, take the edit back and settle.
fn run_once(
    h: &mut Harness<'_>,
    world: &Arc<WorldTerrain>,
    req: &TerrainRequest,
    stamp: &ItemSpec,
    limit: Duration,
) -> Result<CaseRun, String> {
    let before_def = world.definition();
    let mut edited = (*before_def).clone();
    edited.put_item_spec(BodyId::PLANET, stamp.clone()).map_err(|e| format!("the case's stamp: {e:?}"))?;
    let edited = Arc::new(edited);
    let mut out = CaseRun { resident: h.rig.managed.resident().len(), ..Default::default() };
    let before_stats = h.rig.managed.manager().streamer().stats();
    let wanted_keys: Vec<planet_generators::chunkfield::ChunkKey> = h.horizon_wanted.iter().map(|w| w.0).collect();
    let wanted_dist: Vec<f64> = h.horizon_wanted.iter().map(|w| w.1).collect();
    let words_before: Vec<u64> = wanted_keys.iter().map(|k| world.horizon_edit_word(*k)).collect();
    let old_revision = world.field()?.item_revision();
    let sampler_started = Instant::now();
    let job_log = h.rig.managed.manager().streamer().job_log();
    let (jobs_before, t0_us) = (job_log.len(), job_log.now_us());
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    // The edit as the editor does it: swap the definition, name the edited tiles, set the preview.
    let call = Instant::now();
    world.set_definition(Arc::clone(&edited))?;
    out.stages.set_definition_ms = ms(call);
    let call = Instant::now();
    announce_item_change(h, world, &before_def, old_revision);
    let resident: Vec<TileId> = h.rig.managed.resident().keys().copied().collect();
    let found = Instant::now();
    let tiles = found_tiles(stamp, &resident, world.radius_m());
    out.stages.find_ms = ms(found);
    out.levels = level_rows(&tiles, &h.drawn, &req.lod, stamp.max_delta_m);
    let drawn_before: HashSet<TileId> = h.drawn.clone();
    out.refreshed = h.rig.managed.edit(&tiles);
    out.stages.edit_ms = ms(call);
    let call = Instant::now();
    h.rig.managed.set_preview(case_preview(&before_def, &edited, stamp, req.camera.position.0, world.radius_m()));
    out.stages.preview_call_ms = ms(call);
    let mut waiting_for_exact = false;
    // The tiles the plan draws at the frame that first delivers a plan after the edit (the plan can change with the edit).
    let mut drawn_shown: HashSet<TileId> = HashSet::new();
    let mut frame_ms: Vec<f64> = Vec::new();
    h.landed_log.clear();
    h.chunk_frame_max_ms = 0.0;
    h.horizon_landed.clear();
    loop {
        let frame_started = Instant::now();
        let frame = h.step(req)?;
        frame_ms.push(ms(frame_started));
        out.frames += 1;
        let at = ms(sampler_started);
        let stats = h.rig.managed.manager().streamer().stats();
        if out.stages.plan_delivered_ms.is_none() {
            if let Some((_, worker_ms)) = frame.pipeline.delivered {
                out.stages.plan_delivered_ms = Some(at);
                out.stages.plan_worker_ms = Some(worker_ms);
                drawn_shown = h.drawn.clone();
            }
        }
        if out.stages.residency_delivered_ms.is_none() && frame.pipeline.residency_delivered.is_some() {
            out.stages.residency_delivered_ms = Some(at);
        }
        if out.stages.first_dispatch_ms.is_none() && stats.dispatched > before_stats.dispatched {
            out.stages.first_dispatch_ms = Some(at);
        }
        let refreshed = stats.refreshed - before_stats.refreshed;
        if out.trace.len() < 400 && at < 3000.0 {
            out.trace.push([at, ms(frame_started), refreshed as f64, stats.in_flight as f64, h.rig.managed.pending().len() as f64]);
        }
        if out.stages.half_refreshed_ms.is_none() && out.refreshed > 0 && refreshed * 2 >= out.refreshed as u64 {
            out.stages.half_refreshed_ms = Some(at);
        }
        if out.stages.all_refreshed_ms.is_none() && out.refreshed > 0 && refreshed >= out.refreshed as u64 {
            out.stages.all_refreshed_ms = Some(at);
        }
        if out.stages.uploaded_ms.is_none()
            && out.stages.first_dispatch_ms.is_some()
            && h.rig.managed.pending().is_empty()
            && stats.in_flight == 0
        {
            out.stages.uploaded_ms = Some(at);
        }
        if out.stages.drawn_exact_ms.is_none() && out.frames > 1 && frame.pipeline.draw.kind == planet_frame::pipeline::DrawKind::Exact {
            out.stages.drawn_exact_ms = Some(at);
        }
        let previewed = frame.pipeline.draw.plan.nodes.iter().filter(|n| n.preview).count();
        out.previewed_nodes_max = out.previewed_nodes_max.max(previewed);
        if out.preview_ms.is_none() && previewed > 0 {
            // The frame is submitted: wait for the GPU to finish it, then stop the clock.
            h.rig.fi.retire_to(h.rig.renderer.wait_oldest(h.ctx));
            out.preview_ms = Some(sampler_started.elapsed().as_secs_f64() * 1000.0);
        }
        if h.idle(&frame) && (out.frames > 1 || waiting_for_exact) {
            h.rig.fi.retire_to(h.rig.renderer.wait_oldest(h.ctx));
            out.exact_ms = Some(sampler_started.elapsed().as_secs_f64() * 1000.0);
            out.stages.frame_median_ms = median(frame_ms.clone()).unwrap_or(0.0);
            let end = h.rig.managed.manager().streamer().stats();
            out.stages.settle_ms = h.settle_ms;
            out.stages.coalesced = end.refresh_coalesced - before_stats.refresh_coalesced;
            out.stages.cancelled = end.refresh_cancelled - before_stats.refresh_cancelled;
            out.stages.dropped = end.refresh_dropped - before_stats.refresh_dropped;
            break;
        }
        waiting_for_exact = true;
        if sampler_started.elapsed() > limit {
            return Err(format!(
                "the edit did not become exact in {} s: kind {:?}, substituted {}, blank {}, previewed {}, unfinished jobs {}, plan worker busy {}, pending {}, refreshed {} of {} resident",
                limit.as_secs(),
                frame.pipeline.draw.kind,
                frame.pipeline.draw.substituted,
                frame.pipeline.draw.blank,
                previewed,
                h.rig.managed.manager().streamer().unfinished_jobs(),
                h.rig.managed.pipeline().worker().busy(),
                h.rig.managed.pending().len(),
                out.refreshed,
                out.resident
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // The first wave only: the jobs that ended within 2.5 s of the edit (later ones follow the chunks that land, the table below).
    let mut jobs: Vec<_> =
        job_log.since(jobs_before).into_iter().filter(|j| j.refresh && j.ended_us.saturating_sub(t0_us) <= 2_500_000).collect();
    jobs.sort_by_key(|j| j.ended_us);
    let from = jobs.len().saturating_sub(10);
    out.stragglers = jobs[from..]
        .iter()
        .map(|j| {
            format!(
                "{:?} level {}: queued {} ms, worker wait {:.1} ms, ran {:.1} ms, ended {:.0} ms, {} loads beside it",
                (j.tile.face().0, j.tile.x(), j.tile.y()),
                j.level,
                j.queue_ms,
                j.wait_us as f64 / 1000.0,
                j.run_us as f64 / 1000.0,
                j.ended_us.saturating_sub(t0_us) as f64 / 1000.0,
                j.running
            )
        })
        .collect();
    // Of the tiles the edit named (a drawn tile it did not name refreshes only when a chunk lands: the chunk table below).
    let named: HashSet<TileId> = tiles.iter().copied().collect();
    let drawn: HashSet<TileId> = drawn_before.union(&drawn_shown).copied().filter(|t| named.contains(t)).collect();
    let drawn_refreshed_ms = drawn_refreshed_ms(&job_log.since(jobs_before), &drawn, t0_us, out.stages.frame_median_ms);
    // With a chunk stage the plan is not exact until the stale chunks have landed (the tiles' keys follow the chunks), so the frame DrawKind::Exact
    // waits for the eroded result: the M6 budget reads the instant the last named tile the plan draws ended its first refresh job (TEST-056,
    // ADR 0072 point 1), and the chunk table below carries the eroded result.
    if h.chunk_world.is_some() {
        out.stages.drawn_exact_ms = drawn_refreshed_ms.or(out.stages.uploaded_ms);
    }
    // The chunk side: the chunks the edit made stale are solved again while the frames go on; the eroded result is on the screen when the
    // tiles that read the last of them are drawn (M8-7, criterion 6).
    if let Some(manager) = h.chunk_world.as_ref().and_then(|w| w.chunk_manager()) {
        let first_frame_ms = h.chunk_frame_max_ms;
        let hz0 = h.chunk_world.as_ref().and_then(|w| w.horizon_stats()).unwrap_or_default();
        let mut hz_last: Option<f64> = None;
        let limit = Duration::from_secs(120);
        let mut quiet = 0;
        let mut drawn_after_last: Option<f64> = None;
        loop {
            let frame = h.step(req)?;
            let busy = manager.in_flight() > 0 || !h.idle(&frame);
            if h.chunk_world.as_ref().is_some_and(|w| w.horizon_in_flight() > 0) {
                hz_last = Some(ms(sampler_started));
            }
            if !busy {
                quiet += 1;
                if drawn_after_last.is_none() {
                    drawn_after_last = Some(ms(sampler_started));
                }
            } else {
                quiet = 0;
                drawn_after_last = None;
            }
            if quiet >= 3 || sampler_started.elapsed() > limit + Duration::from_secs(60) {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let hz1 = h.chunk_world.as_ref().and_then(|w| w.horizon_stats()).unwrap_or_default();
        let centre = world_centre_chunk(stamp);
        let at = |t: Instant| t.duration_since(sampler_started).as_secs_f64() * 1000.0;
        out.stages.chunks = Some(ChunkStages {
            first_frame_ms,
            resolved: h.landed_log.len() as u64,
            centre_ms: h.landed_log.iter().find(|l| l.1 == centre).map(|l| at(l.0)),
            last_ms: h.landed_log.last().map(|l| at(l.0)),
            drawn_ms: if h.landed_log.is_empty() {
                None
            } else {
                let last = h.landed_log.last().map(|l| at(l.0)).unwrap_or(0.0);
                let tiles = last_refresh_end_ms(&job_log.since(jobs_before), &h.drawn, t0_us, out.stages.frame_median_ms);
                Some(tiles.map_or(last, |t| t.max(last)))
            },
            settled_ms: if h.landed_log.is_empty() { None } else { drawn_after_last },
            horizon_baked: hz1.baked - hz0.baked,
            horizon_rebaked: hz1.rebaked - hz0.rebaked,
            horizon_last_ms: hz_last,
        });
    }
    // The horizon side without a chunk stage: the maps the edit touched (their edit word changed, the maps of the chunks it reaches and of the neighbours inside
    // the march's halo) are baked again after the settle window; each one's time from the edit to its landing is the shadow of the edit on screen (M8 criterion 6).
    if h.horizon_world.is_some() {
        let touched: Vec<(planet_generators::chunkfield::ChunkKey, f64)> = wanted_keys
            .iter()
            .zip(&words_before)
            .zip(&wanted_dist)
            .filter(|((k, w0), _)| world.horizon_edit_word(**k) != **w0)
            .map(|((k, _), d)| (*k, *d))
            .collect();
        let limit = Duration::from_secs(60);
        loop {
            let all = touched.iter().all(|(k, _)| h.horizon_landed.iter().any(|l| l.1 == *k));
            if all || sampler_started.elapsed() > limit {
                break;
            }
            h.step(req)?;
            std::thread::sleep(Duration::from_millis(1));
        }
        out.stages.horizon = Some(
            touched
                .into_iter()
                .map(|(k, d)| {
                    (k, h.horizon_landed.iter().find(|l| l.1 == k).map(|l| l.0.duration_since(sampler_started).as_secs_f64() * 1000.0), d)
                })
                .collect(),
        );
    }
    // Take the edit back: the old definition, the same tiles refreshed, no preview, settled.
    let edited_revision = world.field()?.item_revision();
    world.set_definition(Arc::clone(&before_def))?;
    announce_item_change(h, world, &edited, edited_revision);
    h.rig.managed.edit(&tiles);
    h.rig.managed.set_preview(Vec::new());
    h.settle(req, 20)?;
    h.settle_chunks(req, 20)?;
    Ok(out)
}

/// The chunk under the centre of the edit (the item's anchor).
fn world_centre_chunk(stamp: &ItemSpec) -> planet_generators::chunkfield::ChunkKey {
    let t = TileId::from_direction(&TangentWarp, stamp.dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
    (t.face().0, t.x(), t.y())
}

/// The table of the chunk side of every case (a run with the chunk stage): per case the medians over its repeats.
pub fn chunks_table(rows: &[CaseRow]) -> String {
    let mut s = String::from(
        "| altitude | stamp radius | first chunk frame ms (versions) | chunks re-solved | centre chunk landed ms | last chunk landed ms | drawn again ms (drawn tiles) | all settled ms | verdict (10 s, drawn tiles) | horizon maps baked (re-baked) | horizon last in flight ms |\n|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let cell = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.1}"));
    for r in rows {
        let pick =
            |f: &dyn Fn(&ChunkStages) -> Option<f64>| median(r.runs.iter().filter_map(|x| x.stages.chunks.as_ref().and_then(f)).collect());
        let drawn = pick(&|c| c.drawn_ms);
        let verdict = match drawn {
            None => "-",
            Some(d) if d <= 10_000.0 => "ok",
            Some(_) => "over",
        };
        s += &format!(
            "| {} m | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.altitude_m,
            r.radius_label(),
            cell(pick(&|c| Some(c.first_frame_ms))),
            cell(pick(&|c| Some(c.resolved as f64))),
            cell(pick(&|c| c.centre_ms)),
            cell(pick(&|c| c.last_ms)),
            cell(drawn),
            cell(pick(&|c| c.settled_ms)),
            verdict,
            cell(pick(&|c| Some(c.horizon_baked as f64))) + &format!(" ({})", cell(pick(&|c| Some(c.horizon_rebaked as f64)))),
            cell(pick(&|c| c.horizon_last_ms))
        );
    }
    s
}

/// The table of the horizon side of every case (a run with the horizon jobs and no chunk stage, M8 criterion 6): per case one row per wanted map the
/// edit touched (the first run of the case), the milliseconds from the edit to the map's landing and the ten-second verdict. The header of the last
/// column is read by `accept M8` (TEST-060).
pub fn horizon_table(rows: &[CaseRow]) -> String {
    let mut s = String::from(
        "| altitude | stamp radius | map (face, x, y) | distance m | edit to map ms | verdict (10 s, horizon maps) |\n|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let maps = r.runs.first().and_then(|x| x.stages.horizon.as_ref());
        match maps.filter(|m| !m.is_empty()) {
            None => s += &format!("| {} m | {} | - | - | - | - |\n", r.altitude_m, r.radius_label()),
            Some(maps) => {
                for (k, t, dist) in maps {
                    let (ms, verdict) = match t {
                        None => ("-".to_string(), "over"),
                        Some(t) if *t <= 10_000.0 => (format!("{t:.1}"), "ok"),
                        Some(t) => (format!("{t:.1}"), "over"),
                    };
                    s += &format!("| {} m | {} | {:?} | {dist:.0} | {ms} | {verdict} |\n", r.altitude_m, r.radius_label(), k);
                }
            }
        }
    }
    s
}

/// One case of the scene: what is edited, how big it is (the radius, the length or the side), where the camera looks, and the item that
/// is in the world before the edit, if any.
pub struct Case {
    pub kind: CaseKind,
    pub name: String,
    pub size_m: f64,
    pub spec: ItemSpec,
    pub centre: Vec3,
    pub base: Option<ItemSpec>,
}

/// The cases of an altitude, in the order they run (a smoke run has one altitude, 200 m, the small hill, the brush and the region only): the hill
/// stamps, the regional stamps (TEST-044), the brush strokes (TEST-042), then the ridge and the region (TEST-044).
pub fn cases_at(altitude_m: f64, smoke: bool, planet_radius_m: f64) -> Vec<Case> {
    let stamp = |r: f64| Case {
        kind: CaseKind::Stamp,
        name: format!("stamp radius {r} m"),
        size_m: r,
        spec: case_stamp(r),
        centre: case_centre(),
        base: None,
    };
    let mut v = Vec::new();
    let hill_radii: Vec<f64> = if smoke {
        vec![RADII_M[0]]
    } else if ALTITUDES_M.contains(&altitude_m) {
        RADII_M.to_vec()
    } else {
        Vec::new()
    };
    v.extend(hill_radii.into_iter().map(stamp));
    // A smoke run leaves out the regional stamps and the ridge (5 km and more: the most tiles to generate and draw on the software adapter);
    // their construction is tested at tier A and the measured scene runs them.
    let regional: Vec<f64> = if smoke {
        Vec::new()
    } else if REGIONAL_ALTITUDES_M.contains(&altitude_m) {
        REGIONAL_RADII_M.to_vec()
    } else {
        Vec::new()
    };
    v.extend(regional.into_iter().map(stamp));
    if BRUSH_ALTITUDES_M.contains(&altitude_m) || smoke {
        let brush_radii: Vec<f64> = if smoke { vec![BRUSH_RADII_M[0]] } else { BRUSH_RADII_M.to_vec() };
        for r in brush_radii {
            let (spec, centre) = case_brush(r, planet_radius_m);
            v.push(Case { kind: CaseKind::Brush, name: format!("brush stroke radius {r} m"), size_m: r, spec, centre, base: None });
        }
    }
    if LINE_ALTITUDES_M.contains(&altitude_m) || smoke {
        if !smoke {
            let (spec, centre) = case_ridge(planet_radius_m);
            v.push(Case {
                kind: CaseKind::Line,
                name: format!("ridge of {LINE_LENGTH_M} m"),
                size_m: LINE_LENGTH_M,
                spec,
                centre,
                base: None,
            });
        }
        let (base, _) = case_region(REGION_TYPE_FROM, planet_radius_m);
        let (spec, centre) = case_region(REGION_TYPE_TO, planet_radius_m);
        v.push(Case {
            kind: CaseKind::Region,
            name: format!("region of {REGION_SIDE_M} m, surface type changed"),
            size_m: REGION_SIDE_M,
            spec,
            centre,
            base: Some(base),
        });
    }
    v
}

/// The scene: `test-render --perf --scene edit-latency [--store DIR]`, or `--perf-smoke` for a run of the path.
pub fn perf_edit_latency(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    let smoke = o.perf_smoke;
    crate::streamperf::check_options(o)?;
    let opened = Instant::now();
    // The editor's world is a plain `WorldTerrain`: a stage with an empty footprint passes its key through (GEN-027, ADR 0068).
    let world = Arc::new(crate::streamperf::open_world(o, false)?);
    writeln!(out, "world opened in {:.1} s", opened.elapsed().as_secs_f64()).map_err(err)?;
    let radius_m = world.radius_m();
    // SPIKE-29: `PLANET_EDIT_ITEMS=DIR` loads an item set (the edit-scale scene's `target/edit-scale/n<count>`) into the definition before the cases run,
    // so the latency of one edit is measured in a world with authored edits (the cluster of the set is centred on the place the cases edit).
    if let Some(dir) = std::env::var_os("PLANET_EDIT_ITEMS") {
        let dir = std::path::PathBuf::from(dir);
        let ids = planet_editor::ids::IdMaker::new(
            Box::new(planet_editor::ids::FixedClock(1_760_000_000_000)),
            Box::new(planet_editor::ids::SeededEntropy(9)),
        );
        let t = Instant::now();
        let rt = crate::editwin::EditRuntime::with_ids(
            (*world.definition()).clone(),
            radius_m,
            Some(&dir),
            planet_editor::layout::Layout::default(),
            ids,
        )?;
        world.set_definition(Arc::new(rt.session.editor().definition().clone()))?;
        writeln!(out, "items loaded from {}: {} in {:.1} s", dir.display(), rt.session.editor().items().count(), t.elapsed().as_secs_f64())
            .map_err(err)?;
    }
    if !smoke {
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
        crate::readiness_gate(o, ctx, out)?;
    }
    // The chunk stage on the M8 default (Under, versions, stale marking): with a store and not smoke, unless `--no-chunks` asks for the comparison.
    let with_chunks = !smoke && o.chunks && o.store.is_some() && !o.allow_memory_bake;
    // Without it (the default since the owner's fallback of 2026-10-28) the horizon jobs bake the maps from the base terrain (STRM-064, TEST-065).
    let with_horizon = !smoke && !with_chunks && !o.no_horizon_jobs && o.store.is_some() && !o.allow_memory_bake;
    if with_chunks {
        // The viewer's own pools and chunk configuration, with its horizon jobs (the shared stage of `anetworld::viewer_chunk_stage`), no disk cache.
        if let Some(n) = o.base_threads {
            world.set_base_threads(n);
        }
        if let Some(n) = o.horizon_threads {
            world.set_horizon_threads(n);
        }
        crate::chunkstage::enable_viewer_stage_sized(&world, o.gen_workers(), o.chunk_workers, !o.no_horizon_jobs, false, out)?;
    } else if with_horizon {
        if let Some(n) = o.horizon_threads {
            world.set_horizon_threads(n);
        }
        world.enable_viewer_horizon(o.gen_workers(), Arc::new(planet_streaming::SystemClock::new()))?;
    }
    let size = if smoke { (768u32, 432u32) } else { (2560u32, 1440u32) };
    let tau = o.tau;
    let field = world.field()?;
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level: o.finest_level_for(o.cells),
        min_level: o.min_level(),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m() + 200.0),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, tau))?;
    let aspect = f64::from(size.0) / f64::from(size.1);
    let seed = field.seed();
    let altitudes: Vec<f64> = if smoke {
        vec![200.0]
    } else {
        let mut v = ALTITUDES_M.to_vec();
        v.extend(REGIONAL_ALTITUDES_M.iter().filter(|a| !v.contains(a)).copied().collect::<Vec<_>>());
        v
    };
    let repeats = if smoke { 1 } else { o.repeats.unwrap_or(REPEATS).max(1) };
    let limit = Duration::from_secs(if smoke { 40 } else { 600 });
    if smoke {
        writeln!(
            out,
            "SMOKE RUN: exercises the edit path only (one altitude, one radius, 768x432, no gate); the numbers are not a measurement"
        )
        .map_err(err)?;
    }
    writeln!(
        out,
        "edit-latency: {}x{}, cells {}, tau {} px, finest level {}, {}",
        size.0,
        size.1,
        o.cells,
        tau,
        lod.max_level,
        world.describe()
    )
    .map_err(err)?;
    let ground = |d: Vec3| field.height(d);
    let req_of = |camera: Camera| TerrainRequest {
        camera,
        size,
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed,
    };
    let mut rows = Vec::new();
    let mut costs: Vec<PreviewCost> = Vec::new();
    for &altitude in &altitudes {
        let cases = cases_at(altitude, smoke, radius_m);
        if cases.is_empty() {
            continue;
        }
        let mut focus: Option<(TerrainRequest, Vec3)> = None;
        // One rig per altitude, built for the first case's camera and settled from empty; every case then edits and reverts on it.
        let first = &cases[0];
        let rho = (1.5 * first.size_m).max(altitude);
        let first_req = req_of(camera_for(first.centre, &ground, radius_m, rho, altitude, aspect));
        let mut rig = Rig::new(ctx, &world, &first_req, o.gen_workers())?;
        if with_chunks {
            rig.managed.set_chunk_view(world.chunk_view());
        }
        // The editor's refresh settle window, or the one the run names (ADR 0072): a run at 0 and a run at 40 show its share of a case.
        rig.managed.set_refresh_settle(o.settle_ms);
        let mut h = Harness {
            ctx,
            rig,
            iteration: 0,
            drawn: HashSet::new(),
            settle_limit: if smoke { Duration::from_secs(120) } else { SETTLE_LIMIT },
            settle_ms: o.settle_ms,
            chunk_world: with_chunks.then(|| Arc::clone(&world)),
            features: crate::chunkstage::FrameFeatures { stale_hold: !o.no_stale_hold, ..Default::default() },
            counters: Default::default(),
            landed_log: Vec::new(),
            chunk_frame_max_ms: 0.0,
            horizon_world: with_horizon.then(|| Arc::clone(&world)),
            horizon_landed: Vec::new(),
            horizon_wanted: Vec::new(),
        };
        for case in &cases {
            let rho = (1.5 * case.size_m).max(altitude);
            let req = req_of(camera_for(case.centre, &ground, radius_m, rho, altitude, aspect));
            // A case that edits an item already in the world (the region's surface type) puts the earlier state in first.
            let original = world.definition();
            if let Some(base) = &case.base {
                let mut with_base = (*original).clone();
                with_base.put_item_spec(BodyId::PLANET, base.clone()).map_err(|e| format!("the case's earlier state: {e:?}"))?;
                world.set_definition(Arc::new(with_base))?;
                // The tiles the earlier state touches are refreshed, as an edit's are, so that the frames settle on the world as it is.
                let resident: Vec<TileId> = h.rig.managed.resident().keys().copied().collect();
                h.rig.managed.edit(&found_tiles(base, &resident, radius_m));
            }
            // A smoke run needs the frames to be exact once, not a long calm: every frame is slow on the software adapter.
            let settle = h.settle(&req, if smoke { 3 } else { 20 })?;
            h.settle_chunks(&req, if smoke { 3 } else { 20 })?;
            writeln!(out, "  altitude {altitude} m, {}: settled in {settle:.1} s", case.name).map_err(err)?;
            let mut runs = Vec::new();
            for _ in 0..repeats {
                runs.push(run_once(&mut h, &world, &req, &case.spec, limit)?);
            }
            world.set_definition(original)?;
            if let Some(base) = &case.base {
                let resident: Vec<TileId> = h.rig.managed.resident().keys().copied().collect();
                h.rig.managed.edit(&found_tiles(base, &resident, radius_m));
            }
            rows.push(CaseRow { kind: case.kind, altitude_m: altitude, radius_m: case.size_m, runs });
            focus = Some((req, case.centre));
        }
        // The cost of the preview with the cap of active stamps (TEST-043), on the last view of the altitude.
        if let Some((req, at)) = focus {
            costs.extend(measure_preview_cost(&mut h, &req, at, radius_m, altitude, if smoke { 1 } else { 20 }, smoke)?);
        }
        // What the chunk step counted at this altitude; the edits announce their item changes and hold the stale chunks, and the camera does not
        // travel, so the prefetch is on but not expected to ask (TEST-058).
        if with_chunks {
            writeln!(out, "  altitude {altitude} m, chunk step: {}", h.counters.line(&h.features)).map_err(err)?;
            let expect = crate::chunkstage::Expect { horizon: true, stale_hold: true, announcements: true, ..Default::default() };
            h.counters.check(&h.features, &expect)?;
        }
    }
    let table = table(&rows);
    let stages = stages_table(&rows);
    let levels = levels_table(&rows);
    let chunks = if with_chunks {
        format!("\nthe chunk side of every case (ms from set_definition; the chunk stage on the M8 default, the first frame is a duration):\n\n{}", chunks_table(&rows))
    } else {
        String::from("\n(no chunk stage in this run: the comparison line, or a smoke or memory-bake run)\n")
    };
    let horizon = if with_horizon {
        format!("
the horizon side of every case (ms from set_definition to the landing of each map the edit touched; the maps baked from the base terrain, no chunk stage). On screen is landing plus the upload: the window uploads one landed map per frame (`HorizonRig::decide`, horizonworld.rs), so a map is drawn within one frame of landing when it is the only one pending, and the window gate logs the frame a re-baked map is first drawn:

{}", horizon_table(&rows))
    } else {
        String::new()
    };
    writeln!(out, "{chunks}").map_err(err)?;
    writeln!(out, "{horizon}").map_err(err)?;
    writeln!(out, "\n{table}\nstages (ms from set_definition; the calls and the frame median are durations):\n\n{stages}").map_err(err)?;
    writeln!(out, "\nthe refreshed set by level (first run of each case):\n\n{levels}").map_err(err)?;
    writeln!(
        out,
        "
the refresh frame by frame (first run of each case; rows where a count moved or a frame took over 30 ms):

{}",
        trace_table(&rows)
    )
    .map_err(err)?;
    let cost = preview_cost_table(&costs);
    writeln!(
        out,
        "\nthe cost of the preview with 0, 8 and {} active stamps on every flagged node:\n\n{cost}",
        planet_frame::preview::MAX_PREVIEW_STAMPS
    )
    .map_err(err)?;
    if let Some(dir) = &o.out {
        let path = write_report(dir, &format!("{table}\n{stages}\n{levels}\n{chunks}\n{horizon}\n{cost}"))?;
        writeln!(out, "table written to {}", path.display()).map_err(err)?;
    }
    if smoke && rows.iter().any(|r| r.runs.iter().any(|x| x.exact_ms.is_none())) {
        return Err("the smoke run's edit never became exact".into());
    }
    Ok(())
}

/// The first run of every case frame by frame (the tail of the refresh): ms from `set_definition`, the frame's own ms, tiles refreshed so far, jobs
/// in flight, tiles waiting for an upload; rows where the refreshed count or the in-flight count moved, plus the slow frames.
pub fn trace_table(rows: &[CaseRow]) -> String {
    let mut s = String::new();
    for r in rows {
        let Some(run) = r.runs.first().filter(|x| !x.trace.is_empty()) else { continue };
        s += &format!(
            "{} m, {}: {} refreshed of {}\n\n| at | frame | refreshed | in flight | pending |\n|---|---|---|---|---|\n",
            r.altitude_m,
            r.radius_label(),
            run.refreshed,
            run.resident
        );
        let mut last = (f64::NAN, f64::NAN);
        for t in &run.trace {
            if t[2] != last.0 || t[3] != last.1 || t[1] > 30.0 {
                s += &format!("| {:.0} | {:.1} | {:.0} | {:.0} | {:.0} |\n", t[0], t[1], t[2], t[3], t[4]);
            }
            last = (t[2], t[3]);
        }
        s.push('\n');
        if !run.stragglers.is_empty() {
            s += "the last refresh jobs of the case, by their end (face, x, y):\n";
            for j in &run.stragglers {
                s += &format!("- {j}\n");
            }
        }
        s.push('\n');
    }
    s
}

/// The instant, in ms from `t0_us` of the job log's clock, at which the last refresh job of a tile of `drawn` ended, plus `upload_ms` for the frame
/// that uploads it; `None` when no tile of `drawn` was rebuilt.
pub fn last_refresh_end_ms(jobs: &[planet_streaming::JobSample], drawn: &HashSet<TileId>, t0_us: u64, upload_ms: f64) -> Option<f64> {
    jobs.iter()
        .filter(|j| j.refresh && drawn.contains(&j.tile))
        .map(|j| j.ended_us)
        .max()
        .map(|e| e.saturating_sub(t0_us) as f64 / 1000.0 + upload_ms)
}

/// The instant, in ms from `t0_us` of the job log's clock, at which every tile of `drawn` that a refresh job rebuilt had its first refresh job ended
/// (ADR 0072 point 1: "exact" is the tiles the plan draws), plus `upload_ms` for the frame that uploads the last; `None` when no drawn tile was rebuilt.
pub fn drawn_refreshed_ms(jobs: &[planet_streaming::JobSample], drawn: &HashSet<TileId>, t0_us: u64, upload_ms: f64) -> Option<f64> {
    let mut first_end: std::collections::HashMap<TileId, u64> = std::collections::HashMap::new();
    for j in jobs.iter().filter(|j| j.refresh && drawn.contains(&j.tile)) {
        let e = first_end.entry(j.tile).or_insert(j.ended_us);
        *e = (*e).min(j.ended_us);
    }
    first_end.values().max().map(|&e| e.saturating_sub(t0_us) as f64 / 1000.0 + upload_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;

    // spec: TEST-042
    #[test]
    fn the_brush_case_is_one_raise_stroke_of_twenty_points_a_quarter_radius_apart_that_the_definition_accepts() {
        for radius in BRUSH_RADII_M {
            let (spec, centre) = case_brush(radius, R);
            let stamp = spec.stamp.as_ref().expect("a stamp");
            assert_eq!(stamp.kind, StampKind::Brush);
            let strokes = stamp.strokes();
            assert_eq!(strokes.len(), 1);
            assert_eq!((strokes[0].kind, strokes[0].radius_m()), (BrushKind::Raise, radius));
            let points = strokes[0].to_stroke().points;
            assert_eq!(points.len(), BRUSH_POINTS);
            for w in points.windows(2) {
                let d = R * planet_frame_angle(w[0], w[1]);
                assert!((d - 0.25 * radius).abs() < 1e-3, "{d} for radius {radius}");
            }
            assert!(R * planet_frame_angle(points[0], points[BRUSH_POINTS - 1]) + radius <= spec.reach_m, "the reach holds the stroke");
            assert_eq!(spec.layer, BRUSH_LAYER);
            assert!(spec.max_delta_m >= strokes[0].strength_m());
            // The stroke is centred on the place the views look at.
            let mid = (points[BRUSH_POINTS / 2 - 1] + points[BRUSH_POINTS / 2]).normalized();
            assert!(R * planet_frame_angle(mid, centre) < 1.0, "centred");
            // The index takes it (the first point is the anchor, the reach and the bound hold).
            let mut def = WorldDefinition::single_planet(R, 7);
            def.put_item_spec(BodyId::PLANET, spec).unwrap_or_else(|e| panic!("radius {radius}: {e:?}"));
        }
    }

    // spec: TEST-056
    #[test]
    fn the_chunks_table_gives_the_medians_of_the_chunk_side_per_case_and_the_ten_second_verdict() {
        let run = |drawn: Option<f64>, resolved: u64| CaseRun {
            stages: Stages {
                chunks: Some(ChunkStages {
                    first_frame_ms: 3.5,
                    resolved,
                    centre_ms: Some(4_800.0),
                    last_ms: Some(8_800.0),
                    drawn_ms: drawn,
                    settled_ms: drawn.map(|d| d + 1_000.0),
                    ..ChunkStages::default()
                }),
                ..Stages::default()
            },
            ..CaseRun::default()
        };
        let rows = vec![
            CaseRow {
                kind: CaseKind::Stamp,
                altitude_m: 200.0,
                radius_m: 2_000.0,
                runs: vec![run(Some(9_000.0), 4), run(Some(9_500.0), 4), run(Some(30_000.0), 4)],
            },
            CaseRow { kind: CaseKind::Stamp, altitude_m: 200.0, radius_m: 5_000.0, runs: vec![run(Some(14_000.0), 4)] },
            CaseRow { kind: CaseKind::Stamp, altitude_m: 2_000.0, radius_m: 50.0, runs: vec![CaseRun::default()] },
        ];
        let t = chunks_table(&rows);
        let lines: Vec<&str> = t.lines().collect();
        assert!(
            lines[0].contains("first chunk frame ms")
                && lines[0].contains("verdict (10 s, drawn tiles)")
                && lines[0].contains("all settled ms"),
            "{t}"
        );
        assert!(lines[2].contains("| 3.5 | 4.0 | 4800.0 | 8800.0 | 9500.0 | 10500.0 | ok |"), "the median of three runs: {}", lines[2]);
        assert!(lines[3].contains("| 14000.0 | 15000.0 | over |"), "{}", lines[3]);
        assert!(lines[4].ends_with("| - | - | - | - | - | - | - | - (-) | - |"), "a case without a chunk stage has no cells: {}", lines[4]);
    }

    // spec: TEST-044
    #[test]
    fn the_ridge_and_the_region_cases_are_items_the_index_accepts_and_the_region_case_changes_only_its_surface_type() {
        let (ridge, centre) = case_ridge(R);
        let stamp = ridge.stamp.as_ref().expect("a stamp");
        assert_eq!((stamp.kind, stamp.points().len()), (StampKind::Ridge, 17));
        let pts = stamp.points();
        let length = R * planet_frame_angle(pts[0], pts[16]);
        assert!((length - LINE_LENGTH_M).abs() < 1.0, "{length}");
        assert!(ridge.reach_m >= length + 200.0, "the reach holds the line and its half-width");
        let mid = (pts[7] + pts[9]).normalized();
        assert!(R * planet_frame_angle(mid, centre) < 1.0, "centred on the place the views look at");
        let (from, c1) = case_region(REGION_TYPE_FROM, R);
        let (to, c2) = case_region(REGION_TYPE_TO, R);
        assert_eq!((c1, from.layer.as_str(), to.layer.as_str()), (c2, "regions", "regions"));
        assert_eq!((from.id, from.dir, from.reach_m), (to.id, to.dir, to.reach_m), "the same region, edited in place");
        assert_ne!(from.content_hash, to.content_hash, "only the surface type differs: the content hash does");
        let (fp, tp) = (from.stamp.as_ref().unwrap(), to.stamp.as_ref().unwrap());
        assert_eq!(
            (fp.kind, fp.points(), fp.params()[0], tp.params()[0]),
            (StampKind::Region, tp.points(), REGION_TYPE_FROM, REGION_TYPE_TO)
        );
        let side = R * planet_frame_angle(fp.points()[0], fp.points()[1]);
        assert!((side - REGION_SIDE_M).abs() < 1.0, "{side}");
        // The index takes them all, the earlier region first and then its edit in place.
        let mut def = WorldDefinition::single_planet(R, 7);
        def.put_item_spec(BodyId::PLANET, ridge).unwrap();
        def.put_item_spec(BodyId::PLANET, from).unwrap();
        def.put_item_spec(BodyId::PLANET, to).unwrap();
        assert_eq!(def.footprint_items_cap(BodyId::PLANET, "regions", Cap { centre: case_centre(), angle: 1.0 }).unwrap().len(), 1);
    }

    // spec: TEST-044
    #[test]
    fn the_cases_of_an_altitude_are_the_hills_the_regional_stamps_the_brush_the_ridge_and_the_region_where_they_are_measured() {
        let names = |alt: f64, smoke: bool| -> Vec<String> { cases_at(alt, smoke, R).into_iter().map(|c| c.name).collect() };
        // 1 m: the hills only. 200 m: hills, brush, ridge and region. 2 km: hills, 5 km and 40 km stamps, brush, ridge, region. 20 km: the regional stamps.
        assert_eq!(names(1.0, false).len(), 3);
        assert_eq!(names(200.0, false).len(), 3 + 2 + 2);
        assert_eq!(names(2_000.0, false).len(), 3 + 2 + 2 + 2);
        assert_eq!(names(20_000.0, false), ["stamp radius 5000 m", "stamp radius 40000 m"]);
        const { assert!(REGIONAL_RADII_M[1] <= planet_world_def::cells::CELL_EDGE_M, "the largest stamp an item can be") };
        // A smoke run: one altitude, the small hill, the brush and the region (the regional stamps and the ridge are measured-only).
        let kinds: Vec<CaseKind> = cases_at(200.0, true, R).iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [CaseKind::Stamp, CaseKind::Brush, CaseKind::Region]);
        // Only the region case has an earlier state.
        assert!(cases_at(2_000.0, false, R).iter().all(|c| c.base.is_some() == (c.kind == CaseKind::Region)));
    }

    // spec: TEST-044
    #[test]
    fn the_regional_stamp_the_ridge_and_the_region_are_recorded_not_gated_and_say_what_they_are() {
        let run =
            CaseRun { levels: vec![LevelRow { level: 12, found: 3, drawn: 2, spacing_m: 1.0, below_spacing: 0 }], ..Default::default() };
        let row = |kind, size| CaseRow { kind, altitude_m: 2_000.0, radius_m: size, runs: vec![run.clone()] };
        let rows =
            [row(CaseKind::Stamp, 150.0), row(CaseKind::Stamp, 5_000.0), row(CaseKind::Line, 5_000.0), row(CaseKind::Region, 2_000.0)];
        assert_eq!(rows.iter().map(CaseRow::gated).collect::<Vec<_>>(), [true, false, false, false]);
        assert!(rows.iter().skip(1).all(|r| r.verdict() == "recorded"));
        let t = table(&rows);
        for label in ["| 2000 m | 150 m |", "| 2000 m | 5000 m |", "| 2000 m | 5000 m ridge |", "| 2000 m | 2000 m region |"] {
            assert!(t.contains(label), "{label} in {t}");
        }
        assert_eq!(t.matches("recorded").count(), 6, "the budget cell and the verdict of the three recorded rows: {t}");
    }

    // spec: TEST-043
    #[test]
    fn the_preview_cost_stamps_are_a_row_of_hills_at_the_cap_and_the_table_prints_the_counts_and_the_times() {
        let at = case_centre();
        for count in PREVIEW_COST_COUNTS {
            let stamps = cost_stamps(at, count, 400.0, R);
            assert_eq!(stamps.len(), count);
            assert!(stamps.iter().all(|s| s.kind == StampKind::Hill && s.sign == 1 && (s.dir.length() - 1.0).abs() < 1e-12));
        }
        assert_eq!(PREVIEW_COST_COUNTS, [0, 8, planet_frame::preview::MAX_PREVIEW_STAMPS]);
        let rows = [
            PreviewCost { altitude_m: 200.0, stamps: 32, flagged_nodes: 5, nodes: 40, gpu_ms: Some(1.25), wall_ms: 3.5, renders: 20 },
            PreviewCost { altitude_m: 200.0, stamps: 0, flagged_nodes: 0, nodes: 40, gpu_ms: None, wall_ms: 2.0, renders: 20 },
        ];
        let t = preview_cost_table(&rows);
        assert!(t.contains("| 200 m | 32 | 5 / 40 | 1.250 | 3.50 | 20 |") && t.contains("| 200 m | 0 | 0 / 40 | - | 2.00 | 20 |"), "{t}");
    }

    // spec: TEST-042
    #[test]
    fn a_brush_row_says_so_in_the_radius_column_of_all_three_tables() {
        let run =
            CaseRun { levels: vec![LevelRow { level: 12, found: 3, drawn: 2, spacing_m: 1.0, below_spacing: 0 }], ..Default::default() };
        let rows = [
            CaseRow { kind: CaseKind::Stamp, altitude_m: 200.0, radius_m: 150.0, runs: vec![run.clone()] },
            CaseRow { kind: CaseKind::Brush, altitude_m: 200.0, radius_m: 150.0, runs: vec![run] },
        ];
        for t in [table(&rows), stages_table(&rows), levels_table(&rows)] {
            assert!(t.contains("| 200 m | 150 m |") && t.contains("| 200 m | 150 m brush |"), "{t}");
        }
        assert_eq!(rows[1].radius_label(), "150 m brush");
        assert_eq!(exact_budget_ms(1000.0, 2000.0), EXACT_BUDGET_LARGE_MS, "a 1 km brush has the large budget");
    }

    // spec: TEST-042
    #[test]
    fn a_brush_case_is_previewed_as_its_chain_of_stamps_as_the_editor_does() {
        let before = WorldDefinition::single_planet(R, 7);
        for radius in BRUSH_RADII_M {
            let (spec, centre) = case_brush(radius, R);
            let mut edited = before.clone();
            edited.put_item_spec(BodyId::PLANET, spec.clone()).unwrap();
            let stamps = case_preview(&before, &edited, &spec, centre, R);
            assert!(!stamps.is_empty(), "the {radius} m brush is previewed: the chain of its stroke");
            assert!(stamps.len() <= planet_frame::preview::MAX_PREVIEW_STAMPS, "{} stamps over the cap", stamps.len());
        }
    }

    // spec: TEST-042
    #[test]
    fn a_row_without_a_preview_frame_is_judged_on_its_exact_time_alone() {
        let run = |exact: f64| CaseRun {
            stages: Stages { drawn_exact_ms: Some(exact), ..Default::default() },
            exact_ms: Some(exact),
            ..Default::default()
        };
        let row = |exact| CaseRow { kind: CaseKind::Brush, altitude_m: 200.0, radius_m: 150.0, runs: vec![run(exact)] };
        assert_eq!(row(200.0).verdict(), "ok (no preview)");
        assert_eq!(row(400.0).verdict(), "OVER (no preview)");
        let none = CaseRow { kind: CaseKind::Brush, altitude_m: 200.0, radius_m: 150.0, runs: vec![CaseRun::default()] };
        assert_eq!(none.verdict(), "no result", "no exact frame at all");
    }

    // spec: TEST-041
    #[test]
    fn the_report_is_written_into_an_output_directory_that_does_not_exist_yet() {
        let dir = std::env::temp_dir().join(format!("planet-editperf-{}", std::process::id())).join("a").join("b");
        let path = write_report(&dir, "table").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "table");
        assert!(path.ends_with("edit-latency.md"));
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).unwrap();
    }

    // spec: TEST-056
    #[test]
    fn drawn_exact_is_the_latest_first_refresh_end_of_the_tiles_the_plan_draws() {
        use planet_streaming::JobSample;
        let tile = |x| TileId::new(planet_core::Face(0), 16, x, 0).unwrap();
        let job = |x, ended_us, refresh| JobSample {
            wait_us: 0,
            run_us: 0,
            running: 1,
            done: true,
            level: 16,
            queue_ms: 0,
            refresh,
            tile: tile(x),
            ended_us,
        };
        // Tile 1 refreshed at 150 ms and again at 4 s (a chunk landed): its first end counts; tile 2 is not drawn (slow, resident only); tile 3 is a
        // new load, not a refresh.
        let jobs =
            [job(1, 1_150_000, true), job(2, 1_900_000, true), job(1, 5_000_000, true), job(3, 9_000_000, false), job(4, 1_300_000, true)];
        let drawn: HashSet<TileId> = [tile(1), tile(3), tile(4)].into_iter().collect();
        assert_eq!(drawn_refreshed_ms(&jobs, &drawn, 1_000_000, 5.0), Some(305.0));
        assert_eq!(drawn_refreshed_ms(&jobs, &HashSet::new(), 1_000_000, 5.0), None);
    }

    // spec: TEST-065
    #[test]
    fn the_horizon_table_has_a_row_per_touched_map_with_its_time_and_the_ten_second_verdict() {
        let run = |maps: Vec<(planet_generators::chunkfield::ChunkKey, Option<f64>, f64)>| CaseRun {
            stages: Stages { horizon: Some(maps), ..Stages::default() },
            ..CaseRun::default()
        };
        let rows = vec![
            CaseRow {
                kind: CaseKind::Stamp,
                altitude_m: 200.0,
                radius_m: 400.0,
                runs: vec![run(vec![
                    ((2, 10, 11), Some(2_500.0), 120.0),
                    ((2, 11, 11), Some(12_000.0), 5_100.0),
                    ((2, 10, 12), None, 9_800.0),
                ])],
            },
            CaseRow { kind: CaseKind::Stamp, altitude_m: 2_000.0, radius_m: 50.0, runs: vec![run(Vec::new())] },
            CaseRow { kind: CaseKind::Stamp, altitude_m: 2_000.0, radius_m: 100.0, runs: vec![CaseRun::default()] },
        ];
        let t = horizon_table(&rows);
        let lines: Vec<&str> = t.lines().collect();
        assert!(lines[0].ends_with("| verdict (10 s, horizon maps) |"), "{t}");
        assert!(lines[2].contains("(2, 10, 11)") && lines[2].contains("| 120 | 2500.0") && lines[2].ends_with("| ok |"), "{t}");
        assert!(lines[3].contains("12000.0") && lines[3].ends_with("| over |"), "{t}");
        assert!(lines[4].contains("| - | over |"), "a map that never landed is over: {t}");
        assert!(lines[5].ends_with("| - | - | - |") && lines[6].ends_with("| - | - | - |"), "a case that touched no map: {t}");
    }

    // spec: TEST-056
    #[test]
    fn drawn_again_ends_at_the_last_refresh_of_a_tile_the_plan_draws_not_of_a_resident_only_one() {
        use planet_streaming::JobSample;
        let tile = |x| TileId::new(planet_core::Face(0), 16, x, 0).unwrap();
        let job = |x, ended_us, refresh| JobSample {
            wait_us: 0,
            run_us: 0,
            running: 1,
            done: true,
            level: 16,
            queue_ms: 0,
            refresh,
            tile: tile(x),
            ended_us,
        };
        // Tile 1 (drawn) refreshed at 150 ms and again at 4 s after a chunk landed; tile 2 is resident only and ends last, at 9 s; tile 3 is a load.
        let jobs = [job(1, 1_150_000, true), job(1, 5_000_000, true), job(2, 10_000_000, true), job(3, 11_000_000, false)];
        let drawn: HashSet<TileId> = [tile(1), tile(3)].into_iter().collect();
        assert_eq!(last_refresh_end_ms(&jobs, &drawn, 1_000_000, 5.0), Some(4_005.0));
        assert_eq!(last_refresh_end_ms(&jobs, &HashSet::new(), 1_000_000, 5.0), None);
    }
}
