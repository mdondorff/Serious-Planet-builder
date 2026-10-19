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
use planet_frame::preview::preview_delta;
use planet_frame::{Camera, LodParams, TerrainRequest, TerrainView};
use planet_render::GpuContext;
use planet_world_def::footprint::Cap;
use planet_world_def::stamp::{Stamp, StampKind};
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
    let at = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
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
                "| {} m | {} m | {} | {:.3} | {} | {} | {} | {} |\n",
                r.altitude_m,
                r.radius_m,
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
}

/// One row of the report: a case with its repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseRow {
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
    pub fn preview_median(&self) -> Option<f64> {
        median(self.runs.iter().filter_map(|r| r.preview_ms).collect())
    }

    pub fn exact_median(&self) -> Option<f64> {
        median(self.runs.iter().filter_map(|r| r.exact_ms).collect())
    }

    /// `ok` or `OVER` against the budgets, `no result` when a run produced none.
    pub fn verdict(&self) -> &'static str {
        match (self.preview_median(), self.exact_median()) {
            (Some(p), Some(e)) if p <= PREVIEW_BUDGET_MS && e <= exact_budget_ms(self.radius_m, self.altitude_m) => "ok",
            (Some(_), Some(_)) => "OVER",
            _ => "no result",
        }
    }
}

/// The report table (markdown): one row per case, medians over the repeats, budgets named.
pub fn table(rows: &[CaseRow]) -> String {
    let mut s = String::from(
        "| altitude | stamp radius | preview ms (budget 16) | exact ms (budget) | tiles refreshed / resident | previewed nodes (max) | repeats | verdict |\n|---|---|---|---|---|---|---|---|\n",
    );
    let cell = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.1}"));
    for r in rows {
        let last = r.runs.last().cloned().unwrap_or_default();
        s += &format!(
            "| {} m | {} m | {} | {} ({}) | {} / {} | {} | {} | {} |\n",
            r.altitude_m,
            r.radius_m,
            cell(r.preview_median()),
            cell(r.exact_median()),
            exact_budget_ms(r.radius_m, r.altitude_m),
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
        "| altitude | stamp radius | set_definition call | edit call | preview call | plan delivered (worker ms) | R delivered | first job dispatched | half refreshed | all refreshed | uploaded | preview | exact | frame median |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let cell = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.1}"));
    for r in rows {
        let pick = |f: &dyn Fn(&CaseRun) -> Option<f64>| median(r.runs.iter().filter_map(f).collect());
        let (plan, worker) = (pick(&|x| x.stages.plan_delivered_ms), pick(&|x| x.stages.plan_worker_ms));
        s += &format!(
            "| {} m | {} m | {} | {} | {} | {} ({}) | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.altitude_m,
            r.radius_m,
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
}

impl Harness<'_> {
    /// One frame of the loop: the managed frame, the GPU wait at the frames-in-flight bound, the submit.
    fn step(&mut self, req: &TerrainRequest) -> Result<planet_frame::managed::ManagedFrame, String> {
        self.iteration += 1;
        let ctx = self.ctx;
        let rig = &mut self.rig;
        let frame = rig.managed.begin_frame(req, &mut RendererSink { ctx, renderer: &mut rig.renderer })?;
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

/// The resident tiles the item's footprint touches: the edit's find (WDEF-007), what `ManagedLoop::edit` is given.
fn found_tiles(def: &WorldDefinition, resident: impl Iterator<Item = TileId>) -> Vec<TileId> {
    resident
        .filter(|t| {
            def.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, *t, planet_generators::stamp::STAGE_HALO).is_ok_and(|v| !v.is_empty())
        })
        .collect()
}

/// Items near the stamp, for the preview delta (the whole body would be the same list: only stamps that differ count).
fn items_near(def: &WorldDefinition, at: Vec3) -> Vec<planet_world_def::footprint::FootprintItem> {
    def.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre: at, angle: core::f64::consts::PI }).unwrap_or_default()
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
    let sampler_started = Instant::now();
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    // The edit as the editor does it: swap the definition, name the edited tiles, set the preview.
    let call = Instant::now();
    world.set_definition(Arc::clone(&edited))?;
    out.stages.set_definition_ms = ms(call);
    let call = Instant::now();
    let tiles = found_tiles(&edited, h.rig.managed.resident().keys().copied());
    out.levels = level_rows(&tiles, &h.drawn, &req.lod, stamp.max_delta_m);
    out.refreshed = h.rig.managed.edit(&tiles);
    out.stages.edit_ms = ms(call);
    let call = Instant::now();
    let list = preview_delta(&items_near(&before_def, stamp.dir), &items_near(&edited, stamp.dir), req.camera.position.0);
    h.rig.managed.set_preview(list.stamps);
    out.stages.preview_call_ms = ms(call);
    let mut waiting_for_exact = false;
    let mut frame_ms: Vec<f64> = Vec::new();
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
            }
        }
        if out.stages.residency_delivered_ms.is_none() && frame.pipeline.residency_delivered.is_some() {
            out.stages.residency_delivered_ms = Some(at);
        }
        if out.stages.first_dispatch_ms.is_none() && stats.dispatched > before_stats.dispatched {
            out.stages.first_dispatch_ms = Some(at);
        }
        let refreshed = stats.refreshed - before_stats.refreshed;
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
    // Take the edit back: the old definition, the same tiles refreshed, no preview, settled.
    world.set_definition(before_def)?;
    h.rig.managed.edit(&tiles);
    h.rig.managed.set_preview(Vec::new());
    h.settle(req, 20)?;
    Ok(out)
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
    if !smoke {
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
        crate::readiness_gate(o, ctx, out)?;
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
    let altitudes: Vec<f64> = if smoke { vec![200.0] } else { ALTITUDES_M.to_vec() };
    let radii: Vec<f64> = if smoke { vec![RADII_M[0]] } else { RADII_M.to_vec() };
    let repeats = if smoke { 1 } else { REPEATS };
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
    for &altitude in &altitudes {
        // One rig per altitude, built for the first case's camera and settled from empty; every case then edits and reverts on it.
        let first_stamp = case_stamp(radii[0]);
        let rho = (1.5 * radii[0]).max(altitude);
        let first = req_of(camera_for(first_stamp.dir, &ground, radius_m, rho, altitude, aspect));
        let rig = Rig::new(ctx, &world, &first, o.gen_workers())?;
        let mut h = Harness {
            ctx,
            rig,
            iteration: 0,
            drawn: HashSet::new(),
            settle_limit: if smoke { Duration::from_secs(120) } else { SETTLE_LIMIT },
        };
        for &r in &radii {
            let stamp = case_stamp(r);
            let rho = (1.5 * r).max(altitude);
            let req = req_of(camera_for(stamp.dir, &ground, radius_m, rho, altitude, aspect));
            let settle = h.settle(&req, 20)?;
            writeln!(out, "  altitude {altitude} m, stamp radius {r} m: settled in {settle:.1} s").map_err(err)?;
            let mut runs = Vec::new();
            for _ in 0..repeats {
                runs.push(run_once(&mut h, &world, &req, &stamp, limit)?);
            }
            rows.push(CaseRow { altitude_m: altitude, radius_m: r, runs });
        }
    }
    let table = table(&rows);
    let stages = stages_table(&rows);
    let levels = levels_table(&rows);
    writeln!(out, "\n{table}\nstages (ms from set_definition; the calls and the frame median are durations):\n\n{stages}").map_err(err)?;
    writeln!(out, "\nthe refreshed set by level (first run of each case):\n\n{levels}").map_err(err)?;
    if let Some(dir) = &o.out {
        let path = crate::streamperf::out_log_path(dir, "edit-latency.md")?;
        std::fs::write(&path, format!("{table}\n{stages}\n{levels}")).map_err(|e| format!("{}: {e}", path.display()))?;
        writeln!(out, "table written to {}", path.display()).map_err(err)?;
    }
    if smoke && rows.iter().any(|r| r.runs.iter().any(|x| x.exact_ms.is_none())) {
        return Err("the smoke run's edit never became exact".into());
    }
    Ok(())
}
