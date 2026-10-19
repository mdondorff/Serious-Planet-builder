//! The shape of an authored item on the planet and the handles that edit it (VIEW-017): the outline radius, where the handles sit, and
//! the item a handle drag makes. All f64 on the CPU, `libm` only (CON-11, CON-14); no window, no GPU.
//!
//! The parameters are read by key (`radius_m`, `height_m`) and the rest of the item is untouched, so a kind the stamp stage adds later
//! is edited by the same handles as long as it names its metric parameters this way.

use crate::params;
use planet_core::Vec3;
use planet_frame::Camera;
use planet_world_def::cells::CELL_EDGE_M;
use planet_world_def::item::{Item, MAX_DELTA_LIMIT_M};

/// The parameter that is the radius of an item's outline, metres.
pub const RADIUS_KEY: &str = "radius_m";
/// The parameter that is the height of an item's change, metres.
pub const HEIGHT_KEY: &str = "height_m";
/// The smallest radius a drag makes of a kind the parameter table does not know, metres.
pub const MIN_RADIUS_M: f64 = 1.0;
/// How much further than its outline the reach of an edit reaches, as a factor.
pub const REACH_FACTOR: f64 = 1.25;

/// What a handle changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Handle {
    /// The anchor: where the item is.
    Move,
    /// The radius of the outline.
    Radius,
    /// The height of the change.
    Height,
}

/// The radius of the outline of an item, metres: its `radius_m` parameter when it has a positive one, else its reach.
pub fn outline_radius_m(item: &Item) -> f64 {
    match item.params.get(RADIUS_KEY) {
        Some(r) if r.is_finite() && *r > 0.0 => *r,
        _ => item.reach_m,
    }
}

/// The handles an item has: the anchor always, the radius when it has an outline, the height when it has a `height_m` parameter.
pub fn handles_of(item: &Item) -> Vec<Handle> {
    let mut v = vec![Handle::Move];
    if outline_radius_m(item) > 0.0 {
        v.push(Handle::Radius);
    }
    if item.params.get(HEIGHT_KEY).is_some_and(|h| h.is_finite()) {
        v.push(Handle::Height);
    }
    v
}

/// North and east at the unit direction `dir`: north is the pole's direction projected on the tangent plane. At a pole itself north is
/// the limit along longitude 0 (the spelling an item file gives a pole): -x at the north pole, +x at the south pole.
pub fn tangent_frame(dir: Vec3) -> (Vec3, Vec3) {
    let pole = Vec3::new(0.0, 0.0, 1.0);
    let mut north = pole - dir * pole.dot(dir);
    if north.length() < 1e-15 {
        north = if dir.z > 0.0 { Vec3::new(-1.0, 0.0, 0.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    }
    let north = north.normalized();
    (north, north.cross(dir).normalized())
}

/// The unit direction `arc_m` metres from `dir` along the heading `heading_deg` (clockwise from north) on a sphere of `radius_m`.
pub fn point_at(dir: Vec3, heading_deg: f64, arc_m: f64, radius_m: f64) -> Vec3 {
    let (north, east) = tangent_frame(dir);
    let h = heading_deg.to_radians();
    let t = north * libm::cos(h) + east * libm::sin(h);
    let a = arc_m / radius_m;
    (dir * libm::cos(a) + t * libm::sin(a)).normalized()
}

/// Where a handle sits in `PlanetFixed`, metres, on a body of `radius_m` whose ground at the item is `ground_m` above the sphere: the
/// anchor on the ground, the radius handle on the outline along the item's heading, the height handle above the anchor by the height.
pub fn handle_point(item: &Item, handle: Handle, radius_m: f64, ground_m: f64) -> Vec3 {
    let dir = item.direction();
    let r = radius_m + ground_m;
    match handle {
        Handle::Move => dir * r,
        Handle::Radius => point_at(dir, item.anchor.heading_deg, outline_radius_m(item), radius_m) * r,
        Handle::Height => dir * (r + item.params.get(HEIGHT_KEY).copied().unwrap_or(0.0)),
    }
}

/// Latitude and longitude in degrees of a unit direction, in the spelling an item file takes: longitude in (-180, 180], 0 at a pole.
pub fn lat_lon_of(dir: Vec3) -> (f64, f64) {
    let d = dir.normalized();
    // `atan2` of the height over the distance from the axis keeps its precision at the poles, where `asin` loses it.
    let lat = libm::atan2(d.z, libm::hypot(d.x, d.y)).to_degrees().clamp(-90.0, 90.0);
    if lat.abs() >= 90.0 - 1e-9 {
        return (if lat > 0.0 { 90.0 } else { -90.0 }, 0.0);
    }
    let mut lon = libm::atan2(d.y, d.x).to_degrees();
    if lon <= -180.0 {
        lon += 360.0;
    }
    (lat, if lon == 0.0 { 0.0 } else { lon })
}

/// The item moved so that its anchor is under the unit direction `dir`.
pub fn moved(item: &Item, dir: Vec3) -> Item {
    let (lat, lon) = lat_lon_of(dir);
    let mut it = item.clone();
    it.anchor.latitude_deg = lat;
    it.anchor.longitude_deg = lon;
    it
}

/// The range a drag keeps parameter `key` of `item` in: the kind's range when the table has one, else `(lo, hi)`.
fn drag_range(item: &Item, key: &str, lo: f64, hi: f64) -> (f64, f64) {
    params::def_of(&item.kind, key).map_or((lo, hi), |d| (d.min.max(lo), d.max.min(hi)))
}

/// The item resized so that its outline passes through the unit direction `dir`: the `radius_m` parameter becomes the arc from the anchor
/// to `dir`, kept in the kind's range (else 1 m to one cell edge), and the reach grows to hold the outline with its margin (never shrinks).
pub fn resized(item: &Item, dir: Vec3, radius_m: f64) -> Item {
    let c = item.direction();
    let d = dir.normalized();
    let arc = radius_m * libm::atan2(c.cross(d).length(), c.dot(d));
    let (lo, hi) = drag_range(item, RADIUS_KEY, MIN_RADIUS_M, CELL_EDGE_M);
    let r = arc.clamp(lo, hi);
    let mut it = item.clone();
    it.params.insert(RADIUS_KEY.to_string(), r);
    it.reach_m = it.reach_m.max((r * REACH_FACTOR).min(CELL_EDGE_M));
    it
}

/// The item with its `height_m` changed by `delta_m`, kept in the kind's range (else within the limit on the change) and the bound on the
/// change grown to hold it (never shrinks).
pub fn raised(item: &Item, delta_m: f64) -> Item {
    let mut it = item.clone();
    let (lo, hi) = drag_range(item, HEIGHT_KEY, -MAX_DELTA_LIMIT_M, MAX_DELTA_LIMIT_M);
    let h = (it.params.get(HEIGHT_KEY).copied().unwrap_or(0.0) + delta_m).clamp(lo, hi);
    it.params.insert(HEIGHT_KEY.to_string(), h);
    it.max_delta_m = it.max_delta_m.max(h.abs()).min(MAX_DELTA_LIMIT_M);
    it
}

/// Metres on the screen's vertical axis that one pixel spans at `point` (`PlanetFixed`): the camera's field of view at the point's
/// distance. `NaN`-free: a point at the camera counts as one metre away.
pub fn metres_per_pixel(camera: &Camera, viewport_height_px: f64, point: Vec3) -> f64 {
    let d = (point - camera.position.0).dot(camera.forward.normalized()).max(1.0);
    2.0 * d * libm::tan(0.5 * camera.fov_y_rad) / viewport_height_px.max(1.0)
}
