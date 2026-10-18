//! The views of the M5 lit perf scenes and the coastal flights (TEST-025): where the camera stands and how it moves, decided here, GPU-free
//! (CON-03). The static views are the viewer's: orbit, aerial 20 km and ground 1 m of the terrain scene's site, and two coast views
//! (2 km and 3 m above the water) at a coast found in the world; the two coastal flights are the ones of the water pass's flicker test
//! (a low glide towards the shore, a descent from 300 m to 4 m) with as many frames as the caller asks for.

use crate::camera::Camera;
use crate::terrain::scripted_views_field;
use planet_core::{PlanetFixed, Vec3};
use planet_generators::height::HeightField;

/// The vertical field of view of the coast views and flights, degrees (the terrain scene's).
pub const FOV_DEG: f64 = 60.0;
/// Static views, in the order the scene measures them.
pub const STATIC_VIEWS: [&str; 5] = ["ground-1m", "aerial-20km", "coast-2km", "coast-3m", "orbit"];
/// The coastal flights.
pub const FLIGHTS: [&str; 2] = ["coast-glide", "coast-descent"];

/// A coast: a point on the shoreline and the horizontal direction from the sea to the land there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coast {
    pub dir: Vec3,
    pub heading: Vec3,
}

fn on_parallel(lat_rad: f64, lon_rad: f64) -> Vec3 {
    Vec3::new(libm::cos(lat_rad) * libm::cos(lon_rad), libm::cos(lat_rad) * libm::sin(lon_rad), libm::sin(lat_rad))
}

/// The first shoreline east of `lon0_deg` on the parallels `lat_deg`, `lat_deg` plus and minus 10, 20, 30 degrees (in that order)
/// where the terrain rises through the sea level `level_m` going east with the sea to the west, found by the terrain's height: the
/// walk is 20 km a step along the parallel and the crossing of `level_m` is bisected to a millimetre. The shore counts when the sea
/// reaches at least 2 km out (`carries_sea` at 2 km west, and the terrain at least 5 m below the level there) and the land 500 m in
/// (at least 3 m above the level there), so the camera looks at a real beach and never at a coastal plain below the level.
/// `None` when no parallel has one.
pub fn find_coast(
    carries_sea: &dyn Fn(Vec3) -> bool,
    height: &dyn Fn(Vec3) -> f64,
    level_m: f64,
    lat_deg: f64,
    lon0_deg: f64,
    radius_m: f64,
) -> Option<Coast> {
    const STEP_M: f64 = 20_000.0;
    let wet = |d: Vec3| height(d) < level_m;
    for off in [0.0, 10.0, -10.0, 20.0, -20.0, 30.0, -30.0] {
        let lat = (lat_deg + off).clamp(-80.0, 80.0).to_radians();
        let ring = std::f64::consts::TAU * radius_m * libm::cos(lat);
        let at = |s: f64| on_parallel(lat, lon0_deg.to_radians() + s / (radius_m * libm::cos(lat)));
        let steps = (ring / STEP_M).ceil() as usize;
        let mut prev = wet(at(0.0));
        for k in 1..=steps {
            let s = k as f64 * STEP_M;
            let cur = wet(at(s));
            if prev && !cur {
                let (mut a, mut b) = (s - STEP_M, s);
                for _ in 0..44 {
                    let m = 0.5 * (a + b);
                    if wet(at(m)) {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                let out = at(b - 2_000.0);
                if carries_sea(out) && height(out) <= level_m - 5.0 && height(at(b + 500.0)) >= level_m + 3.0 {
                    let heading = (at(b + 10.0) - at(b - 10.0)).normalized();
                    return Some(Coast { dir: at(b), heading });
                }
            }
            prev = cur;
        }
    }
    None
}

/// A camera `alt_m` above the water level, `back_m` out to sea from the coast plus `along_m` towards the land, looking along the
/// heading pitched down by `pitch_deg` (the water pass's test geometry).
#[allow(clippy::too_many_arguments)]
pub fn coast_camera(c: &Coast, radius_m: f64, level_m: f64, alt_m: f64, back_m: f64, along_m: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let pos_dir = (c.dir + c.heading * ((along_m - back_m) / radius_m)).normalized();
    Camera::at_surface_point(
        PlanetFixed(pos_dir * (radius_m + level_m + alt_m)),
        c.heading,
        pitch_deg.to_radians(),
        FOV_DEG.to_radians(),
        aspect,
        0.1,
    )
}

/// Flight A, the glide: 6 m over the water, from 400 m out, 228 m towards the shore in `frames` equal steps (the test's 20 frames at
/// 12 m are the same path at a coarser step).
pub fn glide_cameras(c: &Coast, radius_m: f64, level_m: f64, aspect: f64, frames: usize) -> Vec<Camera> {
    let n = frames.max(2);
    (0..n).map(|k| coast_camera(c, radius_m, level_m, 6.0, 400.0, 228.0 * k as f64 / (n - 1) as f64, 6.0, aspect)).collect()
}

/// Flight B, the descent: from 300 m to 4 m over the water (log-spaced), 1,200 m out closing to 500 m, pitched 10 degrees, so every
/// distance band of the LOD passes under the camera.
pub fn descent_cameras(c: &Coast, radius_m: f64, level_m: f64, aspect: f64, frames: usize) -> Vec<Camera> {
    let n = frames.max(2);
    (0..n)
        .map(|k| {
            let t = k as f64 / (n - 1) as f64;
            coast_camera(c, radius_m, level_m, 300.0 * libm::pow(4.0 / 300.0, t), 1200.0 - 700.0 * t, 0.0, 10.0, aspect)
        })
        .collect()
}

/// The capture-only shore flights (`cargo xtask capture flights`, never the perf scenes): they stay on the beach, where the coast
/// flights above are mostly open water (the owner's look review, M5).
pub const SHORE_FLIGHTS: [&str; 2] = ["shore-run-30m", "shore-approach-3m"];
/// Frames per second of the captured videos (the flights' frame step).
pub const CAPTURE_FPS: usize = 60;
/// Duration of the shore run and of the shore approach, seconds.
pub const SHORE_RUN_S: usize = 20;
pub const SHORE_APPROACH_S: usize = 15;
/// The shore run: height above the water level, distance seaward of the waterline, length along the beach (about 25 m/s), the yaw
/// toward the land and the pitch down of the view (looking along the shore and slightly toward it).
pub const SHORE_RUN_ALT_M: f64 = 30.0;
pub const SHORE_RUN_OFFSHORE_M: f64 = 150.0;
pub const SHORE_RUN_LENGTH_M: f64 = 500.0;
pub const SHORE_RUN_YAW_DEG: f64 = 25.0;
pub const SHORE_RUN_PITCH_DEG: f64 = 8.0;
/// The shore approach: height above the water level over the sea, the distance out where it starts, how far past the waterline it
/// ends (over the sand; the camera then keeps [`SHORE_APPROACH_CLEARANCE_M`] above the ground), and the pitch down of the view.
pub const SHORE_APPROACH_ALT_M: f64 = 3.0;
pub const SHORE_APPROACH_START_M: f64 = 1_000.0;
pub const SHORE_APPROACH_END_M: f64 = 40.0;
pub const SHORE_APPROACH_CLEARANCE_M: f64 = 2.0;
pub const SHORE_APPROACH_PITCH_DEG: f64 = 3.0;

/// The local frame of a coast: the point on the shoreline, `along` the beach (horizontal, perpendicular to the heading) and the
/// heading from the sea to the land. `point(s, t)` is the unit direction `s` metres along the beach and `t` metres towards the land
/// (a tangent-plane offset, exact to well under a millimetre at the kilometres these flights span).
struct ShoreFrame {
    dir: Vec3,
    along: Vec3,
    heading: Vec3,
    radius_m: f64,
}

impl ShoreFrame {
    fn new(c: &Coast, radius_m: f64) -> Self {
        let up = c.dir.normalized();
        let heading = (c.heading - up * c.heading.dot(up)).normalized();
        Self { dir: up, along: up.cross(heading).normalized(), heading, radius_m }
    }
    fn point(&self, s: f64, t: f64) -> Vec3 {
        (self.dir + self.along * (s / self.radius_m) + self.heading * (t / self.radius_m)).normalized()
    }
    /// The tangent-plane coordinates `(s, t)` of a direction near the coast: the inverse of [`ShoreFrame::point`] (the tests').
    #[cfg(test)]
    fn coords(&self, d: Vec3) -> (f64, f64) {
        let o = d.normalized() - self.dir;
        (o.dot(self.along) * self.radius_m, o.dot(self.heading) * self.radius_m)
    }
}

/// Where the shoreline crosses the line `s` metres along the beach, as metres towards the land from the coast point: the first
/// rise through `level_m` going landward within 400 m, scanned every 10 m and bisected to a millimetre; 0 when the line has none.
fn shore_crossing(f: &ShoreFrame, height: &dyn Fn(Vec3) -> f64, level_m: f64, s: f64) -> f64 {
    let wet = |t: f64| height(f.point(s, t)) < level_m;
    let mut t = -400.0;
    while t < 400.0 {
        if wet(t) && !wet(t + 10.0) {
            let (mut a, mut b) = (t, t + 10.0);
            for _ in 0..24 {
                let m = 0.5 * (a + b);
                if wet(m) {
                    a = m;
                } else {
                    b = m;
                }
            }
            return b;
        }
        t += 10.0;
    }
    0.0
}

/// The shoreline's landward offset at `n` positions from `-length/2` to `+length/2` along the beach, found every 25 m and smoothed
/// (two passes of a three-point mean) so that the camera path follows the beach without following every cusp.
fn shoreline(f: &ShoreFrame, height: &dyn Fn(Vec3) -> f64, level_m: f64, length_m: f64) -> Vec<(f64, f64)> {
    let n = (length_m / 25.0).ceil() as usize + 1;
    let mut pts: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let s = -0.5 * length_m + length_m * k as f64 / (n - 1) as f64;
            (s, shore_crossing(f, height, level_m, s))
        })
        .collect();
    for _ in 0..2 {
        let prev: Vec<f64> = pts.iter().map(|p| p.1).collect();
        for k in 0..n {
            let (lo, hi) = (k.saturating_sub(1), (k + 1).min(n - 1));
            pts[k].1 = (prev[lo] + prev[k] + prev[hi]) / 3.0;
        }
    }
    pts
}

fn interpolate(pts: &[(f64, f64)], s: f64) -> f64 {
    let i = pts.partition_point(|p| p.0 <= s).clamp(1, pts.len() - 1);
    let ((s0, t0), (s1, t1)) = (pts[i - 1], pts[i]);
    t0 + (t1 - t0) * ((s - s0) / (s1 - s0)).clamp(0.0, 1.0)
}

/// The shore run: `frames` cameras 30 m above the water, 150 m seaward of the waterline (which they follow along the beach, found
/// from the terrain-height crossings), moving 500 m along the beach and looking along it and 25 degrees toward the land, pitched 8
/// degrees down.
pub fn shore_run_cameras(c: &Coast, radius_m: f64, level_m: f64, height: &dyn Fn(Vec3) -> f64, aspect: f64, frames: usize) -> Vec<Camera> {
    let f = ShoreFrame::new(c, radius_m);
    let line = shoreline(&f, height, level_m, SHORE_RUN_LENGTH_M);
    let n = frames.max(2);
    let yaw = SHORE_RUN_YAW_DEG.to_radians();
    let look = f.along * libm::cos(yaw) + f.heading * libm::sin(yaw);
    (0..n)
        .map(|k| {
            let s = -0.5 * SHORE_RUN_LENGTH_M + SHORE_RUN_LENGTH_M * k as f64 / (n - 1) as f64;
            let t = interpolate(&line, s) - SHORE_RUN_OFFSHORE_M;
            let position = PlanetFixed(f.point(s, t) * (radius_m + level_m + SHORE_RUN_ALT_M));
            Camera::at_surface_point(position, look, SHORE_RUN_PITCH_DEG.to_radians(), FOV_DEG.to_radians(), aspect, 0.1)
        })
        .collect()
}

/// The shore approach: `frames` cameras 3 m above the water, starting 1 km out and flying straight (perpendicular to the beach)
/// toward it at constant speed, ending 40 m past the waterline over the sand, where the camera keeps 2 m above the ground.
pub fn shore_approach_cameras(
    c: &Coast,
    radius_m: f64,
    level_m: f64,
    height: &dyn Fn(Vec3) -> f64,
    aspect: f64,
    frames: usize,
) -> Vec<Camera> {
    let f = ShoreFrame::new(c, radius_m);
    let shore = shore_crossing(&f, height, level_m, 0.0);
    let n = frames.max(2);
    (0..n)
        .map(|k| {
            let u = k as f64 / (n - 1) as f64;
            let t = shore - SHORE_APPROACH_START_M + (SHORE_APPROACH_START_M + SHORE_APPROACH_END_M) * u;
            let d = f.point(0.0, t);
            let alt = SHORE_APPROACH_ALT_M.max(height(d) - level_m + SHORE_APPROACH_CLEARANCE_M);
            let position = PlanetFixed(d * (radius_m + level_m + alt));
            Camera::at_surface_point(position, f.heading, SHORE_APPROACH_PITCH_DEG.to_radians(), FOV_DEG.to_radians(), aspect, 0.1)
        })
        .collect()
}

/// The capture-only stills very close to the ground (the owner's look review: do problems appear at 0.5 m or below?), one camera
/// each, with the eye height above the local terrain or the water surface, whichever is higher. They bypass the viewer's 1 m ground
/// clamp (VIEW-004): the camera is set directly, so the viewer itself does not go this low.
pub const SHORE_STILLS: [&str; 4] =
    ["still-sand-0p5m-along", "still-water-0p5m-20m-beach", "still-sand-0p2m-seaward", "still-water-0p1m-along"];

/// The four stills of [`SHORE_STILLS`], in that order: 0.5 m above the wet sand at the waterline looking along the shore; 0.5 m above
/// the water 20 m out looking at the beach; 0.2 m above the sand at the waterline looking seaward; 0.1 m above the water 30 m out
/// looking along the shore. Level view (no pitch) except the first, which looks 2 degrees down.
pub fn shore_still_cameras(c: &Coast, radius_m: f64, level_m: f64, height: &dyn Fn(Vec3) -> f64, aspect: f64) -> Vec<Camera> {
    let f = ShoreFrame::new(c, radius_m);
    let shore = shore_crossing(&f, height, level_m, 0.0);
    let cam = |t: f64, eye_m: f64, look: Vec3, pitch_deg: f64| {
        let d = f.point(0.0, t);
        let alt = height(d).max(level_m) - level_m + eye_m;
        Camera::at_surface_point(
            PlanetFixed(d * (radius_m + level_m + alt)),
            look,
            pitch_deg.to_radians(),
            FOV_DEG.to_radians(),
            aspect,
            0.1,
        )
    };
    vec![
        cam(shore, 0.5, f.along, 2.0),
        cam(shore - 20.0, 0.5, f.heading, 0.0),
        cam(shore, 0.2, f.heading * -1.0, 0.0),
        cam(shore - 30.0, 0.1, f.along, 0.0),
    ]
}

/// The coast of the lit scenes and flights (the one `lit_views` uses).
pub fn lit_coast(radius_m: f64, field: &dyn HeightField, level_m: f64, is_sea: &dyn Fn(Vec3) -> bool) -> Result<Coast, String> {
    find_coast(is_sea, &|d| field.height(d), level_m, 20.0, 30.0, radius_m)
        .ok_or("the world has no beach (sea 2 km out below the level by 5 m, land 500 m in above it by 3 m) within three parallels of 20 N: no coast view can be measured".into())
}

/// The capture-only shore flights as views (`SHORE_FLIGHTS`), `frames` as given or the duration at [`CAPTURE_FPS`] when `None`.
pub fn shore_flight_views(
    radius_m: f64,
    aspect: f64,
    field: &dyn HeightField,
    level_m: f64,
    is_sea: &dyn Fn(Vec3) -> bool,
    frames: Option<usize>,
) -> Result<Vec<LitView>, String> {
    let coast = lit_coast(radius_m, field, level_m, is_sea)?;
    let height = |d: Vec3| field.height(d);
    let stills = shore_still_cameras(&coast, radius_m, level_m, &height, aspect);
    let mut views: Vec<LitView> =
        SHORE_STILLS.iter().zip(stills).map(|(name, camera)| LitView { name, cameras: vec![camera], kind: ViewKind::Flight }).collect();
    views.extend([
        LitView {
            name: SHORE_FLIGHTS[0],
            cameras: shore_run_cameras(&coast, radius_m, level_m, &height, aspect, frames.unwrap_or(SHORE_RUN_S * CAPTURE_FPS)),
            kind: ViewKind::Flight,
        },
        LitView {
            name: SHORE_FLIGHTS[1],
            cameras: shore_approach_cameras(&coast, radius_m, level_m, &height, aspect, frames.unwrap_or(SHORE_APPROACH_S * CAPTURE_FPS)),
            kind: ViewKind::Flight,
        },
    ]);
    Ok(views)
}

/// What a view is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// A held camera: several runs, a noise check, gated budgets.
    Static,
    /// A camera path measured frame by frame: report only.
    Flight,
}

#[derive(Clone, Debug)]
pub struct LitView {
    pub name: &'static str,
    pub cameras: Vec<Camera>,
    pub kind: ViewKind,
}

/// The static views and the two flights of the lit scenes for a world with sea level `level_m`; `is_sea` is the world's water
/// field. Errors when the world has no coast (a world without sea cannot measure the water pass).
pub fn lit_views(
    radius_m: f64,
    aspect: f64,
    field: &dyn HeightField,
    level_m: f64,
    is_sea: &dyn Fn(Vec3) -> bool,
    flight_frames: usize,
) -> Result<Vec<LitView>, String> {
    let named = scripted_views_field(radius_m, aspect, field);
    let get = |n: &str| named.iter().find(|(k, _)| *k == n).map(|(_, c)| *c).ok_or_else(|| format!("no scripted view {n}"));
    let coast = lit_coast(radius_m, field, level_m, is_sea)?;
    let cam = |alt, back, pitch| coast_camera(&coast, radius_m, level_m, alt, back, 0.0, pitch, aspect);
    let one = |name, camera| LitView { name, cameras: vec![camera], kind: ViewKind::Static };
    Ok(vec![
        one("ground-1m", get("ground-1m")?),
        one("aerial-20km", get("aerial-20km")?),
        one("coast-2km", cam(2_000.0, 3_000.0, 25.0)),
        one("coast-3m", cam(3.0, 150.0, 8.0)),
        one("orbit", get("orbit-20000km")?),
        LitView { name: "coast-glide", cameras: glide_cameras(&coast, radius_m, level_m, aspect, flight_frames), kind: ViewKind::Flight },
        LitView {
            name: "coast-descent",
            cameras: descent_cameras(&coast, radius_m, level_m, aspect, flight_frames),
            kind: ViewKind::Flight,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_generators::height::Reference;

    const R: f64 = 6_371_000.0;

    /// A world whose sea is the half of the sphere with y < 0 and a third of the ring near lon 180..: east of 30 E on any parallel the
    /// first sea-to-land crossing is at lon 360 (= 0), reached after 330 degrees.
    fn sea(d: Vec3) -> bool {
        d.y < 0.0
    }

    /// The terrain of that world: 30 m under the level in the sea, 20 m above it on the land.
    fn beach(d: Vec3) -> f64 {
        if sea(d) {
            -30.0
        } else {
            20.0
        }
    }

    // spec: TEST-025
    #[test]
    fn the_coast_is_the_first_sea_to_land_crossing_east_of_the_start_with_the_heading_towards_the_land() {
        let c = find_coast(&sea, &beach, 0.0, 20.0, 30.0, R).expect("a coast");
        assert!(c.dir.y.abs() < 1e-9 && c.dir.x > 0.0, "{c:?}");
        assert!((c.dir.length() - 1.0).abs() < 1e-12);
        assert!(c.heading.y > 0.99 && (c.heading.length() - 1.0).abs() < 1e-9, "east: from the sea (y < 0) to the land: {c:?}");
        // The crossing is bisected to far below a metre.
        let b = c.dir;
        assert!(sea((b - c.heading * (1.0 / R)).normalized()) && !sea((b + c.heading * (1.0 / R)).normalized()));
    }

    // spec: TEST-025
    #[test]
    fn a_world_without_a_shoreline_or_with_a_pond_has_no_coast() {
        assert!(find_coast(&|_| false, &|_| 20.0, 0.0, 20.0, 30.0, R).is_none(), "all land");
        assert!(find_coast(&|_| true, &|_| -20.0, 0.0, 20.0, 30.0, R).is_none(), "all sea");
        // A pond 600 m wide: the sea does not reach 2 km out.
        let pond = |d: Vec3| {
            let lon = libm::atan2(d.y, d.x).to_degrees();
            (59.99..60.0).contains(&lon) && d.z.abs() < 0.5
        };
        assert!(find_coast(&pond, &|d| if pond(d) { -20.0 } else { 20.0 }, 0.0, 20.0, 30.0, R).is_none());
    }

    // spec: TEST-025, ATMO-019
    #[test]
    fn a_shore_is_where_the_terrain_crosses_the_level_with_a_beach_and_a_plain_below_the_level_is_not_one() {
        // A plain 25 km wide east of the sea boundary, 17 m under the level, then land 20 m above it.
        let plain = |d: Vec3| {
            let lon = libm::atan2(d.y, d.x).to_degrees();
            !sea(d) && (0.0..0.23).contains(&lon.rem_euclid(360.0))
        };
        let terrain = |d: Vec3| {
            if sea(d) {
                -30.0
            } else if plain(d) {
                -17.0
            } else {
                20.0
            }
        };
        // The sea of the world is the sea only: its shore by the height lies at the east end of the plain, where the sea 2 km out is
        // not sea, so there is no coast; where the plain counts as sea (the band of ADR 0056) the beach is at the plain's east end.
        assert!(find_coast(&sea, &terrain, 0.0, 20.0, 30.0, R).is_none(), "the plain's shore has no sea 2 km out");
        let carries = |d: Vec3| sea(d) || plain(d);
        let c = find_coast(&carries, &terrain, 0.0, 20.0, 30.0, R).expect("the beach at the plain's end");
        let lon = libm::atan2(c.dir.y, c.dir.x).to_degrees().rem_euclid(360.0);
        assert!((0.2..0.26).contains(&lon), "the beach is at the east end of the plain, not at the sea boundary: lon {lon}");
        assert!(
            terrain((c.dir - c.heading * (2_000.0 / R)).normalized()) <= -5.0
                && terrain((c.dir + c.heading * (500.0 / R)).normalized()) >= 3.0
        );
    }

    // spec: TEST-025
    #[test]
    fn the_flights_start_and_end_where_the_water_tests_do_and_scale_with_the_frame_count() {
        let c = find_coast(&sea, &beach, 0.0, 20.0, 30.0, R).unwrap();
        let alt = |cam: &Camera| cam.position.0.length() - R;
        for n in [2usize, 24, 240] {
            let g = glide_cameras(&c, R, 0.0, 1.6, n);
            let d = descent_cameras(&c, R, 0.0, 1.6, n);
            assert_eq!((g.len(), d.len()), (n, n));
            assert!((alt(&g[0]) - 6.0).abs() < 1e-6 && (alt(&g[n - 1]) - 6.0).abs() < 1e-6, "the glide holds 6 m over the water");
            assert!((alt(&d[0]) - 300.0).abs() < 1e-6 && (alt(&d[n - 1]) - 4.0).abs() < 1e-6, "{} {}", alt(&d[0]), alt(&d[n - 1]));
        }
        // 20 frames of the glide are the test's: 12 m a frame, 228 m in all.
        let g = glide_cameras(&c, R, 0.0, 1.6, 20);
        let d = (g[1].position.0 - g[0].position.0).length();
        assert!((d - 12.0).abs() < 0.01, "{d}");
        // Equal steps of the glide.
        let g = glide_cameras(&c, R, 0.0, 1.6, 240);
        let steps: Vec<f64> = g.windows(2).map(|w| (w[1].position.0 - w[0].position.0).length()).collect();
        let (lo, hi) = steps.iter().fold((f64::MAX, 0.0f64), |(l, h), s| (l.min(*s), h.max(*s)));
        assert!(hi - lo < 1e-3 && (steps[0] - 228.0 / 239.0).abs() < 1e-3, "{lo} {hi}");
    }

    // spec: TEST-025
    #[test]
    fn the_view_set_has_the_static_views_in_order_then_the_flights_and_refuses_a_world_without_a_coast() {
        let field = Reference(1);
        let reference_sea = |d: Vec3| field.height(d) < 0.0;
        let views = lit_views(R, 16.0 / 9.0, &field, 0.0, &reference_sea, 30).unwrap();
        let names: Vec<&str> = views.iter().map(|v| v.name).collect();
        let want: Vec<&str> = STATIC_VIEWS.iter().chain(FLIGHTS.iter()).copied().collect();
        assert_eq!(names, want);
        for v in &views {
            match v.kind {
                ViewKind::Static => assert_eq!(v.cameras.len(), 1, "{}", v.name),
                ViewKind::Flight => assert_eq!(v.cameras.len(), 30, "{}", v.name),
            }
        }
        let alt = |n: &str| {
            let c = views.iter().find(|v| v.name == n).unwrap().cameras[0];
            c.position.0.length() - R
        };
        assert!((alt("coast-2km") - 2_000.0).abs() < 1e-6 && (alt("coast-3m") - 3.0).abs() < 1e-6);
        let e = lit_views(R, 1.6, &field, 0.0, &|_| false, 30).unwrap_err();
        assert!(e.contains("no beach"), "{e}");
    }

    /// A 1:50 beach whose waterline wanders 20 m (wavelength about 380 m) around the coast point, in the coast's own frame.
    fn wavy_beach() -> (Coast, impl Fn(Vec3) -> f64) {
        let coast = Coast { dir: Vec3::new(1.0, 0.0, 0.0), heading: Vec3::new(0.0, 1.0, 0.0) };
        let f = ShoreFrame::new(&coast, R);
        let height = move |d: Vec3| {
            let (s, t) = f.coords(d);
            0.02 * (t - 20.0 * libm::sin(s / 60.0))
        };
        (coast, height)
    }

    /// Metres seaward of the waterline of the line through the camera's ground point, and the camera's height above the level.
    fn offshore_and_altitude(c: &Coast, height: &dyn Fn(Vec3) -> f64, cam: &Camera) -> (f64, f64) {
        let f = ShoreFrame::new(c, R);
        let (s, t) = f.coords(cam.position.0);
        (shore_crossing(&f, height, 0.0, s) - t, cam.position.0.length() - R)
    }

    // spec: TEST-025
    #[test]
    fn the_shore_run_stays_30_m_up_and_100_to_200_m_off_a_wandering_beach_for_20_seconds() {
        let (coast, height) = wavy_beach();
        let cams = shore_run_cameras(&coast, R, 0.0, &height, 16.0 / 9.0, SHORE_RUN_S * CAPTURE_FPS);
        assert_eq!(cams.len(), 1_200);
        let (mut lo, mut hi) = (f64::MAX, 0.0f64);
        for cam in &cams {
            let (off, alt) = offshore_and_altitude(&coast, &height, cam);
            assert!((alt - SHORE_RUN_ALT_M).abs() < 1e-6, "altitude {alt}");
            assert!((100.0..=200.0).contains(&off), "{off} m off the waterline");
            assert!(height(cam.position.0.normalized()) < 0.0, "over the sea");
            (lo, hi) = (lo.min(off), hi.max(off));
        }
        assert!(hi - lo > 1.0, "the path follows the wandering beach: {lo}..{hi}");
        let travelled = (cams[1_199].position.0 - cams[0].position.0).length();
        assert!((travelled - SHORE_RUN_LENGTH_M).abs() < 30.0, "{travelled} m along the beach");
        // Looking along the beach and slightly toward the land, pitched down.
        let f = ShoreFrame::new(&coast, R);
        let c = cams[600];
        assert!(c.forward.dot(f.along) > 0.8 && c.forward.dot(f.heading) > 0.2 && c.forward.dot(c.up) < -0.1);
    }

    // spec: TEST-025
    #[test]
    fn the_shore_approach_flies_3_m_up_over_the_sea_straight_at_the_beach_and_ends_over_the_sand() {
        let (coast, height) = wavy_beach();
        let cams = shore_approach_cameras(&coast, R, 0.0, &height, 16.0 / 9.0, SHORE_APPROACH_S * CAPTURE_FPS);
        assert_eq!(cams.len(), 900);
        let f = ShoreFrame::new(&coast, R);
        let shore = shore_crossing(&f, &height, 0.0, 0.0);
        let mut last_t = f64::MIN;
        for cam in &cams {
            let (s, t) = f.coords(cam.position.0);
            let ground = height(cam.position.0.normalized());
            let alt = cam.position.0.length() - R;
            assert!(s.abs() < 0.01, "straight at the beach: {s} m along it");
            assert!(t > last_t, "moving landward");
            last_t = t;
            // 3 m above the level over the sea; over the sand never lower than 2 m above the ground.
            assert!((alt - SHORE_APPROACH_ALT_M.max(ground + SHORE_APPROACH_CLEARANCE_M)).abs() < 1e-6, "altitude {alt}, ground {ground}");
            assert!(alt >= SHORE_APPROACH_ALT_M - 1e-9);
        }
        let (_, t0) = f.coords(cams[0].position.0);
        assert!(((shore - t0) - SHORE_APPROACH_START_M).abs() < 0.01, "starts {} m out", shore - t0);
        let (_, t1) = f.coords(cams[899].position.0);
        assert!(
            ((t1 - shore) - SHORE_APPROACH_END_M).abs() < 0.01 && height(cams[899].position.0.normalized()) > 0.0,
            "ends over the sand"
        );
        assert!(cams[0].forward.dot(f.heading) > 0.99, "looking at the beach");
    }

    // spec: TEST-025
    #[test]
    fn the_four_stills_sit_at_their_eye_heights_above_the_higher_of_the_terrain_and_the_water_and_face_their_way() {
        let (coast, height) = wavy_beach();
        let cams = shore_still_cameras(&coast, R, 0.0, &height, 16.0 / 9.0);
        assert_eq!(cams.len(), SHORE_STILLS.len());
        let f = ShoreFrame::new(&coast, R);
        let shore = shore_crossing(&f, &height, 0.0, 0.0);
        // (eye height, metres seaward of the waterline, looking: dot with along, dot with heading)
        let want = [(0.5, 0.0, (1.0, 0.0)), (0.5, 20.0, (0.0, 1.0)), (0.2, 0.0, (0.0, -1.0)), (0.1, 30.0, (1.0, 0.0))];
        for (cam, (eye, out, (along, toward))) in cams.iter().zip(want) {
            let (_, t) = f.coords(cam.position.0);
            let ground = height(cam.position.0.normalized()).max(0.0);
            assert!((cam.position.0.length() - R - ground - eye).abs() < 1e-6, "eye height {eye}");
            assert!((shore - t - out).abs() < 0.01, "{out} m seaward");
            assert!(
                (cam.forward.dot(f.along) - along).abs() < 0.05 && (cam.forward.dot(f.heading) - toward).abs() < 0.05,
                "{:?}",
                cam.forward
            );
        }
    }

    // spec: TEST-025
    #[test]
    fn the_shore_flight_views_are_the_two_named_capture_flights_and_the_perf_flights_are_unchanged() {
        let field = Reference(1);
        let reference_sea = |d: Vec3| field.height(d) < 0.0;
        let v = shore_flight_views(R, 16.0 / 9.0, &field, 0.0, &reference_sea, Some(10)).unwrap();
        let want: Vec<&str> = SHORE_STILLS.iter().chain(SHORE_FLIGHTS.iter()).copied().collect();
        assert_eq!(v.iter().map(|x| x.name).collect::<Vec<_>>(), want);
        assert!(v.iter().all(|x| x.kind == ViewKind::Flight));
        assert!(v[..4].iter().all(|x| x.cameras.len() == 1) && v[4..].iter().all(|x| x.cameras.len() == 10));
        // The perf scenes' view list does not contain them.
        let perf = lit_views(R, 16.0 / 9.0, &field, 0.0, &reference_sea, 10).unwrap();
        assert!(perf.iter().all(|x| !want.contains(&x.name)) && FLIGHTS == ["coast-glide", "coast-descent"]);
        assert!(shore_flight_views(R, 1.6, &field, 0.0, &|_| false, None).is_err());
    }
}
