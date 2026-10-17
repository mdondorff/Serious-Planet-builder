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
/// where the sea lies to the west and the land to the east, with the sea reaching at least 2 km out and the land 500 m in: the walk
/// is 20 km a step along the parallel and the shoreline is bisected to a millimetre. `None` when no parallel has one.
pub fn find_coast(is_sea: &dyn Fn(Vec3) -> bool, lat_deg: f64, lon0_deg: f64, radius_m: f64) -> Option<Coast> {
    const STEP_M: f64 = 20_000.0;
    for off in [0.0, 10.0, -10.0, 20.0, -20.0, 30.0, -30.0] {
        let lat = (lat_deg + off).clamp(-80.0, 80.0).to_radians();
        let ring = std::f64::consts::TAU * radius_m * libm::cos(lat);
        let at = |s: f64| on_parallel(lat, lon0_deg.to_radians() + s / (radius_m * libm::cos(lat)));
        let steps = (ring / STEP_M).ceil() as usize;
        let mut prev = is_sea(at(0.0));
        for k in 1..=steps {
            let s = k as f64 * STEP_M;
            let cur = is_sea(at(s));
            if prev && !cur {
                let (mut a, mut b) = (s - STEP_M, s);
                for _ in 0..44 {
                    let m = 0.5 * (a + b);
                    if is_sea(at(m)) {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                if is_sea(at(b - 2_000.0)) && !is_sea(at(b + 500.0)) {
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
    let coast = find_coast(is_sea, 20.0, 30.0, radius_m)
        .ok_or("the world has no shoreline with sea to its west within three parallels of 20 N: no coast view can be measured")?;
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

    // spec: TEST-025
    #[test]
    fn the_coast_is_the_first_sea_to_land_crossing_east_of_the_start_with_the_heading_towards_the_land() {
        let c = find_coast(&sea, 20.0, 30.0, R).expect("a coast");
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
        assert!(find_coast(&|_| false, 20.0, 30.0, R).is_none(), "all land");
        assert!(find_coast(&|_| true, 20.0, 30.0, R).is_none(), "all sea");
        // A pond 600 m wide: the sea does not reach 2 km out.
        let pond = |d: Vec3| {
            let lon = libm::atan2(d.y, d.x).to_degrees();
            (59.99..60.0).contains(&lon) && d.z.abs() < 0.5
        };
        assert!(find_coast(&pond, 20.0, 30.0, R).is_none());
    }

    // spec: TEST-025
    #[test]
    fn the_flights_start_and_end_where_the_water_tests_do_and_scale_with_the_frame_count() {
        let c = find_coast(&sea, 20.0, 30.0, R).unwrap();
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
        let views = lit_views(R, 16.0 / 9.0, &field, 0.0, &sea, 30).unwrap();
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
        assert!(e.contains("no shoreline"), "{e}");
    }
}
