//! The G-RT frame-time scene (`--scene river-flight`, item 15, architect 2026-11-12): the lit path with the viewer's river plan over the G-RT fixture, a commit baked by `bake_region` (named by `PLANET_RIVER_STORE`)
//! under the in-memory fixture base (`WorldTerrain::open_fixture`: TEST-ONLY, reached by this scene alone). It is the lit perf scene (`litperf::perf_lit`: the same gate, warm-up, timing and log) with the world, the
//! views and the report of the river plan swapped in: two flights over the commit's rivers (a run along the longest channel at 300 m, a descent over its middle), a river plan every frame, the nodes it carried
//! and dropped, the plan's CPU time and the reader's cache peak. Report only; a gate figure needs the architect's perf window. G-RT on this fixture is PROVISIONAL: it covers the river plan, the coverage, the L read
//! and its cache, the lit pass and the LOD, not the reads of a base store (the base is built in memory before the first frame, outside the measured frames, and the output says so).

use std::io::Write;

use planet_core::cube::face_to_direction;
use planet_core::{Face, PlanetFixed, TangentWarp};
use planet_frame::litscenes::{LitView, ViewKind, FOV_DEG};
use planet_frame::Camera;
use planet_generators::height::HeightField;

use crate::worldfield::WorldTerrain;
use crate::{err, Options};

/// The scene's name.
pub const SCENE: &str = "river-flight";
/// The environment variable that names the fixture's commit folder (the output of the `bake_region` example).
pub const STORE_ENV: &str = "PLANET_RIVER_STORE";

/// Whether `name` is the river flight scene.
pub fn is_river_scene(name: &str) -> bool {
    name == SCENE
}

/// The fixture world of the run: the commit named by [`STORE_ENV`] under the in-memory fixture base. The base statement is printed first.
pub fn open_world(out: &mut dyn Write) -> Result<WorldTerrain, String> {
    let dir = std::env::var(STORE_ENV).map_err(|_| {
        format!("--scene {SCENE} needs {STORE_ENV}=<the folder `bake_region` wrote: a commit with a channel graph (the G-RT fixture, target/stores/g-rt-fixture)>")
    })?;
    let dir = std::path::PathBuf::from(dir);
    let commit = planet_cache::l_writer::Commit::read(&dir)?.ok_or_else(|| format!("{} holds no commit", dir.display()))?;
    let def = planet_world_def::WorldDefinition::single_planet(commit.spec.radius_m, commit.spec.seed);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let world = WorldTerrain::open_fixture(def, &dir, threads)?;
    writeln!(out, "{}", world.fixture_note()).map_err(err)?;
    Ok(world)
}

/// The face coordinate of cell-index position `p` on a face of `cells` cells (cell `i` has its centre at `s = 2 (i + 1/2) / cells - 1`).
fn face_coord(p: f64, cells: f64) -> f64 {
    2.0 * (p + 0.5) / cells - 1.0
}

/// The ground height above which a point of the course counts as land.
const LAND_MIN_M: f64 = 20.0;

/// The course of the river scenes: the longest channel's points inside the commit's tiles shrunk by one tile, with the discharge at each point.
pub struct Course {
    points: Vec<(f64, f64)>,
    q: Vec<f64>,
    face: Face,
    cells: f64,
    cell_m: f64,
}

impl Course {
    /// The direction of the face-local cell-index position `p`.
    fn dir_of(&self, p: (f64, f64)) -> planet_core::Vec3 {
        face_to_direction(&TangentWarp, self.face, face_coord(p.0, self.cells), face_coord(p.1, self.cells))
    }

    /// A position along the course by parameter `u` in `[0, 1]` (linear between points, by point index).
    fn at(&self, u: f64) -> (f64, f64) {
        let best = &self.points;
        let x = u.clamp(0.0, 1.0) * (best.len() - 1) as f64;
        let k = (libm::floor(x) as usize).min(best.len() - 2);
        let f = x - k as f64;
        (best[k].0 + (best[k + 1].0 - best[k].0) * f, best[k].1 + (best[k + 1].1 - best[k].1) * f)
    }

    /// The width in metres the river is drawn with at parameter `u`: the law's width at the nearest point, at least one cell (REND-059).
    pub fn drawn_width_m(&self, u: f64) -> f64 {
        let k = (libm::round(u.clamp(0.0, 1.0) * (self.points.len() - 1) as f64) as usize).min(self.points.len() - 1);
        let q = self.q.get(k).copied().unwrap_or(0.0);
        planet_generators::bake::k7::WidthLaw::STANDARD.width_m(q).max(self.cell_m)
    }

    /// The camera at course parameter `u`, `alt_m` above the ground (and the sea), looking along the river turned by `yaw_deg` to the right, pitched `pitch_deg` down.
    pub fn camera(&self, field: &dyn HeightField, radius_m: f64, level_m: f64, aspect: f64, u: f64, alt_m: f64, pitch_deg: f64, yaw_deg: f64) -> Camera {
        let ahead = 1.5 / (self.points.len() - 1) as f64;
        let (p, q) = (self.at(u), self.at((u + ahead).min(1.0)));
        let d = self.dir_of(p);
        let ground = field.height(d).max(level_m);
        let along = self.dir_of(q) - d;
        let flat = (along - d * along.dot(d)).normalized();
        let (s, c) = libm::sincos(yaw_deg.to_radians());
        let heading = flat * c + d.cross(flat) * s;
        Camera::at_surface_point(
            PlanetFixed(d * (radius_m + ground + alt_m)),
            heading,
            pitch_deg.to_radians(),
            FOV_DEG.to_radians(),
            aspect,
            0.1,
        )
    }
}

/// The two flights over the commit's rivers: `frames` cameras each. The course is the longest channel's points inside the commit's tiles shrunk by one tile (the flight stays a tile from the edge of the commit's
/// tiles, where the exact base and L end: the fixture's seam).
pub fn views(
    world: &WorldTerrain,
    field: &dyn HeightField,
    radius_m: f64,
    level_m: f64,
    aspect: f64,
    frames: usize,
) -> Result<Vec<LitView>, String> {
    let course = course(world, field)?;
    let n = frames.max(2);
    let camera = |u: f64, alt_m: f64, pitch_deg: f64| course.camera(field, radius_m, level_m, aspect, u, alt_m, pitch_deg, 0.0);
    let run: Vec<Camera> = (0..n).map(|k| camera(k as f64 / (n - 1) as f64, 300.0, 15.0)).collect();
    // The descent over the course's middle: 3 km down to 30 m, log-spaced, looking along the river, 25 degrees down.
    let descent: Vec<Camera> = (0..n)
        .map(|k| {
            let u = k as f64 / (n - 1) as f64;
            camera(0.5, 3000.0 * libm::pow(30.0 / 3000.0, u), 25.0)
        })
        .collect();
    Ok(vec![
        LitView { name: "river-run", cameras: run, kind: ViewKind::Flight },
        LitView { name: "river-descent", cameras: descent, kind: ViewKind::Flight },
    ])
}

/// One frame of the river look capture (`--scene river-frames`): its camera, the sun's elevation when it is not the look's, and the width the river is drawn with there.
pub struct FramePose {
    pub name: String,
    pub camera: Camera,
    /// The sun above the horizon in degrees, from the camera's right (a grazing light across the river); none: the look's own sun.
    pub sun_deg: Option<f64>,
    pub drawn_width_m: f64,
}

/// The altitudes, pitches and heading offsets of the river look frames: low over three wide points of the course, along the river and across it.
const FRAME_ALTS_M: [f64; 3] = [30.0, 80.0, 200.0];
const FRAME_VIEWS: [(&str, f64, f64); 3] = [("along-12", 12.0, 0.0), ("along-40", 40.0, 0.0), ("across-25", 25.0, 90.0)];
const FRAME_US: [f64; 3] = [0.3, 0.5, 0.7];
/// The sun of the low-sun frames.
const LOW_SUN_DEG: f64 = 2.0;

/// The river look frames (item 15, queue row 2): the table of [`FRAME_ALTS_M`] x [`FRAME_VIEWS`] at the middle course point (9 frames), the same three altitudes along the river at the other two course points, and the
/// 9 poses again under a sun of 2 degrees from the side. Names carry every parameter, so a frame is found by its file name.
pub fn frame_poses(world: &WorldTerrain, field: &dyn HeightField, radius_m: f64, level_m: f64, aspect: f64) -> Result<Vec<FramePose>, String> {
    let course = course(world, field)?;
    let mut poses = Vec::new();
    for sun in [None, Some(LOW_SUN_DEG)] {
        let sun_tag = if sun.is_some() { "sun2" } else { "sun-look" };
        for (vi, (vname, pitch, yaw)) in FRAME_VIEWS.iter().enumerate() {
            for alt in FRAME_ALTS_M {
                // The shallow along view is also taken at the other two course points: three points of the river.
                let us: &[f64] = if vi < 1 { &FRAME_US } else { &FRAME_US[1..2] };
                for &u in us {
                    poses.push(FramePose {
                        name: format!("{vname}-alt{alt:.0}-u{:02.0}-{sun_tag}", u * 100.0),
                        camera: course.camera(field, radius_m, level_m, aspect, u, alt, *pitch, *yaw),
                        sun_deg: sun,
                        drawn_width_m: course.drawn_width_m(u),
                    });
                }
            }
        }
    }
    Ok(poses)
}

fn course(world: &WorldTerrain, field: &dyn HeightField) -> Result<Course, String> {
    let src = world.rivers().ok_or("the fixture commit holds no channel graph")?;
    let reader = src.reader();
    let commit = reader.commit();
    let (cells, tile, cell_m) = reader.grid_dims();
    let (cells, tile) = (f64::from(cells), f64::from(tile));
    let face = Face(commit.tiles.keys().next().ok_or("the commit holds no tile")?.0);
    let min_i = commit.tiles.keys().map(|k| k.1).min().expect("a tile");
    let max_i = commit.tiles.keys().map(|k| k.1).max().expect("a tile");
    let min_j = commit.tiles.keys().map(|k| k.2).min().expect("a tile");
    let max_j = commit.tiles.keys().map(|k| k.2).max().expect("a tile");
    // The interior of the commit's tiles, a tile in from each edge, in cells.
    let (x0, x1) = ((f64::from(min_i) + 1.0) * tile, f64::from(max_i) * tile);
    let (y0, y1) = ((f64::from(min_j) + 1.0) * tile, f64::from(max_j) * tile);
    if x1 <= x0 || y1 <= y0 {
        return Err("the commit's tiles leave no interior a tile from their edge".into());
    }
    let dir_of = |p: (f64, f64)| face_to_direction(&TangentWarp, face, face_coord(p.0, cells), face_coord(p.1, cells));
    // On land: the burned beds are floored at the sea level, so a course in the coastal plain flies over the sea (a camera over water draws no river).
    let inside = |p: &(f64, f64)| p.0 >= x0 && p.0 < x1 && p.1 >= y0 && p.1 < y1 && field.height(dir_of(*p)) > LAND_MIN_M;
    // The interior run of each channel (its longest stretch of consecutive points inside), the longest of all.
    // The interior run of each channel (its longest stretch of consecutive points inside), the longest of all; the discharge at each point goes with it.
    let mut best: Vec<((f64, f64), f64)> = Vec::new();
    for &(f, ti, tj) in commit.tiles.keys() {
        for c in reader.tile_channels(Face(f), ti, tj)?.iter() {
            let mut run: Vec<((f64, f64), f64)> = Vec::new();
            for (i, p) in c.points.iter().enumerate() {
                if inside(p) {
                    run.push((*p, c.q.get(i).copied().unwrap_or(0.0)));
                } else {
                    if run.len() > best.len() {
                        best = run.clone();
                    }
                    run.clear();
                }
            }
            if run.len() > best.len() {
                best = run;
            }
        }
    }
    if best.len() < 8 {
        return Err(format!("no channel runs 8 points inside the commit's tiles a tile from their edge (the longest has {})", best.len()));
    }
    let course = Course { points: best.iter().map(|b| b.0).collect(), q: best.iter().map(|b| b.1).collect(), face, cells, cell_m: f64::from(cell_m) };
    let (p0, p1) = (course.at(0.0), course.at(1.0));
    let km = libm::hypot(p1.0 - p0.0, p1.1 - p0.1) * 1.0e-3 * course.cell_m;
    println!(
        "river course: {} points of the longest channel inside the commit's tiles, from cell ({:.0}, {:.0}) to ({:.0}, {:.0}) (about {km:.0} km as the crow flies); ground at the start {:.0} m",
        course.points.len(),
        p0.0,
        p0.1,
        p1.0,
        p1.1,
        field.height(course.dir_of(p0))
    );
    Ok(course)
}

/// The lines about the river plans of a run (after each flight): the plan's CPU time, the nodes carried and dropped, the reader's cache peak.
pub(crate) fn report(
    c: &crate::litperf::RiverCounters,
    cache: Option<planet_frame::river::CacheCounters>,
    out: &mut dyn Write,
) -> Result<(), String> {
    if c.plans == 0 {
        return writeln!(out, "  river plans: none (the world has no river source)").map_err(err);
    }
    writeln!(
        out,
        "  terrain plan nodes: levels {} to {}, up to {} at the finest
  river plans (a lookup in the coverage cache): {} frames, CPU {:.3} ms mean, {:.3} ms max; river nodes up to {}, dropped up to {}, NOT READY up to {} a frame ({} in all); coverage cache {} B at most; reader cache peak {} B of {} B",
        c.level_min,
        c.level_max,
        c.finest_nodes_max,
        c.plans,
        c.plan_ns_total as f64 / c.plans as f64 / 1.0e6,
        c.plan_ns_max as f64 / 1.0e6,
        c.nodes_max,
        c.dropped_max,
        c.not_ready_max,
        c.not_ready_total,
        c.cache_bytes_max,
        c.reader_peak_bytes,
        planet_cache::river_read::DEFAULT_CACHE_BYTES
    )
    .map_err(err)?;
    if let Some(k) = cache {
        writeln!(out, "  coverage cache: {} hits, {} misses, {} tiles built, {} evicted", k.hits, k.misses, k.builds, k.evictions)
            .map_err(err)?;
    }
    Ok(())
}

/// Checks the options of the scene: the fixture is named by the environment, never by a store option.
pub fn check_options(o: &Options) -> Result<(), String> {
    if o.store.is_some() || o.world.is_some() {
        return Err(format!("--scene {SCENE} takes its world from {STORE_ENV} (the fixture commit): --store and --world do not apply"));
    }
    Ok(())
}
