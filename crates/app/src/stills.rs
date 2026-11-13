//! The pose stills (TEST-048): `test-render --scene capture-stills --poses FILE --out-dir DIR`, the lit path drawn at fixed poses over a
//! world from the base terrain, for the owner's look reviews (M8-4, M8-8) and later regression checks.
//!
//! A still draws the lit frame with the sun shadow of the march (`--shadow-march`, REND-051) and no horizon data.

use crate::litperf::{capture_lod, new_renderer, request, LitSetup, Rig, WATER_T0};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{Camera, SelectionCache};
use planet_render::GpuContext;
use std::collections::BTreeSet;
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

/// Whether `--scene NAME` names the pose stills.
pub fn is_stills_scene(name: &str) -> bool {
    name == "capture-stills"
}

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
    /// `[place] slope_table_stride`: write `slopes.txt` ([`slope_table`]) over every this-many-th cell of the coupled lattice before the poses.
    pub slope_table_stride: Option<usize>,
    pub poses: Vec<Pose>,
    /// `[look]`: the strength of the ground detail and curves of the ground materials that replace the look's (a sample look for the owner's review;
    /// nothing here is stored or keyed, ADR 0035).
    pub look: LookOverride,
}

/// The `[look]` table of a poses file: `ground_detail = 0.6`, `ground_patch = 0.12` (the patch break-up of the material boundaries, REND-055) and any number of `[[look.curve]]` with `material` (rock, sand, grass, soil), `input`
/// (slope_cos, height, temperature, precipitation, surface_type, soil_depth, moisture, insolation), `lo` and `hi` (a `lo == hi` pair leaves the input out).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LookOverride {
    pub ground_detail: Option<f64>,
    pub ground_patch: Option<f64>,
    pub curves: Vec<(usize, usize, [f64; 2])>,
}

impl LookOverride {
    /// `lit` with the override applied (the detail strength and the curves).
    pub fn apply(&self, lit: &mut planet_frame::look::LitPlan) {
        if let Some(d) = self.ground_detail {
            lit.ground_detail = d;
        }
        if let Some(p) = self.ground_patch {
            lit.ground_patch = p;
        }
        for &(material, input, pair) in &self.curves {
            lit.materials.curves[material][input] = pair;
        }
    }
}

fn parse_look(root: &toml::Table) -> Result<LookOverride, String> {
    use planet_frame::look::materials::INPUT_NAMES as INPUTS;
    let Some(look) = root.get("look") else { return Ok(LookOverride::default()) };
    let look = look.as_table().ok_or("the poses file: [look] must be a table")?;
    let ground_detail = number(look, "ground_detail", "[look]")?;
    if let Some(d) = ground_detail {
        if !(0.0..=1.0).contains(&d) {
            return Err(format!("[look]: ground_detail {d} must be from 0 to 1"));
        }
    }
    let ground_patch = number(look, "ground_patch", "[look]")?;
    if let Some(p) = ground_patch {
        if !(0.0..=planet_frame::look::file::PATCH_MAX).contains(&p) {
            return Err(format!("[look]: ground_patch {p} must be from 0 to {}", planet_frame::look::file::PATCH_MAX));
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
                .ok_or_else(|| {
                    format!(
                        "{whose}: material {:?} is not one of {:?}",
                        name("material").unwrap_or(""),
                        planet_frame::look::materials::MATERIAL_NAMES
                    )
                })?;
            let input = INPUTS
                .iter()
                .position(|i| *i == name("input").unwrap_or(""))
                .ok_or_else(|| format!("{whose}: input {:?} is not one of {}", name("input").unwrap_or(""), INPUTS.join(", ")))?;
            curves.push((material, input, [required(t, "lo", &whose)?, required(t, "hi", &whose)?]));
        }
    }
    Ok(LookOverride { ground_detail, ground_patch, curves })
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

/// Read the poses file: `[place]` (`anchor = "largest-river"`, or `lat` and `lon` in degrees) and one
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
                    format!("{whose}: view {} is not a view of the J cycle (lit, shadow, soil, moisture, insolation, ...)", show(v))
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
    Ok(StillsConfig { anchor, slope_table_stride, poses, look: parse_look(&root)? })
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

/// The distance along the view axis from the eye to the ground or the sea (the higher) at the centre of the picture, or `None` when the axis meets none within 600 km (the sky, space). The ray is marched in steps that grow
/// with the distance and the crossing refined by bisection: a scale for the review images' bar (ADR 0118 point 5), not a measurement.
pub fn centre_distance_m(camera: &Camera, ground_m: &dyn Fn(Vec3) -> f64, radius_m: f64, level_m: f64) -> Option<f64> {
    let (eye, fwd) = (camera.position.0, camera.forward);
    let below = |t: f64| {
        let p = eye + fwd * t;
        let r = p.length();
        r - radius_m <= ground_m(p * (1.0 / r)).max(level_m)
    };
    if below(0.0) {
        return Some(0.0);
    }
    let (mut lo, mut t) = (0.0f64, 1.0f64);
    while t < 600_000.0 {
        if below(t) {
            // Bisection between the last step above and this one below.
            let mut hi = t;
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                if below(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(hi);
        }
        lo = t;
        t += (0.02 * t).max(1.0);
    }
    None
}

/// The pose label of a review picture (ADR 0118 point 5): the altitude of the eye over the ground below it, the pitch of the view axis below the horizon, the field of view and the distance along the axis to the ground.
pub fn pose_label(camera: &Camera, ground_m: &dyn Fn(Vec3) -> f64, radius_m: f64, level_m: f64) -> planet_render::annotate::PoseLabel {
    let eye = camera.position.0;
    let up = eye.normalized();
    let alt = eye.length() - radius_m - ground_m(up).max(level_m);
    planet_render::annotate::PoseLabel {
        altitude_m: alt,
        pitch_deg: -libm::asin(camera.forward.dot(up).clamp(-1.0, 1.0)).to_degrees(),
        fov_deg: camera.fov_y_rad.to_degrees(),
        centre_distance_m: centre_distance_m(camera, ground_m, radius_m, level_m),
    }
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
        let chunks: Vec<&[usize]> = cells.chunks(chunk).collect();
        let parts: Vec<Vec<(f64, f64)>> = planet_core::par::map(&threads, chunks.len(), &|c| {
            chunks[c]
                .iter()
                .filter_map(|&c| {
                    let d = lat.direction(c);
                    let h = ground(d);
                    (h > sea_m).then(|| (h, libm::atan(local_slope(ground, d, radius_m, step_m, planet).0).to_degrees()))
                })
                .collect()
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
/// [--views A,B] [--look FILE]` (TEST-048, REND-055; the look file's `ground` table drives the lit plan): one PNG per pose, `DIR/<pose>.png`, and `DIR/stills.txt` with the camera and sun of each. A tool beside
/// the harness, never a measurement: no gate, no timestamps, no log; not to be run during a perf window.
pub fn capture_stills(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    run_stills(o, ctx, out, None, None)
}

/// The poses of `capture-stills`; with `cost` the terrain pass is timed at each settled pose in four variants (REND-044, the `shadow-cost` scene).
fn run_stills(
    o: &Options,
    ctx: &GpuContext,
    out: &mut dyn Write,
    mut cost: Option<&mut Vec<CostRow>>,
    mut layer1: Option<&mut Vec<Layer1Row>>,
) -> Result<(), String> {
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
    // `--erosion on` draws the erosion layer in the stills as the viewer does (VIEW-062); the header line says what the world is.
    let world = Arc::new(crate::streamperf::open_world(o, false)?.with_erosion_if(o)?.bounding_water());
    if !world.has_bake() {
        return Err("the stills need a world with a bake (--store DIR, or --allow-memory-bake with --global-res N): the anchor reads the coupled run".into());
    }
    let size = o.size.unwrap_or((2560, 1440));
    let aspect = f64::from(size.0) / f64::from(size.1);
    let radius_m = world.radius_m();
    writeln!(
        out,
        "{}: {}x{}, {} poses, {}{}",
        if cost.is_some() {
            "shadow-cost"
        } else if layer1.is_some() {
            "shadow-layer1"
        } else {
            "capture-stills: NOT a measurement (no gate, no timestamps, no log)"
        },
        size.0,
        size.1,
        config.poses.len(),
        world.describe(),
        if o.store.is_some() || o.commit.is_some() { "" } else { " (in-memory bake)" }
    )
    .map_err(err)?;
    // Every still carries the run's note and, for a commit opened in memory, that the base is not a stored bake (VIEW-069, TEST-128): one line each.
    let still_note: Option<String> = {
        let base = if o.commit.is_some() { Some("base: computed in memory, not read from a stored bake") } else { None };
        let lines: Vec<&str> = o.note.as_deref().into_iter().chain(base).collect();
        (!lines.is_empty()).then(|| lines.join(&char::from(10).to_string()))
    };
    let maps = crate::horizonstep::wait_coupled(&world, out)?;
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
        notes + "# pose width height eye_lat_deg eye_lon_deg eye_height_m heading_deg pitch_deg sun_local_hours nodes capped_nodes\n";
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
        let mut r = new_renderer(&rig, &setup, camera)?;
        crate::skeleton::upload_missing_meshes(ctx, Some(&world), &mut r, &plan, rig.field, radius_m, o.cells)?;
        let (_, eye_lon) = camera.position.0.lat_lon();
        let sun = sun_for(pose.sun_local_hours, eye_lon.to_degrees(), pose.sun_declination_deg);
        let mut march = if o.shadow_march && cost.is_none() {
            Some(StillShadow::new(ctx, o, &world, rig.field, &mut r, &tiles, radius_m, lod.max_height_m)?)
        } else {
            None
        };
        // The exposure adapts over a few frames of half a second, the first snapping to its target.
        for k in 0..4 {
            let mut lit = setup.plan(&world, rig.field, &plan, WATER_T0 + k as f64 * 0.5, 0.5, k == 0)?;
            if let Some(l) = lit.lit.as_mut() {
                l.sun_dir = sun;
                config.look.apply(l);
                l.view = pose.view.unwrap_or(l.view);
            }
            r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
        }
        let mut lit = setup.plan(&world, rig.field, &plan, WATER_T0 + 2.0, 0.5, false)?;
        if let Some(l) = lit.lit.as_mut() {
            l.sun_dir = sun;
            config.look.apply(l);
            l.view = pose.view.unwrap_or(l.view);
        }
        match march.as_mut() {
            Some(m) => {
                let sun64 = Vec3::new(f64::from(sun[0]), f64::from(sun[1]), f64::from(sun[2]));
                for k in 0..SHADOW_STILL_FRAMES {
                    r.set_shadow_frame(m.frame(&camera, sun64, k));
                    r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
                }
            }
            None => {
                r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
            }
        }
        if let Some(rows) = cost.as_deref_mut() {
            rows.push(measure_cost(ctx, &mut r, &mut lit, &pose.name)?);
        }
        if let (Some(rows), Some(m)) = (layer1.as_deref_mut(), march.as_mut()) {
            let sun64 = Vec3::new(f64::from(sun[0]), f64::from(sun[1]), f64::from(sun[2]));
            rows.push(measure_layer1(ctx, &mut r, m, &camera, sun64, &lit, &pose.name)?);
        }
        // The owner's copy carries the scale bar and the pose (ADR 0118 point 5): drawn on this review picture, never on a golden.
        let mut shot = r.read_color(ctx);
        planet_render::annotate::annotate_with_note(
            &mut shot,
            &pose_label(&camera, &|d| field.height(d), radius_m, setup.level_m),
            still_note.as_deref(),
        );
        shot.write_png(&dir.join(format!("{}.png", pose.name))).map_err(|e| format!("writing {}: {e}", pose.name))?;
        let (eye_lat, _) = camera.position.0.lat_lon();
        let height = camera.position.0.length() - radius_m;
        writeln!(
            out,
            "{}: {} shadow frames, drawn in {:.1} s -> {}",
            pose.name,
            r.shadow_frames_run(),
            started.elapsed().as_secs_f64(),
            dir.join(format!("{}.png", pose.name)).display()
        )
        .map_err(err)?;
        index += &format!(
            "{} {} {} {:.5} {:.5} {:.1} {} {} {} {} {}\n",
            pose.name,
            size.0,
            size.1,
            eye_lat.to_degrees(),
            eye_lon.to_degrees(),
            height,
            pose.heading_deg,
            pose.pitch_deg,
            pose.sun_local_hours,
            plan.nodes.len(),
            plan.nodes.iter().filter(|n| n.capped).count()
        );
    }
    std::fs::write(dir.join("stills.txt"), index).map_err(err)?;
    writeln!(out, "wrote {} (one PNG per pose and stills.txt)", dir.display()).map_err(err)
}

/// The clipmap shadow of a still (ADR 0085, REND-051): what the viewer draws by default. The resident set is the drawn tiles and their ancestors down to the
/// render floor (the streamer keeps them; a clipmap level coarser than a drawn tile is written from its ancestor), planned at once on this thread.
pub(crate) struct StillShadow {
    planner: planet_frame::clipmap::ClipmapPlanner,
    set: BTreeSet<TileId>,
    resident: Vec<TileId>,
    floor: Vec<TileId>,
    max_height_m: f64,
    radius_m: f64,
}

impl StillShadow {
    /// Enable the shadow pass of `r` and upload the ancestors of `tiles`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ctx: &GpuContext,
        o: &Options,
        world: &WorldTerrain,
        field: &dyn planet_generators::height::HeightField,
        r: &mut planet_render::TerrainRenderer,
        tiles: &[TileId],
        radius_m: f64,
        max_height_m: f64,
    ) -> Result<StillShadow, String> {
        let mut set: BTreeSet<TileId> = BTreeSet::new();
        // The coarse cube beyond the clipmap is made from the floor tiles of every face.
        let floor_edge = 1u32 << planet_frame::DEFAULT_RENDER_FLOOR;
        for face in 0..6 {
            for x in 0..floor_edge {
                for y in 0..floor_edge {
                    set.insert(TileId::new(planet_core::Face(face), planet_frame::DEFAULT_RENDER_FLOOR, x, y).expect("a floor tile"));
                }
            }
        }
        // A software adapter gathers 1024^2 levels in seconds a frame: its stills use the smoke size of the viewer's smoke run.
        let texels = if ctx.driver_string().contains("Cpu") { 256 } else { planet_frame::clipmap::CLIPMAP_TEXELS };
        let params = planet_frame::clipmap::ClipmapParams { texels, ..planet_frame::clipmap::ClipmapParams::new(radius_m, o.cells) };
        r.enable_shadow(ctx, params.texels, params.levels, radius_m);
        let mut me = StillShadow {
            floor: Vec::new(),
            planner: planet_frame::clipmap::ClipmapPlanner::new(params),
            set,
            resident: Vec::new(),
            max_height_m,
            radius_m,
        };
        me.extend(ctx, o, world, field, r, tiles)?;
        me.floor = crate::shadowrig::floor_leaves(&me.resident);
        Ok(me)
    }

    /// Add `tiles` (the drawn ones of a frame) and their ancestors down to the render floor to the resident set and upload what the renderer lacks.
    pub(crate) fn extend(
        &mut self,
        ctx: &GpuContext,
        o: &Options,
        world: &WorldTerrain,
        field: &dyn planet_generators::height::HeightField,
        r: &mut planet_render::TerrainRenderer,
        tiles: &[TileId],
    ) -> Result<(), String> {
        let before = self.set.len();
        for &t in tiles {
            let mut t = t;
            self.set.insert(t);
            while let Some(parent) = t.parent().filter(|p| p.level() >= planet_frame::DEFAULT_RENDER_FLOOR) {
                if !self.set.insert(parent) {
                    break;
                }
                t = parent;
            }
        }
        if self.set.len() == before && !self.resident.is_empty() {
            return Ok(());
        }
        self.resident = self.set.iter().copied().collect();
        crate::skeleton::upload_tiles(ctx, Some(world), r, &self.resident, field, self.radius_m, o.cells)?;
        Ok(())
    }
    /// The shadow frame of the still camera at its `k`-th frame (the first is a cut, the next ones fill the low sun's rays and settle the history).
    pub(crate) fn frame(&mut self, camera: &Camera, sun: Vec3, k: usize) -> planet_render::shadow_path::ShadowFrame {
        let plan = self.planner.plan(camera.position, &self.resident, &[], &[], &planet_core::cube::TangentWarp);
        planet_render::shadow_path::ShadowFrame {
            view: planet_frame::shadowview::ShadowView::new(plan.anchor, camera, sun, self.radius_m),
            plan,
            preview: planet_render::shadow::ClipmapPreview::default(),
            floor_tiles: if k == 0 { self.floor.clone() } else { Vec::new() },
            cut: k == 0,
            ceiling_m: planet_frame::shadowview::ceiling_m(self.max_height_m, &[]),
        }
    }
}

/// Frames a still renders with the shadow so that the temporal pass settles (the checkerboard of a low sun fills every ray within two).
const SHADOW_STILL_FRAMES: usize = 6;

/// Whether `--scene NAME` names the layer-1 frame scene (REND-052).
pub fn is_layer1_scene(name: &str) -> bool {
    name == "shadow-layer1"
}

/// The GPU time of the clipmap shadow of one pose, milliseconds: the median over the timed frames of each resolve pass
/// ([`planet_render::resolve::RESOLVE_PASS_NAMES`] order) and of their sum (REND-052).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer1Row {
    pub pose: String,
    pub passes_ms: [f64; 5],
    pub total_ms: f64,
}

/// REND-052's budget for layer 1 at 1440p: the clipmap upkeep, march, upsample and temporal pass together, median GPU milliseconds per pose. The upkeep of a
/// settled pose is nil (nothing is dirty); the cost of scrolling the clipmap is the one-texel scroll item, not this scene's.
pub const LAYER1_BUDGET_MS: f64 = 1.5;
/// A reading within this share of the budget is flagged, not called a pass or a failure (ADR 0087, REND-052).
pub const LAYER1_NEAR: f64 = 0.05;

/// The failures (above the budget by more than [`LAYER1_NEAR`], or no reading) and the poses within [`LAYER1_NEAR`] of it (REND-052).
pub fn layer1_verdict(rows: &[Layer1Row]) -> (Vec<String>, Vec<String>) {
    let (mut over, mut near) = (Vec::new(), Vec::new());
    for r in rows {
        let t = r.total_ms;
        if !t.is_finite() || t > LAYER1_BUDGET_MS * (1.0 + LAYER1_NEAR) {
            over.push(format!("{}: layer 1 {t:.3} ms exceeds {LAYER1_BUDGET_MS} ms (REND-052)", r.pose));
        } else if t > LAYER1_BUDGET_MS * (1.0 - LAYER1_NEAR) {
            near.push(format!("{}: layer 1 {t:.3} ms is within 5 % of the {LAYER1_BUDGET_MS} ms budget: not called met or missed", r.pose));
        }
    }
    (over, near)
}

/// Time the shadow passes of the settled pose with GPU timestamps: [`COST_WARM`] untimed frames, then [`COST_FRAMES`] timed ones, each with its shadow
/// frame (not a cut: the history is settled, the sun's march scale and spread as the frame decides, REND-049, REND-050).
fn measure_layer1(
    ctx: &GpuContext,
    r: &mut planet_render::TerrainRenderer,
    m: &mut StillShadow,
    camera: &Camera,
    sun: Vec3,
    lit: &planet_frame::TerrainPlan,
    pose: &str,
) -> Result<Layer1Row, String> {
    if !r.enable_shadow_timing(ctx) {
        return Err("the layer-1 scene needs GPU timestamp queries: the discrete GPU".into());
    }
    let mut per: [Vec<f64>; 5] = Default::default();
    let mut totals = Vec::with_capacity(COST_FRAMES);
    for k in 0..COST_WARM + COST_FRAMES {
        r.set_shadow_frame(m.frame(camera, sun, k + 1));
        r.render(ctx, lit, false).map_err(|e| e.to_string())?;
        if k >= COST_WARM {
            let ms = r.shadow_pass_ms(ctx).ok_or("the shadow passes gave no timestamps")?;
            for (v, x) in per.iter_mut().zip(ms) {
                v.push(x);
            }
            totals.push(ms.iter().sum());
        }
    }
    let [a, b, c, d, e] = per;
    Ok(Layer1Row { pose: pose.to_string(), passes_ms: [median(a), median(b), median(c), median(d), median(e)], total_ms: median(totals) })
}

/// `test-render --perf --scene shadow-layer1 --poses FILE --out-dir DIR --store DIR --global-res N`: after the readiness gate, the settled poses of
/// the file at 1440p with the clipmap shadow's passes timed (REND-052); the log is `perf/logs/shadow-layer1.json`. Run by `perf stream shadow-layer1`.
pub fn perf_shadow_layer1(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    if !o.shadow_march {
        return Err("shadow-layer1 measures the clipmap shadow: it cannot run with --no-shadow-march".into());
    }
    let mut rows = Vec::new();
    run_stills(o, ctx, out, None, Some(&mut rows))?;
    let (over, near) = layer1_verdict(&rows);
    let names = planet_render::resolve::RESOLVE_PASS_NAMES;
    for r in &rows {
        let parts: Vec<String> = names.iter().zip(r.passes_ms).map(|(n, ms)| format!("{n} {ms:.3}")).collect();
        writeln!(out, "{}: layer 1 {:.3} ms ({})", r.pose, r.total_ms, parts.join(", ")).map_err(err)?;
    }
    writeln!(
        out,
        "1440p, each pose the median of {COST_FRAMES} frames after {COST_WARM} warm-up frames; the passes are the resolve's ({}); the upkeep of a settled pose is nil",
        names.join(", ")
    )
    .map_err(err)?;
    for n in &near {
        writeln!(out, "NEAR: {n}").map_err(err)?;
    }
    let poses: Vec<String> = rows
        .iter()
        .map(|r| {
            let passes: Vec<String> = names.iter().zip(r.passes_ms).map(|(n, ms)| format!("{n:?}: {ms:.4}")).collect();
            format!("    {{\"pose\": {:?}, \"layer1_ms\": {:.4}, \"passes_ms\": {{{}}}}}", r.pose, r.total_ms, passes.join(", "))
        })
        .collect();
    let size = o.size.unwrap_or((2560, 1440));
    let json = format!(
        "{{\n  \"scene\": \"shadow-layer1\",\n  \"adapter\": \"{}\",\n  \"owner_override\": {:?},\n  \"size\": [{}, {}],\n  \"frames\": {COST_FRAMES},\n  \"budget_ms\": {LAYER1_BUDGET_MS},\n  \"poses\": [\n{}\n  ],\n  \"near_budget\": {:?},\n  \"over_budget\": {:?}\n}}\n",
        ctx.adapter_name().replace('"', "'"),
        owner_override,
        size.0,
        size.1,
        poses.join(",\n"),
        near,
        over
    );
    let path = o.out.clone().unwrap_or_else(|| std::path::Path::new("perf").join("logs").join("shadow-layer1.json"));
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

/// Whether `--scene NAME` names the shadow and material cost scene (REND-044).
pub fn is_cost_scene(name: &str) -> bool {
    name == "shadow-cost"
}

/// The budget of the material map, milliseconds of terrain-pass GPU time at 1440p (REND-044, M8 criterion 5).
pub const COST_BUDGET_MS: f64 = 1.5;
/// A share within this fraction of the budget is flagged, not called met (the compiled-out variants have no branch left) (opus-planner, 2026-10-27).
pub const COST_NEAR: f64 = 0.2;
/// Frames dropped before the timed ones of a variant, and the timed frames whose median is the variant's time.
const COST_WARM: usize = 8;
const COST_FRAMES: usize = 41;
/// Seconds of untimed frames before the first variant of a pose (the GPU idled while the pose settled) and before each later one.
const COST_WARM_FIRST_S: f64 = 3.0;
const COST_WARM_S: f64 = 1.0;
/// Rounds of the round-robin over the product shader and the build with the material map compiled out.
const COST_ROUNDS: usize = 3;

/// The terrain-pass GPU time of one pose, milliseconds, medians of [`COST_FRAMES`] waited frames (REND-044).
#[derive(Clone, Debug, PartialEq)]
pub struct CostRow {
    pub pose: String,
    /// Ground detail at full strength.
    pub full: f64,
    /// Ground detail at strength 0.
    pub no_material: f64,
    /// The same without the material map with the code compiled out of the shader (`MeasureVariant::NoMaterial`); no branch is left, so the
    /// share from it does not over-read the material map.
    pub compiled_out: Option<f64>,
    /// The spread (largest minus smallest) of the round medians of `full` and the compiled-out variant; `full` and `compiled_out` are the
    /// medians of those [`COST_ROUNDS`] round medians, the variants interleaved round-robin so a slow period hits both alike.
    pub spread: Option<[f64; 2]>,
    /// The draw with the patch break-up at 0 (REND-055) and everything else as `full`; none when the plan's patch is 0.
    pub no_patch: Option<f64>,
    /// The draw with the coefficients of the 64 m and 8 m octaves at 0 (REND-057) and everything else as `full`; none when both are 0 in the plan.
    pub no_octaves: Option<f64>,
}

impl CostRow {
    /// The patch break-up's share (full minus the draw with the patch 0), the runtime-off difference; none without a patch.
    pub fn patch_share(&self) -> Option<f64> {
        self.no_patch.map(|n| self.full - n)
    }

    /// The share of the 64 m and 8 m octaves (full minus the draw with them at 0); none when the plan has them at 0.
    pub fn octave_share(&self) -> Option<f64> {
        self.no_octaves.map(|n| self.full - n)
    }

    /// The material map's share (full minus the variant without it), from the compiled-out variant when the row has it, else from the
    /// runtime one.
    pub fn share(&self) -> f64 {
        self.full - self.compiled_out.unwrap_or(self.no_material)
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

/// Time the terrain pass of the settled pose in the two variants: the product shader and the build with the material map compiled out,
/// round-robin, then the runtime-off draw (ground detail at strength 0).
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
    // The product shader and the build with the material map compiled out (REND-044), round-robin for [`COST_ROUNDS`] rounds: a slow period
    // of the machine falls on both alike, and the value of a variant is the median of its round medians (the minimum would bias the share low).
    use planet_render::MeasureVariant as V;
    let variants = [V::Product, V::NoMaterial];
    // `rounds[round][variant]`: the median of one round of one variant.
    let mut rounds = [[0.0; 2]; COST_ROUNDS];
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
    let (full, compiled_out) = (med(0), med(1));
    let spread = [spread(0), spread(1)];
    detail(plan, 0.0);
    let no_material = terrain_ms(ctx, r, plan, COST_WARM_S)?;
    detail(plan, 1.0);
    // The patch's own share: the same draw with the patch 0 (the plan keeps its value afterwards).
    let patch = plan.lit.as_ref().map_or(0.0, |l| l.ground_patch);
    let no_patch = if patch > 0.0 {
        if let Some(l) = plan.lit.as_mut() {
            l.ground_patch = 0.0;
        }
        let ms = terrain_ms(ctx, r, plan, COST_WARM_S)?;
        if let Some(l) = plan.lit.as_mut() {
            l.ground_patch = patch;
        }
        Some(ms)
    } else {
        None
    };
    // The two octaves' own share (REND-057): the same draw with their coefficients 0, which the shader skips (the plan keeps its table afterwards).
    let zero =
        planet_frame::look::materials::OctaveCoef { albedo: [0.0; 3], grass: [0.0; 3], tilt: [0.0; 4], scree: [0.0; 2], wet: [0.0; 2] };
    let coef = plan.lit.as_ref().map(|l| l.coef);
    let no_octaves = match coef {
        Some(c) if c.octaves[4..].iter().any(|o| *o != zero) => {
            if let Some(l) = plan.lit.as_mut() {
                l.coef.octaves[4] = zero;
                l.coef.octaves[5] = zero;
            }
            let ms = terrain_ms(ctx, r, plan, COST_WARM_S)?;
            if let Some(l) = plan.lit.as_mut() {
                l.coef = c;
            }
            Some(ms)
        }
        _ => None,
    };
    Ok(CostRow { pose: pose.to_string(), full, no_material, compiled_out: Some(compiled_out), spread: Some(spread), no_patch, no_octaves })
}

/// The failures of the budget (the material map at most [`COST_BUDGET_MS`] in every pose) and the poses within
/// [`COST_NEAR`] of it, which are flagged, not called met (REND-044).
pub fn cost_verdict(rows: &[CostRow]) -> (Vec<String>, Vec<String>) {
    let (mut over, mut near) = (Vec::new(), Vec::new());
    for r in rows {
        let m = r.share();
        if !(m.is_finite() && m <= COST_BUDGET_MS) {
            over.push(format!("{}: material map {m:.3} ms exceeds {COST_BUDGET_MS} ms (REND-044)", r.pose));
        } else if m > COST_BUDGET_MS * (1.0 - COST_NEAR) {
            near.push(format!(
                "{}: {m:.3} ms is within 20 % of the {COST_BUDGET_MS} ms budget (by the compiled-out shader variant when the row has it): not called met",
                r.pose
            ));
        }
    }
    (over, near)
}

// spec: REND-044
/// `test-render --perf --scene shadow-cost --poses FILE --out-dir DIR --store DIR --global-res N`: after the readiness gate, the settled poses of
/// the stills at 1440p with the terrain pass timed in two variants (the product shader and the material map compiled out); the log is `perf/logs/shadow-cost.json`.
pub fn perf_shadow_cost(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    let mut rows = Vec::new();
    run_stills(o, ctx, out, Some(&mut rows), None)?;
    let (over, near) = cost_verdict(&rows);
    for r in &rows {
        let m = r.share();
        writeln!(
            out,
            "{}: terrain pass {:.3} ms; material map {m:.3} ms (runtime off: without material {:.3}; compiled out: {}; spread across rounds: {})",
            r.pose,
            r.full,
            r.no_material,
            r.compiled_out.map_or("none".to_string(), |c| format!("{c:.3}")),
            r.spread.map_or("none".to_string(), |c| format!("{:.3}, {:.3}", c[0], c[1]))
        )
        .map_err(err)?;
        if let Some(p) = r.patch_share() {
            writeln!(out, "{}: patch break-up {p:.3} ms (full minus the draw with the patch 0; runtime difference, in the material map's share above)", r.pose)
                .map_err(err)?;
        }
    }
    writeln!(
        out,
        "the share is full minus the shader with the material map compiled out (no branch left; the runtime-off draw over-reads it); 1440p, each variant the median of {COST_ROUNDS} round medians of {COST_FRAMES} frames, interleaved round-robin"
    )
    .map_err(err)?;
    for n in &near {
        writeln!(out, "NEAR: {n}").map_err(err)?;
    }
    let poses: Vec<String> = rows
        .iter()
        .map(|r| {
            let m = r.share();
            format!(
                "    {{\"pose\": {:?}, \"full_ms\": {:.4}, \"no_material_ms\": {:.4}, \"compiled_out_ms\": {}, \"round_spread_ms\": {}, \"no_patch_ms\": {}, \"patch_ms\": {}, \"no_octaves_ms\": {}, \"octaves_ms\": {}, \"material_ms\": {m:.4}, \"together_ms\": {m:.4}}}",
                r.pose,
                r.full,
                r.no_material,
                r.compiled_out.map_or("null".to_string(), |c| format!("[{c:.4}]")),
                r.spread.map_or("null".to_string(), |c| format!("[{:.4}, {:.4}]", c[0], c[1])),
                r.no_patch.map_or("null".to_string(), |c| format!("{c:.4}")),
                r.patch_share().map_or("null".to_string(), |c| format!("{c:.4}")),
                r.no_octaves.map_or("null".to_string(), |c| format!("{c:.4}")),
                r.octave_share().map_or("null".to_string(), |c| format!("{c:.4}"))
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
    fn the_cost_share_is_a_difference_and_a_pose_near_the_budget_is_flagged_not_called_met() {
        let row = |pose: &str, full: f64, material: f64| CostRow {
            pose: pose.into(),
            full,
            no_material: material,
            compiled_out: None,
            spread: None,
            no_patch: None,
            no_octaves: None,
        };
        // The compiled-out variant, when a row has it, is what the share and the verdict read.
        let with = CostRow { compiled_out: Some(4.2), ..row("w", 5.0, 4.6) };
        assert_eq!(with.share(), 5.0 - 4.2);
        // The patch's share is the difference to the draw with the patch 0, and absent without a patch.
        assert_eq!(with.patch_share(), None);
        assert_eq!(CostRow { no_patch: Some(4.75), ..row("p", 5.0, 4.6) }.patch_share(), Some(0.25));
        // The octaves' share likewise, absent when the plan has them at 0.
        assert_eq!(with.octave_share(), None);
        assert_eq!(CostRow { no_octaves: Some(4.5), ..row("o", 5.0, 4.6) }.octave_share(), Some(0.5));
        let ok = row("a", 5.0, 4.0);
        assert_eq!(ok.share(), 1.0);
        assert_eq!(cost_verdict(&[ok]), (vec![], vec![]));
        let (over, flagged) = cost_verdict(&[row("b", 5.0, 3.7)]);
        assert!(over.is_empty() && flagged.len() == 1 && flagged[0].contains("within 20 %"), "{flagged:?}");
        let (over, _) = cost_verdict(&[row("c", 5.0, 3.0)]);
        assert!(over.len() == 1 && over[0].starts_with("c:") && over[0].contains("REND-044"), "{over:?}");
        // A pose with no reading is not met.
        assert_eq!(cost_verdict(&[row("d", f64::NAN, 1.0)]).0.len(), 1);
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert!(median(vec![]).is_nan());
    }

    // spec: TEST-050
    #[test]
    fn the_keys_of_the_horizon_era_in_an_old_poses_file_are_ignored_not_refused() {
        let old = format!(
            "chunk_timeout_s = 300
horizon_cell_m = 300
coarse_horizon = true
{GOOD}"
        );
        assert_eq!(parse_config(&old).unwrap().poses.len(), parse_config(GOOD).unwrap().poses.len());
    }

    // spec: REND-052
    #[test]
    fn the_layer1_verdict_fails_above_the_budget_and_flags_a_pose_within_five_percent() {
        let row = |pose: &str, ms: f64| Layer1Row { pose: pose.into(), passes_ms: [ms / 5.0; 5], total_ms: ms };
        assert_eq!(layer1_verdict(&[row("a", 1.4)]), (vec![], vec![]));
        let (over, near) = layer1_verdict(&[row("b", 1.6)]);
        assert!(over.len() == 1 && over[0].starts_with("b:") && over[0].contains("REND-052") && near.is_empty(), "{over:?}");
        let (over, near) = layer1_verdict(&[row("c", 1.52)]);
        assert!(over.is_empty() && near.len() == 1 && near[0].contains("within 5 %"), "{near:?}");
        assert_eq!(layer1_verdict(&[row("d", f64::NAN)]).0.len(), 1);
        assert!(is_layer1_scene("shadow-layer1") && !is_layer1_scene("shadow-cost"));
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

    // spec: LOOK-012
    #[test]
    fn the_poses_file_sets_the_patch_break_up_within_its_range() {
        let c = parse_config(&format!(
            "{GOOD}
[look]
ground_patch = 0.12
"
        ))
        .unwrap();
        assert_eq!(c.look, LookOverride { ground_patch: Some(0.12), ..LookOverride::default() });
        let mut lit = planet_frame::look::LitPlan::fixed(12.0);
        assert_eq!(lit.ground_patch, 0.0);
        c.look.apply(&mut lit);
        assert_eq!(lit.ground_patch, 0.12);
        for bad in ["0.9", "-0.1"] {
            let e = parse_config(&format!(
                "{GOOD}
[look]
ground_patch = {bad}
"
            ))
            .unwrap_err();
            assert!(e.contains("ground_patch") && e.contains(bad), "{bad}: {e}");
        }
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
        assert_eq!(c.look, LookOverride { ground_detail: Some(0.6), ground_patch: None, curves: vec![(3, 6, [0.5, 0.8])] });
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

    // spec: TEST-128
    #[test]
    fn the_centre_distance_of_a_view_is_the_range_to_the_ground_along_its_axis_and_none_for_the_sky() {
        let r = 6_371_000.0;
        // A flat ground 100 m up: an eye 400 m above it looking 30 degrees down meets it after 400 / sin(30 deg) = 800 m.
        let eye = Vec3::new(1.0, 0.0, 0.0) * (r + 500.0);
        let cam =
            Camera::at_surface_point(PlanetFixed(eye), Vec3::new(0.0, 1.0, 0.0), 30f64.to_radians(), 60f64.to_radians(), 16.0 / 9.0, 0.1);
        let d = centre_distance_m(&cam, &|_| 100.0, r, 0.0).expect("the axis meets the ground");
        assert!((d - 800.0).abs() < 1.0, "{d}");
        // The sea level is the floor under a lower ground: the same eye over a ground at -50 m meets the sea at 0 m: 500 / sin(30 deg) = 1000 m.
        let d = centre_distance_m(&cam, &|_| -50.0, r, 0.0).expect("the sea");
        assert!((d - 1000.0).abs() < 1.5, "{d}");
        // Looking up: no ground. The label of such a view says so and the pitch is negative.
        let up = Camera::at_surface_point(
            PlanetFixed(eye),
            Vec3::new(0.0, 1.0, 0.0),
            (-10f64).to_radians(),
            60f64.to_radians(),
            16.0 / 9.0,
            0.1,
        );
        assert_eq!(centre_distance_m(&up, &|_| 100.0, r, 0.0), None);
        let label = pose_label(&cam, &|_| 100.0, r, 0.0);
        assert!(
            (label.altitude_m - 400.0).abs() < 1e-6 && (label.pitch_deg - 30.0).abs() < 1e-6 && (label.fov_deg - 60.0).abs() < 1e-9,
            "{label:?}"
        );
        assert!(pose_label(&up, &|_| 100.0, r, 0.0).pitch_deg < 0.0);
    }
}
