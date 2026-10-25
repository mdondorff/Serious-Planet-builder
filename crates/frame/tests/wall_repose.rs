//! LOD-008 (ADR 0074): a channel cut into a plateau is capped only where its walls are steeper than the parameters can draw (`drawable_slope`). On a
//! synthetic trench along a great circle, 100 m and 300 m deep, with walls of slope 0.5, 0.9 (the angle of repose of the chunk stage's hillslopes),
//! 2.0, 4.0 and a 1 m ramp (vertical), the plan from poses like the stills' (300 m, 30 m, 1.8 m over the ground) caps no tile below the drawable
//! slope and caps tiles above it. Tier A, no GPU.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, Vec3};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);
const PLATEAU_M: f64 = 3_000.0;
/// Half the width of the trench floor.
const FLOOR_HALF_M: f64 = 10.0;

/// A plateau of `PLATEAU_M` cut by a trench along the great circle with the given pole: floor `depth_m` down over `FLOOR_HALF_M` each side, then a
/// linear wall of the given slope (rise over run) up to the plateau. Monotone in the distance from the circle, so the bounds over a cap are exact.
struct Trench {
    pole: Vec3,
    depth_m: f64,
    slope: f64,
}

impl Trench {
    fn wall_m(&self) -> f64 {
        self.depth_m / self.slope
    }
    /// Height as a function of the distance from the circle in metres.
    fn profile(&self, d_m: f64) -> f64 {
        let up = ((d_m - FLOOR_HALF_M) / self.wall_m()).clamp(0.0, 1.0);
        PLATEAU_M - self.depth_m * (1.0 - up)
    }
    fn distance_m(&self, dir: Vec3) -> f64 {
        libm::asin(self.pole.dot(dir).clamp(-1.0, 1.0)).abs() * R
    }
}

impl HeightField for Trench {
    fn height(&self, dir: Vec3) -> f64 {
        self.profile(self.distance_m(dir))
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "test-trench"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        PLATEAU_M
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let d = self.distance_m(centre);
        let reach = angular_radius * R;
        (self.profile((d - reach).max(0.0)) - 1e-9, self.profile(d + reach) + 1e-9)
    }
    fn bounds_monotone(&self) -> bool {
        true
    }
    fn inputs_hash(&self) -> u64 {
        29
    }
}

fn lod() -> LodParams {
    LodParams { viewport_h_px: f64::from(SIZE.1), tau_px: 6.0, min_level: 0, max_height_m: 9_000.0, ..LodParams::earth_1080p() }
}

/// The poses: the eye `altitude_m` over the ground `back_m` before the origin of the trench along it, `side_m` to the right of its axis, looking
/// along the trench pitched down by `pitch_deg`. Returns (capped, total) nodes of the plan.
fn plan_counts(trench: &Trench, origin: Vec3, along: Vec3, back_m: f64, side_m: f64, altitude_m: f64, pitch_deg: f64) -> (usize, usize) {
    let right = along.cross(origin).normalized();
    let eye_dir = (origin * R - along * back_m + right * side_m).normalized();
    let eye = eye_dir * (R + trench.height(eye_dir) + altitude_m);
    let hint = (along - eye_dir * along.dot(eye_dir)).normalized();
    let camera = Camera::at_surface_point(
        PlanetFixed(eye),
        hint,
        pitch_deg.to_radians(),
        60f64.to_radians(),
        f64::from(SIZE.0) / f64::from(SIZE.1),
        0.1,
    );
    let request =
        TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 1 };
    let plan = plan_terrain_field(&request, trench).expect("a plan");
    (plan.nodes.iter().filter(|n| n.capped).count(), plan.nodes.len())
}

// spec: LOD-008
#[test]
fn a_wall_under_the_drawable_slope_caps_no_tile_and_a_steeper_one_does() {
    let origin = Vec3::from_lat_lon(0.2, 0.4);
    let along = (Vec3::new(0.0, 0.0, 1.0) - origin * origin.z).normalized();
    let pole = along.cross(origin).normalized();
    let drawable = lod().drawable_slope();
    // The stills' poses: (altitude, back, side, pitch).
    let poses = [("300m", 300.0, 1500.0, 300.0, 15.0), ("30m", 30.0, 400.0, 150.0, 8.0), ("1.8m", 1.8, 60.0, 0.0, 0.0)];
    let mut table = String::new();
    let mut under = 0;
    let mut over = 0;
    for depth_m in [100.0, 300.0] {
        for slope in [0.5, 0.9, 2.0, 4.0, 300.0] {
            let trench = Trench { pole, depth_m, slope };
            let counts: Vec<(usize, usize)> =
                poses.iter().map(|&(_, alt, back, side, pitch)| plan_counts(&trench, origin, along, back, side, alt, pitch)).collect();
            table += &format!("depth {depth_m:>3} m slope {slope:>5}: capped/total {counts:?}\n");
            let capped: usize = counts.iter().map(|c| c.0).sum();
            if slope < drawable * 0.9 {
                assert_eq!(capped, 0, "slope {slope} is under the drawable {drawable}: {table}");
                under += 1;
            } else if slope > drawable * 1.2 {
                over += 1;
                assert!(capped > 0, "slope {slope} is over the drawable {drawable}: {table}");
            }
        }
    }
    eprintln!("drawable slope {drawable}\n{table}");
    assert!(under > 0 && over > 0);
}
