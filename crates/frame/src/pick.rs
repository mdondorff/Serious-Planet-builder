//! Surface picking (VIEW-012): the mouse ray to the planet surface and the item under it, all float64 on the CPU (ADR 0001, CON-11).
//!
//! The ray starts at the camera in `PlanetFixed` and runs against the height field `r(d) = R + h(d)` (`d` the unit direction from the
//! centre), not against the drawn tiles, so the answer is the same at every level of detail and before a tile has arrived. The march
//! is sphere tracing with a slope bound (`PickLimits::max_slope`) inside the shell `[R - max_height, R + max_height]`, followed by a
//! bisection of the bracketing step; `libm` only (CON-14).

use crate::camera::Camera;
use planet_core::Vec3;

/// A ray: `origin + t dir`, `dir` a unit vector, `t` in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

/// Where the mouse ray meets the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceHit {
    /// The hit point in `PlanetFixed`, metres.
    pub point: Vec3,
    /// The unit direction from the planet centre to the hit point.
    pub dir: Vec3,
    /// The height above the radius `R` at `dir`, metres.
    pub height_m: f64,
    /// The distance from the ray origin along the ray, metres.
    pub distance_m: f64,
}

/// The bounds the march needs from the height field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickLimits {
    /// The planet radius `R`, metres.
    pub radius_m: f64,
    /// The height field stays within `[-max_height_m, max_height_m]` (metres): the shell the march runs in.
    pub max_height_m: f64,
    /// An upper bound of the height field's slope `|dh/ds|` along the surface (metres per metre). The march steps `f / (1 + max_slope)`
    /// where `f` is the distance of the ray above the surface, so a slope beyond the bound may be stepped over.
    pub max_slope: f64,
    /// The bisection stops when the bracket is shorter than this (metres); a value that is not positive means 1e-9.
    pub tolerance_m: f64,
    /// A march step never shrinks below this (metres), so a ray skimming the surface keeps moving. A ray that meets the surface at
    /// an incidence angle below about 1e-4 rad (within a hundredth of a degree of the horizon) can use up the step budget and is
    /// reported as a miss.
    pub min_step_m: f64,
}

impl PickLimits {
    pub fn new(radius_m: f64, max_height_m: f64) -> PickLimits {
        PickLimits { radius_m, max_height_m, max_slope: 2.0, tolerance_m: 1e-4, min_step_m: 0.05 }
    }
}

/// Upper bound of the march steps; a ray that has not left the shell by then is a miss.
const MAX_STEPS: u32 = 100_000;
/// Upper bound of the bisection steps (an f64 bracket cannot halve more often than this).
const MAX_BISECTIONS: u32 = 200;

/// The ray through a cursor position. `x_px`, `y_px` count from the top-left corner of a `width_px` x `height_px` viewport; pixel
/// centres are at `+ 0.5`. A viewport without area (a minimised window) gives the camera's axis.
pub fn cursor_ray(camera: &Camera, x_px: f64, y_px: f64, width_px: f64, height_px: f64) -> Ray {
    let a = if width_px > 0.0 { 2.0 * x_px / width_px - 1.0 } else { 0.0 };
    let b = if height_px > 0.0 { 1.0 - 2.0 * y_px / height_px } else { 0.0 };
    let (right, up, fwd) = camera.ray_frame();
    Ray { origin: camera.position.0, dir: (fwd + right * a + up * b).normalized() }
}

/// The nearest intersections of a ray (unit direction) with a sphere about the origin: `(t_near, t_far)`, `None` when it misses or
/// an input is not finite. Computed from the closest approach, so the discriminant keeps its precision at planet radii.
fn sphere_interval(ray: &Ray, radius: f64) -> Option<(f64, f64)> {
    let tc = -ray.origin.dot(ray.dir);
    let closest = ray.origin + ray.dir * tc;
    let d2 = closest.dot(closest);
    let r2 = radius * radius;
    if d2.is_nan() || d2 > r2 {
        return None;
    }
    let half = libm::sqrt((radius - libm::sqrt(d2)) * (radius + libm::sqrt(d2)));
    Some((tc - half, tc + half))
}

/// The hit at distance `t` along the ray.
fn hit_at(ray: &Ray, height_m: &dyn Fn(Vec3) -> f64, t: f64) -> SurfaceHit {
    let point = ray.origin + ray.dir * t;
    let dir = point.normalized();
    SurfaceHit { point, dir, height_m: height_m(dir), distance_m: t }
}

/// The first point where the ray goes from above the surface to on or below it, `None` for a miss (the ray never enters the
/// shell, leaves it without touching the surface, starts below the surface or inside the shell's inner sphere, runs out of steps
/// (see `PickLimits::min_step_m`) or meets a height that is not finite). A ray that enters the shell where the surface is already
/// at or above the ray (the field at its bound) hits at the entry. A direction that is not a unit vector is normalised; a zero or
/// non-finite one is a miss.
pub fn pick_surface(ray: &Ray, limits: &PickLimits, height_m: &dyn Fn(Vec3) -> f64) -> Option<SurfaceHit> {
    let len = ray.dir.length();
    if !(len.is_finite() && len > 0.0) {
        return None;
    }
    let ray = &Ray { origin: ray.origin, dir: ray.dir * (1.0 / len) };
    let inner = limits.radius_m - limits.max_height_m;
    let origin_len = ray.origin.length();
    if origin_len.is_nan() || origin_len < inner {
        return None;
    }
    let (t_in, t_out) = sphere_interval(ray, limits.radius_m + limits.max_height_m)?;
    if t_out <= 0.0 {
        return None;
    }
    let f_at = |t: f64| -> f64 {
        let p = ray.origin + ray.dir * t;
        let len = p.length();
        len - limits.radius_m - height_m(p * (1.0 / len))
    };
    let mut t = t_in.max(0.0);
    let mut f = f_at(t);
    if !f.is_finite() {
        return None;
    }
    if f <= 0.0 {
        // Entering the shell onto a surface at its bound is a hit; starting below the surface is not.
        return if t_in > 0.0 { Some(hit_at(ray, height_m, t)) } else { None };
    }
    // The distance along the ray changes the surface arc by at most `r / (r - max_height)`-fold below the radius, so the slope term
    // is scaled by the largest such factor in the shell.
    let growth = limits.radius_m / inner.max(1.0);
    let scale = 1.0 / (1.0 + limits.max_slope * growth.max(1.0));
    for _ in 0..MAX_STEPS {
        if t >= t_out {
            return None;
        }
        // The floor grows with the distance from the camera, always far below a pixel's footprint there (about 1e-3 of the distance).
        let floor = limits.min_step_m.max(t * 1e-6);
        let next = (t + (f * scale).max(floor)).min(t_out);
        let f_next = f_at(next);
        if !f_next.is_finite() {
            return None;
        }
        if f_next <= 0.0 {
            return Some(refine(ray, limits, height_m, t, next));
        }
        t = next;
        f = f_next;
    }
    None
}

/// Bisect `[above, below]` (the surface is above `ray(above)`'s height, and `ray(below)` is on or under it).
fn refine(ray: &Ray, limits: &PickLimits, height_m: &dyn Fn(Vec3) -> f64, above: f64, below: f64) -> SurfaceHit {
    let tolerance = if limits.tolerance_m > 0.0 { limits.tolerance_m } else { 1e-9 };
    let (mut lo, mut hi) = (above, below);
    for _ in 0..MAX_BISECTIONS {
        if hi - lo <= tolerance {
            break;
        }
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        let p = ray.origin + ray.dir * mid;
        let len = p.length();
        if len - limits.radius_m - height_m(p * (1.0 / len)) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hit_at(ray, height_m, 0.5 * (lo + hi))
}

/// An item as the picker sees it: a spherical cap on the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub id: u64,
    /// The unit direction of the footprint's centre.
    pub dir: Vec3,
    /// The footprint's radius along the surface, metres.
    pub radius_m: f64,
}

/// The great-circle distance between two unit directions, metres on a sphere of `radius_m`. `atan2` of the cross and dot products
/// keeps the precision at the small angles of a footprint (an `acos` of the dot product loses it below about 1e-4 rad).
pub fn surface_distance_m(a: Vec3, b: Vec3, radius_m: f64) -> f64 {
    radius_m * a.angle_to(b)
}

/// The item under a surface hit: of the footprints that contain it, the one whose centre is nearest in units of its own radius (so a
/// small item inside a large one wins near its centre), then the smaller radius, then the lower id. `None` when no footprint
/// contains the hit. A footprint without a direction (zero or not finite) or with a radius that is not finite contains nothing.
pub fn pick_footprint(hit: &SurfaceHit, footprints: &[Footprint], radius_m: f64) -> Option<u64> {
    let mut best: Option<(f64, f64, u64)> = None;
    for f in footprints {
        let n = f.dir.length();
        if !(n.is_finite() && n > 0.0 && f.radius_m.is_finite()) {
            continue;
        }
        let d = surface_distance_m(hit.dir, f.dir, radius_m);
        if d.is_nan() || d > f.radius_m {
            continue;
        }
        let key = (if f.radius_m > 0.0 { d / f.radius_m } else { 0.0 }, f.radius_m, f.id);
        if best.is_none_or(|b| key < b) {
            best = Some(key);
        }
    }
    best.map(|b| b.2)
}
