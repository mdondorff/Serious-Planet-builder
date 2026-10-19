//! The globe overview (VIEW-013): an orbit camera around the planet, item markers projected to the screen, a click on a marker or on
//! the surface, and the flight that takes the camera there. All float64 on the CPU, GPU-free (CON-03, CON-11); the lit path draws the
//! planet as it does from any orbit, the editor draws the markers over it with the screen positions computed here.

use crate::camera::Camera;
use planet_core::{PlanetFixed, Vec3};

/// The farthest the overview goes, in planet radii above the surface.
pub const MAX_ALTITUDE_RADII: f64 = 8.0;
/// The nearest the overview goes: the altitude the globe flight ends at over a clicked place, never below this (metres).
pub const MIN_ALTITUDE_M: f64 = 1_000.0;

/// An item as the globe shows it: a marker on the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub id: u64,
    /// The unit direction from the planet's centre.
    pub dir: Vec3,
    /// The item's footprint radius along the surface, metres: a flight to it ends this far (times 4) above it.
    pub radius_m: f64,
}

/// The overview's camera: over `center` at `altitude_m` above the sphere of radius `radius_m`, looking at the planet's centre with
/// north up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeView {
    /// The unit direction of the point under the camera.
    pub center: Vec3,
    /// Camera height above the sphere, metres.
    pub altitude_m: f64,
}

fn north_or_fallback(center: Vec3) -> Vec3 {
    let north = Vec3::new(0.0, 0.0, 1.0);
    if north.cross(center).length() < 1e-9 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        north
    }
}

/// The unit tangent vectors (east, north) of the sphere at `center`.
fn tangents(center: Vec3) -> (Vec3, Vec3) {
    let east = north_or_fallback(center).cross(center).normalized();
    let north = center.cross(east).normalized();
    (east, north)
}

impl GlobeView {
    pub fn new(center: Vec3, altitude_m: f64) -> GlobeView {
        GlobeView { center: center.normalized(), altitude_m }
    }

    /// The camera of this view.
    pub fn camera(&self, radius_m: f64, fov_y_rad: f64, aspect: f64, near_m: f64) -> Camera {
        let position = PlanetFixed(self.center * (radius_m + self.altitude_m));
        Camera::looking_at_centre(position, north_or_fallback(self.center), fov_y_rad, aspect, near_m)
    }

    /// The ground angle (radians) one pixel of a `height_px` viewport spans at the centre of the view.
    fn radians_per_pixel(&self, radius_m: f64, fov_y_rad: f64, height_px: f64) -> f64 {
        let metres_per_pixel = 2.0 * libm::tan(fov_y_rad / 2.0) * self.altitude_m.max(1.0) / height_px;
        metres_per_pixel / radius_m
    }

    /// Drag the globe by `(dx_px, dy_px)` pixels: the surface follows the cursor (dragging right brings the western side into view).
    pub fn drag(&mut self, dx_px: f64, dy_px: f64, radius_m: f64, fov_y_rad: f64, height_px: f64) {
        let k = self.radians_per_pixel(radius_m, fov_y_rad, height_px);
        let (east, north) = tangents(self.center);
        self.center = (self.center - east * (dx_px * k) + north * (dy_px * k)).normalized();
    }

    /// Zoom by `factor` (greater than 1 goes farther): the altitude stays in `[MIN_ALTITUDE_M, MAX_ALTITUDE_RADII * radius_m]`.
    pub fn zoom(&mut self, factor: f64, radius_m: f64) {
        self.altitude_m = (self.altitude_m * factor).clamp(MIN_ALTITUDE_M, MAX_ALTITUDE_RADII * radius_m);
    }

    /// The view that shows a marker closely: over its centre, four footprint radii up (never below `MIN_ALTITUDE_M`).
    pub fn closing_on(marker: &Marker) -> GlobeView {
        GlobeView { center: marker.dir, altitude_m: (4.0 * marker.radius_m).max(MIN_ALTITUDE_M) }
    }
}

/// A viewport in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width_px: f64,
    pub height_px: f64,
}

/// True when the point on the sphere in direction `dir` is on the side of the planet the camera sees (above the tangent plane at the
/// point), so a marker beyond the horizon is not shown.
pub fn faces_camera(camera_position: Vec3, dir: Vec3, radius_m: f64) -> bool {
    (camera_position - dir * radius_m).dot(dir) > 0.0
}

/// The pixel `(x, y)` (origin top-left) of a point in `PlanetFixed`, `None` when it is behind the camera or outside the viewport.
pub fn project(camera: &Camera, point: Vec3, viewport: Viewport) -> Option<[f64; 2]> {
    let clip = camera.project(point - camera.position.0);
    let depth = clip[3];
    if depth.is_nan() || depth <= 0.0 {
        return None;
    }
    let (ndc_x, ndc_y) = (clip[0] / depth, clip[1] / depth);
    if ndc_x.is_nan() || ndc_y.is_nan() || ndc_x.abs() > 1.0 || ndc_y.abs() > 1.0 {
        return None;
    }
    Some([(ndc_x + 1.0) * 0.5 * viewport.width_px, (1.0 - ndc_y) * 0.5 * viewport.height_px])
}

/// A marker on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenMarker {
    pub id: u64,
    pub pixel: [f64; 2],
}

/// The markers that can be seen from the camera, in the order given, with their pixels.
pub fn visible_markers(camera: &Camera, markers: &[Marker], radius_m: f64, viewport: Viewport) -> Vec<ScreenMarker> {
    markers
        .iter()
        .filter(|m| faces_camera(camera.position.0, m.dir, radius_m))
        .filter_map(|m| project(camera, m.dir * radius_m, viewport).map(|pixel| ScreenMarker { id: m.id, pixel }))
        .collect()
}

/// The marker under the cursor: the nearest of the visible ones within `hit_px` pixels (ties go to the lower id), `None` when no
/// marker is that close.
pub fn marker_at(visible: &[ScreenMarker], cursor: [f64; 2], hit_px: f64) -> Option<u64> {
    let mut best: Option<(f64, u64)> = None;
    for m in visible {
        let d = libm::hypot(m.pixel[0] - cursor[0], m.pixel[1] - cursor[1]);
        if d.is_nan() || d > hit_px {
            continue;
        }
        if best.is_none_or(|b| (d, m.id) < b) {
            best = Some((d, m.id));
        }
    }
    best.map(|b| b.1)
}

/// The flight from one view to another: the point under the camera moves along the great circle, the altitude follows a smooth curve
/// that rises on a long trip (so the whole way is seen) and ends exactly at the target's altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeFlight {
    pub from: GlobeView,
    pub to: GlobeView,
    /// The angle between the two centres, radians.
    pub angle_rad: f64,
    pub duration_s: f64,
    /// The planet radius, metres: the scale of the rise on a long trip.
    pub radius_m: f64,
}

/// The shortest flight, seconds; a longer trip adds `SECONDS_PER_RADIAN` for each radian of arc.
pub const MIN_FLIGHT_S: f64 = 0.8;
pub const SECONDS_PER_RADIAN: f64 = 1.2;

fn smootherstep(s: f64) -> f64 {
    s * s * s * (s * (s * 6.0 - 15.0) + 10.0)
}

impl GlobeFlight {
    pub fn new(from: GlobeView, to: GlobeView, radius_m: f64) -> GlobeFlight {
        let angle_rad = libm::atan2(from.center.cross(to.center).length(), from.center.dot(to.center));
        GlobeFlight { from, to, angle_rad, duration_s: MIN_FLIGHT_S + SECONDS_PER_RADIAN * angle_rad, radius_m }
    }

    /// The view `t_s` seconds into the flight (clamped to the flight).
    pub fn at(&self, t_s: f64) -> GlobeView {
        let s = smootherstep((t_s / self.duration_s).clamp(0.0, 1.0));
        // The centre: spherical interpolation, by the angle travelled (a straight blend when the two coincide or are opposite is
        // not needed: opposite centres take the great circle through `tangents` of the start).
        let center = if self.angle_rad < 1e-12 {
            self.to.center
        } else if self.angle_rad > core::f64::consts::PI - 1e-9 {
            let (east, _) = tangents(self.from.center);
            let a = s * core::f64::consts::PI;
            (self.from.center * libm::cos(a) + east * libm::sin(a)).normalized()
        } else {
            let sin_total = libm::sin(self.angle_rad);
            let (wa, wb) = (libm::sin((1.0 - s) * self.angle_rad) / sin_total, libm::sin(s * self.angle_rad) / sin_total);
            (self.from.center * wa + self.to.center * wb).normalized()
        };
        // The altitude: geometric interpolation between the two, plus a rise that peaks at the middle of the trip at 1.5 planet radii
        // times the arc's share of a half circle (so a trip over a quarter of the planet is seen from 0.75 radii up).
        let altitude_m = if s <= 0.0 {
            self.from.altitude_m
        } else if s >= 1.0 {
            self.to.altitude_m
        } else {
            let log_alt = (1.0 - s) * libm::log(self.from.altitude_m) + s * libm::log(self.to.altitude_m);
            let rise = 1.5 * self.radius_m * (self.angle_rad / core::f64::consts::PI) * libm::sin(core::f64::consts::PI * s);
            libm::exp(log_alt) + rise
        };
        GlobeView { center, altitude_m }
    }

    pub fn is_done(&self, t_s: f64) -> bool {
        t_s >= self.duration_s
    }
}
