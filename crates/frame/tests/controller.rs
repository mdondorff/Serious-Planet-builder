//! Adaptive camera (REND-010) and the instance culling reference (REND-011).

use planet_core::hash::SplitMix64;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::{
    camera_relative_positions, cull_instances, indirect_args, Camera, CameraController, CameraInput, CameraMode, ControllerParams, Instance,
};

const R: f64 = 6_371_000.0;

fn flat(_: Vec3) -> f64 {
    0.0
}

fn hilly(d: Vec3) -> f64 {
    planet_generators::height::height_at(1, d)
}

fn controller(height: f64) -> CameraController {
    let dir = Vec3::new(0.5, 0.5, 0.7).normalized();
    CameraController::new(PlanetFixed(dir * (R + height)), Vec3::new(0.0, 0.0, 1.0), ControllerParams::default())
}

fn go(forward: f64, right: f64, up: f64) -> CameraInput {
    CameraInput { forward, right, up, speed_scale: 1.0, ..Default::default() }
}

fn moved(c: &mut CameraController, input: CameraInput, dt: f64, terrain: &dyn Fn(Vec3) -> f64) -> Vec3 {
    let before = c.position.0;
    c.step(input, dt, terrain);
    c.position.0 - before
}

// spec: REND-010
#[test]
fn speed_is_proportional_to_the_height_above_the_terrain_within_limits() {
    let c = controller(1000.0);
    assert_eq!(c.speed_for_height(1000.0), 1000.0);
    assert_eq!(c.speed_for_height(2000.0), 2000.0);
    assert_eq!(c.speed_for_height(0.0), c.params.min_speed_mps);
    assert_eq!(c.speed_for_height(1.0e12), c.params.max_speed_mps);
    // One second of forward flight at 10 km over flat ground covers 10 km along the view direction.
    let mut c = controller(10_000.0);
    let d = moved(&mut c, go(1.0, 0.0, 0.0), 1.0, &flat);
    assert!((d.length() - 10_000.0).abs() < 1.0, "moved {} m", d.length());
    // The height is measured above the TERRAIN: 1,000 m of terrain below leaves 9,000 m, so the camera is slower.
    let mut c = controller(10_000.0);
    let d = moved(&mut c, go(1.0, 0.0, 0.0), 1.0, &|_| 1000.0);
    assert!((d.length() - 9_000.0).abs() < 1.0, "terrain-aware speed gave {} m", d.length());
    // `speed_scale` multiplies the adaptive speed.
    let mut c = controller(10_000.0);
    let d = moved(&mut c, CameraInput { forward: 1.0, speed_scale: 2.0, ..Default::default() }, 1.0, &flat);
    assert!((d.length() - 20_000.0).abs() < 30.0, "boost gave {} m", d.length());
}

// spec: REND-010
#[test]
fn the_clearance_engages_exactly_and_follows_the_terrain() {
    // Start inside a 300 m plateau: the next step lifts the camera to exactly terrain + clearance.
    let mut c = controller(100.0);
    c.step(go(0.0, 0.0, 0.0), 0.01, &|_| 300.0);
    let h = c.height_above_terrain(&|_| 300.0);
    assert!((h - c.params.min_clearance_m).abs() < 1e-6, "lifted to {h} m above the plateau, expected the 1 m clearance");
    // Flying into rising ground: the camera ends within a metre of the clearance and stays above the terrain.
    let ramp = |d: Vec3| 200.0 + 4000.0 * (d.z - 0.7).max(0.0);
    let mut c = controller(500.0);
    for _ in 0..400 {
        c.step(go(1.0, 0.0, 0.0), 0.1, &ramp);
        assert!(c.height_above_terrain(&ramp) >= c.params.min_clearance_m - 1e-6);
    }
    let mut rng = SplitMix64(3);
    let mut c = controller(50.0);
    for _ in 0..2000 {
        let input = CameraInput {
            forward: rng.range(-1.0, 1.0),
            right: rng.range(-1.0, 1.0),
            up: rng.range(-1.0, 1.0),
            yaw_rate: rng.range(-1.0, 1.0),
            pitch_rate: rng.range(-1.0, 1.0),
            speed_scale: 1.0,
        };
        c.step(input, 0.1, &hilly);
        assert!(c.position.0.length().is_finite() && (c.forward().length() - 1.0).abs() < 1e-9);
        assert!(c.height_above_terrain(&hilly) >= c.params.min_clearance_m - 1e-6);
    }
}

// spec: REND-010
#[test]
fn a_dive_slows_down_and_stops_at_the_clearance() {
    let mut c = controller(2.0e6);
    let mut last_speed = f64::MAX;
    for _ in 0..400 {
        let d = moved(&mut c, go(0.0, 0.0, -1.0), 0.5, &flat);
        let speed = d.length() / 0.5;
        assert!(speed <= last_speed + 1e-6, "speed rose from {last_speed} to {speed} while descending");
        last_speed = speed;
    }
    let h = c.height_above_terrain(&flat);
    assert!((h - c.params.min_clearance_m).abs() < 0.5, "ended at {h} m, expected the clearance");
}

// spec: REND-010
#[test]
fn large_steps_do_not_tunnel_through_the_planet_and_bad_input_is_ignored() {
    let mut c = controller(1.0e7);
    c.step(go(0.0, 0.0, -1.0), 50.0, &flat);
    assert!(c.position.0.length() >= R + c.params.min_clearance_m - 1e-6, "tunnelled to {} m from the centre", c.position.0.length());
    let snapshot = controller(5000.0);
    let mut c = snapshot;
    c.step(
        CameraInput {
            forward: f64::NAN,
            right: f64::INFINITY,
            up: f64::NEG_INFINITY,
            yaw_rate: f64::NAN,
            pitch_rate: f64::NAN,
            speed_scale: f64::NAN,
        },
        1.0,
        &flat,
    );
    assert!(c.position.0.length().is_finite() && c.heading.length() > 0.99);
    let mut c = snapshot;
    for dt in [-1.0, f64::NAN] {
        c.step(go(1.0, 0.0, 0.0), dt, &flat);
        assert_eq!(c.position, snapshot.position, "a step of {dt} s must not move the camera");
        assert!(c.heading.distance(snapshot.heading) < 1e-12);
    }
}

// spec: REND-010
#[test]
fn pitch_accumulates_is_limited_and_tilts_the_view_direction() {
    let mut c = controller(10_000.0);
    for _ in 0..3 {
        c.step(CameraInput { pitch_rate: 1.0, ..Default::default() }, 0.1, &flat);
    }
    assert!((c.pitch - 0.3).abs() < 1e-9, "pitch accumulates: {}", c.pitch);
    assert!((c.forward().dot(c.up()) - libm::sin(0.3)).abs() < 1e-9, "the view direction tilts by the pitch");
    for _ in 0..100 {
        c.step(CameraInput { pitch_rate: 5.0, ..Default::default() }, 0.1, &flat);
    }
    assert!((c.pitch - 85f64.to_radians()).abs() < 0.001, "pitch limited to about 85 degrees, got {}", c.pitch.to_degrees());
    for _ in 0..200 {
        c.step(CameraInput { pitch_rate: -5.0, ..Default::default() }, 0.1, &flat);
    }
    assert!((c.pitch + 85f64.to_radians()).abs() < 0.001, "limited downwards too");
    // Flying forward while looking up climbs.
    let mut c = controller(10_000.0);
    c.step(CameraInput { pitch_rate: 1.0, ..Default::default() }, libm::atan(1.0), &flat); // 45 degrees
    let h0 = c.height_above_terrain(&flat);
    c.step(go(1.0, 0.0, 0.0), 1.0, &flat);
    assert!(c.height_above_terrain(&flat) > h0 + 1000.0, "forward flight along a raised view must gain height");
}

// spec: REND-010
#[test]
fn yaw_turns_right_and_sideways_input_moves_along_the_right_axis() {
    let mut c = controller(10_000.0);
    let (heading0, right0) = (c.heading, c.heading.cross(c.up()).normalized());
    c.step(CameraInput { yaw_rate: core::f64::consts::FRAC_PI_2, ..Default::default() }, 1.0, &flat);
    assert!(c.heading.dot(right0) > 0.999, "a positive yaw turns the heading to the right");
    assert!(c.heading.dot(heading0).abs() < 1e-6);
    let mut c = controller(10_000.0);
    let right = c.heading.cross(c.up()).normalized();
    let d = moved(&mut c, go(0.0, 1.0, 0.0), 1.0, &flat);
    assert!(d.normalized().dot(right) > 0.999, "right input moves to the right");
    let d = moved(&mut c, go(-1.0, 0.0, 0.0), 1.0, &flat);
    assert!(d.normalized().dot(c.heading) < -0.999, "backward moves against the view direction");
}

// spec: REND-010
#[test]
fn orbit_moves_zooms_and_keeps_looking_at_the_centre() {
    let run = || {
        let mut c = controller(1.0e7);
        c.mode = CameraMode::Orbit;
        for k in 0..100 {
            c.step(go(0.0, 1.0, if k % 2 == 0 { 0.5 } else { -0.2 }), 0.2, &flat);
        }
        c
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b, "deterministic");
    let toward_centre = (-a.position.0).normalized();
    assert!(a.forward().dot(toward_centre) > 0.999999, "orbit view must point at the centre");
    assert!((a.position.0.length() - (R + 1.0e7)).abs() < 1.0, "orbiting keeps the distance");
    // Directions: right moves screen-right, up moves screen-up (the heading), forward zooms in.
    let mut c = controller(1.0e7);
    c.mode = CameraMode::Orbit;
    let (right, up_on_screen, d0) = (c.heading.cross(c.up()).normalized(), c.heading, c.position.0.length());
    let dr = moved(&mut c, go(0.0, 1.0, 0.0), 1.0, &flat);
    assert!(dr.normalized().dot(right) > 0.99, "orbit right goes screen-right");
    let mut c = controller(1.0e7);
    c.mode = CameraMode::Orbit;
    let du = moved(&mut c, go(0.0, 0.0, 1.0), 1.0, &flat);
    assert!(du.normalized().dot(up_on_screen) > 0.99, "orbit up goes screen-up");
    let mut c = controller(1.0e7);
    c.mode = CameraMode::Orbit;
    c.step(go(1.0, 0.0, 0.0), 1.0, &flat);
    assert!(c.position.0.length() < d0 - 1.0e6, "forward zooms in");
    // Angular motion is speed / distance, and the speed is proportional to the height: 1,000 km up moves 1,000 km/s.
    let angle = |height: f64| {
        let mut c = controller(height);
        c.mode = CameraMode::Orbit;
        let before = c.position.0.normalized();
        c.step(go(0.0, 1.0, 0.0), 1.0, &flat);
        libm::acos(before.dot(c.position.0.normalized()).clamp(-1.0, 1.0))
    };
    let near = angle(1.0e6);
    let expected = 1.0e6 / (R + 1.0e6);
    assert!((near - expected).abs() < 0.01 * expected, "angular step {near} rad, expected {expected}");
    // Over a pole the basis stays well defined.
    let mut c = CameraController::new(PlanetFixed(Vec3::new(0.0, 0.0, R + 1.0e7)), Vec3::new(0.0, 0.0, 1.0), ControllerParams::default());
    c.mode = CameraMode::Orbit;
    for _ in 0..50 {
        c.step(go(0.0, 1.0, 1.0), 0.5, &flat);
    }
    assert!(c.camera(1.0, 1.3, 0.1).view_projection().iter().all(|v| v.is_finite()));
}

fn cloud(n: usize, around: Vec3, spread: f64, radius: (f64, f64), seed: u64) -> Vec<Instance> {
    let mut rng = SplitMix64(seed);
    (0..n)
        .map(|_| Instance {
            position: PlanetFixed(around + Vec3::new(rng.range(-spread, spread), rng.range(-spread, spread), rng.range(-spread, spread))),
            radius_m: rng.range(radius.0, radius.1) as f32,
        })
        .collect()
}

fn view() -> Camera {
    Camera::at_surface_point(PlanetFixed(Vec3::new(R + 5.0, 0.0, 0.0)), Vec3::new(0.0, 0.0, 1.0), 0.2, 1.0, 1.5, 0.1)
}

// spec: REND-011
#[test]
fn instance_culling_is_conservative_and_tight_on_the_side_planes() {
    let cam = view();
    let fr = cam.frustum();
    // Instances all around the camera, so a good share are near plane boundaries relative to their radius.
    let instances = cloud(6000, cam.position.0, 800.0, (0.5, 60.0), 9);
    let visible = cull_instances(&fr, &instances);
    assert!(visible.windows(2).all(|w| w[0] < w[1]), "ascending and unique");
    let set: std::collections::BTreeSet<u32> = visible.iter().copied().collect();
    let (mut kept_outside, mut culled_near) = (0, 0);
    for (k, i) in instances.iter().enumerate() {
        let r = f64::from(i.radius_m);
        let rel = i.position.0 - fr.origin;
        let dist: Vec<f64> = fr.normals.iter().map(|n| rel.dot(*n)).collect();
        let touches = dist.iter().all(|d| *d >= -r);
        assert_eq!(set.contains(&(k as u32)), touches, "instance {k}: plane distances {dist:?}, radius {r}");
        // Tightness: a sphere strictly inside one plane's half-space by more than its radius is never culled by it.
        if !touches {
            assert!(dist.iter().any(|d| *d < -r));
            culled_near += usize::from(dist.iter().any(|d| *d < 0.0 && *d >= -2.0 * r));
        } else if dist.iter().any(|d| *d < 0.0) {
            kept_outside += 1; // outside a plane by less than its radius: kept because it still touches the frustum
        }
    }
    assert!(
        kept_outside > 20 && culled_near > 20,
        "the scene must exercise the boundary: {kept_outside} kept-outside, {culled_near} culled-near"
    );
    assert!(!visible.is_empty() && visible.len() < instances.len());
}

// spec: REND-011
#[test]
fn the_radius_decides_borderline_instances_and_growing_radii_only_add_instances() {
    let cam = view();
    let fr = cam.frustum();
    // A centre 10 m outside the left plane, straight ahead otherwise.
    let ahead = cam.position.0 + cam.forward * 200.0;
    let left_normal = fr.normals[0];
    let rel = ahead - fr.origin;
    let to_plane = rel.dot(left_normal); // distance from the plane (positive = inside)
    let outside = ahead - left_normal * (to_plane + 10.0); // exactly 10 m outside the plane
    let at = |radius_m: f32| Instance { position: PlanetFixed(outside), radius_m };
    assert!(cull_instances(&fr, &[at(5.0)]).is_empty(), "radius 5 m < 10 m outside: culled");
    assert_eq!(cull_instances(&fr, &[at(15.0)]), [0], "radius 15 m > 10 m outside: kept");
    let small = cloud(2000, cam.position.0, 800.0, (1.0, 5.0), 4);
    let big: Vec<Instance> = small.iter().map(|i| Instance { radius_m: i.radius_m * 4.0, ..*i }).collect();
    let (a, b) = (cull_instances(&fr, &small), cull_instances(&fr, &big));
    assert!(a.iter().all(|k| b.contains(k)) && b.len() > a.len(), "bigger spheres keep a superset ({} -> {})", a.len(), b.len());
}

// spec: REND-011
#[test]
fn indirect_arguments_and_instance_positions_are_exact() {
    let cam = view();
    let instances = cloud(500, cam.position.0 + cam.forward * 300.0, 100.0, (1.0, 2.0), 5);
    let visible = cull_instances(&cam.frustum(), &instances);
    assert!(visible.len() > 100);
    let args = indirect_args(36, &visible);
    assert_eq!(
        (args.index_count, args.instance_count, args.first_index, args.base_vertex, args.first_instance),
        (36, visible.len() as u32, 0, 0, 0)
    );
    let rel = camera_relative_positions(cam.position, &instances, &visible);
    assert_eq!(rel.len(), visible.len());
    for (slot, &k) in visible.iter().enumerate() {
        let exact = instances[k as usize].position.0 - cam.position.0;
        assert_eq!(rel[slot], [exact.x as f32, exact.y as f32, exact.z as f32], "slot {slot} must hold instance {k}");
    }
}
