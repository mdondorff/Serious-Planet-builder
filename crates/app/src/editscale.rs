//! The edit-scale stress scene (SPIKE-29): `test-render --scene edit-scale --store DIR [--global-res N]`. How far do authored edits scale, and is the plan job
//! the bottleneck? Synthetic item sets from a seed (stamps, lines, regions and brush items, 80 % in one 50 km area, the rest over the planet) are made
//! through the editor session (so every put pays what an edit pays), written as item files, and for each count the scene measures: the load of the files
//! into an editor (time and working set), the clone and swap of the definition, the field's `height`, `height_bounds` and `inputs_hash_for` per call,
//! the plan job (cold and warm, p50 and p95) at 2 km, 200 m and 30 m over the cluster and over empty ground, the generation of tiles in the cluster, and
//! the editor's per-frame CPU with the editor on. The edit-to-visible latency of one stamp in the cluster is the `edit-latency` scene run over the same
//! files (`PLANET_EDIT_ITEMS=DIR`, the cluster is centred on the place that scene edits). CPU only (no GPU context); indicative, no readiness gate.
//!
//! Environment: `PLANET_SCALE_COUNTS` (comma separated, default `100,1000,5000,20000`), `PLANET_SCALE_DIR` (where the item sets are kept, default
//! `target/edit-scale`; a set that exists is reused), `PLANET_SCALE_SEED` (default 1), `PLANET_SCALE_FRAMES` (warm plans and editor frames per
//! case, default 12).

use crate::editwin::{EditRuntime, Pointer};
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{TileId, Vec3};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::layout::Layout;
use planet_editor::session::Tool;
use planet_editor::shape;
use planet_frame::globe::Viewport;
use planet_frame::pick::PickLimits;
use planet_frame::{plan_terrain_cached, Camera, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::HeightField;
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, ReadRecord};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Instant;

pub const SCENE: &str = "edit-scale";

/// The radius of the cluster, metres (about 80 % of the items are within it: "one 50 km area").
pub const CLUSTER_RADIUS_M: f64 = 25_000.0;
/// The share of the items in the cluster.
pub const CLUSTER_SHARE: f64 = 0.8;

pub fn is_scale_scene(name: &str) -> bool {
    name == SCENE
}

/// A small deterministic generator (splitmix64): the item sets are a function of the seed.
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `[0, 1)`.
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }

    pub fn log_range(&mut self, lo: f64, hi: f64) -> f64 {
        libm::exp(self.range(libm::log(lo), libm::log(hi)))
    }
}

/// What a generator makes: the item kinds of the editor, by their share of the actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Make {
    Hill,
    Crater,
    Plateau,
    Ridge,
    Channel,
    Region,
    Brush,
}

pub fn choose(rng: &mut Rng) -> Make {
    let mut r = rng.f();
    // `PLANET_SCALE_NOBRUSH`: the same mix without the brush items (to tell what the brushes cost: they make ground steeper than the plan can draw).
    if std::env::var_os("PLANET_SCALE_NOBRUSH").is_some() && r >= 0.70 {
        r *= 0.70 / 1.0;
    }
    match r {
        r if r < 0.22 => Make::Hill,
        r if r < 0.40 => Make::Crater,
        r if r < 0.52 => Make::Plateau,
        r if r < 0.60 => Make::Ridge,
        r if r < 0.65 => Make::Channel,
        r if r < 0.70 => Make::Region,
        _ => Make::Brush,
    }
}

/// The place of a new edit: in the cluster with the probability [`CLUSTER_SHARE`], else anywhere on the planet.
pub fn place_of(rng: &mut Rng, centre: Vec3, radius_m: f64) -> Vec3 {
    if rng.f() < CLUSTER_SHARE {
        shape::point_at(centre, rng.range(0.0, 360.0), CLUSTER_RADIUS_M * libm::sqrt(rng.f()), radius_m)
    } else {
        let z = rng.range(-1.0, 1.0);
        let a = rng.range(0.0, core::f64::consts::TAU);
        let r = libm::sqrt(1.0 - z * z);
        Vec3::new(r * libm::cos(a), r * libm::sin(a), z)
    }
}

/// One action of the generator through the editor session, as a user would do it. Errors (a refused stroke, a parameter out of range) are not
/// failures of the scene: the editor said no, the set is made of what it said yes to.
pub fn make_one(rt: &mut EditRuntime, rng: &mut Rng, centre: Vec3, radius_m: f64) {
    let at = place_of(rng, centre, radius_m);
    let s = &mut rt.session;
    match choose(rng) {
        kind @ (Make::Hill | Make::Crater | Make::Plateau) => {
            let name = match kind {
                Make::Hill => "hill",
                Make::Crater => "crater",
                _ => "plateau",
            };
            if s.place(name, at).is_ok() {
                let _ = s.set_param_text(shape::RADIUS_KEY, &format!("{:.0}", rng.log_range(40.0, 4_000.0)));
            }
        }
        kind @ (Make::Ridge | Make::Channel) => {
            s.set_tool(Tool::Line(if kind == Make::Ridge { "ridge" } else { "channel" }.to_string()));
            let mut p = at;
            let mut heading = rng.range(0.0, 360.0);
            for _ in 0..(2 + (rng.f() * 4.0) as usize) {
                let _ = s.add_draft_point(p);
                heading += rng.range(-40.0, 40.0);
                p = shape::point_at(p, heading, rng.range(300.0, 1_500.0), radius_m);
            }
            let _ = s.finish_draft();
        }
        Make::Region => {
            s.set_tool(Tool::Region);
            let r = rng.log_range(300.0, 2_000.0);
            let n = 3 + (rng.f() * 4.0) as usize;
            for k in 0..n {
                let _ = s.add_draft_point(shape::point_at(at, 360.0 * k as f64 / n as f64, r, radius_m));
            }
            let _ = s.finish_draft();
        }
        Make::Brush => {
            // A pile of strokes near one place: 1 to 64, mostly few, as a session paints them (the strokes join one item while it has room).
            let kinds = [BrushKind::Raise, BrushKind::Lower, BrushKind::Smooth, BrushKind::Flatten];
            let strokes = 1 + (libm::pow(rng.f(), 3.0) * 63.0) as usize;
            let _ = s.set_brush_radius(rng.log_range(20.0, 400.0));
            let _ = s.set_brush_strength(rng.range(0.5, 6.0));
            for _ in 0..strokes {
                s.set_tool(Tool::Brush(kinds[(rng.next_u64() % 4) as usize]));
                let start = shape::point_at(at, rng.range(0.0, 360.0), rng.range(0.0, 600.0), radius_m);
                if s.begin_stroke(start, 0.0).is_err() {
                    continue;
                }
                let mut p = start;
                let heading = rng.range(0.0, 360.0);
                for _ in 0..(1 + (rng.f() * 6.0) as usize) {
                    p = shape::point_at(p, heading + rng.range(-20.0, 20.0), rng.range(30.0, 300.0), radius_m);
                    s.extend_stroke(p);
                }
                let _ = s.end_stroke();
            }
            s.set_tool(Tool::Select);
        }
    }
}

/// How the items of a set are made up: counts by kind and the strokes in all.
pub fn composition(rt: &EditRuntime) -> String {
    let mut by: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut strokes = 0usize;
    let mut max_strokes = 0usize;
    for i in rt.session.editor().items() {
        *by.entry(i.kind.as_str()).or_default() += 1;
        strokes += i.strokes.len();
        max_strokes = max_strokes.max(i.strokes.len());
    }
    let kinds: Vec<String> = by.iter().map(|(k, n)| format!("{n} {k}")).collect();
    format!("{} ({strokes} strokes in all, at most {max_strokes} in an item)", kinds.join(", "))
}

fn ids(seed: u64) -> IdMaker {
    IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

/// The working set of this process now, bytes: `VmRSS` on Linux, `WorkingSet64` through PowerShell on Windows (about a second a call).
pub fn working_set_bytes() -> Option<u64> {
    let pid = std::process::id();
    if cfg!(windows) {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(Get-Process -Id {pid}).WorkingSet64")])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    } else {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let kb: u64 = status.lines().find(|l| l.starts_with("VmRSS:"))?.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    }
}

fn mib(b: Option<u64>) -> String {
    b.map_or("n/a".to_string(), |b| format!("{:.0}", b as f64 / 1_048_576.0))
}

fn quantile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 - 1.0) * q).round() as usize]
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                n += count_files(&e.path());
            } else {
                n += 1;
            }
        }
    }
    n
}

/// Make the sets of `counts` (ascending) in `dir/n<count>`: one session fills a working directory and the item files are copied at each count.
/// A set that exists is kept. Logs the time of the puts: the last hundred of each stage show what an edit costs at that size.
fn generate(
    base: &planet_world_def::WorldDefinition,
    radius_m: f64,
    centre: Vec3,
    counts: &[usize],
    seed: u64,
    dir: &Path,
    out: &mut dyn Write,
) -> Result<(), String> {
    if counts.iter().all(|n| dir.join(format!("n{n}")).join("ready").is_file()) {
        writeln!(out, "item sets: reused from {}", dir.display()).map_err(err)?;
        return Ok(());
    }
    let work = dir.join("work");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let work_buf: PathBuf = work.clone();
    let mut rt = EditRuntime::with_ids(base.clone(), radius_m, Some(&work_buf), Layout::default(), ids(seed))?;
    let mut rng = Rng(seed);
    let started = Instant::now();
    let mut stage_start = Instant::now();
    let mut last_count = 0usize;
    for &n in counts {
        let mut tail = Vec::new();
        while rt.session.editor().items().count() < n {
            let before = rt.session.editor().items().count();
            let t = Instant::now();
            make_one(&mut rt, &mut rng, centre, radius_m);
            let made = rt.session.editor().items().count() - before.min(rt.session.editor().items().count());
            if made > 0 && rt.session.editor().items().count() + 100 >= n {
                tail.push(t.elapsed().as_secs_f64() * 1000.0 / made as f64);
            }
        }
        let have = rt.session.editor().items().count();
        let target = dir.join(format!("n{n}"));
        let _ = std::fs::remove_dir_all(&target);
        copy_dir(&work, &target)?;
        std::fs::write(target.join("ready"), format!("{have} items, seed {seed}\n")).map_err(|e| e.to_string())?;
        writeln!(
            out,
            "made {have} items for the set of {n} in {:.1} s (this stage {:.1} s, {} to {have} items); the last stage's actions cost {:.2} ms per item at the end (p50 of the last 100); {}",
            started.elapsed().as_secs_f64(),
            stage_start.elapsed().as_secs_f64(),
            last_count,
            quantile(&mut tail, 0.5),
            composition(&rt)
        )
        .map_err(err)?;
        last_count = have;
        stage_start = Instant::now();
    }
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}

/// The camera `altitude_m` above `at`, backed off as the item sweep does (1.5 altitudes, at least the altitude).
fn camera_over(at: Vec3, ground: &dyn Fn(Vec3) -> f64, radius_m: f64, altitude_m: f64, aspect: f64) -> Camera {
    crate::editperf::camera_for(at, ground, radius_m, altitude_m, altitude_m, aspect)
}

/// A direction on the far side of the cluster with nothing near it: the empty ground the plan job is compared on.
fn empty_place(centre: Vec3) -> Vec3 {
    let _ = centre;
    let (la, lo) = (-40f64.to_radians(), 100f64.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

/// A field that counts and times the calls the plan makes into the field under it: the attribution of a plan job (SPIKE-29).
pub struct Timed<'a> {
    pub inner: &'a dyn HeightField,
    /// Calls and nanoseconds of `height`, `height_bounds`, `inputs_hash_for`, `read_records_for`, `passes_through`, `max_abs_height_m`.
    pub calls: [AtomicU64; 6],
    pub ns: [AtomicU64; 6],
    /// Calls and nanoseconds of `inputs_hash_for` by the level of the tile.
    pub hash_levels: [[AtomicU64; 2]; 32],
}

impl<'a> Timed<'a> {
    pub fn new(inner: &'a dyn HeightField) -> Timed<'a> {
        Timed { inner, calls: Default::default(), ns: Default::default(), hash_levels: Default::default() }
    }

    fn timed<R>(&self, k: usize, f: impl FnOnce() -> R) -> R {
        let t = Instant::now();
        let r = f();
        self.calls[k].fetch_add(1, Relaxed);
        self.ns[k].fetch_add(t.elapsed().as_nanos() as u64, Relaxed);
        r
    }

    /// `inputs_hash_for` calls and milliseconds by tile level, for the levels that were asked.
    pub fn hash_level_report(&self) -> String {
        (0..32)
            .filter(|l| self.hash_levels[*l][0].load(Relaxed) > 0)
            .map(|l| {
                format!(
                    "L{l}: {} calls {:.0} ms",
                    self.hash_levels[l][0].load(Relaxed),
                    self.hash_levels[l][1].load(Relaxed) as f64 / 1.0e6
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn report(&self) -> String {
        ["height", "height_bounds", "inputs_hash_for", "read_records_for", "passes_through", "max_abs_height_m"]
            .iter()
            .enumerate()
            .map(|(k, n)| format!("{n} {} calls {:.1} ms", self.calls[k].load(Relaxed), self.ns[k].load(Relaxed) as f64 / 1.0e6))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

impl HeightField for Timed<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.timed(0, || self.inner.height(d))
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.inner.implementation_id()
    }
    fn version(&self) -> u32 {
        self.inner.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.timed(5, || self.inner.max_abs_height_m())
    }
    fn inputs_hash(&self) -> u64 {
        self.inner.inputs_hash()
    }
    fn read_records(&self) -> Vec<ReadRecord> {
        self.inner.read_records()
    }
    fn read_records_for(&self, t: TileId, m: &dyn FaceMapping) -> Vec<ReadRecord> {
        self.timed(3, || self.inner.read_records_for(t, m))
    }
    fn reads_cells(&self) -> bool {
        self.inner.reads_cells()
    }
    fn inputs_hash_for(&self, t: TileId, m: &dyn FaceMapping) -> u64 {
        let start = Instant::now();
        let h = self.timed(2, || self.inner.inputs_hash_for(t, m));
        let level = &self.hash_levels[usize::from(t.level()).min(31)];
        level[0].fetch_add(1, Relaxed);
        level[1].fetch_add(start.elapsed().as_nanos() as u64, Relaxed);
        h
    }
    fn body(&self) -> BodyId {
        self.inner.body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.inner.source_key()
    }
    fn upstream(&self) -> Option<&dyn HeightField> {
        self.inner.upstream()
    }
    fn stage_params(&self) -> Option<(&str, u32)> {
        self.inner.stage_params()
    }
    fn item_revision(&self) -> u64 {
        self.inner.item_revision()
    }
    fn passes_through(&self, tile: TileId, map: &dyn FaceMapping) -> bool {
        self.timed(4, || self.inner.passes_through(tile, map))
    }
    fn surface_id(&self) -> &'static str {
        self.inner.surface_id()
    }
    fn height_bounds(&self, c: Vec3, r: f64) -> (f64, f64) {
        self.timed(1, || self.inner.height_bounds(c, r))
    }
    fn bounds_monotone(&self) -> bool {
        self.inner.bounds_monotone()
    }
    fn water_cull_level(&self) -> Option<f64> {
        self.inner.water_cull_level()
    }
}

/// `perf`-free entry of the scene.
pub fn run(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let counts: Vec<usize> = std::env::var("PLANET_SCALE_COUNTS")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![100, 1_000, 5_000, 20_000]);
    let dir = PathBuf::from(std::env::var("PLANET_SCALE_DIR").unwrap_or_else(|_| "target/edit-scale".to_string()));
    let seed: u64 = std::env::var("PLANET_SCALE_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let frames_env: usize = std::env::var("PLANET_SCALE_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let altitudes: Vec<f64> = std::env::var("PLANET_SCALE_ALTS")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![2_000.0, 200.0, 30.0]);
    let world = Arc::new(crate::streamperf::open_world(o, false)?);
    let radius_m = world.radius_m();
    let base = (*world.definition()).clone();
    let centre = crate::editperf::case_centre();
    let empty = empty_place(centre);
    writeln!(out, "edit-scale: {} ; seed {seed}, counts {counts:?}, the cluster {:.0} km radius at the edit-latency case centre, {:.0} % of the items in it", world.describe(), CLUSTER_RADIUS_M / 1000.0, CLUSTER_SHARE * 100.0)
        .map_err(err)?;
    generate(&base, radius_m, centre, &counts, seed, &dir, out)?;

    let size = (2560u32, 1440u32);
    let aspect = f64::from(size.0) / f64::from(size.1);
    let mut summary: Vec<String> = Vec::new();
    for &n in &counts {
        let set: PathBuf = dir.join(format!("n{n}"));
        writeln!(out, "\n## N = {n} ({} files in {})", count_files(&set).saturating_sub(1), set.display()).map_err(err)?;
        // --- the load
        let ws0 = working_set_bytes();
        let t = Instant::now();
        let mut rt = EditRuntime::with_ids(base.clone(), radius_m, Some(&set), Layout::default(), ids(seed ^ 0x9E37_79B9_7F4A_7C15))?;
        let load_ms = t.elapsed().as_secs_f64() * 1000.0;
        let ws1 = working_set_bytes();
        let count = rt.session.editor().items().count();
        writeln!(
            out,
            "load: {count} items in {load_ms:.0} ms; working set {} MiB before, {} MiB after ({} B per item); {}",
            mib(ws0),
            mib(ws1),
            ws1.zip(ws0).map_or(0, |(a, b)| a.saturating_sub(b) / count.max(1) as u64),
            composition(&rt)
        )
        .map_err(err)?;
        // --- the editor's lists, one by one (first call, then the repeat): what a frame of the editor pays
        {
            let cam_pos = centre * radius_m;
            let s = &rt.session;
            let ms = |f: &dyn Fn()| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1000.0
            };
            let (steep1, steep2) = (ms(&|| drop(s.steep_items())), ms(&|| drop(s.steep_items())));
            let (stack1, stack2) = (ms(&|| drop(s.stacked_items())), ms(&|| drop(s.stacked_items())));
            let near = ms(&|| drop(s.near(&s.editor().item_ids(), cam_pos)));
            let groups = ms(&|| drop(s.layer_groups(cam_pos)));
            let chain = ms(&|| drop(s.chain(cam_pos)));
            let problems = ms(&|| drop(s.problems(&[], cam_pos)));
            let search = ms(&|| drop(s.search("hill", None, cam_pos)));
            writeln!(
                out,
                "editor lists (ms): steep_items {steep1:.1} then {steep2:.2}; stacked_items {stack1:.1} then {stack2:.2}; near(all) {near:.1}; layer_groups {groups:.1}; chain {chain:.1}; problems {problems:.1}; search {search:.1}"
            )
            .map_err(err)?;
        }
        // --- the clone and the swap of the definition (what the window does for every edit)
        let t = Instant::now();
        let def = Arc::new(rt.session.editor().definition().clone());
        let clone_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        world.set_definition(def)?;
        let swap_ms = t.elapsed().as_secs_f64() * 1000.0;
        writeln!(out, "definition: clone {clone_ms:.1} ms, set_definition {swap_ms:.1} ms").map_err(err)?;
        let field = world.field()?;
        let seedf = field.seed();
        let lod = LodParams {
            cells: o.cells,
            viewport_h_px: f64::from(size.1),
            tau_px: o.tau,
            max_level: o.finest_level_for(o.cells),
            min_level: o.min_level(),
            radius_m,
            max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m() + 5_000.0),
            ..LodParams::earth_1080p()
        };
        lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, o.tau))?;
        let req_of = |camera: Camera| TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: TangentWarp.id().to_string(),
            seed: seedf,
        };

        // --- the field's queries
        let mut rng = Rng(seed ^ 0xABCD);
        for (label, at, spread) in [("cluster", centre, CLUSTER_RADIUS_M), ("empty ground", empty, 5_000.0)] {
            let dirs: Vec<Vec3> =
                (0..1_500).map(|_| shape::point_at(at, rng.range(0.0, 360.0), spread * libm::sqrt(rng.f()), radius_m)).collect();
            let t = Instant::now();
            let h: f64 = dirs.iter().map(|d| field.height(*d)).sum();
            let height_us = t.elapsed().as_secs_f64() * 1.0e6 / dirs.len() as f64;
            let mut bounds_us = Vec::new();
            for angle_m in [200.0, 2_000.0, 20_000.0] {
                let t = Instant::now();
                let b: f64 = dirs.iter().take(300).map(|d| field.height_bounds(*d, angle_m / radius_m).1).sum();
                bounds_us.push((angle_m, t.elapsed().as_secs_f64() * 1.0e6 / 300.0, b));
            }
            let tiles: Vec<TileId> = dirs.iter().take(200).map(|d| TileId::from_direction(&TangentWarp, *d, 12)).collect();
            let t = Instant::now();
            let mut acc = 0u64;
            for tile in &tiles {
                acc ^= field.inputs_hash_for(*tile, &TangentWarp);
            }
            let hash_us = t.elapsed().as_secs_f64() * 1.0e6 / tiles.len() as f64;
            writeln!(
                out,
                "queries over the {label}: height {height_us:.1} us; height_bounds {}; inputs_hash_for (level-12 tile) {hash_us:.1} us  [{h:.0} {acc}]",
                bounds_us.iter().map(|(a, us, _)| format!("{:.0} m cap {us:.1} us", a)).collect::<Vec<_>>().join(", ")
            )
            .map_err(err)?;
        }

        // --- what the plan asks of the field once per plan
        {
            let t = Instant::now();
            let h = field.inputs_hash();
            let hash_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let recs = field.read_records().len();
            let rec_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let rev = field.item_revision();
            let rev_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let keyer = planet_generators::mesh::MeshKeyer::new(&*field, &TangentWarp, o.cells, radius_m);
            let keyer_ms = t.elapsed().as_secs_f64() * 1000.0;
            let tile = TileId::from_direction(&TangentWarp, centre, 16);
            let t = Instant::now();
            let k = keyer.key(tile);
            let key_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let rr = field.read_records_for(tile, &TangentWarp).len();
            let rr_ms = t.elapsed().as_secs_f64() * 1000.0;
            writeln!(out, "once per plan: inputs_hash {hash_ms:.2} ms; read_records ({recs}) {rec_ms:.2} ms; item_revision {rev_ms:.3} ms; MeshKeyer::new {keyer_ms:.2} ms; key of a cluster tile {key_ms:.2} ms; read_records_for it ({rr}) {rr_ms:.2} ms  [{h} {rev} {k}]").map_err(err)?;
        }
        if std::env::var_os("PLANET_SCALE_NOPLAN").is_some() {
            continue;
        }

        // --- the plan job: cold (a fresh field and selection cache, the first plan at a place) and warm (the camera drifting 0.5 m a frame)
        let mut frames = frames_env;
        let mut edit_counter = 0u128;
        let mut row = format!("| {n} |");
        let mut editor_rows: Vec<String> = Vec::new();
        for (label, at) in [("cluster", centre), ("empty", empty)] {
            for &altitude in &altitudes {
                let fresh = world.field()?;
                let mut cache = SelectionCache::new();
                let base_cam = camera_over(at, &|d| fresh.height(d), radius_m, altitude, aspect);
                let t = Instant::now();
                let plan = plan_terrain_cached(&req_of(base_cam), &*fresh, &mut cache).map_err(|e| e.to_string())?;
                let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
                let nodes = plan.nodes.len();
                let capped = plan.nodes.iter().filter(|n| n.capped).count();
                let mut warm = Vec::new();
                for k in 1..=frames {
                    let mut cam = base_cam;
                    cam.position = planet_core::PlanetFixed(cam.position.0 + cam.forward.cross(cam.up).normalized() * (0.5 * k as f64));
                    let t = Instant::now();
                    let _ = plan_terrain_cached(&req_of(cam), &*fresh, &mut cache).map_err(|e| e.to_string())?;
                    warm.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                {
                    // The selection alone against the whole plan (the plan is the selection, the origin heights, the keys and the nodes).
                    let mut cam = base_cam;
                    cam.position =
                        planet_core::PlanetFixed(cam.position.0 + cam.forward.cross(cam.up).normalized() * (0.5 * (frames + 2) as f64));
                    let t = Instant::now();
                    let sel = planet_frame::select_nodes_in_view_cached(&TangentWarp, &cam, &*fresh, &lod, &mut cache);
                    let sel_ms = t.elapsed().as_secs_f64() * 1000.0;
                    let t = Instant::now();
                    let full = plan_terrain_cached(&req_of(cam), &*fresh, &mut cache).map_err(|e| e.to_string())?;
                    let full_ms = t.elapsed().as_secs_f64() * 1000.0;
                    writeln!(
                        out,
                        "  selection alone {sel_ms:.1} ms ({} nodes); the whole plan right after {full_ms:.1} ms ({} nodes)",
                        sel.nodes.len(),
                        full.nodes.len()
                    )
                    .map_err(err)?;
                }
                let (p50, p95) = (quantile(&mut warm.clone(), 0.5), quantile(&mut warm, 0.95));
                // Where the warm plan's time goes: one more plan through the counting field.
                {
                    let timed = Timed::new(&*fresh);
                    let mut cam = base_cam;
                    cam.position =
                        planet_core::PlanetFixed(cam.position.0 + cam.forward.cross(cam.up).normalized() * (0.5 * (frames + 1) as f64));
                    let t = Instant::now();
                    let _ = plan_terrain_cached(&req_of(cam), &timed, &mut cache).map_err(|e| e.to_string())?;
                    writeln!(
                        out,
                        "  one more warm plan through the counting field: {:.1} ms; {}",
                        t.elapsed().as_secs_f64() * 1000.0,
                        timed.report()
                    )
                    .map_err(err)?;
                }
                // The plan right after one edit: a hill is put at the view, the definition handed to the field and the change announced to the plan
                // cache as the window does (STRM-034; `PLANET_SCALE_UNANNOUNCED` leaves it out, so every tile is checked again), no tile named
                // (`invalidate_tiles(&[])`: the content keys of the tiles find what the edit changed, STRM-027), the plan made again.
                if std::env::var_os("PLANET_SCALE_NOEDITPLAN").is_none() {
                    // A hill of 400 m reach put into a copy of the definition (in memory: the set's files are not touched).
                    let old_def = world.definition();
                    let old_revision = fresh.item_revision();
                    let mut edited = (*old_def).clone();
                    let spec = planet_world_def::ItemSpec {
                        id: u128::MAX - edit_counter,
                        layer: planet_editor::session::STAMP_LAYER.into(),
                        priority: 0,
                        dir: at,
                        content_hash: 0,
                        reach_m: 400.0,
                        max_delta_m: 20.0,
                        stamp: None,
                    }
                    .with_stamp(planet_world_def::stamp::Stamp::new(planet_world_def::stamp::StampKind::Hill, [20.0, 0.0, 0.0]));
                    edit_counter += 1;
                    let placed = edited
                        .put_item_spec(planet_world_def::BodyId::PLANET, spec)
                        .map_or_else(|e| format!("refused: {e:?}"), |_| "yes".to_string());
                    let def = Arc::new(edited);
                    world.set_definition(Arc::clone(&def))?;
                    let after = world.field()?;
                    let changes: Vec<planet_frame::lod::ItemChange> = def
                        .item_changes_from(&old_def)
                        .into_iter()
                        .map(|(dir, reach_m)| planet_frame::lod::ItemChange { dir, reach_m })
                        .collect();
                    let announced = changes.len();
                    if std::env::var_os("PLANET_SCALE_UNANNOUNCED").is_none() {
                        cache.note_item_change(old_revision, after.item_revision(), changes);
                    }
                    cache.invalidate_tiles(&[]);
                    let mut cam = base_cam;
                    cam.position =
                        planet_core::PlanetFixed(cam.position.0 + cam.forward.cross(cam.up).normalized() * (0.5 * (frames + 3) as f64));
                    let timed = Timed::new(&*after);
                    let t = Instant::now();
                    let _ = plan_terrain_cached(&req_of(cam), &timed, &mut cache).map_err(|e| e.to_string())?;
                    let first_ms = t.elapsed().as_secs_f64() * 1000.0;
                    writeln!(out, "    where it went (calls into the field): {}", timed.report()).map_err(err)?;
                    writeln!(out, "    inputs_hash_for by tile level: {}", timed.hash_level_report()).map_err(err)?;
                    cam.position = planet_core::PlanetFixed(cam.position.0 + cam.forward.cross(cam.up).normalized() * 0.5);
                    let t = Instant::now();
                    let _ = plan_terrain_cached(&req_of(cam), &*after, &mut cache).map_err(|e| e.to_string())?;
                    let next_ms = t.elapsed().as_secs_f64() * 1000.0;
                    writeln!(
                        out,
                        "  the plan right after one hill edit at the {label} (placed: {placed}; {announced} places in the change, no tile named): {first_ms:.1} ms, the next plan {next_ms:.1} ms"
                    )
                    .map_err(err)?;
                }
                if p50 > 1_500.0 {
                    frames = frames.min(4);
                }
                writeln!(out, "plan job over the {label} at {altitude:.0} m: {nodes} nodes ({capped} capped); cold {cold_ms:.1} ms; warm p50 {p50:.1} ms, p95 {p95:.1} ms ({frames} plans)").map_err(err)?;
                row += &format!(" {cold_ms:.0} / {p50:.0} / {p95:.0} |");
                // --- the editor's frame at this view (cluster only): relevance, outlines, the lists with the problems tab
                if label == "cluster" {
                    if !rt.on {
                        rt.toggle();
                    }
                    let wanted: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
                    let limits = PickLimits::new(radius_m, lod.max_height_m);
                    let viewport = Viewport { width_px: f64::from(size.0), height_px: f64::from(size.1) };
                    let mut frame_ms = Vec::new();
                    for _ in 0..frames {
                        let t = Instant::now();
                        rt.update(&base_cam, viewport, &wanted, &*fresh, &limits, Pointer { cursor: [1280.0, 720.0], over_ui: false });
                        frame_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                    }
                    let shown = rt.panel_data("").near.len();
                    let problems = rt.panel_data("").problems.len();
                    let (f50, f95) = (quantile(&mut frame_ms.clone(), 0.5), quantile(&mut frame_ms, 0.95));
                    writeln!(out, "editor frame at {altitude:.0} m: update p50 {f50:.2} ms, p95 {f95:.2} ms; {shown} edits shown, {problems} problems").map_err(err)?;
                    editor_rows.push(format!(" {f50:.2} / {f95:.2} ({shown}) |"));
                    // --- tile generation in the cluster at this view (the nodes with the finest level, a sample)
                    if altitude == 200.0 {
                        let finest = plan.nodes.iter().map(|n| n.level).max().unwrap_or(0);
                        let mut sample: Vec<TileId> = plan.nodes.iter().filter(|n| n.level + 1 >= finest).map(|n| n.tile).collect();
                        sample.truncate(24);
                        let mut gen_ms = Vec::new();
                        for tile in &sample {
                            let t = Instant::now();
                            let _ = compact_tile_field(&*fresh, &TangentWarp, *tile, o.cells, radius_m);
                            gen_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                        }
                        let empty_cam = camera_over(empty, &|d| fresh.height(d), radius_m, altitude, aspect);
                        let empty_plan =
                            plan_terrain_cached(&req_of(empty_cam), &*fresh, &mut SelectionCache::new()).map_err(|e| e.to_string())?;
                        let mut empty_ms = Vec::new();
                        for tile in empty_plan.nodes.iter().filter(|n| n.level + 1 >= finest).map(|n| n.tile).take(24) {
                            let t = Instant::now();
                            let _ = compact_tile_field(&*fresh, &TangentWarp, tile, o.cells, radius_m);
                            empty_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                        }
                        writeln!(
                            out,
                            "tile generation (level {finest} and one above, {} tiles in the cluster, {} over empty ground): cluster p50 {:.2} ms, p95 {:.2} ms; empty p50 {:.2} ms",
                            gen_ms.len(),
                            empty_ms.len(),
                            quantile(&mut gen_ms.clone(), 0.5),
                            quantile(&mut gen_ms, 0.95),
                            quantile(&mut empty_ms, 0.5)
                        )
                        .map_err(err)?;
                    }
                }
            }
        }
        summary.push(row + &editor_rows.concat());
    }
    writeln!(out, "\n| N | cluster 2 km cold / warm p50 / warm p95 ms | cluster 200 m | cluster 30 m | empty 2 km | empty 200 m | empty 30 m | editor frame 2 km p50 / p95 (edits shown) | 200 m | 30 m |").map_err(err)?;
    writeln!(out, "|---|---|---|---|---|---|---|---|---|---|").map_err(err)?;
    for r in summary {
        writeln!(out, "{r}").map_err(err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_world_def::WorldDefinition;

    const R: f64 = 6_371_000.0;

    fn set(seed: u64, actions: usize) -> String {
        let mut rt = EditRuntime::with_ids(WorldDefinition::single_planet(R, 7), R, None, Layout::default(), ids(seed)).unwrap();
        let mut rng = Rng(seed);
        let centre = crate::editperf::case_centre();
        for _ in 0..actions {
            make_one(&mut rt, &mut rng, centre, R);
        }
        composition(&rt)
    }

    // spec: TEST-045
    #[test]
    fn a_set_is_a_function_of_the_seed_and_holds_several_kinds_of_edit() {
        let (a, b, c) = (set(3, 60), set(3, 60), set(4, 60));
        assert_eq!(a, b, "the same seed, the same set");
        assert_ne!(a, c, "another seed, another set");
        let kinds = ["brush", "channel", "crater", "hill", "plateau", "region", "ridge"].iter().filter(|k| a.contains(*k)).count();
        assert!(kinds >= 4, "{a}");
    }

    // spec: TEST-045
    #[test]
    fn the_mix_and_the_cluster_share_are_the_declared_ones() {
        let mut rng = Rng(11);
        let n = 10_000usize;
        let mut counts = std::collections::BTreeMap::new();
        for _ in 0..n {
            *counts.entry(format!("{:?}", choose(&mut rng))).or_insert(0usize) += 1;
        }
        let share = |k: &str| counts.get(k).copied().unwrap_or(0) as f64 / n as f64;
        for (k, want) in
            [("Hill", 0.22), ("Crater", 0.18), ("Plateau", 0.12), ("Ridge", 0.08), ("Channel", 0.05), ("Region", 0.05), ("Brush", 0.30)]
        {
            assert!((share(k) - want).abs() < 0.02, "{k}: {} against {want}", share(k));
        }
        let centre = crate::editperf::case_centre();
        let places = 4_000usize;
        let inside = (0..places)
            .filter(|_| {
                let p = place_of(&mut rng, centre, R);
                assert!((p.length() - 1.0).abs() < 1e-9, "a unit direction");
                libm::acos(p.dot(centre).clamp(-1.0, 1.0)) * R <= CLUSTER_RADIUS_M * 1.001
            })
            .count();
        assert!((inside as f64 / places as f64 - CLUSTER_SHARE).abs() < 0.02, "{inside} of {places}");
    }

    // spec: TEST-045
    #[test]
    fn the_counting_field_forwards_and_counts_each_kind_of_call() {
        let base = planet_generators::height::Reference(1);
        let timed = Timed::new(&base);
        let d = Vec3::new(1.0, 0.0, 0.0);
        assert_eq!(timed.height(d), base.height(d));
        assert_eq!(timed.height_bounds(d, 0.001), base.height_bounds(d, 0.001));
        let tile = TileId::from_direction(&TangentWarp, d, 8);
        assert_eq!(timed.inputs_hash_for(tile, &TangentWarp), base.inputs_hash_for(tile, &TangentWarp));
        assert_eq!([0, 1, 2].map(|k| timed.calls[k].load(std::sync::atomic::Ordering::Relaxed)), [1, 1, 1]);
        assert!(timed.report().contains("height 1 calls") && timed.report().contains("height_bounds 1 calls"), "{}", timed.report());
        assert_eq!(quantile(&mut [3.0, 1.0, 2.0], 0.5), 2.0);
    }
}
