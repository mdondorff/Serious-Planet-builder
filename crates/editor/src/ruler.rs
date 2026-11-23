//! 3D viewport ruler, cursor terrain readout, and elevation/slope transect model (VIEW-076).
//! GPU-free, float64 throughout, `libm` for transcendentals (CON-02, CON-03, CON-11, CON-14).

use crate::bookmarks::format_deg;
use crate::params::format_metres;
use crate::polyline::{self, GeoPoint};
use crate::relevance::ViewState;
use crate::shape;
use planet_core::surface::RAD_TO_DEG;
use planet_core::Vec3;
use planet_frame::globe::{faces_camera, project};

/// Slope threshold above which the heightfield LOD/realism target stops (ADR 0091: 45 degrees).
pub const HEIGHTFIELD_SLOPE_LIMIT_DEG: f64 = 45.0;
/// Default sample points along a pinned transect (64 intervals along the great-circle arc).
pub const DEFAULT_TRANSECT_SAMPLES: usize = 65;
/// Finite-difference half-baseline for cursor local slope estimation, metres.
pub const GRADIENT_STEP_M: f64 = 1.0;
/// Great-circle visual subdivisions for the two-endpoint hover rubber-band.
pub const RUBBER_ARC_SEGMENTS: usize = 32;
/// Status hint shown when `Tool::Ruler` is active.
pub const RULER_HINT: &str = "Click point A on the planet, then point B to measure distance, elevation and slope · Esc clears";

const COINCIDENT_EPS: f64 = 1e-12;

fn is_valid_dir(dir: Vec3) -> bool {
    dir.x.is_finite() && dir.y.is_finite() && dir.z.is_finite() && dir.length() > 0.0
}

fn format_signed_height(height_m: f64) -> String {
    if !height_m.is_finite() {
        return "0.0 m".to_string();
    }
    let h = if height_m == 0.0 { 0.0 } else { height_m };
    if libm::fabs(h) >= 10_000.0 {
        format!("{:+.2} km", h / 1000.0)
    } else {
        format!("{h:+.1} m")
    }
}

/// Live terrain readout under the pointer (`lat/lon`, elevation, local surface slope, VIEW-076).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorReadout {
    pub point: GeoPoint,
    pub height_m: f64,
    pub gradient: f64,
    pub slope_deg: f64,
    pub over_limit: bool,
}

impl CursorReadout {
    /// Compact one-line summary for the editor status bar and ruler panel.
    pub fn summary(&self) -> String {
        let warn = if self.over_limit { " (>45°)" } else { "" };
        format!(
            "{}, {} · h {} · slope {:.1}°{warn}",
            format_deg(self.point.lat_deg, 'N', 'S'),
            format_deg(self.point.lon_deg, 'E', 'W'),
            format_signed_height(self.height_m),
            self.slope_deg,
        )
    }
}

/// Finite-difference local terrain gradient `(dz/ds, slope_deg)` at `dir` on a sphere of `radius_m`.
/// Uses pole-safe tangent cardinal points from [`shape::point_at`]; returns `(0.0, 0.0)` on non-finite input.
pub fn local_gradient(dir: Vec3, radius_m: f64, step_m: f64, ground_m: &dyn Fn(Vec3) -> f64) -> (f64, f64) {
    if !is_valid_dir(dir) || !radius_m.is_finite() || radius_m <= 0.0 || !step_m.is_finite() || step_m <= 0.0 {
        return (0.0, 0.0);
    }
    let d = dir.normalized();
    let n = shape::point_at(d, 0.0, step_m, radius_m);
    let s = shape::point_at(d, 180.0, step_m, radius_m);
    let e = shape::point_at(d, 90.0, step_m, radius_m);
    let w = shape::point_at(d, 270.0, step_m, radius_m);
    let span = 2.0 * step_m;
    let dz_dn = (ground_m(n) - ground_m(s)) / span;
    let dz_de = (ground_m(e) - ground_m(w)) / span;
    if !dz_dn.is_finite() || !dz_de.is_finite() {
        return (0.0, 0.0);
    }
    let grad = libm::hypot(dz_dn, dz_de);
    let slope_deg = libm::atan(grad) * RAD_TO_DEG;
    (grad, slope_deg)
}

/// Builds a [`CursorReadout`] for a picked surface direction `dir` and elevation `height_m`.
pub fn cursor_readout(dir: Vec3, height_m: f64, radius_m: f64, ground_m: &dyn Fn(Vec3) -> f64) -> CursorReadout {
    let unit = if is_valid_dir(dir) { dir.normalized() } else { Vec3::new(1.0, 0.0, 0.0) };
    let point = GeoPoint::of_dir(unit).canonical();
    let (gradient, slope_deg) = local_gradient(unit, radius_m, GRADIENT_STEP_M, ground_m);
    let safe_h = if height_m.is_finite() { height_m } else { 0.0 };
    CursorReadout { point, height_m: safe_h, gradient, slope_deg, over_limit: slope_deg > HEIGHTFIELD_SLOPE_LIMIT_DEG }
}

/// Two-point ruler interaction state in editor mode (`VIEW-076`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RulerState {
    #[default]
    Idle,
    Anchored {
        start: GeoPoint,
        start_height_m: f64,
    },
    Measured {
        start: GeoPoint,
        end: GeoPoint,
    },
}

impl RulerState {
    pub fn is_idle(&self) -> bool {
        matches!(self, RulerState::Idle)
    }

    pub fn clear(&mut self) {
        *self = RulerState::Idle;
    }

    /// Reverses `start` and `end` when a two-point measurement is pinned. Returns `true` if swapped.
    pub fn swap(&mut self) -> bool {
        match *self {
            RulerState::Measured { start, end } => {
                *self = RulerState::Measured { start: end, end: start };
                true
            }
            _ => false,
        }
    }

    /// Handles a planet click at `dir` with height `height_m`.
    /// First click (or click after a finished measurement) anchors A; second non-coincident click pins B.
    pub fn click(&mut self, dir: Vec3, height_m: f64) -> bool {
        if !is_valid_dir(dir) {
            return false;
        }
        let unit = dir.normalized();
        let pt = GeoPoint::of_dir(unit).canonical();
        let h = if height_m.is_finite() { height_m } else { 0.0 };
        match *self {
            RulerState::Idle | RulerState::Measured { .. } => {
                *self = RulerState::Anchored { start: pt, start_height_m: h };
                true
            }
            RulerState::Anchored { start, .. } => {
                if start.dir().distance(unit) < COINCIDENT_EPS {
                    return false;
                }
                *self = RulerState::Measured { start, end: pt };
                true
            }
        }
    }
}

/// One sampled vertex along a great-circle transect from A to B.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransectSample {
    pub fraction: f64,
    pub point: GeoPoint,
    pub distance_m: f64,
    pub height_m: f64,
    /// Signed slope along A -> B in degrees (positive = rising).
    pub along_slope_deg: f64,
    /// Magnitude of the steepest adjacent interval slope at this sample, degrees.
    pub slope_deg: f64,
    pub over_limit: bool,
}

/// Elevation and slope cross-section along the great-circle arc between two surface points (`VIEW-076`).
#[derive(Clone, Debug, PartialEq)]
pub struct TransectProfile {
    pub start: GeoPoint,
    pub end: GeoPoint,
    pub arc_distance_m: f64,
    pub chord_distance_m: f64,
    pub start_height_m: f64,
    pub end_height_m: f64,
    pub delta_height_m: f64,
    pub net_grade: f64,
    pub net_slope_deg: f64,
    pub min_height_m: f64,
    pub max_height_m: f64,
    pub max_slope_deg: f64,
    pub steep_fraction: f64,
    pub slope_limit_deg: f64,
    pub samples: Vec<TransectSample>,
}

impl TransectProfile {
    /// Samples `sample_count` points evenly along the great-circle arc from `start` to `end`.
    pub fn sample(
        start: GeoPoint,
        end: GeoPoint,
        radius_m: f64,
        sample_count: usize,
        slope_limit_deg: f64,
        ground_m: &dyn Fn(Vec3) -> f64,
    ) -> TransectProfile {
        let a = start.dir();
        let b = end.dir();
        let n = sample_count.max(2);
        let arc_distance_m = polyline::arc_m(a, b, radius_m);
        let mut pts = Vec::with_capacity(n);
        let mut heights = Vec::with_capacity(n);
        for k in 0..n {
            let t = k as f64 / (n - 1) as f64;
            let d = polyline::slerp(a, b, t);
            let h = ground_m(d);
            pts.push(GeoPoint::of_dir(d).canonical());
            heights.push(if h.is_finite() { h } else { 0.0 });
        }
        Self::from_heights(start.canonical(), end.canonical(), radius_m, arc_distance_m, slope_limit_deg, pts, heights)
    }

    /// Lightweight 2-point rubber-band profile between anchored A and hover point B without reading `ground_m`.
    pub fn rubber_band(
        start: GeoPoint,
        start_height_m: f64,
        end: GeoPoint,
        end_height_m: f64,
        radius_m: f64,
        slope_limit_deg: f64,
    ) -> TransectProfile {
        let arc_distance_m = polyline::arc_m(start.dir(), end.dir(), radius_m);
        let s_h = if start_height_m.is_finite() { start_height_m } else { 0.0 };
        let e_h = if end_height_m.is_finite() { end_height_m } else { 0.0 };
        Self::from_heights(
            start.canonical(),
            end.canonical(),
            radius_m,
            arc_distance_m,
            slope_limit_deg,
            vec![start.canonical(), end.canonical()],
            vec![s_h, e_h],
        )
    }

    fn from_heights(
        start: GeoPoint,
        end: GeoPoint,
        radius_m: f64,
        arc_distance_m: f64,
        slope_limit_deg: f64,
        pts: Vec<GeoPoint>,
        heights: Vec<f64>,
    ) -> TransectProfile {
        let n = heights.len();
        let a = start.dir();
        let b = end.dir();
        let start_height_m = heights[0];
        let end_height_m = heights[n - 1];
        let chord_distance_m = (b * (radius_m + end_height_m) - a * (radius_m + start_height_m)).length();
        let delta_height_m = end_height_m - start_height_m;
        let net_grade = if arc_distance_m > 1e-9 { delta_height_m / arc_distance_m } else { 0.0 };
        let net_slope_deg = if arc_distance_m > 1e-9 { libm::atan2(delta_height_m, arc_distance_m) * RAD_TO_DEG } else { 0.0 };

        let ds = arc_distance_m / (n - 1) as f64;
        let mut interval_deg = Vec::with_capacity(n - 1);
        let mut max_slope_deg = 0.0_f64;
        let mut steep_intervals = 0usize;
        for w in heights.windows(2) {
            let deg = if ds > 1e-9 { libm::atan((w[1] - w[0]) / ds) * RAD_TO_DEG } else { 0.0 };
            let mag = libm::fabs(deg);
            max_slope_deg = max_slope_deg.max(mag);
            if mag > slope_limit_deg {
                steep_intervals += 1;
            }
            interval_deg.push(deg);
        }
        let steep_fraction = steep_intervals as f64 / (n - 1) as f64;
        let min_height_m = heights.iter().copied().fold(f64::INFINITY, f64::min);
        let max_height_m = heights.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        let mut samples = Vec::with_capacity(n);
        for k in 0..n {
            let fraction = k as f64 / (n - 1) as f64;
            let along_slope_deg = if k == 0 {
                interval_deg[0]
            } else if k + 1 == n {
                interval_deg[n - 2]
            } else if ds > 1e-9 {
                libm::atan((heights[k + 1] - heights[k - 1]) / (2.0 * ds)) * RAD_TO_DEG
            } else {
                0.0
            };
            let slope_deg = if k == 0 {
                libm::fabs(interval_deg[0])
            } else if k + 1 == n {
                libm::fabs(interval_deg[n - 2])
            } else {
                libm::fabs(interval_deg[k - 1]).max(libm::fabs(interval_deg[k]))
            };
            samples.push(TransectSample {
                fraction,
                point: pts[k],
                distance_m: fraction * arc_distance_m,
                height_m: heights[k],
                along_slope_deg,
                slope_deg,
                over_limit: slope_deg > slope_limit_deg,
            });
        }

        TransectProfile {
            start,
            end,
            arc_distance_m,
            chord_distance_m,
            start_height_m,
            end_height_m,
            delta_height_m,
            net_grade,
            net_slope_deg,
            min_height_m,
            max_height_m,
            max_slope_deg,
            steep_fraction,
            slope_limit_deg,
            samples,
        }
    }

    /// Interpolates the transect at normalized fraction `0.0..=1.0` (used for chart-to-3D hover scrubbing).
    pub fn sample_at_fraction(&self, fraction: f64) -> Option<TransectSample> {
        if !fraction.is_finite() || self.samples.is_empty() {
            return None;
        }
        let n = self.samples.len();
        if n == 1 {
            return Some(self.samples[0]);
        }
        let f = fraction.clamp(0.0, 1.0);
        let scaled = f * (n - 1) as f64;
        let idx = (scaled as usize).min(n - 2);
        let t = (scaled - idx as f64).clamp(0.0, 1.0);
        let s0 = &self.samples[idx];
        let s1 = &self.samples[idx + 1];
        let dir = polyline::slerp(s0.point.dir(), s1.point.dir(), t);
        let lerp = |u: f64, v: f64| u + (v - u) * t;
        let slope_deg = lerp(s0.slope_deg, s1.slope_deg);
        Some(TransectSample {
            fraction: f,
            point: GeoPoint::of_dir(dir).canonical(),
            distance_m: lerp(s0.distance_m, s1.distance_m),
            height_m: lerp(s0.height_m, s1.height_m),
            along_slope_deg: lerp(s0.along_slope_deg, s1.along_slope_deg),
            slope_deg,
            over_limit: slope_deg > self.slope_limit_deg,
        })
    }

    /// Signed slope of interval `i` (between sample `i` and `i + 1`) in degrees.
    pub fn interval_slope_deg(&self, interval: usize) -> f64 {
        if interval + 1 >= self.samples.len() {
            return 0.0;
        }
        let ds = self.arc_distance_m / (self.samples.len() - 1) as f64;
        if ds <= 1e-9 {
            return 0.0;
        }
        let dh = self.samples[interval + 1].height_m - self.samples[interval].height_m;
        libm::atan(dh / ds) * RAD_TO_DEG
    }

    /// Compact summary badge displayed beside the 3D overlay midpoint and status bar.
    pub fn callout_label(&self) -> String {
        format!(
            "{} · Δh {} · max {:.1}°",
            format_metres(self.arc_distance_m),
            format_signed_height(self.delta_height_m),
            self.max_slope_deg
        )
    }
}

/// One projected segment of the ruler path in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulerSegment {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub over_limit: bool,
}

/// Projected 3D ruler overlay for the egui viewport pass (`VIEW-076`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RulerOverlay {
    pub start: Option<[f64; 2]>,
    pub end: Option<[f64; 2]>,
    pub segments: Vec<RulerSegment>,
    pub midpoint: Option<[f64; 2]>,
    pub label: Option<String>,
    pub scrub: Option<[f64; 2]>,
    pub is_rubber_band: bool,
}

/// Projects a [`TransectProfile`] onto the camera viewport (`faces_camera`-culled, zero GPU work).
pub fn build_overlay(profile: &TransectProfile, view: &ViewState, is_rubber_band: bool, scrub_fraction: Option<f64>) -> RulerOverlay {
    let see = |dir: Vec3, height_m: f64| -> Option<[f64; 2]> {
        let u = dir.normalized();
        faces_camera(view.camera.position.0, u, view.radius_m)
            .then(|| project(&view.camera, u * (view.radius_m + height_m), view.viewport))
            .flatten()
    };

    let start = see(profile.start.dir(), profile.start_height_m);
    let end = see(profile.end.dir(), profile.end_height_m);

    let mut segments = Vec::new();
    if is_rubber_band && profile.samples.len() == 2 {
        let a = profile.start.dir();
        let b = profile.end.dir();
        let over_limit = libm::fabs(profile.interval_slope_deg(0)) > profile.slope_limit_deg;
        let pts: Vec<Option<[f64; 2]>> = (0..=RUBBER_ARC_SEGMENTS)
            .map(|k| {
                let t = k as f64 / RUBBER_ARC_SEGMENTS as f64;
                let h = profile.start_height_m + (profile.end_height_m - profile.start_height_m) * t;
                see(polyline::slerp(a, b, t), h)
            })
            .collect();
        for w in pts.windows(2) {
            if let (Some(pa), Some(pb)) = (w[0], w[1]) {
                segments.push(RulerSegment { a: pa, b: pb, over_limit });
            }
        }
    } else if profile.samples.len() >= 2 {
        let pts: Vec<Option<[f64; 2]>> = profile.samples.iter().map(|s| see(s.point.dir(), s.height_m)).collect();
        for (i, w) in pts.windows(2).enumerate() {
            if let (Some(pa), Some(pb)) = (w[0], w[1]) {
                let over_limit = libm::fabs(profile.interval_slope_deg(i)) > profile.slope_limit_deg;
                segments.push(RulerSegment { a: pa, b: pb, over_limit });
            }
        }
    }

    let midpoint = profile.sample_at_fraction(0.5).and_then(|s| see(s.point.dir(), s.height_m));
    let scrub = scrub_fraction.and_then(|f| profile.sample_at_fraction(f)).and_then(|s| see(s.point.dir(), s.height_m));
    RulerOverlay { start, end, segments, midpoint, label: Some(profile.callout_label()), scrub, is_rubber_band }
}
