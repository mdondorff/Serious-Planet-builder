//! The pose stills (TEST-048): `test-render --scene capture-stills --poses FILE --out-dir DIR`, the lit path drawn at fixed poses over a
//! world from the base terrain, for the owner's look reviews (M8-4, M8-8) and later regression checks.
//!
//! A still waits for the horizon maps of the chunks near its eye (REND-038): when they are not all baked within the timeout the run fails and
//! names what was missing.

use crate::litperf::{capture_lod, new_renderer, request, LitSetup, Rig, WATER_T0};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{Camera, SelectionCache};
use planet_generators::chunkfield::ChunkKey;
use planet_render::GpuContext;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Whether `--scene NAME` names the pose stills.
pub fn is_stills_scene(name: &str) -> bool {
    name == "capture-stills"
}

/// Seconds a pose may wait for its horizon maps when the file gives no `chunk_timeout_s`.
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
    /// `view = "soil"` (any name of the viewer's J cycle): the debug view drawn instead of the lit one; `None` is the lit view.
    pub view: Option<planet_frame::look::shading::ShadeView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StillsConfig {
    pub anchor: Anchor,
    pub chunk_timeout_s: f64,
    /// The cell of the horizon chunks in metres (the viewer's 8 m unless the file says otherwise, `horizon_cell_m`; a test uses coarse chunks to stay quick).
    pub horizon_cell_m: f64,
    /// `coarse_horizon = false`: the stills are drawn without the coarse horizon map (the far-field shadows of the global run, REND-043), to tell what
    /// it adds to a frame; the chunk horizons stay. A diagnostic of the owner's frames, on by default.
    pub coarse_horizon: bool,
    /// `[place] slope_table_stride`: write `slopes.txt` ([`slope_table`]) over every this-many-th cell of the coupled lattice before the poses.
    pub slope_table_stride: Option<usize>,
    pub poses: Vec<Pose>,
    /// `[look]`: the strength of the ground detail and curves of the ground materials that replace the look's (a sample look for the owner's review;
    /// nothing here is stored or keyed, ADR 0035).
    pub look: LookOverride,
}

/// The `[look]` table of a poses file: `ground_detail = 0.6` and any number of `[[look.curve]]` with `material` (rock, sand, grass, soil), `input`
/// (slope_cos, height, temperature, precipitation, surface_type, soil_depth, moisture, insolation), `lo` and `hi` (a `lo == hi` pair leaves the input out).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LookOverride {
    pub ground_detail: Option<f64>,
    pub curves: Vec<(usize, usize, [f64; 2])>,
}

impl LookOverride {
    /// `lit` with the override applied (the detail strength and the curves).
    pub fn apply(&self, lit: &mut planet_frame::look::LitPlan) {
        if let Some(d) = self.ground_detail {
            lit.ground_detail = d;
        }
        for &(material, input, pair) in &self.curves {
            lit.materials.curves[material][input] = pair;
        }
    }
}

fn parse_look(root: &toml::Table) -> Result<LookOverride, String> {
    const INPUTS: [&str; 8] =
        ["slope_cos", "height", "temperature", "precipitation", "surface_type", "soil_depth", "moisture", "insolation"];
    let Some(look) = root.get("look") else { return Ok(LookOverride::default()) };
    let look = look.as_table().ok_or("the poses file: [look] must be a table")?;
    let ground_detail = number(look, "ground_detail", "[look]")?;
    if let Some(d) = ground_detail {
        if !(0.0..=1.0).contains(&d) {
            return Err(format!("[look]: ground_detail {d} must be from 0 to 1"));
        }
    }
    let mut curves = Vec::new();
    if let Some(list) = look.get("curve") {
        for (k, c) in list.as_array().ok_or("[look]: 'curve' must be an array of tables")?.iter().enumerate() {
            let whose = format!("[[look.curve]] {}", k + 1);
            let t = c.as_table().ok_or_else(|| format!("{whose}: must be a table"))?;
            let name = |key: &str| t.get(key).and_then(|v| v.as_str()).ok_or_else(|| format!("{whose}: '{key}' is missing"));
            let material = planet_frame::look::materials::MATERIAL_NAMES
                .iter()
                .position(|m| *m == name("material").unwrap_or(""))
                .ok_or_else(|| format!("{whose}: material {:?} is not rock, sand, grass or soil", name("material").unwrap_or("")))?;
            let input = INPUTS
                .iter()
                .position(|i| *i == name("input").unwrap_or(""))
                .ok_or_else(|| format!("{whose}: input {:?} is not one of {}", name("input").unwrap_or(""), INPUTS.join(", ")))?;
            curves.push((material, input, [required(t, "lo", &whose)?, required(t, "hi", &whose)?]));
        }
    }
    Ok(LookOverride { ground_detail, curves })
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
    let coarse_horizon = match root.get("coarse_horizon") {
        None => true,
        Some(v) => v.as_bool().ok_or_else(|| format!("coarse_horizon must be true or false, got {}", show(v)))?,
    };
    let horizon_cell_m = number(&root, "horizon_cell_m", "the poses file")?.unwrap_or(crate::horizonworld::HORIZON_CELL_M);
    if horizon_cell_m <= 0.0 {
        return Err(format!("horizon_cell_m must be positive, got {horizon_cell_m}"));
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
            view: match t.get("view") {
                None => None,
                Some(v) => Some(v.as_str().and_then(planet_frame::look::shading::ShadeView::parse).ok_or_else(|| {
                    format!(
                        "{whose}: view {} is not a view of the J cycle (lit, horizon, shadow, soil, moisture, insolation, ...)",
                        show(v)
                    )
                })?),
            },
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
    let slope_table_stride = match number(place, "slope_table_stride", "[place]")? {
        None => None,
        Some(k) if k >= 1.0 && k.fract() == 0.0 => Some(k as usize),
        Some(k) => return Err(format!("[place]: slope_table_stride {k} must be a whole number of at least 1")),
    };
    Ok(StillsConfig { anchor, chunk_timeout_s, horizon_cell_m, coarse_horizon, slope_table_stride, poses, look: parse_look(&root)? })
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
    // The heading carried to the eye: the tangent at the eye along the great circle through the anchor (at the anchor itself the heading: that vector is zero).
    let ahead = if pose.back_m > 0.0 { (anchor - eye_dir * anchor.dot(eye_dir)).normalized() } else { heading };
    let near = (pose.altitude_m * 0.05).clamp(0.05, 0.1);
    Camera::at_surface_point(PlanetFixed(eye_dir * (radius_m + eye_alt)), ahead, pose.pitch_deg.to_radians(), fov_y_rad, aspect, near)
}

/// How far from the eye a still waits for chunk horizon maps, metres (two chunk edges and a bit).
const HORIZON_WAIT_M: f64 = 12_000.0;

/// A still waited too long for the horizon maps of the chunks near its eye.
fn no_map_message(pose: &str, secs: f64, missing: &[ChunkKey], wanted: usize) -> String {
    let some: Vec<String> = missing.iter().take(4).map(|k| format!("face {} x {} y {}", k.0, k.1, k.2)).collect();
    format!(
        "pose '{pose}': {} of {wanted} chunks had no horizon map after {secs:.0} s (first missing: {}); raise chunk_timeout_s",
        missing.len(),
        some.join(", ")
    )
}

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
            .filter(|k| {
                let tile =
                    TileId::new(planet_core::Face(k.0), planet_generators::chunk::CHUNK_GRID_LEVEL, k.1, k.2).expect("a chunk of the grid");
                (tile.center_direction(&planet_core::cube::TangentWarp) * radius_m - camera.0).length() <= HORIZON_WAIT_M
            })
            .collect()
    };
    loop {
        crate::horizonstep::frame_step(world, tiles, camera, &crate::horizonstep::settling_features(true), &mut Default::default());
        let missing: Vec<&ChunkKey> = keys.iter().filter(|k| world.horizon_map(**k).is_none()).collect();
        if missing.is_empty() {
            return Ok(keys.len());
        }
        let waited = started.elapsed().as_secs_f64();
        if waited > timeout_s {
            let missing: Vec<ChunkKey> = missing.into_iter().copied().collect();
            return Err(no_map_message(pose, waited, &missing, keys.len()));
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
    run_stills(o, ctx, out, None)
}

/// The poses of `capture-stills`; with `cost` the terrain pass is timed at each settled pose in four variants (REND-044, the `shadow-cost` scene).
fn run_stills(o: &Options, ctx: &GpuContext, out: &mut dyn Write, mut cost: Option<&mut Vec<CostRow>>) -> Result<(), String> {
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
        return Err("the stills need a world with a bake (--store DIR, or --allow-memory-bake with --global-res N): the anchor reads the coupled run".into());
    }
    let size = o.size.unwrap_or((2560, 1440));
    let aspect = f64::from(size.0) / f64::from(size.1);
    let radius_m = world.radius_m();
    writeln!(
        out,
        "{}: {}x{}, {} poses, {}{}",
        if cost.is_some() { "shadow-cost" } else { "capture-stills: NOT a measurement (no gate, no timestamps, no log)" },
        size.0,
        size.1,
        config.poses.len(),
        world.describe(),
        if o.store.is_some() { "" } else { " (in-memory bake)" }
    )
    .map_err(err)?;
    let maps = crate::horizonstep::wait_coupled(&world, out)?;
    world.set_horizon_cells(crate::horizonworld::cells_per_chunk_for(radius_m, config.horizon_cell_m));
    world.enable_viewer_horizon(o.gen_workers(), Arc::new(planet_streaming::SystemClock::new()))?;
    crate::horizonstep::wait_coarse_horizon(&world, out)?;
    let base = world.field()?;
    let step_m = std::f64::consts::FRAC_PI_2 * radius_m / f64::from(world.global_res());
    let (anchor, flow, measured) = anchor_direction(config.anchor, &maps, &|d| base.height(d), step_m)?;
    let (lat, lon) = anchor.lat_lon();
    let ground = base.height(anchor);
    let anchor_line = format!(
        "anchor: latitude {:.4} longitude {:.4} degrees, ground {ground:.1} m{}",
        lat.to_degrees(),
        lon.to_degrees(),
        flow.map_or(String::new(), |b| format!(", the river flows towards bearing {b:.0} degrees{measured}"))
    );
    writeln!(out, "{anchor_line}").map_err(err)?;
    let notes = format!(
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
    std::fs::create_dir_all(&dir).map_err(err)?;
    let mut index =
        notes + "# pose width height eye_lat_deg eye_lon_deg eye_height_m heading_deg pitch_deg sun_local_hours horizon_maps horizon_s nodes capped_nodes\n";
    for pose in &config.poses {
        let pose = &Pose { heading_deg: pose.heading_deg.rem_euclid(360.0), ..pose.clone() };
        let started = Instant::now();
        // The eye is `altitude_m` above the ground of the base terrain.
        let camera = {
            let field = world.field()?;
            let fov = planet_frame::litscenes::FOV_DEG.to_radians();
            pose_camera(anchor, pose, &|d| field.height(d), radius_m, level_m, aspect, fov)
        };
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
            let report = capped_report(&plan.nodes, &|d| field.height(d), radius_m);
            std::fs::write(dir.join("capped.txt"), report).map_err(err)?;
        }
        let tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
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
                config.look.apply(l);
                l.view = pose.view.unwrap_or(l.view);
            }
            // One map is uploaded a frame: the frames of the exposure give the first four, the settling frames the rest.
            horizon.frame(&world, ctx, &mut r, &tiles, &world.horizon_frame(&tiles, camera.position), sun);
            r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
        }
        for _ in 0..horizon_maps {
            horizon.frame(&world, ctx, &mut r, &tiles, &world.horizon_frame(&tiles, camera.position), sun);
        }
        if !config.coarse_horizon {
            r.set_horizon_coarse(ctx, None);
        }
        let mut lit = setup.plan(&world, rig.field, &plan, WATER_T0 + 2.0, 0.5, false)?;
        if let Some(l) = lit.lit.as_mut() {
            l.sun_dir = sun;
            config.look.apply(l);
            l.view = pose.view.unwrap_or(l.view);
        }
        r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
        if let Some(rows) = cost.as_deref_mut() {
            rows.push(measure_cost(ctx, &mut r, &mut lit, &pose.name)?);
        }
        r.read_color(ctx).write_png(&dir.join(format!("{}.png", pose.name))).map_err(|e| format!("writing {}: {e}", pose.name))?;
        // SPIKE-63 (spike code): `SPIKE63_COST=1` times the terrain pass after the still (no gate, indicative): the product shader with
        // the detail on, and with the ground detail and shadows compiled out (REND-044's variants).
        if cost.is_none() && std::env::var_os("SPIKE63_COST").is_some() {
            let row = measure_cost(ctx, &mut r, &mut lit, &pose.name)?;
            let (s, m, both) = row.shares();
            writeln!(
                out,
                "SPIKE63 {}: full {:.3} ms; shadows {s:.3}, ground detail {m:.3}, both {both:.3}; compiled out {:?}; spread {:?}; nodes {}",
                pose.name,
                row.full,
                row.compiled_out,
                row.spread,
                plan.nodes.len()
            )
            .map_err(err)?;
        }
        let (eye_lat, _) = camera.position.0.lat_lon();
        let height = camera.position.0.length() - radius_m;
        writeln!(
            out,
            "{}: {horizon_maps} horizon maps in {horizon_s:.1} s, drawn in {:.1} s -> {}",
            pose.name,
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
            horizon_maps,
            horizon_s,
            plan.nodes.len(),
            plan.nodes.iter().filter(|n| n.capped).count()
        );
    }
    std::fs::write(dir.join("stills.txt"), index).map_err(err)?;
    writeln!(out, "wrote {} (one PNG per pose and stills.txt)", dir.display()).map_err(err)
}

/// Whether `--scene NAME` names the shadow and material cost scene (REND-044).
pub fn is_cost_scene(name: &str) -> bool {
    name == "shadow-cost"
}

/// The budget of the horizon shadows and the material map together, milliseconds of terrain-pass GPU time at 1440p (REND-044, M8 criterion 5).
pub const COST_BUDGET_MS: f64 = 1.5;
/// A share within this fraction of the budget is flagged, not called met (the compiled-out variants have no branch left) (opus-planner, 2026-10-27).
pub const COST_NEAR: f64 = 0.2;
/// Frames dropped before the timed ones of a variant, and the timed frames whose median is the variant's time.
const COST_WARM: usize = 8;
const COST_FRAMES: usize = 41;
/// Seconds of untimed frames before the first variant of a pose (the GPU idled while the pose settled) and before each later one.
const COST_WARM_FIRST_S: f64 = 3.0;
const COST_WARM_S: f64 = 1.0;
/// Rounds of the round-robin over the product shader and the three compiled-out builds.
const COST_ROUNDS: usize = 3;

/// The terrain-pass GPU time of one pose, milliseconds, medians of [`COST_FRAMES`] waited frames (REND-044).
#[derive(Clone, Debug, PartialEq)]
pub struct CostRow {
    pub pose: String,
    /// Shadows on, ground detail at full strength.
    pub full: f64,
    /// Horizon maps cleared (every node reads slot -1) and the coarse map off.
    pub no_shadow: f64,
    /// Ground detail at strength 0.
    pub no_material: f64,
    /// Both off.
    pub neither: f64,
    /// The same three variants with the code compiled out of the shader (`MeasureVariant`: no shadow, no material, neither); no branch is left,
    /// so the shares from them neither under-read the shadows nor over-read the material map.
    pub compiled_out: Option<[f64; 3]>,
    /// The spread (largest minus smallest) of the round medians of `full` and the three compiled-out variants; `full` and `compiled_out` are
    /// the medians of those [`COST_ROUNDS`] round medians, the variants interleaved round-robin so a slow period hits every variant alike.
    pub spread: Option<[f64; 4]>,
}

impl CostRow {
    /// The shadows' share, the material map's share and both together (full minus the variant without them), from the compiled-out
    /// variants when the row has them, else from the runtime ones.
    pub fn shares(&self) -> (f64, f64, f64) {
        match self.compiled_out {
            Some([shadow, material, both]) => (self.full - shadow, self.full - material, self.full - both),
            None => (self.full - self.no_shadow, self.full - self.no_material, self.full - self.neither),
        }
    }
}

/// The median of `v` (even counts: the mean of the middle two); NaN when empty.
pub fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// The median terrain-pass time of [`COST_FRAMES`] timed frames of `plan` after untimed frames for `warm_s` seconds (at least [`COST_WARM`]): the
/// GPU idles while a pose settles and its clocks need a moment to rise, which showed as a pose at 2 to 3 times its neighbours in window 22.
fn terrain_ms(
    ctx: &GpuContext,
    r: &mut planet_render::TerrainRenderer,
    plan: &planet_frame::TerrainPlan,
    warm_s: f64,
) -> Result<f64, String> {
    let started = Instant::now();
    let mut warmed = 0;
    while warmed < COST_WARM || started.elapsed().as_secs_f64() < warm_s {
        r.render(ctx, plan, false).map_err(|e| e.to_string())?;
        warmed += 1;
    }
    let mut v = Vec::with_capacity(COST_FRAMES);
    for _ in 0..COST_FRAMES {
        let (t, _) = r.render(ctx, plan, true).map_err(|e| e.to_string())?;
        v.push(t.gpu_ms.ok_or("the device gave no terrain-pass timestamp: the cost needs the discrete GPU")?);
    }
    Ok(median(v))
}

/// Time the terrain pass of the settled pose in the four variants. The renderer keeps no way to put the horizon maps back, so the shadows are
/// cleared after the variants that need them: full, no material, then neither, then no shadow.
fn measure_cost(
    ctx: &GpuContext,
    r: &mut planet_render::TerrainRenderer,
    plan: &mut planet_frame::TerrainPlan,
    pose: &str,
) -> Result<CostRow, String> {
    let detail = |plan: &mut planet_frame::TerrainPlan, d: f64| {
        if let Some(l) = plan.lit.as_mut() {
            l.ground_detail = d;
        }
    };
    detail(plan, 1.0);
    // The product shader and the three builds with the code compiled out (REND-044), while the maps and the detail are still on, round-robin
    // for [`COST_ROUNDS`] rounds: a slow period of the machine falls on every variant alike, and the value of a variant is the median of its
    // round medians (the minimum would bias the shares low).
    use planet_render::MeasureVariant as V;
    let variants = [V::Product, V::NoShadow, V::NoMaterial, V::Neither];
    // `rounds[round][variant]`: the median of one round of one variant.
    let mut rounds = [[0.0; 4]; COST_ROUNDS];
    let mut first = true;
    for row in rounds.iter_mut() {
        for (slot, variant) in row.iter_mut().zip(variants) {
            r.set_measure_variant(ctx, variant)?;
            *slot = terrain_ms(ctx, r, plan, if std::mem::take(&mut first) { COST_WARM_FIRST_S } else { COST_WARM_S })?;
        }
    }
    r.set_measure_variant(ctx, V::Product)?;
    let column = |k: usize| rounds.iter().map(|row| row[k]).collect::<Vec<f64>>();
    let med = |k: usize| median(column(k));
    let spread = |k: usize| column(k).iter().cloned().fold(f64::MIN, f64::max) - column(k).iter().cloned().fold(f64::MAX, f64::min);
    let full = med(0);
    let compiled_out = [med(1), med(2), med(3)];
    let spread = [spread(0), spread(1), spread(2), spread(3)];
    detail(plan, 0.0);
    let no_material = terrain_ms(ctx, r, plan, COST_WARM_S)?;
    r.clear_horizon();
    r.set_horizon_coarse(ctx, None);
    let neither = terrain_ms(ctx, r, plan, COST_WARM_S)?;
    detail(plan, 1.0);
    let no_shadow = terrain_ms(ctx, r, plan, COST_WARM_S)?;
    Ok(CostRow { pose: pose.to_string(), full, no_shadow, no_material, neither, compiled_out: Some(compiled_out), spread: Some(spread) })
}

/// The failures of the budget (the shadows and the material map together at most [`COST_BUDGET_MS`] in every pose) and the poses within
/// [`COST_NEAR`] of it, which are flagged, not called met (REND-044).
pub fn cost_verdict(rows: &[CostRow]) -> (Vec<String>, Vec<String>) {
    let (mut over, mut near) = (Vec::new(), Vec::new());
    for r in rows {
        let both = r.shares().2;
        if !(both.is_finite() && both <= COST_BUDGET_MS) {
            over.push(format!("{}: shadows and material map {both:.3} ms exceed {COST_BUDGET_MS} ms (REND-044)", r.pose));
        } else if both > COST_BUDGET_MS * (1.0 - COST_NEAR) {
            near.push(format!(
                "{}: {both:.3} ms is within 20 % of the {COST_BUDGET_MS} ms budget (by the compiled-out shader variants when the row has them): not called met",
                r.pose
            ));
        }
    }
    (over, near)
}

// spec: REND-044
/// `test-render --perf --scene shadow-cost --poses FILE --out-dir DIR --store DIR --global-res N`: after the readiness gate, the settled poses of
/// the stills at 1440p with the terrain pass timed in four variants; the log is `perf/logs/shadow-cost.json`.
pub fn perf_shadow_cost(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    let mut rows = Vec::new();
    run_stills(o, ctx, out, Some(&mut rows))?;
    let (over, near) = cost_verdict(&rows);
    for r in &rows {
        let (s, m, both) = r.shares();
        writeln!(
            out,
            "{}: terrain pass {:.3} ms; shadows {s:.3} ms, material map {m:.3} ms, together {both:.3} ms (runtime off: without shadows {:.3}, without material {:.3}, without both {:.3}; compiled out: {}; spread across rounds: {})",
            r.pose,
            r.full,
            r.no_shadow,
            r.no_material,
            r.neither,
            r.compiled_out.map_or("none".to_string(), |c| format!("{:.3}, {:.3}, {:.3}", c[0], c[1], c[2])),
            r.spread.map_or("none".to_string(), |c| format!("{:.3}, {:.3}, {:.3}, {:.3}", c[0], c[1], c[2], c[3]))
        )
        .map_err(err)?;
    }
    writeln!(
        out,
        "the shares are full minus the shader with the code compiled out (no branch left; the runtime-off variants under-read the shadows and over-read the material map); 1440p, each variant the median of {COST_ROUNDS} round medians of {COST_FRAMES} frames, interleaved round-robin"
    )
    .map_err(err)?;
    for n in &near {
        writeln!(out, "NEAR: {n}").map_err(err)?;
    }
    let poses: Vec<String> = rows
        .iter()
        .map(|r| {
            let (s, m, both) = r.shares();
            format!(
                "    {{\"pose\": {:?}, \"full_ms\": {:.4}, \"no_shadow_ms\": {:.4}, \"no_material_ms\": {:.4}, \"neither_ms\": {:.4}, \"compiled_out_ms\": {}, \"round_spread_ms\": {}, \"shadow_ms\": {s:.4}, \"material_ms\": {m:.4}, \"together_ms\": {both:.4}}}",
                r.pose,
                r.full,
                r.no_shadow,
                r.no_material,
                r.neither,
                r.compiled_out.map_or("null".to_string(), |c| format!("[{:.4}, {:.4}, {:.4}]", c[0], c[1], c[2])),
                r.spread.map_or("null".to_string(), |c| format!("[{:.4}, {:.4}, {:.4}, {:.4}]", c[0], c[1], c[2], c[3]))
            )
        })
        .collect();
    let size = o.size.unwrap_or((2560, 1440));
    let json = format!(
        "{{\n  \"scene\": \"shadow-cost\",\n  \"adapter\": \"{}\",\n  \"owner_override\": {:?},\n  \"size\": [{}, {}],\n  \"frames\": {COST_FRAMES},\n  \"budget_ms\": {COST_BUDGET_MS},\n  \"poses\": [\n{}\n  ],\n  \"near_budget\": {:?},\n  \"budget_failures\": {:?}\n}}\n",
        ctx.adapter_name().replace('"', "'"),
        owner_override,
        size.0,
        size.1,
        poses.join(",\n"),
        near,
        over
    );
    let path = o.out.clone().unwrap_or_else(|| std::path::Path::new("perf").join("logs").join("shadow-cost.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)?;
    if over.is_empty() {
        Ok(())
    } else {
        Err(over.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REND-044
    #[test]
    fn the_cost_shares_are_differences_and_a_pose_near_the_budget_is_flagged_not_called_met() {
        let row = |pose: &str, full: f64, shadow: f64, material: f64, neither: f64| CostRow {
            pose: pose.into(),
            full,
            no_shadow: shadow,
            no_material: material,
            neither,
            compiled_out: None,
            spread: None,
        };
        // The compiled-out variants, when a row has them, are what the shares and the verdict read.
        let with = CostRow { compiled_out: Some([4.6, 4.2, 3.9]), ..row("w", 5.0, 4.5, 4.6, 4.0) };
        assert_eq!(with.shares(), (5.0 - 4.6, 5.0 - 4.2, 5.0 - 3.9));
        let ok = row("a", 5.0, 4.5, 4.6, 4.0);
        assert_eq!(ok.shares().2, 1.0);
        assert_eq!(cost_verdict(&[ok]), (vec![], vec![]));
        let (over, flagged) = cost_verdict(&[row("b", 5.0, 4.0, 4.0, 3.7)]);
        assert!(over.is_empty() && flagged.len() == 1 && flagged[0].contains("within 20 %"), "{flagged:?}");
        let (over, _) = cost_verdict(&[row("c", 5.0, 4.0, 4.0, 3.0)]);
        assert!(over.len() == 1 && over[0].starts_with("c:") && over[0].contains("REND-044"), "{over:?}");
        // A pose with no reading is not met.
        assert_eq!(cost_verdict(&[row("d", f64::NAN, 1.0, 1.0, 1.0)]).0.len(), 1);
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert!(median(vec![]).is_nan());
    }

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

    // spec: TEST-048
    #[test]
    fn the_coarse_horizon_is_on_unless_the_file_turns_it_off_and_a_non_boolean_is_refused() {
        assert!(parse_config(GOOD).unwrap().coarse_horizon, "on by default");
        assert!(
            !parse_config(&format!(
                "coarse_horizon = false
{GOOD}"
            ))
            .unwrap()
            .coarse_horizon
        );
        assert!(
            parse_config(&format!(
                "coarse_horizon = true
{GOOD}"
            ))
            .unwrap()
            .coarse_horizon
        );
        let e = parse_config(&format!(
            "coarse_horizon = 0
{GOOD}"
        ))
        .unwrap_err();
        assert!(e.contains("coarse_horizon must be true or false"), "{e}");
    }

    // spec: LOOK-011
    #[test]
    fn the_sample_look_file_holds_the_ground_look_of_the_sample_poses_files_look_table() {
        use planet_frame::look::{file::parse_look, LitPlan};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/milestones/M8/look-review-2b");
        let poses = std::fs::read_to_string(root.join("hills-b/sample-driver-look/poses.toml")).unwrap();
        let file = parse_look(&std::fs::read_to_string(root.join("sample-driver-look.look.toml")).unwrap()).unwrap();
        let mut lit = LitPlan::fixed(12.0);
        parse_config(&poses).unwrap().look.apply(&mut lit);
        let g = file.body(planet_world_def::BodyId(0)).ground;
        assert_eq!((g.detail, g.materials), (lit.ground_detail, lit.materials), "F8 shows what the stills of the sample look show");
    }

    // spec: TEST-053
    #[test]
    fn the_look_table_and_a_poses_view_are_read_and_a_bad_name_is_refused_with_the_names_it_knows() {
        use planet_frame::look::shading::ShadeView;
        let text = format!(
            "{GOOD}
view = \"soil\"
[look]
ground_detail = 0.6
[[look.curve]]
material = \"soil\"
input = \"moisture\"
lo = 0.5
hi = 0.8
"
        );
        // The trailing `view` lands in the last pose's table; the [look] table follows it.
        let c = parse_config(&text).unwrap();
        assert_eq!(c.poses[1].view, Some(ShadeView::SoilDepth));
        assert_eq!(c.poses[0].view, None);
        assert_eq!(c.look, LookOverride { ground_detail: Some(0.6), curves: vec![(3, 6, [0.5, 0.8])] });
        let mut lit = planet_frame::look::LitPlan::fixed(12.0);
        c.look.apply(&mut lit);
        assert_eq!((lit.ground_detail, lit.materials.curves[3][6]), (0.6, [0.5, 0.8]));
        assert_eq!(parse_config(GOOD).unwrap().look, LookOverride::default());
        for (bad, want) in [
            (
                "[look]
ground_detail = 2
",
                "ground_detail",
            ),
            (
                "[[look.curve]]
material = \"mud\"
input = \"moisture\"
lo = 0
hi = 1
",
                "mud",
            ),
            (
                "[[look.curve]]
material = \"soil\"
input = \"wetness\"
lo = 0
hi = 1
",
                "wetness",
            ),
            (
                "[[look.curve]]
material = \"soil\"
input = \"moisture\"
lo = 0
",
                "hi",
            ),
        ] {
            let e = parse_config(&format!(
                "{GOOD}
{bad}"
            ))
            .unwrap_err();
            assert!(e.contains(want), "{e}");
        }
        let e = parse_config(&GOOD.replace(
            "sun_local_hours = 17.5",
            "sun_local_hours = 17.5
view = \"x\"",
        ))
        .unwrap_err();
        assert!(e.contains("\"x\"") && e.contains("300m-sunset"), "{e}");
    }

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
        assert_eq!(c.horizon_cell_m, 8.0);
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
    fn a_pose_at_its_anchor_with_nothing_behind_it_looks_along_the_heading_and_does_not_normalise_a_zero_vector() {
        // On the equator at longitude 0 the anchor is exactly (1, 0, 0): the tangent at the eye along the great circle through the anchor is the zero vector.
        let anchor = direction_of(0.0, 0.0);
        assert_eq!(anchor.dot(anchor), 1.0);
        let pose = Pose {
            name: "p".into(),
            altitude_m: 300.0,
            back_m: 0.0,
            heading_deg: 90.0,
            pitch_deg: 0.0,
            sun_local_hours: 12.0,
            sun_declination_deg: 0.0,
            view: None,
        };
        let cam = pose_camera(anchor, &pose, &|_| 100.0, 6_371_000.0, 0.0, 16.0 / 9.0, 1.0);
        assert!((cam.position.0.length() - 6_371_000.0 - 400.0).abs() < 1e-6);
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
            view: None,
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

    // spec: TEST-048
    #[test]
    fn the_wait_names_the_pose_and_the_chunks_without_a_horizon_map() {
        let m = no_map_message("300m-sunset", 900.4, &[(2, 5, 7), (2, 5, 8)], 12);
        assert!(
            m.contains("pose '300m-sunset'") && m.contains("2 of 12 chunks") && m.contains("900 s") && m.contains("face 2 x 5 y 7"),
            "{m}"
        );
    }
}
