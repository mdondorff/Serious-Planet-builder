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

/// How close to a pole the overview's centre may come (radians, 1 degree): north stays up and the east axis stays well defined, so
/// a drag over the pole slides along the edge of the cap and a flight over it bends round it.
pub const POLE_MARGIN_RAD: f64 = 0.017_453_292_519_943_295;

/// The overview's camera: over `center` at `altitude_m` above the sphere of radius `radius_m`, looking at the planet's centre with
/// north up. Build one with [`GlobeView::new`], which keeps the centre a unit vector outside the polar caps and the altitude a finite
/// number of at least `MIN_ALTITUDE_M`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeView {
    /// The unit direction of the point under the camera.
    pub center: Vec3,
    /// Camera height above the sphere, metres.
    pub altitude_m: f64,
}

/// A unit vector, or +x for a zero or non-finite one.
fn unit_or_x(v: Vec3) -> Vec3 {
    let len = v.length();
    if len.is_finite() && len > 0.0 {
        v * (1.0 / len)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    }
}

/// The point on the edge of a polar cap at the longitude of `(x, y)` (longitude 0 for the zero vector), north or south.
fn cap_edge(x: f64, y: f64, north: bool) -> Vec3 {
    let rho = libm::hypot(x, y);
    let (cx, cy) = if rho > 0.0 { (x / rho, y / rho) } else { (1.0, 0.0) };
    let ring = libm::sin(POLE_MARGIN_RAD);
    let z = libm::cos(POLE_MARGIN_RAD);
    Vec3::new(cx * ring, cy * ring, if north { z } else { -z })
}

/// `center` moved out of the polar caps (to the edge of the cap, keeping its longitude; longitude 0 at an exact pole).
fn outside_caps(center: Vec3) -> Vec3 {
    let c = unit_or_x(center);
    let colatitude = libm::atan2(libm::hypot(c.x, c.y), c.z);
    if (POLE_MARGIN_RAD..=core::f64::consts::PI - POLE_MARGIN_RAD).contains(&colatitude) {
        c
    } else {
        cap_edge(c.x, c.y, c.z >= 0.0)
    }
}

/// The cap (north is true) the great-circle path from `from` toward the tangent `toward` over `theta` radians enters, if any. The
/// height along the path is `A cos(phi - phi0)`, whose extremes are at the ends of the path and at `phi0` and `phi0 + pi`.
fn cap_entered(from: Vec3, toward: Vec3, theta: f64) -> Option<bool> {
    let height = |phi: f64| from.z * libm::cos(phi) + toward.z * libm::sin(phi);
    let phi0 = libm::atan2(toward.z, from.z);
    let pi = core::f64::consts::PI;
    let mut candidates = vec![0.0, theta];
    for extreme in [phi0, phi0 + pi, phi0 - pi, phi0 + 2.0 * pi, phi0 - 2.0 * pi] {
        if extreme > 0.0 && extreme < theta {
            candidates.push(extreme);
        }
    }
    let edge = libm::cos(POLE_MARGIN_RAD);
    let (mut highest, mut lowest) = (f64::MIN, f64::MAX);
    for phi in candidates {
        let z = height(phi);
        highest = highest.max(z);
        lowest = lowest.min(z);
    }
    if highest > edge && from.z >= 0.0 {
        Some(true)
    } else if lowest < -edge && from.z < 0.0 {
        Some(false)
    } else {
        None
    }
}

/// The unit tangent vectors (east, north) of the sphere at `center` (outside the caps).
fn tangents(center: Vec3) -> (Vec3, Vec3) {
    let east = Vec3::new(0.0, 0.0, 1.0).cross(center).normalized();
    let north = center.cross(east).normalized();
    (east, north)
}

impl GlobeView {
    /// A view over `center` (made a unit vector and kept out of the polar caps; a zero or non-finite vector gives +x) at `altitude_m`
    /// (at least `MIN_ALTITUDE_M`; a non-finite value gives the minimum).
    pub fn new(center: Vec3, altitude_m: f64) -> GlobeView {
        let altitude_m = if altitude_m.is_finite() { altitude_m.max(MIN_ALTITUDE_M) } else { MIN_ALTITUDE_M };
        GlobeView { center: outside_caps(center), altitude_m }
    }

    /// The camera of this view.
    pub fn camera(&self, radius_m: f64, fov_y_rad: f64, aspect: f64, near_m: f64) -> Camera {
        let position = PlanetFixed(self.center * (radius_m + self.altitude_m));
        Camera::looking_at_centre(position, Vec3::new(0.0, 0.0, 1.0), fov_y_rad, aspect, near_m)
    }

    /// The ground angle (radians) one pixel of a `height_px` viewport spans at the centre of the view.
    fn radians_per_pixel(&self, radius_m: f64, fov_y_rad: f64, height_px: f64) -> f64 {
        let metres_per_pixel = 2.0 * libm::tan(fov_y_rad / 2.0) * self.altitude_m.max(1.0) / height_px;
        metres_per_pixel / radius_m
    }

    /// Drag the globe by `(dx_px, dy_px)` pixels: the surface follows the cursor (dragging right brings the western side into view).
    /// The centre turns by exactly the angle the pixels span at the centre, along the great circle in the drag's direction, and stays
    /// outside the polar caps. A viewport without height, or a value that is not finite, changes nothing.
    pub fn drag(&mut self, dx_px: f64, dy_px: f64, radius_m: f64, fov_y_rad: f64, height_px: f64) {
        let k = self.radians_per_pixel(radius_m, fov_y_rad, height_px);
        let pixels = libm::hypot(dx_px, dy_px);
        if !(height_px > 0.0 && k.is_finite() && k > 0.0 && pixels.is_finite()) || pixels == 0.0 {
            return;
        }
        let (east, north) = tangents(self.center);
        let toward = (north * dy_px - east * dx_px) * (1.0 / pixels);
        let theta = k * pixels;
        let moved = self.center * libm::cos(theta) + toward * libm::sin(theta);
        // A drag whose path enters a polar cap stops at the cap's edge on the side it came from (a drag never flips the view).
        self.center = match cap_entered(self.center, toward, theta) {
            Some(north) => cap_edge(self.center.x, self.center.y, north),
            None => outside_caps(moved),
        };
    }

    /// Zoom by `factor` (greater than 1 goes farther): the altitude stays in `[MIN_ALTITUDE_M, MAX_ALTITUDE_RADII * radius_m]`. A
    /// factor that is not positive and finite changes nothing.
    pub fn zoom(&mut self, factor: f64, radius_m: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let ceiling = (MAX_ALTITUDE_RADII * radius_m).max(MIN_ALTITUDE_M);
        self.altitude_m = (self.altitude_m * factor).clamp(MIN_ALTITUDE_M, ceiling);
    }

    /// The view that shows a marker closely: over its centre, four footprint radii up (never below `MIN_ALTITUDE_M`).
    pub fn closing_on(marker: &Marker) -> GlobeView {
        GlobeView::new(marker.dir, 4.0 * marker.radius_m)
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
    /// The flight between two views (made valid by [`GlobeView::new`]) on a planet of radius `radius_m`.
    pub fn new(from: GlobeView, to: GlobeView, radius_m: f64) -> GlobeFlight {
        let (from, to) = (GlobeView::new(from.center, from.altitude_m), GlobeView::new(to.center, to.altitude_m));
        let angle_rad = libm::atan2(from.center.cross(to.center).length(), from.center.dot(to.center));
        let radius_m = if radius_m.is_finite() && radius_m > 0.0 { radius_m } else { 0.0 };
        GlobeFlight { from, to, angle_rad, duration_s: MIN_FLIGHT_S + SECONDS_PER_RADIAN * angle_rad, radius_m }
    }

    /// The view `t_s` seconds into the flight (clamped to the flight; a time that is not a number is the start). The start and the end
    /// are the two views exactly; in between the centre stays outside the polar caps, so a flight over a pole bends round it.
    pub fn at(&self, t_s: f64) -> GlobeView {
        let t = if t_s.is_nan() { 0.0 } else { t_s };
        let s = smootherstep((t / self.duration_s).clamp(0.0, 1.0));
        if s <= 0.0 {
            return self.from;
        }
        if s >= 1.0 {
            return self.to;
        }
        // The centre: spherical interpolation, by the angle travelled (opposite centres take the great circle through the east axis
        // of the start).
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
        let log_alt = (1.0 - s) * libm::log(self.from.altitude_m) + s * libm::log(self.to.altitude_m);
        let rise = 1.5 * self.radius_m * (self.angle_rad / core::f64::consts::PI) * libm::sin(core::f64::consts::PI * s);
        GlobeView { center: outside_caps(center), altitude_m: libm::exp(log_alt) + rise }
    }

    pub fn is_done(&self, t_s: f64) -> bool {
        t_s >= self.duration_s
    }
}

// -------------------------------------------------------------------------------------------------------------------------
// The overview as the editor window runs it

/// The altitude the overview starts at over the point the camera was over, in planet radii.
pub const OVERVIEW_ALTITUDE_RADII: f64 = 1.5;
/// The altitude of the flight that a click on bare surface ends at, metres: a close look from where the free camera takes over.
pub const LANDING_ALTITUDE_M: f64 = 20_000.0;
/// How far from a marker's pixel a click still names it.
pub const HIT_RADIUS_PX: f64 = 12.0;
/// A press and release that stay within this many pixels are a click, not a drag.
pub const CLICK_SLOP_PX: f64 = 4.0;
/// The pitch below the horizon the free camera starts at after a flight over a place, radians.
pub const LANDING_PITCH_RAD: f64 = 0.5;

/// What a click on the overview found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Click {
    /// A marker: the flight goes to its closing view.
    Marker(u64),
    /// Bare surface at this direction: the flight goes to a close look over it.
    Surface(Vec3),
}

/// The overview while the editor shows it: the view, the flight a click started, and what to do when the flight ends. GPU-free; the window
/// feeds it the mouse and the frame time and takes the camera from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeSession {
    pub view: GlobeView,
    pub radius_m: f64,
    flight: Option<(GlobeFlight, f64)>,
}

impl GlobeSession {
    /// The overview over the point under a camera at `camera_position` (any distance from the centre), at 1.5 radii up.
    pub fn enter(camera_position: Vec3, radius_m: f64) -> GlobeSession {
        let altitude = OVERVIEW_ALTITUDE_RADII * radius_m;
        GlobeSession { view: GlobeView::new(camera_position, altitude), radius_m, flight: None }
    }

    /// True while a click's flight runs: the overview takes no drag, zoom or click then.
    pub fn flying(&self) -> bool {
        self.flight.is_some()
    }

    /// Drag by pixels (see [`GlobeView::drag`]); nothing while flying.
    pub fn drag(&mut self, dx_px: f64, dy_px: f64, fov_y_rad: f64, height_px: f64) {
        if self.flight.is_none() {
            self.view.drag(dx_px, dy_px, self.radius_m, fov_y_rad, height_px);
        }
    }

    /// Zoom by `factor` (see [`GlobeView::zoom`]); nothing while flying.
    pub fn zoom(&mut self, factor: f64) {
        if self.flight.is_none() {
            self.view.zoom(factor, self.radius_m);
        }
    }

    /// A click at `cursor` (pixels, origin top-left) in a view seen by `camera`: the marker within [`HIT_RADIUS_PX`], else the surface
    /// under the cursor (`pick_surface` against the height field `height_m`), and the flight to it starts. `None` while flying or when the
    /// click hits neither (the sky, past the horizon).
    pub fn click(
        &mut self,
        cursor: [f64; 2],
        camera: &Camera,
        viewport: Viewport,
        markers: &[Marker],
        limits: &crate::pick::PickLimits,
        height_m: &dyn Fn(Vec3) -> f64,
    ) -> Option<Click> {
        if self.flight.is_some() {
            return None;
        }
        let visible = visible_markers(camera, markers, self.radius_m, viewport);
        let (target, click) = if let Some(id) = marker_at(&visible, cursor, HIT_RADIUS_PX) {
            let marker = markers.iter().find(|m| m.id == id)?;
            (GlobeView::closing_on(marker), Click::Marker(id))
        } else {
            let ray = crate::pick::cursor_ray(camera, cursor[0], cursor[1], viewport.width_px, viewport.height_px);
            let hit = crate::pick::pick_surface(&ray, limits, height_m)?;
            (GlobeView::new(hit.dir, LANDING_ALTITUDE_M), Click::Surface(hit.dir))
        };
        self.flight = Some((GlobeFlight::new(self.view, target, self.radius_m), 0.0));
        Some(click)
    }

    /// Advance a running flight by `dt_s` seconds. The frame the flight ends returns the view it ended at: the window leaves the overview
    /// there (see [`landing`]).
    pub fn step(&mut self, dt_s: f64) -> Option<GlobeView> {
        let (flight, t) = self.flight.as_mut()?;
        *t += dt_s.max(0.0);
        self.view = flight.at(*t);
        if flight.is_done(*t) {
            let end = flight.to;
            self.flight = None;
            return Some(end);
        }
        None
    }
}

/// Where the free camera starts after a flight that ended at `view`: over the place at the view's altitude, looking north and down by
/// [`LANDING_PITCH_RAD`]. `(position, heading hint, pitch)`, the arguments of `FreePose::new`.
pub fn landing(view: &GlobeView, radius_m: f64) -> (PlanetFixed, Vec3, f64) {
    (PlanetFixed(view.center * (radius_m + view.altitude_m)), Vec3::new(0.0, 0.0, 1.0), -LANDING_PITCH_RAD)
}

/// The zoom factor of one wheel line over the overview: each line brings the camera 10 % nearer (a negative count goes farther).
pub const WHEEL_FACTOR_PER_LINE: f64 = 0.9;

/// The factor `GlobeSession::zoom` takes for `lines` wheel lines (positive: towards the planet).
pub fn wheel_zoom_factor(lines: f64) -> f64 {
    libm::pow(WHEEL_FACTOR_PER_LINE, lines)
}

/// A press of the left button over the overview that becomes a click when it is released within [`CLICK_SLOP_PX`] of where it went down
/// and a drag otherwise. The window cancels it whenever the button's state is in doubt (the overview toggled, the focus lost, a release
/// the overview did not see), so a stale press never drags the globe.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClickTracker {
    moved_px: Option<f64>,
}

impl ClickTracker {
    pub fn press(&mut self) {
        self.moved_px = Some(0.0);
    }

    /// True while a button is held; the cursor moved by `distance_px` meanwhile (a drag when it returns true).
    pub fn moved_by(&mut self, distance_px: f64) -> bool {
        match self.moved_px.as_mut() {
            Some(m) => {
                *m += distance_px;
                true
            }
            None => false,
        }
    }

    /// The release: true when it ends a click (a press that stayed within the slop). The press is over either way.
    pub fn release(&mut self) -> bool {
        self.moved_px.take().is_some_and(|m| m < CLICK_SLOP_PX)
    }

    /// Forget the press.
    pub fn cancel(&mut self) {
        self.moved_px = None;
    }

    pub fn pressed(&self) -> bool {
        self.moved_px.is_some()
    }
}
