//! Tier A: surface picking (VIEW-012). The mouse ray meets the height field in f64; the footprint under the hit is the item.

use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::pick::{cursor_ray, pick_footprint, pick_surface, surface_distance_m, Footprint, PickLimits, Ray};

const R: f64 = 6_371_000.0;
const W: f64 = 1600.0;
const H: f64 = 900.0;

fn camera(position: Vec3, forward: Vec3, up: Vec3) -> Camera {
    Camera { position: PlanetFixed(position), forward: forward.normalized(), up, fov_y_rad: 1.0, aspect: W / H, near_m: 1.0 }
}

fn flat(_: Vec3) -> f64 {
    0.0
}

/// A 3 km hill at +z, 1 km of angular-scale width on the sphere.
fn hill(d: Vec3) -> f64 {
    let angle = libm::atan2(Vec3::new(d.x, d.y, 0.0).length(), d.z);
    let s = angle * R / 2000.0;
    3000.0 * libm::exp(-s * s)
}

/// The independent reference: the ray against the sphere of radius `r`, by the quadratic.
fn analytic_sphere_hit(ray: &Ray, r: f64) -> f64 {
    let b = ray.origin.dot(ray.dir);
    let c = ray.origin.dot(ray.origin) - r * r;
    -b - libm::sqrt(b * b - c)
}

// spec: VIEW-012
#[test]
fn the_centre_pixel_of_a_camera_over_a_bare_sphere_hits_the_surface_below_it() {
    let cam = camera(Vec3::new(0.0, 0.0, 2.0 * R), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let ray = cursor_ray(&cam, W / 2.0, H / 2.0, W, H);
    let hit = pick_surface(&ray, &PickLimits::new(R, 10_000.0), &flat).expect("a hit");
    assert!((hit.distance_m - R).abs() < 1e-3, "distance {} vs {R}", hit.distance_m);
    assert!((hit.point.length() - R).abs() < 1e-3);
    assert!((hit.dir.z - 1.0).abs() < 1e-12);
    assert_eq!(hit.height_m, 0.0);
}

// spec: VIEW-012
#[test]
fn off_centre_pixels_hit_where_the_analytic_sphere_says_at_orbit_and_near_the_ground() {
    let limits = PickLimits::new(R, 10_000.0);
    for (altitude, forward) in [(R, Vec3::new(0.0, 0.0, -1.0)), (50_000.0, Vec3::new(0.0, 0.5, -1.0)), (300.0, Vec3::new(0.0, 0.7, -0.7))] {
        let cam = camera(Vec3::new(0.0, 0.0, R + altitude), forward, Vec3::new(0.0, 1.0, 0.0));
        for (x, y) in [(10.0, 10.0), (800.0, 450.0), (1590.0, 890.0), (300.0, 700.0), (1200.0, 120.0)] {
            let ray = cursor_ray(&cam, x, y, W, H);
            let b = ray.origin.dot(ray.dir);
            let discriminant = b * b - (ray.origin.dot(ray.origin) - R * R);
            let hit = pick_surface(&ray, &limits, &flat);
            if discriminant < 0.0 {
                assert!(hit.is_none(), "altitude {altitude} pixel ({x},{y}) misses the sphere");
                continue;
            }
            let hit = hit.unwrap_or_else(|| panic!("altitude {altitude} pixel ({x},{y}) hits the sphere"));
            let want = analytic_sphere_hit(&ray, R);
            assert!((hit.distance_m - want).abs() < 2e-3, "altitude {altitude} pixel ({x},{y}): {} vs {want}", hit.distance_m);
        }
    }
}

// spec: VIEW-012
#[test]
fn a_ray_past_the_limb_or_away_from_the_planet_misses() {
    let limits = PickLimits::new(R, 10_000.0);
    let cam = camera(Vec3::new(0.0, 0.0, 1.5 * R), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
    assert!(pick_surface(&cursor_ray(&cam, W / 2.0, H / 2.0, W, H), &limits, &flat).is_none(), "looks along the tangent plane's side");
    let away = camera(Vec3::new(0.0, 0.0, 1.5 * R), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0));
    assert!(pick_surface(&cursor_ray(&away, W / 2.0, H / 2.0, W, H), &limits, &flat).is_none(), "looks away");
}

// spec: VIEW-012
#[test]
fn a_ray_over_a_hill_hits_the_hill_where_a_dense_march_does_and_reports_its_height() {
    let limits = PickLimits { max_slope: 2.0, ..PickLimits::new(R, 4_000.0) };
    // From 30 km up and 20 km to the side, descending at the hill's top.
    let origin = Vec3::new(20_000.0, 0.0, R + 30_000.0);
    let top = Vec3::new(0.0, 0.0, R + 3000.0);
    let ray = Ray { origin, dir: (top - origin).normalized() };
    let hit = pick_surface(&ray, &limits, &hill).expect("a hit");
    let mut reference = None;
    let mut t = 0.0;
    while t < 80_000.0 {
        let p = ray.origin + ray.dir * t;
        if p.length() - R - hill(p.normalized()) <= 0.0 {
            reference = Some(t);
            break;
        }
        t += 0.25;
    }
    let reference = reference.expect("the dense march hits");
    assert!((hit.distance_m - reference).abs() < 0.3, "{} vs dense {reference}", hit.distance_m);
    assert!((hit.height_m - hill(hit.dir)).abs() < 1e-9);
    assert!(hit.height_m > 2500.0, "it hit the hill's upper part: {}", hit.height_m);
    assert!((hit.point.length() - (R + hit.height_m)).abs() < 1e-3, "the point is on the surface");
}

// spec: VIEW-012
#[test]
fn a_camera_inside_the_shell_picks_and_one_below_the_surface_does_not() {
    let limits = PickLimits::new(R, 4_000.0);
    // 100 m above the flat sphere, inside the shell of the hill's bound.
    let low = camera(Vec3::new(0.0, 0.0, R + 100.0), Vec3::new(0.0, 0.8, -0.6), Vec3::new(0.0, 0.0, 1.0));
    let hit = pick_surface(&cursor_ray(&low, W / 2.0, H / 2.0, W, H), &limits, &flat).expect("a hit from inside the shell");
    assert!(hit.distance_m > 100.0 && hit.distance_m < 200.0, "{}", hit.distance_m);
    let buried = camera(Vec3::new(0.0, 0.0, R - 50.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
    assert!(pick_surface(&cursor_ray(&buried, W / 2.0, H / 2.0, W, H), &limits, &flat).is_none());
}

// spec: VIEW-012
#[test]
fn the_hit_projects_back_to_the_cursor_pixel() {
    let cam = camera(Vec3::new(1_000.0, 2_000.0, R + 20_000.0), Vec3::new(0.2, 0.3, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let limits = PickLimits::new(R, 4_000.0);
    for (x, y) in [(100.0, 80.0), (800.0, 450.0), (1500.0, 800.0), (400.0, 600.0)] {
        let hit = pick_surface(&cursor_ray(&cam, x, y, W, H), &limits, &hill).expect("a hit");
        let clip = cam.project(hit.point - cam.position.0);
        let (ndc_x, ndc_y) = (clip[0] / clip[3], clip[1] / clip[3]);
        let (px, py) = ((ndc_x + 1.0) * 0.5 * W, (1.0 - ndc_y) * 0.5 * H);
        assert!((px - x).abs() < 1e-6 && (py - y).abs() < 1e-6, "({x},{y}) came back as ({px},{py})");
    }
}

// spec: VIEW-012
#[test]
fn picking_is_deterministic_bit_for_bit() {
    let cam = camera(Vec3::new(500.0, -300.0, R + 8_000.0), Vec3::new(0.1, 0.2, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let limits = PickLimits::new(R, 4_000.0);
    let a = pick_surface(&cursor_ray(&cam, 700.5, 333.5, W, H), &limits, &hill).expect("a hit");
    let b = pick_surface(&cursor_ray(&cam, 700.5, 333.5, W, H), &limits, &hill).expect("a hit");
    assert_eq!(
        (a.point.x.to_bits(), a.point.y.to_bits(), a.point.z.to_bits()),
        (b.point.x.to_bits(), b.point.y.to_bits(), b.point.z.to_bits())
    );
    assert_eq!(a.distance_m.to_bits(), b.distance_m.to_bits());
}

fn dir_at(arc_m: f64, bearing_rad: f64) -> Vec3 {
    // Start at +z and move `arc_m` along the great circle at the bearing.
    let angle = arc_m / R;
    Vec3::new(libm::sin(angle) * libm::cos(bearing_rad), libm::sin(angle) * libm::sin(bearing_rad), libm::cos(angle))
}

fn hit_at(dir: Vec3) -> planet_frame::pick::SurfaceHit {
    planet_frame::pick::SurfaceHit { point: dir * R, dir, height_m: 0.0, distance_m: 0.0 }
}

// spec: VIEW-012
#[test]
fn the_surface_distance_is_exact_at_small_angles() {
    for arc in [0.001, 0.5, 40.0, 5_000.0, 2_000_000.0] {
        let d = surface_distance_m(Vec3::new(0.0, 0.0, 1.0), dir_at(arc, 0.7), R);
        assert!((d - arc).abs() < arc * 1e-12, "{d} vs {arc}");
    }
}

// spec: VIEW-012
#[test]
fn the_footprint_under_a_hit_is_the_item_and_nested_items_resolve_to_the_nearest_in_their_own_radius() {
    let big = Footprint { id: 7, dir: dir_at(0.0, 0.0), radius_m: 1000.0 };
    let small = Footprint { id: 3, dir: dir_at(300.0, 0.0), radius_m: 100.0 };
    let far = Footprint { id: 9, dir: dir_at(50_000.0, 1.0), radius_m: 1000.0 };
    let items = [big, small, far];
    assert_eq!(pick_footprint(&hit_at(dir_at(300.0, 0.0)), &items, R), Some(3), "centre of the small item inside the big one");
    assert_eq!(
        pick_footprint(&hit_at(dir_at(330.0, 0.0)), &items, R),
        Some(3),
        "0.30 of the small item's radius against 0.33 of the big one's"
    );
    assert_eq!(
        pick_footprint(&hit_at(dir_at(390.0, 0.0)), &items, R),
        Some(7),
        "0.90 of the small item's radius against 0.39 of the big one's"
    );
    assert_eq!(pick_footprint(&hit_at(dir_at(900.0, 0.0)), &items, R), Some(7), "outside the small item, inside the big one");
    assert_eq!(pick_footprint(&hit_at(dir_at(2_000.0, 0.0)), &items, R), None, "outside every footprint");
    assert_eq!(pick_footprint(&hit_at(dir_at(1000.0, 0.0)), &[big], R), Some(7), "the rim belongs to the footprint");
    assert_eq!(pick_footprint(&hit_at(dir_at(50_000.0, 1.0)), &items, R), Some(9));
}

// spec: VIEW-012
#[test]
fn equal_candidates_resolve_to_the_smaller_radius_then_the_lower_id() {
    let d = dir_at(0.0, 0.0);
    let items = [
        Footprint { id: 5, dir: d, radius_m: 100.0 },
        Footprint { id: 2, dir: d, radius_m: 100.0 },
        Footprint { id: 1, dir: d, radius_m: 200.0 },
    ];
    assert_eq!(pick_footprint(&hit_at(d), &items, R), Some(2), "all at distance 0: the smallest radius, then the lowest id");
    assert_eq!(pick_footprint(&hit_at(d), &[], R), None);
}

// spec: VIEW-012
#[test]
fn a_ray_entering_the_shell_onto_a_surface_at_its_bound_hits_at_the_entry() {
    let from_orbit = camera(Vec3::new(0.0, 0.0, 2.0 * R), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let bare = PickLimits::new(R, 0.0);
    let plateau = |_: Vec3| 4_000.0;
    for (x, y) in [(800.0, 450.0), (900.0, 500.0), (700.0, 400.0), (810.0, 440.0)] {
        let ray = cursor_ray(&from_orbit, x, y, W, H);
        let hit = pick_surface(&ray, &bare, &flat).unwrap_or_else(|| panic!("a bound of 0 m on a bare sphere: pixel ({x},{y})"));
        assert!((hit.distance_m - analytic_sphere_hit(&ray, R)).abs() < 2e-3);
        let hit = pick_surface(&ray, &PickLimits::new(R, 4_000.0), &plateau).unwrap_or_else(|| panic!("a plateau at the bound: pixel ({x},{y})"));
        assert!((hit.distance_m - analytic_sphere_hit(&ray, R + 4_000.0)).abs() < 2e-3);
    }
}

// spec: VIEW-012
#[test]
fn rays_that_graze_the_surface_hit_or_miss_with_the_sphere() {
    let limits = PickLimits::new(R, 10_000.0);
    // Straight down at a distance `x` from the axis through the centre: the closest approach to the centre is `x`.
    let down = |x: f64| Ray { origin: Vec3::new(x, 0.0, 1.5 * R), dir: Vec3::new(0.0, 0.0, -1.0) };
    let inside = down(R - 1_000.0);
    let hit = pick_surface(&inside, &limits, &flat).expect("a ray 1 km inside the limb hits");
    assert!((hit.distance_m - analytic_sphere_hit(&inside, R)).abs() < 2e-3, "{}", hit.distance_m);
    assert!(pick_surface(&down(R + 1.0), &limits, &flat).is_none(), "a ray 1 m outside the limb enters the shell and misses");
}

// spec: VIEW-012
#[test]
fn degenerate_inputs_are_misses_and_never_hang() {
    let limits = PickLimits::new(R, 4_000.0);
    let centre = Ray { origin: Vec3::new(0.0, 0.0, 0.0), dir: Vec3::new(0.0, 0.0, 1.0) };
    assert!(pick_surface(&centre, &limits, &hill).is_none(), "a camera at the centre is below the surface");
    let from_orbit = Ray { origin: Vec3::new(0.0, 0.0, 2.0 * R), dir: Vec3::new(0.0, 0.0, -1.0) };
    for bad in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(f64::NAN, 0.0, 1.0)] {
        assert!(pick_surface(&Ray { dir: bad, ..from_orbit }, &limits, &flat).is_none());
    }
    let unnormalised = Ray { dir: Vec3::new(0.0, 0.0, -2.0), ..from_orbit };
    let hit = pick_surface(&unnormalised, &limits, &flat).expect("a direction is normalised");
    assert!((hit.distance_m - R).abs() < 1e-3);
    let no_tolerance = PickLimits { tolerance_m: 0.0, ..limits };
    let hit = pick_surface(&from_orbit, &no_tolerance, &flat).expect("a zero tolerance still ends");
    assert!((hit.distance_m - R).abs() < 1e-3);
    let nan_field = |_: Vec3| f64::NAN;
    assert!(pick_surface(&from_orbit, &limits, &nan_field).is_none());
    let cam = camera(Vec3::new(0.0, 0.0, 2.0 * R), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let flat_viewport = cursor_ray(&cam, 0.0, 0.0, 0.0, 0.0);
    assert!(flat_viewport.dir.length().is_finite() && (flat_viewport.dir.z + 1.0).abs() < 1e-12, "a viewport without area gives the axis");
}

// spec: VIEW-012
#[test]
fn a_footprint_without_a_direction_or_a_radius_contains_nothing() {
    let real = Footprint { id: 4, dir: dir_at(0.0, 0.0), radius_m: 100.0 };
    let hit = hit_at(dir_at(5_000.0, 0.0));
    let zero = Footprint { id: 1, dir: Vec3::new(0.0, 0.0, 0.0), radius_m: 1.0e9 };
    let nan_dir = Footprint { id: 2, dir: Vec3::new(f64::NAN, 0.0, 1.0), radius_m: 1.0e9 };
    let nan_radius = Footprint { id: 3, dir: dir_at(5_000.0, 0.0), radius_m: f64::NAN };
    for order in [[zero, nan_dir, nan_radius, real], [real, nan_radius, nan_dir, zero]] {
        assert_eq!(pick_footprint(&hit, &order, R), None, "far from the real item, the broken ones contain nothing");
        assert_eq!(pick_footprint(&hit_at(dir_at(10.0, 0.0)), &order, R), Some(4));
    }
}
