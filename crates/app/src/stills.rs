//! The pose stills (TEST-048): `test-render --scene capture-stills --poses FILE --out-dir DIR`, the lit path drawn at fixed poses over a
//! world with its A-net chunk stage, for the owner's look reviews (M8-4, M8-8) and later regression checks.
//!
//! A still waits for its chunks: the loop plans the pose, asks the chunk manager for every chunk the plan's tiles read, and plans again
//! over the field that includes what landed until every tile of the plan reads only ready chunks. So no tile of the image is a
//! stand-in; when the chunks are not all ready within the timeout the run fails and names what was missing.

use crate::litperf::{capture_lod, new_renderer, request, LitSetup, Rig, WATER_T0};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::chunkshare::ChunkShare;
use planet_frame::{Camera, SelectionCache};
use planet_generators::chunkfield::ChunkKey;
use planet_render::GpuContext;
use planet_streaming::chunks::ChunkConfig;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Whether `--scene NAME` names the pose stills.
pub fn is_stills_scene(name: &str) -> bool {
    name == "capture-stills"
}

/// Rounds of settling the chunks and moving the eye to the ground they give that a pose may take.
const MAX_CAMERA_ROUNDS: usize = 4;

/// Seconds a pose may wait for its chunks when the file gives no `chunk_timeout_s`.
pub const DEFAULT_CHUNK_TIMEOUT_S: f64 = 900.0;

/// The local relief an anchor must stand in: the highest minus the lowest coupled-run height within `radius_m` of the cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relief {
    pub radius_m: f64,
    pub min_m: f64,
    pub max_m: f64,
}

/// Where the poses look: the largest river of the world's coupled run, or a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// The land cell of the coupled run at least this many metres high with the largest drainage area that drains on (0: the cell of the
    /// chunk scenes, which can be a river mouth over the sea; a higher bound puts the anchor inland on the river).
    /// With `relief`, only cells whose surrounding ground (`radius_m` around, by lattice steps) spans a height range inside `min_m..=max_m`
    /// (hilly ground); the river is the largest drainage area among those.
    LargestRiver { min_height_m: f64, relief: Option<Relief> },
    /// The land river cell of the coupled run whose ground on the base field is `ground_min_m` to `ground_max_m` high and whose surroundings
    /// within `radius_m` have the steepest slope on the base field (sampled at the bake's cell spacing), among the cells with a drainage area of
    /// at least `min_area_m2`.
    SteepestRiver { ground_min_m: f64, ground_max_m: f64, radius_m: f64, min_area_m2: f64 },
    /// Latitude and longitude in degrees.
    LatLon(f64, f64),
}

/// One still: the eye `back_m` before the anchor against the heading, `altitude_m` above the ground there, looking along the heading
/// pitched down by `pitch_deg`, with the sun at `sun_local_hours` of local solar time at the anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub name: String,
    pub altitude_m: f64,
    pub back_m: f64,
    /// Degrees clockwise from north.
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub sun_local_hours: f64,
    pub sun_declination_deg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StillsConfig {
    pub anchor: Anchor,
    pub chunk_timeout_s: f64,
    /// The cell of the chunks in metres (the viewer's 8 m unless the file says otherwise; a test uses coarse chunks to stay quick).
    pub chunk_cell_m: f64,
    /// `[place] snap_to_channel_m`: the anchor moves to the lowest ground of the landed chunks within this distance of it (the channel the
    /// chunk stage cut), and a pose's `heading_deg` is then relative to the channel's downstream bearing (0: downstream).
    pub snap_to_channel_m: Option<f64>,
    /// `[place] slope_table_stride`: write `slopes.txt` ([`slope_table`]) over every this-many-th cell of the coupled lattice before the poses.
    pub slope_table_stride: Option<usize>,
    pub poses: Vec<Pose>,
}

/// A value as the file wrote it, for a message.
fn show(v: &toml::Value) -> String {
    v.as_str().map_or_else(|| format!("{v:?}"), |s| format!("{s:?}"))
}

fn number(t: &toml::Table, key: &str, whose: &str) -> Result<Option<f64>, String> {
    match t.get(key) {
        None => Ok(None),
        Some(v) => match v.as_float().or_else(|| v.as_integer().map(|i| i as f64)) {
            Some(x) if x.is_finite() => Ok(Some(x)),
            _ => Err(format!("{whose}: '{key}' must be a finite number, got {}", show(v))),
        },
    }
}

fn required(t: &toml::Table, key: &str, whose: &str) -> Result<f64, String> {
    number(t, key, whose)?.ok_or_else(|| format!("{whose}: '{key}' is missing"))
}

/// Read the poses file: `[place]` (`anchor = "largest-river"`, or `lat` and `lon` in degrees), an optional `chunk_timeout_s`, and one
/// `[[pose]]` per still with `name`, `altitude_m`, `back_m`, `heading_deg`, `pitch_deg` and `sun_local_hours` (`sun_declination_deg`
/// defaults to 0). A bad file is refused naming the key and the value.
pub fn parse_config(text: &str) -> Result<StillsConfig, String> {
    let root: toml::Table = text.parse().map_err(|e| format!("the poses file: {e}"))?;
    let place = root.get("place").and_then(|v| v.as_table()).ok_or("the poses file: [place] is missing")?;
    let anchor = match (place.get("anchor"), place.get("lat"), place.get("lon")) {
        (Some(a), None, None) if a.as_str() == Some("largest-river") => {
            let relief = match (number(place, "relief_radius_m", "[place]")?, number(place, "relief_min_m", "[place]")?) {
                (Some(radius_m), Some(min_m)) => {
                    let max_m = number(place, "relief_max_m", "[place]")?.unwrap_or(f64::INFINITY);
                    if radius_m <= 0.0 || min_m < 0.0 || max_m < min_m {
                        return Err(format!("[place]: relief_radius_m {radius_m}, relief_min_m {min_m}, relief_max_m {max_m} must be positive, non-negative and ordered"));
                    }
                    Some(Relief { radius_m, min_m, max_m })
                }
                (None, None) => None,
                _ => return Err("[place]: relief_radius_m and relief_min_m go together".into()),
            };
            Anchor::LargestRiver { min_height_m: number(place, "min_height_m", "[place]")?.unwrap_or(0.0), relief }
        }
        (Some(a), None, None) if a.as_str() == Some("steepest-river") => {
            let ground_min_m = number(place, "ground_min_m", "[place]")?.unwrap_or(0.0);
            let ground_max_m = number(place, "ground_max_m", "[place]")?.unwrap_or(f64::INFINITY);
            let (radius_m, min_area_m2) =
                (required(place, "slope_radius_m", "[place]")?, number(place, "min_area_m2", "[place]")?.unwrap_or(0.0));
            if radius_m <= 0.0 || ground_max_m < ground_min_m || min_area_m2 < 0.0 {
                return Err(format!("[place]: ground {ground_min_m} to {ground_max_m}, slope_radius_m {radius_m}, min_area_m2 {min_area_m2} are not a range, a radius and an area"));
            }
            Anchor::SteepestRiver { ground_min_m, ground_max_m, radius_m, min_area_m2 }
        }
        (Some(a), None, None) => {
            return Err(format!(
                "[place]: anchor {} is unknown (known: \"largest-river\", \"steepest-river\"; or give lat and lon)",
                show(a)
            ))
        }
        (None, Some(_), Some(_)) => {
            let (lat, lon) = (required(place, "lat", "[place]")?, required(place, "lon", "[place]")?);
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return Err(format!("[place]: lat {lat} and lon {lon} must be within 90 and 180 degrees"));
            }
            Anchor::LatLon(lat, lon)
        }
        _ => return Err("[place]: give anchor = \"largest-river\", or lat and lon".into()),
    };
    let chunk_timeout_s = number(&root, "chunk_timeout_s", "the poses file")?.unwrap_or(DEFAULT_CHUNK_TIMEOUT_S);
    if chunk_timeout_s <= 0.0 {
        return Err(format!("chunk_timeout_s must be positive, got {chunk_timeout_s}"));
    }
    let chunk_cell_m = number(&root, "chunk_cell_m", "the poses file")?.unwrap_or(crate::anetworld::ANET_CELL_M);
    if chunk_cell_m <= 0.0 {
        return Err(format!("chunk_cell_m must be positive, got {chunk_cell_m}"));
    }
    let list = root.get("pose").and_then(|v| v.as_array()).ok_or("the poses file: no [[pose]]")?;
    let mut poses: Vec<Pose> = Vec::new();
    for (i, v) in list.iter().enumerate() {
        let t = v.as_table().ok_or_else(|| format!("[[pose]] {}: not a table", i + 1))?;
        let name = t.get("name").and_then(|n| n.as_str()).ok_or_else(|| format!("[[pose]] {}: 'name' is missing", i + 1))?.to_string();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
            return Err(format!("[[pose]] {}: name '{name}' must be letters, digits, '-', '_' or '.' (it names the PNG)", i + 1));
        }
        if poses.iter().any(|p| p.name == name) {
            return Err(format!("[[pose]] '{name}' is given twice"));
        }
        let whose = format!("pose '{name}'");
        let pose = Pose {
            altitude_m: required(t, "altitude_m", &whose)?,
            back_m: required(t, "back_m", &whose)?,
            heading_deg: required(t, "heading_deg", &whose)?,
            pitch_deg: required(t, "pitch_deg", &whose)?,
            sun_local_hours: required(t, "sun_local_hours", &whose)?,
            sun_declination_deg: number(t, "sun_declination_deg", &whose)?.unwrap_or(0.0),
            name,
        };
        if pose.altitude_m <= 0.0 || pose.back_m < 0.0 {
            return Err(format!(
                "{whose}: altitude_m must be positive and back_m not negative, got {} and {}",
                pose.altitude_m, pose.back_m
            ));
        }
        if !(-90.0..=90.0).contains(&pose.pitch_deg) {
            return Err(format!("{whose}: pitch_deg {} must be within 90 degrees", pose.pitch_deg));
        }
        poses.push(pose);
    }
    if poses.is_empty() {
        return Err("the poses file: no [[pose]]".into());
    }
    let snap_to_channel_m = number(place, "snap_to_channel_m", "[place]")?;
    if snap_to_channel_m.is_some_and(|m| m <= 0.0) {
        return Err(format!("[place]: snap_to_channel_m {} must be positive", snap_to_channel_m.unwrap_or(0.0)));
    }
    let slope_table_stride = match number(place, "slope_table_stride", "[place]")? {
        None => None,
        Some(k) if k >= 1.0 && k.fract() == 0.0 => Some(k as usize),
        Some(k) => return Err(format!("[place]: slope_table_stride {k} must be a whole number of at least 1")),
    };
    Ok(StillsConfig { anchor, chunk_timeout_s, chunk_cell_m, snap_to_channel_m, slope_table_stride, poses })
}

/// The unit vector towards the sun when it stands at `local_hours` of local solar time (12 is noon) at longitude `lon_deg`, with
/// the declination `declination_deg`: the planet-fixed axes of COORD, through [`planet_frame::look::shading::sun_direction`].
pub fn sun_for(local_hours: f64, lon_deg: f64, declination_deg: f64) -> [f32; 3] {
    // The sub-solar longitude of the shading helper is -15 degrees per hour from noon at longitude 0; local time T at longitude L is the
    // time at which the sun is (T - 12) * 15 degrees west of L.
    planet_frame::look::shading::sun_direction(local_hours - lon_deg / 15.0, declination_deg)
}

/// The unit direction at latitude and longitude in degrees.
fn direction_of(lat_deg: f64, lon_deg: f64) -> Vec3 {
    Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians())
}

/// The unit tangent at `at` pointing `heading_deg` clockwise from north.
fn heading_vector(at: Vec3, heading_deg: f64) -> Vec3 {
    let (lat, lon) = at.lat_lon();
    let north = Vec3::new(-libm::sin(lat) * libm::cos(lon), -libm::sin(lat) * libm::sin(lon), libm::cos(lat));
    let east = Vec3::new(-libm::sin(lon), libm::cos(lon), 0.0);
    let h = heading_deg.to_radians();
    (north * libm::cos(h) + east * libm::sin(h)).normalized()
}

/// The camera of a pose: the eye on the great circle `back_m` behind `anchor` against the heading, `altitude_m` above the ground or the
/// sea there (the higher), looking along the heading pitched down. `ground_m` is the height above the reference sphere.
pub fn pose_camera(
    anchor: Vec3,
    pose: &Pose,
    ground_m: &dyn Fn(Vec3) -> f64,
    radius_m: f64,
    level_m: f64,
    aspect: f64,
    fov_y_rad: f64,
) -> Camera {
    let heading = heading_vector(anchor, pose.heading_deg);
    let theta = pose.back_m / radius_m;
    let eye_dir = (anchor * libm::cos(theta) - heading * libm::sin(theta)).normalized();
    let eye_alt = ground_m(eye_dir).max(level_m) + pose.altitude_m;
    // The heading carried to the eye: the tangent at the eye along the great circle through the anchor.
    let ahead = (anchor - eye_dir * anchor.dot(eye_dir)).normalized();
    let ahead = if pose.back_m > 0.0 { ahead } else { heading };
    let near = (pose.altitude_m * 0.05).clamp(0.05, 0.1);
    Camera::at_surface_point(PlanetFixed(eye_dir * (radius_m + eye_alt)), ahead, pose.pitch_deg.to_radians(), fov_y_rad, aspect, near)
}

/// The chunk keys the tiles of a plan read.
fn needed(world: &WorldTerrain, tiles: &[TileId]) -> Vec<ChunkKey> {
    let Some(m) = world.chunk_manager() else { return Vec::new() };
    let mut seen = HashSet::new();
    tiles.iter().flat_map(|&t| m.reads(t)).filter(|k| seen.insert(*k)).collect()
}

/// A still waited too long for its chunks.
fn not_ready(pose: &str, secs: f64, missing: &[ChunkKey], needed_n: usize, in_flight: usize) -> String {
    let some: Vec<String> = missing.iter().take(4).map(|k| format!("face {} x {} y {}", k.0, k.1, k.2)).collect();
    format!(
        "pose '{pose}': {} of {needed_n} chunks were not ready after {secs:.0} s ({in_flight} jobs in flight; first missing: {}); raise chunk_timeout_s, or check the chunk cache's byte cap",
        missing.len(),
        some.join(", ")
    )
}

/// The result of waiting for one pose: the field is rebuilt by the caller from the world; the number of rounds and the chunks it read.
struct Settled {
    rounds: usize,
    chunks: usize,
    waited_s: f64,
}

/// Plan `camera` until every tile of the plan reads only chunks that were ready before the plan's field was built, driving the chunk
/// jobs nearest first. Fails with [`not_ready`] after `timeout_s`.
fn settle(
    o: &Options,
    ctx: &GpuContext,
    world: &WorldTerrain,
    pose: &Pose,
    camera: Camera,
    size: (u32, u32),
    timeout_s: f64,
) -> Result<Settled, String> {
    let manager = world.chunk_manager().ok_or("the world has no chunk stage")?;
    let started = Instant::now();
    let mut wanted: HashSet<ChunkKey> = HashSet::new();
    let mut rounds = 0;
    loop {
        rounds += 1;
        // Ready is monotonic while the chunks are wanted, so what is ready now is in the snapshot of the field built after this line.
        let ready: HashSet<ChunkKey> = wanted.iter().copied().filter(|k| manager.is_ready(*k)).collect();
        let field = world.field()?;
        let lod = capture_lod(o, size, world.radius_m(), field.as_ref())?;
        let rig = Rig {
            ctx,
            world,
            field: field.as_ref(),
            lod: &lod,
            size,
            smoke: true,
            calibrate: false,
            frames: 0,
            cells: o.cells,
            radius_m: world.radius_m(),
        };
        let plan = planet_frame::plan_terrain_cached(&request(&rig, camera), field.as_ref(), &mut SelectionCache::new())
            .map_err(|e| format!("pose '{}': {e}", pose.name))?;
        let tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
        let reads = needed(world, &tiles);
        let missing: Vec<ChunkKey> = reads.iter().copied().filter(|k| !ready.contains(k)).collect();
        if missing.is_empty() {
            return Ok(Settled { rounds, chunks: reads.len(), waited_s: started.elapsed().as_secs_f64() });
        }
        wanted.extend(reads.iter().copied());
        drop(plan);
        drop(field);
        world.chunk_frame(&tiles, camera.position, &[]);
        let waited = started.elapsed().as_secs_f64();
        if waited > timeout_s {
            let still: Vec<ChunkKey> = missing.into_iter().filter(|k| !manager.is_ready(*k)).collect();
            return Err(not_ready(&pose.name, waited, &still, reads.len(), manager.in_flight()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Wait for the coupled run of the world (made and stored if the store holds none) and give the world its A-net chunk stage.
fn enable_chunks(
    world: &Arc<WorldTerrain>,
    cell_m: f64,
    workers: usize,
    out: &mut dyn Write,
) -> Result<Arc<planet_generators::coupled::CoupledMaps>, String> {
    use crate::coupledworld::Standing;
    world.request_coupled();
    let started = Instant::now();
    let maps = loop {
        match world.coupled_standing() {
            Standing::Ready(m, _) => break m,
            Standing::Failed(e) => return Err(format!("the coupled run failed: {e}")),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    writeln!(out, "coupled run ready after {:.1} s", started.elapsed().as_secs_f64()).map_err(err)?;
    let pool = planet_streaming::GenWorkers::new(workers).pool();
    let cfg = ChunkConfig { workers, evict_ms: 3_600_000, cap_bytes: 4 << 30, publish_gate: false };
    world.enable_anet(maps.clone(), cell_m, pool, Arc::new(planet_streaming::SystemClock::new()), cfg)?;
    // The horizon shadows (REND-038): the second job of every chunk, and the stored far-field horizon read now (a still waits for both).
    let lat = world.chunk_lattice().ok_or("the world has no chunk lattice")?;
    let params = crate::horizonworld::chunk_horizon_params(&lat, world.radius_m());
    let horizon_pool = planet_streaming::GenWorkers::new(workers).pool();
    world.enable_horizon(
        params,
        horizon_pool,
        Arc::new(planet_streaming::SystemClock::new()),
        planet_streaming::chunks::HorizonConfig { workers, settle_ms: 1_000 },
        workers,
    )?;
    world.request_coarse_horizon();
    let coarse = loop {
        match world.coarse_standing() {
            crate::horizonworld::CoarseStanding::Idle | crate::horizonworld::CoarseStanding::Working => {
                std::thread::sleep(Duration::from_millis(50))
            }
            crate::horizonworld::CoarseStanding::Ready(h) => {
                break format!("coarse horizon: {} observer cells per face edge, {} azimuths", h.m, h.azimuths)
            }
            crate::horizonworld::CoarseStanding::Absent(t) | crate::horizonworld::CoarseStanding::Failed(t) => {
                break format!("horizon: {t}")
            }
        }
    };
    writeln!(out, "{coarse}").map_err(err)?;
    Ok(maps)
}

/// How far from the eye a still waits for chunk horizon maps, metres (two chunk edges and a bit).
const HORIZON_WAIT_M: f64 = 12_000.0;

/// Run the horizon jobs for the plan `tiles` until every chunk a tile lies in has its map and no job is in flight, or `timeout_s` passes
/// (an error naming the chunks that have none). Returns the number of maps.
fn settle_horizon(world: &WorldTerrain, pose: &str, tiles: &[TileId], camera: PlanetFixed, timeout_s: f64) -> Result<usize, String> {
    let started = Instant::now();
    // The chunks within `HORIZON_WAIT_M` of the eye: the farther ones read the coarse horizon (the stand-in rule), as they do in the viewer.
    let radius_m = world.radius_m();
    let keys: Vec<ChunkKey> = {
        let mut seen = HashSet::new();
        tiles
            .iter()
            .filter_map(|&t| planet_frame::horizonview::tile_in_chunk(t).map(|(k, _)| k))
            .filter(|k| seen.insert(*k))
            // A chunk no tile reads is not made (a coarse tile lies in it): its tiles read the coarse horizon.
            .filter(|k| world.chunk_manager().is_some_and(|m| m.is_ready(*k)))
            .filter(|k| {
                let tile =
                    TileId::new(planet_core::Face(k.0), planet_generators::chunk::CHUNK_GRID_LEVEL, k.1, k.2).expect("a chunk of the grid");
                (tile.center_direction(&planet_core::cube::TangentWarp) * radius_m - camera.0).length() <= HORIZON_WAIT_M
            })
            .collect()
    };
    loop {
        world.horizon_frame(tiles, camera);
        world.chunk_frame(tiles, camera, &[]);
        let missing: Vec<&ChunkKey> = keys.iter().filter(|k| world.horizon_map(**k).is_none()).collect();
        if missing.is_empty() {
            return Ok(keys.len());
        }
        let waited = started.elapsed().as_secs_f64();
        if waited > timeout_s {
            let some: Vec<String> = missing.iter().take(4).map(|k| format!("face {} x {} y {}", k.0, k.1, k.2)).collect();
            return Err(format!(
                "pose '{pose}': {} of {} chunks had no horizon map after {waited:.0} s (first missing: {}); raise chunk_timeout_s",
                missing.len(),
                keys.len(),
                some.join(", ")
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The highest minus the lowest height of the coupled run over the cells within `radius_m` of cell `c` (flood over the eight neighbours, by
/// the great-circle distance of the cell centres).
pub fn local_relief_m(maps: &planet_generators::coupled::CoupledMaps, c: usize, radius_m: f64) -> f64 {
    let lat = maps.lattice();
    let centre = lat.direction(c);
    let sphere_m = lat.radius_m();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![c];
    seen.insert(c);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    while let Some(k) = stack.pop() {
        let z = f64::from(maps.z(k));
        lo = lo.min(z);
        hi = hi.max(z);
        for n in lat.neighbours(k) {
            let n = n as usize;
            if n as u32 == planet_generators::coupled::NONE || seen.contains(&n) {
                continue;
            }
            let arc = libm::acos(centre.dot(lat.direction(n)).clamp(-1.0, 1.0)) * sphere_m;
            if arc <= radius_m {
                seen.insert(n);
                stack.push(n);
            }
        }
    }
    hi - lo
}

/// The direction of the anchor and, for a river cell, the bearing of the river's flow there (degrees clockwise from north), so the poses can
/// be given along the river.
fn anchor_direction(
    anchor: Anchor,
    maps: &planet_generators::coupled::CoupledMaps,
    ground: &dyn Fn(Vec3) -> f64,
    step_m: f64,
) -> Result<(Vec3, Option<f64>, String), String> {
    let (dir, flow) = anchor_cell(anchor, maps, ground, step_m)?;
    let note = match anchor {
        Anchor::SteepestRiver { radius_m, .. } => {
            let (slope, relief) = local_slope(ground, dir, radius_m, step_m, maps.lattice().radius_m());
            format!(", steepest slope {:.1} degrees and relief {relief:.0} m within {radius_m:.0} m", libm::atan(slope).to_degrees())
        }
        _ => String::new(),
    };
    Ok((dir, flow, note))
}

/// Sections of the ground across `heading` at 0 to 2000 m downstream of `anchor` every 100 m, `+-1000 m` every 8 m (the chunk cell): per section
/// the median height (the ground the channel is cut into), the lowest point and its offset from the anchor's line (positive to the right of
/// the heading), the width of what lies more than 20 m under the median, and the highest point over the median with its offset. One line per
/// section.
pub fn channel_sections(ground: &dyn Fn(Vec3) -> f64, anchor: Vec3, heading_deg: f64, planet_radius_m: f64) -> String {
    let mut out = String::from(
        "# downstream_m median_m lowest_m lowest_offset_m cut_width_m highest_over_median_m highest_offset_m
",
    );
    let ahead = heading_vector(anchor, heading_deg);
    for k in 0..=20 {
        let along = f64::from(k) * 100.0;
        // The point `along` downstream on the great circle through the anchor, and the tangent across the heading there.
        let theta = along / planet_radius_m;
        let here = (anchor * libm::cos(theta) + ahead * libm::sin(theta)).normalized();
        let forward = (ahead * libm::cos(theta) - anchor * libm::sin(theta)).normalized();
        let right = forward.cross(here).normalized();
        let row: Vec<(f64, f64)> = (-125..=125)
            .map(|i| (f64::from(i) * 8.0, ground((here * planet_radius_m + right * (f64::from(i) * 8.0)).normalized())))
            .collect();
        let mut sorted: Vec<f64> = row.iter().map(|r| r.1).collect();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let low = row.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((0.0, median));
        let high = row.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((0.0, median));
        let width = row.iter().filter(|r| r.1 < median - 20.0).count() as f64 * 8.0;
        out.push_str(&format!(
            "{along:.0} {median:.1} {:.1} {:.0} {width:.0} {:.1} {:.0}
",
            low.1,
            low.0,
            high.1 - median,
            high.0
        ));
    }
    out
}

/// The channel nearest the coupled run's anchor as the chunks cut it: the lowest ground within `radius_m` of `anchor` (sampled every `step_m`),
/// which must lie at least 50 m under the median of the samples, and the bearing of the channel's floor downstream there (degrees clockwise
/// from north): the lowest point of a ring 500 m round it, among the points within 90 degrees of `prior_bearing` when there is one.
pub fn snap_to_channel(
    ground: &dyn Fn(Vec3) -> f64,
    anchor: Vec3,
    prior_bearing: Option<f64>,
    radius_m: f64,
    step_m: f64,
    planet_radius_m: f64,
) -> Result<(Vec3, f64), String> {
    let (north, east) = (heading_vector(anchor, 0.0), heading_vector(anchor, 90.0));
    let n = (radius_m / step_m).floor() as i64;
    let mut samples = Vec::new();
    for j in -n..=n {
        for i in -n..=n {
            let (x, y) = (i as f64 * step_m, j as f64 * step_m);
            if x * x + y * y <= radius_m * radius_m {
                let d = (anchor * planet_radius_m + east * x + north * y).normalized();
                samples.push((ground(d), d));
            }
        }
    }
    let mut heights: Vec<f64> = samples.iter().map(|s| s.0).collect();
    heights.sort_by(f64::total_cmp);
    let median = heights[heights.len() / 2];
    let (low, at) = samples.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0)).ok_or("no ground to look for a channel in")?;
    if low > median - 50.0 {
        return Err(format!("no channel within {radius_m} m of the anchor: the lowest ground is {low:.0} m, the median {median:.0} m"));
    }
    let (n2, e2) = (heading_vector(at, 0.0), heading_vector(at, 90.0));
    let ring = (0..72)
        .map(|k| f64::from(k) * 5.0)
        .filter(|b| prior_bearing.is_none_or(|p| ((b - p + 540.0).rem_euclid(360.0) - 180.0).abs() <= 90.0))
        .map(|b| {
            let h = b.to_radians();
            let d = (at * planet_radius_m + (n2 * libm::cos(h) + e2 * libm::sin(h)) * 500.0).normalized();
            (ground(d), b)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .ok_or("no direction to follow the channel in")?;
    Ok((at, ring.1))
}

/// Where the capped nodes of a plan sit (ADR 0074), one line per capped node: its level and edge, the height range and the steepest slope (rise over
/// run between neighbouring samples, axis and diagonal) of the heights `ground` gives on a grid of at most 8 m, the distance of its centre from the
/// nearest edge of its chunk (the level-11 tile), and the morph interval the plan gave it. Then totals by level and by slope.
pub fn capped_report(nodes: &[planet_frame::TerrainDraw], ground: &dyn Fn(Vec3) -> f64, planet_radius_m: f64) -> String {
    use planet_core::cube::{direction_to_face, TangentWarp};
    let chunk_level = planet_generators::chunk::CHUNK_GRID_LEVEL;
    let chunk_edge_m = planet_radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << chunk_level);
    let mut out =
        String::from("# level edge_m grid_res height_min_m height_max_m max_slope morph_start_m morph_end_m chunk_edge_distance_m\n");
    let (mut by_level, mut steep, mut total) = (std::collections::BTreeMap::<u8, usize>::new(), 0usize, 0usize);
    let mut slopes: Vec<f64> = Vec::new();
    let mut near_edge = 0usize;
    for n in nodes.iter().filter(|n| n.capped) {
        let level = n.tile.level();
        let edge_m = planet_radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level);
        let res = ((edge_m / 8.0).ceil() as u32).next_power_of_two().clamp(4, 128);
        let side = (res + 1) as usize;
        let mut h = vec![0.0f64; side * side];
        for l in 0..=res {
            for k in 0..=res {
                h[l as usize * side + k as usize] = ground(n.tile.sample_direction(&TangentWarp, res, k, l));
            }
        }
        let spacing = edge_m / f64::from(res);
        let (mut slope, mut lo, mut hi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
        for l in 0..side {
            for k in 0..side {
                let here = h[l * side + k];
                lo = lo.min(here);
                hi = hi.max(here);
                for (dk, dl) in [(1usize, 0usize), (0, 1), (1, 1)] {
                    if k + dk < side && l + dl < side {
                        let run = spacing * libm::sqrt((dk * dk + dl * dl) as f64);
                        slope = slope.max((h[(l + dl) * side + k + dk] - here).abs() / run);
                    }
                }
            }
        }
        let (_, u, v) = direction_to_face(&TangentWarp, n.tile.center_direction(&TangentWarp));
        let cells = f64::from(1u32 << chunk_level);
        let (fu, fv) = ((u + 1.0) * 0.5 * cells, (v + 1.0) * 0.5 * cells);
        let (fu, fv) = (fu - libm::floor(fu), fv - libm::floor(fv));
        let edge_distance = fu.min(1.0 - fu).min(fv.min(1.0 - fv)) * chunk_edge_m;
        out.push_str(&format!(
            "{level} {edge_m:.0} {res} {lo:.1} {hi:.1} {slope:.2} {:.0} {:.0} {edge_distance:.0}\n",
            n.morph_start_m, n.morph_end_m
        ));
        *by_level.entry(level).or_default() += 1;
        steep += usize::from(slope > 0.9);
        near_edge += usize::from(edge_distance < edge_m);
        total += 1;
        slopes.push(slope);
    }
    slopes.sort_by(f64::total_cmp);
    out.push_str(&format!(
        "# capped {total}; by level {by_level:?}; max slope p50 {:.2} p90 {:.2} max {:.2}; over the repose slope 0.9: {steep}; within one tile edge of a chunk edge: {near_edge}\n",
        slopes.get(slopes.len() / 2).copied().unwrap_or(f64::NAN),
        slopes.get(slopes.len() * 9 / 10).copied().unwrap_or(f64::NAN),
        slopes.last().copied().unwrap_or(f64::NAN)
    ));
    out
}

/// The steepest slope (rise over run between samples) and the height range of the ground within `radius_m` of `dir`, sampled on a grid of
/// `step_m` and read along the grid's lines and diagonals.
pub fn local_slope(ground: &dyn Fn(Vec3) -> f64, dir: Vec3, radius_m: f64, step_m: f64, planet_radius_m: f64) -> (f64, f64) {
    let n = (radius_m / step_m).floor() as i64;
    let side = (2 * n + 1) as usize;
    let (north, east) = (heading_vector(dir, 0.0), heading_vector(dir, 90.0));
    // Every sample inside the circle, read once.
    let mut grid: Vec<Option<f64>> = vec![None; side * side];
    for j in -n..=n {
        for i in -n..=n {
            let (x, y) = (i as f64 * step_m, j as f64 * step_m);
            if x * x + y * y <= radius_m * radius_m {
                grid[(j + n) as usize * side + (i + n) as usize] =
                    Some(ground((dir * planet_radius_m + east * x + north * y).normalized()));
            }
        }
    }
    let at = |i: i64, j: i64| -> Option<f64> {
        let (u, v) = (i + n, j + n);
        (u >= 0 && v >= 0 && u < side as i64 && v < side as i64).then(|| grid[v as usize * side + u as usize]).flatten()
    };
    let (mut slope, mut lo, mut hi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
    for j in -n..=n {
        for i in -n..=n {
            let Some(h) = at(i, j) else { continue };
            lo = lo.min(h);
            hi = hi.max(h);
            for (di, dj) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                if let Some(g) = at(i + di, j + dj) {
                    let run = step_m * libm::sqrt((di * di + dj * dj) as f64);
                    slope = slope.max((g - h).abs() / run);
                }
            }
        }
    }
    (slope, hi - lo)
}

/// Ground bands of the slope table, in metres: lower bound inclusive, upper exclusive.
const BANDS: [(f64, f64); 5] = [(0.0, 500.0), (500.0, 1000.0), (1000.0, 2500.0), (2500.0, 5000.0), (5000.0, f64::INFINITY)];

/// The 2 km slope on the base field by ground band: over the land cells of the coupled lattice taken every `stride` cells (offset by half a
/// stride) and over its river cells (drainage area at least `min_area_m2`, every cell). Per band: the share of the land (or of the rivers) and
/// the 50th, 90th and 99th percentile of the steepest slope in degrees within `radius_m` (sampled every `step_m`, see [`local_slope`]).
/// Land is ground over `sea_m`. The result does not depend on the thread count.
pub fn slope_table(
    maps: &planet_generators::coupled::CoupledMaps,
    ground: &(dyn Fn(Vec3) -> f64 + Sync),
    sea_m: f64,
    radius_m: f64,
    step_m: f64,
    stride: usize,
    min_area_m2: f64,
) -> String {
    let lat = maps.lattice();
    let (n, planet) = (lat.n(), lat.radius_m());
    let stride = stride.max(1);
    let land: Vec<usize> = (0..6)
        .flat_map(|f| {
            (stride / 2..n).step_by(stride).flat_map(move |j| (stride / 2..n).step_by(stride).map(move |i| f * n * n + j * n + i))
        })
        .collect();
    let rec = maps.receivers();
    let rivers: Vec<usize> = (0..lat.len())
        .filter(|&c| rec[c] != planet_generators::coupled::NONE && f64::from(maps.network_area_m2(c)) >= min_area_m2)
        .collect();
    // (ground, slope in degrees) of every cell that stands over the sea, computed on all threads, kept in cell order.
    let measure = |cells: &[usize]| -> Vec<(f64, f64)> {
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let chunk = cells.len().div_ceil(threads).max(1);
        let parts: Vec<Vec<(f64, f64)>> = std::thread::scope(|sc| {
            let jobs: Vec<_> = cells
                .chunks(chunk)
                .map(|part| {
                    sc.spawn(move || {
                        part.iter()
                            .filter_map(|&c| {
                                let d = lat.direction(c);
                                let h = ground(d);
                                (h > sea_m).then(|| (h, libm::atan(local_slope(ground, d, radius_m, step_m, planet).0).to_degrees()))
                            })
                            .collect()
                    })
                })
                .collect();
            jobs.into_iter().map(|j| j.join().expect("a slope worker")).collect()
        });
        parts.concat()
    };
    let percentile = |v: &mut Vec<f64>, q: f64| -> f64 {
        v.sort_by(f64::total_cmp);
        v.get(((v.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(f64::NAN)
    };
    let mut out = format!(
        "# the steepest slope in degrees within {radius_m:.0} m (grid {step_m:.0} m) of the base field by ground band; land: every {stride}th lattice cell over the sea; rivers: every cell of at least {min_area_m2:e} m2
"
    );
    out.push_str(
        "# band_m land_share_pct land_cells land_p50 land_p90 land_p99 river_share_pct river_cells river_p50 river_p90 river_p99
",
    );
    let (land, rivers) = (measure(&land), measure(&rivers));
    for (lo, hi) in BANDS {
        let pick = |all: &[(f64, f64)]| -> Vec<f64> { all.iter().filter(|(h, _)| *h >= lo && *h < hi).map(|(_, s)| *s).collect() };
        let (mut l, mut r) = (pick(&land), pick(&rivers));
        let share = |k: usize, of: usize| 100.0 * k as f64 / of.max(1) as f64;
        let name = if hi.is_finite() { format!("{lo:.0}-{hi:.0}") } else { format!(">{lo:.0}") };
        let (ln, rn) = (l.len(), r.len());
        out.push_str(&format!(
            "{name} {:.1} {ln} {:.2} {:.2} {:.2} {:.1} {rn} {:.2} {:.2} {:.2}
",
            share(ln, land.len()),
            percentile(&mut l, 0.5),
            percentile(&mut l, 0.9),
            percentile(&mut l, 0.99),
            share(rn, rivers.len()),
            percentile(&mut r, 0.5),
            percentile(&mut r, 0.9),
            percentile(&mut r, 0.99),
        ));
    }
    out
}

fn anchor_cell(
    anchor: Anchor,
    maps: &planet_generators::coupled::CoupledMaps,
    ground: &dyn Fn(Vec3) -> f64,
    step_m: f64,
) -> Result<(Vec3, Option<f64>), String> {
    match anchor {
        Anchor::LatLon(lat, lon) => Ok((direction_of(lat, lon), None)),
        Anchor::SteepestRiver { ground_min_m, ground_max_m, radius_m, min_area_m2 } => {
            let rec = maps.receivers();
            let lat = maps.lattice();
            let mut best: Option<(usize, f64)> = None;
            for (c, &r) in rec.iter().enumerate() {
                if r == planet_generators::coupled::NONE || f64::from(maps.network_area_m2(c)) < min_area_m2 {
                    continue;
                }
                let d = lat.direction(c);
                if !(ground_min_m..=ground_max_m).contains(&ground(d)) {
                    continue;
                }
                let (slope, _) = local_slope(ground, d, radius_m, step_m, lat.radius_m());
                if best.is_none_or(|(_, b)| slope > b) {
                    best = Some((c, slope));
                }
            }
            let (cell, _) = best.ok_or_else(|| {
                format!("the coupled run has no river of at least {min_area_m2} m2 with ground from {ground_min_m} to {ground_max_m} m")
            })?;
            let here = lat.direction(cell);
            let there = lat.direction(rec[cell] as usize);
            let (north, east) = (heading_vector(here, 0.0), heading_vector(here, 90.0));
            let bearing = libm::atan2(there.dot(east), there.dot(north)).to_degrees().rem_euclid(360.0);
            Ok((here, Some(bearing)))
        }
        Anchor::LargestRiver { min_height_m, relief } => {
            let rec = maps.receivers();
            let mut best: Option<(usize, f32)> = None;
            for (c, &r) in rec.iter().enumerate() {
                let a = maps.network_area_m2(c);
                if f64::from(maps.z(c)) > min_height_m.max(0.0)
                    && r != planet_generators::coupled::NONE
                    && best.is_none_or(|(_, b)| a > b)
                    && relief.is_none_or(|rl| {
                        let r = local_relief_m(maps, c, rl.radius_m);
                        (rl.min_m..=rl.max_m).contains(&r)
                    })
                {
                    best = Some((c, a));
                }
            }
            let (cell, _) = best.ok_or_else(|| {
                format!(
                    "the coupled run has no land river at least {min_height_m} m high{}",
                    if relief.is_some() { " in that relief" } else { "" }
                )
            })?;
            let lat = maps.lattice();
            let here = lat.direction(cell);
            let there = lat.direction(rec[cell] as usize);
            let (north, east) = (heading_vector(here, 0.0), heading_vector(here, 90.0));
            let bearing = libm::atan2(there.dot(east), there.dot(north)).to_degrees().rem_euclid(360.0);
            Ok((here, Some(bearing)))
        }
    }
}

/// `test-render --scene capture-stills --poses FILE --out-dir DIR [--store DIR | --allow-memory-bake --global-res N] [--size WxH]
/// [--views A,B]` (TEST-048): one PNG per pose, `DIR/<pose>.png`, and `DIR/stills.txt` with the camera and sun of each. A tool beside
/// the harness, never a measurement: no gate, no timestamps, no log; not to be run during a perf window.
pub fn capture_stills(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.out_dir.clone().ok_or("capture-stills needs --out-dir")?;
    let file = o.poses.clone().ok_or("capture-stills needs --poses FILE")?;
    let text = std::fs::read_to_string(&file).map_err(|e| format!("--poses {}: {e}", file.display()))?;
    let mut config = parse_config(&text).map_err(|e| format!("--poses {}: {e}", file.display()))?;
    if let Some(names) = &o.views {
        if let Some(bad) = names.iter().find(|n| !config.poses.iter().any(|p| &p.name == *n)) {
            let known: Vec<&str> = config.poses.iter().map(|p| p.name.as_str()).collect();
            return Err(format!("--views: no pose '{bad}' (the poses: {})", known.join(", ")));
        }
        config.poses.retain(|p| names.contains(&p.name));
    }
    let world = Arc::new(crate::streamperf::open_world(o, false)?.bounding_water());
    if !world.has_bake() {
        return Err("the stills need a world with a bake (--store DIR, or --allow-memory-bake with --global-res N): the chunk stage reads the coupled run".into());
    }
    let size = o.size.unwrap_or((2560, 1440));
    let aspect = f64::from(size.0) / f64::from(size.1);
    let radius_m = world.radius_m();
    writeln!(
        out,
        "capture-stills: NOT a measurement (no gate, no timestamps, no log); {}x{}, {} poses, {}{}",
        size.0,
        size.1,
        config.poses.len(),
        world.describe(),
        if o.store.is_some() { "" } else { " (in-memory bake)" }
    )
    .map_err(err)?;
    let workers = o.gen_workers().workers.max(1);
    let maps = enable_chunks(&world, config.chunk_cell_m, workers, out)?;
    let base = world.field()?;
    let step_m = std::f64::consts::FRAC_PI_2 * radius_m / f64::from(world.global_res());
    let (mut anchor, flow, measured) = anchor_direction(config.anchor, &maps, &|d| base.height(d), step_m)?;
    let (lat, lon) = anchor.lat_lon();
    let ground = base.height(anchor);
    let anchor_line = format!(
        "anchor: latitude {:.4} longitude {:.4} degrees, ground {ground:.1} m{}",
        lat.to_degrees(),
        lon.to_degrees(),
        flow.map_or(String::new(), |b| format!(", the river flows towards bearing {b:.0} degrees{measured}"))
    );
    writeln!(out, "{anchor_line}").map_err(err)?;
    let mut notes = format!(
        "# {anchor_line}
"
    );
    let level_m = world.definition().sea(planet_world_def::BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
    if let Some(stride) = config.slope_table_stride {
        let min_area = match config.anchor {
            Anchor::SteepestRiver { min_area_m2, .. } => min_area_m2,
            _ => 1e10,
        };
        let sea_m =
            world.definition().sea(planet_world_def::BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
        let started = Instant::now();
        let table = slope_table(&maps, &|d| base.height(d), sea_m, 2000.0, step_m, stride, min_area);
        std::fs::create_dir_all(&dir).map_err(err)?;
        std::fs::write(dir.join("slopes.txt"), &table).map_err(err)?;
        writeln!(out, "slopes.txt written in {:.0} s", started.elapsed().as_secs_f64()).map_err(err)?;
    }
    // The chunk stage cuts its channel within a cell of the coupled run's anchor, not through it: with `snap_to_channel_m` the anchor moves onto the
    // channel once the chunks under the first pose have landed, and the headings are read from the channel's downstream bearing.
    let mut heading_base = 0.0;
    if let Some(snap_m) = config.snap_to_channel_m {
        let first = &config.poses[0];
        let fov = planet_frame::litscenes::FOV_DEG.to_radians();
        let camera = {
            let f = world.field()?;
            pose_camera(anchor, first, &|d| f.height(d), radius_m, level_m, aspect, fov)
        };
        settle(o, ctx, &world, first, camera, size, config.chunk_timeout_s)?;
        let f = world.field()?;
        let level = crate::anetworld::ANET_TOP_LEVEL;
        let (at, bearing) = snap_to_channel(&|d| f.height_at_level(d, level), anchor, flow, snap_m, 16.0, radius_m)?;
        let (lat, lon) = at.lat_lon();
        let snap_line = format!(
            "snapped to the channel, {:.0} m from the anchor: latitude {:.4} longitude {:.4} degrees, ground {:.1} m, downstream bearing {bearing:.0} degrees",
            libm::acos(anchor.dot(at).clamp(-1.0, 1.0)) * radius_m,
            lat.to_degrees(),
            lon.to_degrees(),
            f.height_at_level(at, level)
        );
        writeln!(out, "{snap_line}").map_err(err)?;
        notes += &format!(
            "# {snap_line}
"
        );
        (anchor, heading_base) = (at, bearing);
    }
    std::fs::create_dir_all(&dir).map_err(err)?;
    let mut index =
        notes + "# pose width height eye_lat_deg eye_lon_deg eye_height_m heading_deg pitch_deg sun_local_hours chunks waited_s nodes capped_nodes\n";
    for pose in &config.poses {
        let pose = &Pose { heading_deg: (pose.heading_deg + heading_base).rem_euclid(360.0), ..pose.clone() };
        let started = Instant::now();
        // The eye is `altitude_m` above the ground the chunks give, so the camera follows the field as chunks land: settle, move the
        // camera to the new ground, and settle again until the camera stays.
        let camera_over = |world: &WorldTerrain| -> Result<Camera, String> {
            let field = world.field()?;
            let fov = planet_frame::litscenes::FOV_DEG.to_radians();
            Ok(pose_camera(anchor, pose, &|d| field.height(d), radius_m, level_m, aspect, fov))
        };
        let mut camera = camera_over(&world)?;
        let mut settled = settle(o, ctx, &world, pose, camera, size, config.chunk_timeout_s)?;
        for attempt in 1.. {
            let next = camera_over(&world)?;
            if next.position == camera.position {
                break;
            }
            if attempt > MAX_CAMERA_ROUNDS {
                return Err(format!(
                    "pose '{}': the eye did not stay over the ground after {MAX_CAMERA_ROUNDS} rounds of chunks",
                    pose.name
                ));
            }
            camera = next;
            let again = settle(o, ctx, &world, pose, camera, size, config.chunk_timeout_s)?;
            settled = Settled { rounds: settled.rounds + again.rounds, chunks: again.chunks, waited_s: settled.waited_s + again.waited_s };
        }
        // The field of the settled round includes every chunk the plan reads.
        let field = world.field()?;
        let lod = capture_lod(o, size, radius_m, field.as_ref())?;
        let mut setup = LitSetup::new(o, &world, &lod)?;
        let rig = Rig {
            ctx,
            world: &world,
            field: field.as_ref(),
            lod: &lod,
            size,
            smoke: true,
            calibrate: false,
            frames: 0,
            cells: o.cells,
            radius_m,
        };
        let plan =
            planet_frame::plan_terrain_cached(&request(&rig, camera), rig.field, &mut SelectionCache::new()).map_err(|e| e.to_string())?;
        if std::ptr::eq(pose, &config.poses[0]) || pose.name == config.poses[0].name {
            let level = crate::anetworld::ANET_TOP_LEVEL;
            let report = capped_report(&plan.nodes, &|d| field.height_at_level(d, level), radius_m);
            std::fs::write(dir.join("capped.txt"), report).map_err(err)?;
        }
        let tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
        let missing = needed(&world, &tiles).into_iter().filter(|k| !world.chunk_manager().is_some_and(|m| m.is_ready(*k))).count();
        if missing > 0 {
            return Err(format!(
                "pose '{}': {missing} chunks left the cache between the plan and the draw (the byte cap is too small)",
                pose.name
            ));
        }
        let share = world.chunk_view().map(|v| ChunkShare::of(tiles.iter().copied(), v.as_ref()));
        if let Some(s) = share.filter(|s| s.chunk_stand_in > 0) {
            return Err(format!("pose '{}': {} of {} chunk nodes are stand-in", pose.name, s.chunk_stand_in, s.chunk_nodes));
        }
        // The horizon maps of the chunks the tiles lie in (REND-038): a still shows the shadows, never the stand-in.
        let horizon_started = Instant::now();
        let horizon_maps = settle_horizon(&world, &pose.name, &tiles, camera.position, config.chunk_timeout_s)?;
        let horizon_s = horizon_started.elapsed().as_secs_f64();
        let mut r = new_renderer(&rig, &setup, camera)?;
        crate::skeleton::upload_missing_meshes(ctx, Some(&world), &mut r, &plan, rig.field, radius_m, o.cells)?;
        let (_, eye_lon) = camera.position.0.lat_lon();
        let sun = sun_for(pose.sun_local_hours, eye_lon.to_degrees(), pose.sun_declination_deg);
        let mut horizon = crate::horizonworld::HorizonRig::default();
        // The exposure adapts over a few frames of half a second, the first snapping to its target.
        for k in 0..4 {
            let mut lit = setup.plan(&world, rig.field, &plan, WATER_T0 + k as f64 * 0.5, 0.5, k == 0)?;
            if let Some(l) = lit.lit.as_mut() {
                l.sun_dir = sun;
            }
            // One map is uploaded a frame: the frames of the exposure give the first four, the settling frames the rest.
            horizon.frame(&world, ctx, &mut r, &tiles, camera.position, sun);
            r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
        }
        for _ in 0..horizon_maps {
            horizon.frame(&world, ctx, &mut r, &tiles, camera.position, sun);
        }
        let mut lit = setup.plan(&world, rig.field, &plan, WATER_T0 + 2.0, 0.5, false)?;
        if let Some(l) = lit.lit.as_mut() {
            l.sun_dir = sun;
        }
        r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
        r.read_color(ctx).write_png(&dir.join(format!("{}.png", pose.name))).map_err(|e| format!("writing {}: {e}", pose.name))?;
        let (eye_lat, _) = camera.position.0.lat_lon();
        let height = camera.position.0.length() - radius_m;
        writeln!(
            out,
            "{}: {} chunks after {} rounds and {:.1} s, {horizon_maps} horizon maps in {horizon_s:.1} s, drawn in {:.1} s -> {}",
            pose.name,
            settled.chunks,
            settled.rounds,
            settled.waited_s,
            started.elapsed().as_secs_f64(),
            dir.join(format!("{}.png", pose.name)).display()
        )
        .map_err(err)?;
        index += &format!(
            "{} {} {} {:.5} {:.5} {:.1} {} {} {} {} {:.1} {} {}\n",
            pose.name,
            size.0,
            size.1,
            eye_lat.to_degrees(),
            eye_lon.to_degrees(),
            height,
            pose.heading_deg,
            pose.pitch_deg,
            pose.sun_local_hours,
            settled.chunks,
            settled.waited_s,
            plan.nodes.len(),
            plan.nodes.iter().filter(|n| n.capped).count()
        );
    }
    std::fs::write(dir.join("stills.txt"), index).map_err(err)?;
    // The channel below the anchor as the landed chunks give it: sections across the heading, 8 m apart (the chunk cell), every 100 m downstream.
    let heading = config.poses.first().map_or(0.0, |p| p.heading_deg + heading_base);
    let field = world.field()?;
    let sections = channel_sections(&|d| field.height_at_level(d, crate::anetworld::ANET_TOP_LEVEL), anchor, heading, radius_m);
    std::fs::write(dir.join("sections.txt"), sections).map_err(err)?;
    writeln!(out, "wrote {} (one PNG per pose, stills.txt and sections.txt)", dir.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[place]
anchor = "largest-river"

[[pose]]
name = "300m-sunset"
altitude_m = 300
back_m = 1200.0
heading_deg = 90
pitch_deg = 12
sun_local_hours = 17.5

[[pose]]
name = "1.8m-midday"
altitude_m = 1.8
back_m = 0
heading_deg = 0
pitch_deg = 0
sun_local_hours = 12
sun_declination_deg = 10
"#;

    // spec: TEST-051
    #[test]
    fn the_relief_of_an_anchor_is_the_height_range_around_it_and_the_poses_file_asks_for_it() {
        use planet_generators::coupled::{CoupledMaps, Lattice};
        let radius = 6.0e6;
        let lat = Lattice::new(8, radius);
        let cell_m = lat.area_m2(0).sqrt();
        let mut z = vec![100.0f32; lat.len()];
        let c = lat.cell_at(direction_of(10.0, 10.0));
        z[c] = 900.0;
        let near = lat.neighbours(c)[0] as usize;
        z[near] = -100.0;
        let maps = CoupledMaps::new(lat, z, vec![1.0; 6 * 64], 0);
        // A radius under one cell sees the cell alone; one cell and a half sees the edge neighbours and the 1000 m range.
        assert_eq!(local_relief_m(&maps, c, 0.5 * cell_m), 0.0);
        assert_eq!(local_relief_m(&maps, c, 1.5 * cell_m), 1000.0);
        let c = parse_config(&GOOD.replace(
            "anchor = \"largest-river\"",
            "anchor = \"largest-river\"
relief_radius_m = 10000
relief_min_m = 1000
relief_max_m = 2500",
        ))
        .unwrap();
        assert_eq!(
            c.anchor,
            Anchor::LargestRiver { min_height_m: 0.0, relief: Some(Relief { radius_m: 10000.0, min_m: 1000.0, max_m: 2500.0 }) }
        );
        assert!(parse_config(&GOOD.replace(
            "anchor = \"largest-river\"",
            "anchor = \"largest-river\"
relief_min_m = 5"
        ))
        .unwrap_err()
        .contains("go together"));
    }

    // spec: TEST-052
    #[test]
    fn the_slope_around_an_anchor_is_read_off_the_ground_and_the_poses_file_asks_for_a_steepest_river() {
        let radius = 6.0e6;
        let at = direction_of(10.0, 10.0);
        let east = heading_vector(at, 90.0);
        // A ramp of 0.5 rise over run towards the east: every pair of samples along it shows 0.5, the diagonals less or more by the projection.
        let ramp = |d: Vec3| 0.5 * radius * d.dot(east);
        let (slope, relief) = local_slope(&ramp, at, 1000.0, 100.0, radius);
        assert!((slope - 0.5).abs() < 0.01, "{slope}");
        assert!((relief - 1000.0).abs() < 60.0, "{relief}");
        let flat = local_slope(&|_| 3.0, at, 1000.0, 100.0, radius);
        assert_eq!(flat, (0.0, 0.0));
        let text = GOOD.replace(
            "anchor = \"largest-river\"",
            "anchor = \"steepest-river\"
ground_min_m = 1000
ground_max_m = 2500
slope_radius_m = 2000
min_area_m2 = 1e9",
        );
        assert_eq!(
            parse_config(&text).unwrap().anchor,
            Anchor::SteepestRiver { ground_min_m: 1000.0, ground_max_m: 2500.0, radius_m: 2000.0, min_area_m2: 1e9 }
        );
        assert!(parse_config(&GOOD.replace("anchor = \"largest-river\"", "anchor = \"steepest-river\""))
            .unwrap_err()
            .contains("'slope_radius_m' is missing"));
    }

    // spec: TEST-052
    #[test]
    fn the_anchor_snaps_to_the_channel_the_chunks_cut_and_the_sections_show_its_depth() {
        let radius = 6.0e6;
        let at = direction_of(10.0, 10.0);
        let (north, east) = (heading_vector(at, 0.0), heading_vector(at, 90.0));
        // A trench 300 m deep and 100 m wide along bearing 100 degrees, 600 m south-east of the anchor, whose floor falls 2 m per 100 m towards it.
        let bearing = 100.0f64.to_radians();
        let axis = north * libm::cos(bearing) + east * libm::sin(bearing);
        let side = axis.cross(at).normalized();
        let origin = at + (east * 400.0 - north * 450.0) * (1.0 / radius);
        let ground = move |d: Vec3| {
            let rel = (d - origin) * radius;
            let (along, across) = (rel.dot(axis), rel.dot(side));
            let floor = -0.02 * along;
            100.0 - if across.abs() < 50.0 { 300.0 + floor } else { 0.0 }
        };
        let (snapped, found) = snap_to_channel(&ground, at, Some(110.0), 1200.0, 16.0, radius).unwrap();
        let (rel, across) = ((snapped - origin) * radius, ((snapped - origin) * radius).dot(side));
        assert!(across.abs() < 50.0 && rel.length() < 2000.0, "{across}");
        assert!((found - 100.0).abs() <= 10.0, "downstream bearing {found}");
        let flat = snap_to_channel(&|_| 5.0, at, None, 1200.0, 16.0, radius).unwrap_err();
        assert!(flat.contains("no channel within 1200"), "{flat}");
        let sections = channel_sections(&ground, snapped, found, radius);
        let first: Vec<&str> = sections.lines().nth(1).unwrap().split(' ').collect();
        assert!(first[2].parse::<f64>().unwrap() < -150.0 && first[4].parse::<f64>().unwrap() > 50.0, "{sections}");
        assert_eq!(sections.lines().count(), 22, "a header and 21 sections");
        assert_eq!(
            parse_config(&GOOD.replace(
                "anchor = \"largest-river\"",
                "anchor = \"largest-river\"
snap_to_channel_m = 1200"
            ))
            .unwrap()
            .snap_to_channel_m,
            Some(1200.0)
        );
        assert!(parse_config(&GOOD.replace(
            "anchor = \"largest-river\"",
            "anchor = \"largest-river\"
snap_to_channel_m = 0"
        ))
        .unwrap_err()
        .contains("must be positive"));
    }

    // spec: TEST-052
    #[test]
    fn the_slope_table_counts_the_land_by_band_and_leaves_out_the_sea() {
        use planet_generators::coupled::{CoupledMaps, Lattice};
        let radius = 6.0e6;
        let lat = Lattice::new(8, radius);
        let n = lat.len();
        let maps = CoupledMaps::new(lat, vec![1.0; n], vec![1.0; n], 0);
        // The northern half is a plateau of 600 m, the southern half sea.
        let ground = |d: Vec3| if d.z > 0.0 { 600.0 } else { -10.0 };
        let table = slope_table(&maps, &ground, 0.0, 2000.0, 500.0, 2, 0.0);
        let rows: Vec<Vec<&str>> = table.lines().filter(|l| !l.starts_with('#')).map(|l| l.split(' ').collect()).collect();
        assert_eq!(rows.len(), 5, "{table}");
        assert_eq!(rows[0][0], "0-500");
        assert_eq!((rows[1][0], rows[1][1]), ("500-1000", "100.0"), "all the land is in the band of 600 m: {table}");
        assert_eq!((rows[1][3], rows[1][5]), ("0.00", "0.00"), "a plateau has no slope");
        assert_eq!((rows[0][2], rows[2][2], rows[4][2]), ("0", "0", "0"));
        assert!(rows[1][2].parse::<usize>().unwrap() > 0);
        assert_eq!(table, slope_table(&maps, &ground, 0.0, 2000.0, 500.0, 2, 0.0), "the same table every time");
    }

    // spec: TEST-050
    #[test]
    fn the_poses_file_is_read_with_its_defaults_and_a_bad_one_names_the_key() {
        let c = parse_config(GOOD).unwrap();
        assert_eq!(c.anchor, Anchor::LargestRiver { min_height_m: 0.0, relief: None });
        assert_eq!(c.chunk_timeout_s, DEFAULT_CHUNK_TIMEOUT_S);
        assert_eq!(c.chunk_cell_m, 8.0);
        assert_eq!(c.poses.len(), 2);
        assert_eq!(c.poses[0].sun_declination_deg, 0.0);
        assert_eq!(c.poses[1].sun_declination_deg, 10.0);
        let e = |s: &str| parse_config(s).unwrap_err();
        assert!(e(&GOOD.replace("heading_deg = 90", "heading_deg = \"east\"")).contains("'heading_deg' must be a finite number"));
        assert!(e(&GOOD.replace("pitch_deg = 12", "")).contains("'pitch_deg' is missing"));
        assert!(e(&GOOD.replace("1.8m-midday", "300m-sunset")).contains("given twice"));
        assert!(e(&GOOD.replace("1.8m-midday", "a b")).contains("must be letters"));
        assert!(e(&GOOD.replace("anchor = \"largest-river\"", "anchor = \"lake\"")).contains("anchor \"lake\" is unknown"));
        assert!(e(&GOOD.replace("anchor = \"largest-river\"", "lat = 95\nlon = 0")).contains("within 90 and 180"));
        assert!(e("[place]\nanchor = \"largest-river\"\n").contains("no [[pose]]"));
        assert!(parse_config(&GOOD.replace("anchor = \"largest-river\"", "lat = 12.5\nlon = -40")).is_ok());
    }

    // spec: TEST-048
    #[test]
    fn the_sun_stands_where_local_time_puts_it() {
        for lon in [-120.0_f64, 0.0, 77.0] {
            // Noon: the sun is overhead at the equator, straight along the anchor's direction.
            let noon = sun_for(12.0, lon, 0.0);
            let up = direction_of(0.0, lon);
            let dot = noon[0] as f64 * up.x + noon[1] as f64 * up.y + noon[2] as f64 * up.z;
            assert!(dot > 0.999_999, "noon at {lon}: {dot}");
            // 18:00 is a quarter turn west: the sun is on the horizon of the anchor, and the unit vector stays a unit vector.
            let dusk = sun_for(18.0, lon, 0.0);
            let dot = dusk[0] as f64 * up.x + dusk[1] as f64 * up.y + dusk[2] as f64 * up.z;
            assert!(dot.abs() < 1e-6, "18:00 at {lon}: {dot}");
            let west = Vec3::new(libm::sin(lon.to_radians()), -libm::cos(lon.to_radians()), 0.0);
            assert!(dusk[0] as f64 * west.x + dusk[1] as f64 * west.y > 0.999_999, "the sun sets in the west");
        }
    }

    // spec: TEST-048
    #[test]
    fn a_pose_puts_the_eye_behind_the_anchor_at_the_altitude_looking_along_the_heading() {
        let radius = 6_371_000.0;
        let anchor = direction_of(30.0, 40.0);
        let pose = Pose {
            name: "p".into(),
            altitude_m: 300.0,
            back_m: 2_000.0,
            heading_deg: 90.0,
            pitch_deg: 10.0,
            sun_local_hours: 12.0,
            sun_declination_deg: 0.0,
        };
        let ground = |d: Vec3| 100.0 + 50.0 * d.z;
        let cam = pose_camera(anchor, &pose, &ground, radius, 0.0, 16.0 / 9.0, 1.0);
        let eye = cam.position.0;
        let dir = eye.normalized();
        // 2 km behind along the great circle, 300 m over the ground there.
        let arc = libm::acos(dir.dot(anchor).clamp(-1.0, 1.0)) * radius;
        assert!((arc - 2_000.0).abs() < 0.01, "arc {arc}");
        assert!((eye.length() - radius - (ground(dir) + 300.0)).abs() < 1e-6);
        // Heading east: the anchor is east of the eye, and the forward vector is pitched 10 degrees down.
        let east = heading_vector(dir, 90.0);
        assert!(cam.forward.dot(east) > 0.98 && cam.forward.dot(dir) < -0.17 && cam.forward.dot(dir) > -0.18, "{:?}", cam.forward);
        // The sea is the floor of the ground: over deep water the eye keeps `altitude_m` above the level.
        let sea = pose_camera(anchor, &pose, &|_| -4000.0, radius, 0.0, 1.0, 1.0);
        assert!((sea.position.0.length() - radius - 300.0).abs() < 1e-6);
    }

    // spec: TEST-049
    #[test]
    fn the_wait_names_the_pose_and_the_missing_chunks() {
        let m = not_ready("300m-sunset", 900.4, &[(2, 5, 7), (2, 5, 8)], 12, 3);
        assert!(
            m.contains("pose '300m-sunset'") && m.contains("2 of 12 chunks") && m.contains("900 s") && m.contains("face 2 x 5 y 7"),
            "{m}"
        );
    }
}
