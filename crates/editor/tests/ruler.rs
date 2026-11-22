use planet_core::surface::{DEG_TO_RAD, RAD_TO_DEG};
use planet_core::{PlanetFixed, Vec3};
use planet_editor::polyline::{self, GeoPoint};
use planet_editor::relevance::ViewState;
use planet_editor::ruler::{
    build_overlay, cursor_readout, local_gradient, RulerState, TransectProfile, DEFAULT_TRANSECT_SAMPLES, HEIGHTFIELD_SLOPE_LIMIT_DEG,
};
use planet_frame::globe::Viewport;
use planet_frame::Camera;

const R: f64 = 6_371_000.0;
const VIEWPORT: Viewport = Viewport { width_px: 1280.0, height_px: 800.0 };

fn eq_dir(east_m: f64) -> Vec3 {
    GeoPoint::new(0.0, east_m / R * RAD_TO_DEG).dir()
}

fn eq_pt(east_m: f64) -> GeoPoint {
    GeoPoint::new(0.0, east_m / R * RAD_TO_DEG)
}

fn camera_above(dir: Vec3, altitude_m: f64) -> Camera {
    Camera::at_surface_point(
        PlanetFixed(dir * (R + altitude_m)),
        Vec3::new(0.0, 0.0, 1.0),
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::FRAC_PI_3,
        16.0 / 10.0,
        1.0,
    )
}

// spec: VIEW-076
#[test]
fn cursor_readout_and_local_gradient_report_flat_45_degree_cliff_and_pole_safe_slopes() {
    // Flat terrain gives 0° slope and canonical coordinates.
    let eq = eq_dir(0.0);
    let flat = cursor_readout(eq, 0.0, R, &|_| 0.0);
    assert!((flat.slope_deg).abs() < 1e-9 && !flat.over_limit);
    assert!(flat.summary().contains("0.00000° N"));
    assert!(flat.summary().contains("slope 0.0°"));

    // Analytical 1:1 east-facing ramp near the equator yields 45.0° (grade 1.0).
    let ramp45 = |d: Vec3| -> f64 { R * libm::atan2(d.y, d.x) };
    let (_, slope45) = local_gradient(eq, R, 1.0, &ramp45);
    assert!((slope45 - 45.0).abs() < 0.01, "expected ~45.0°, got {slope45}");

    // Steeper ramp (1.3 m/m ~ 52.4°) crosses the ADR 0091 45° heightfield limit.
    let cliff = cursor_readout(eq, 500.0, R, &|d| 1.3 * R * libm::atan2(d.y, d.x));
    assert!(cliff.slope_deg > HEIGHTFIELD_SLOPE_LIMIT_DEG && cliff.over_limit);
    assert!(cliff.summary().contains("(>45°)"), "{}", cliff.summary());

    // Pole singularity check: smooth linear ramp in x at north pole stays finite and accurate.
    let pole = GeoPoint::new(90.0, 0.0).dir();
    let pole_readout = cursor_readout(pole, 120.0, R, &|d| 120.0 - 0.5 * R * d.x);
    let expected_deg = libm::atan(0.5) * RAD_TO_DEG;
    assert!((pole_readout.slope_deg - expected_deg).abs() < 0.05, "{pole_readout:?}");
    assert_eq!(pole_readout.point, GeoPoint::new(90.0, 0.0));
}

// spec: VIEW-076
#[test]
fn transect_profile_measures_geodesic_arc_chord_elevation_delta_and_flags_steep_segments() {
    // 10 km transect across a flat apron (0..4 km), a 55° scarp (4..6 km), and high plateau (6..10 km).
    // Wait: let slope over 4..6 km be tan(55°) ~ 1.428 m/m => rise = 2856 m over 2 km.
    let rise = 2_000.0 * libm::tan(55.0 * DEG_TO_RAD);
    let terrain = |d: Vec3| -> f64 {
        let east_m = d.angle_to(Vec3::new(1.0, 0.0, 0.0)) * R * if d.y >= 0.0 { 1.0 } else { -1.0 };
        if east_m <= 4_000.0 {
            100.0
        } else if east_m >= 6_000.0 {
            100.0 + rise
        } else {
            100.0 + (east_m - 4_000.0) / 2_000.0 * rise
        }
    };

    let start = eq_pt(0.0);
    let end = eq_pt(10_000.0);
    let p = TransectProfile::sample(start, end, R, DEFAULT_TRANSECT_SAMPLES, HEIGHTFIELD_SLOPE_LIMIT_DEG, &terrain);
    assert_eq!(p.samples.len(), DEFAULT_TRANSECT_SAMPLES);
    assert!((p.arc_distance_m - 10_000.0).abs() < 0.05, "arc={}", p.arc_distance_m);
    assert!(p.chord_distance_m > 0.0 && (p.delta_height_m - rise).abs() < 1e-3);
    assert!((p.min_height_m - 100.0).abs() < 1e-3);
    assert!((p.max_height_m - (100.0 + rise)).abs() < 1e-3);
    assert!(p.max_slope_deg > 50.0, "max_slope={}", p.max_slope_deg);
    assert!(p.steep_fraction > 0.1 && p.steep_fraction < 0.3, "steep_fraction={}", p.steep_fraction);

    // Midpoint scrub interpolation and antimeridian geodesic distance check.
    let mid = p.sample_at_fraction(0.5).unwrap();
    assert!((mid.distance_m - 5_000.0).abs() < 1.0);
    assert!((mid.height_m - (100.0 + 0.5 * rise)).abs() < 1.0);

    let wrap_arc = polyline::arc_m(GeoPoint::new(0.0, 179.95).dir(), GeoPoint::new(0.0, -179.95).dir(), R);
    let expected_wrap = 0.1 * DEG_TO_RAD * R;
    assert!((wrap_arc - expected_wrap).abs() < 1.0, "wrap_arc={wrap_arc}");

    // Overlay projection keeps visible steep intervals marked over_limit and drops far-side points.
    let view = ViewState { camera: camera_above(eq_pt(5_000.0).dir(), 25_000.0), viewport: VIEWPORT, radius_m: R };
    let ov = build_overlay(&p, &view, false, Some(0.5));
    assert!(ov.start.is_some() && ov.end.is_some() && ov.midpoint.is_some() && ov.scrub.is_some());
    assert_eq!(ov.segments.len(), DEFAULT_TRANSECT_SAMPLES - 1);
    assert!(ov.segments.iter().any(|s| s.over_limit) && ov.segments.iter().any(|s| !s.over_limit));

    let far = TransectProfile::sample(GeoPoint::new(0.0, -170.0), GeoPoint::new(0.0, -169.0), R, 9, HEIGHTFIELD_SLOPE_LIMIT_DEG, &|_| 0.0);
    let far_ov = build_overlay(&far, &view, false, None);
    assert!(far_ov.start.is_none() && far_ov.end.is_none() && far_ov.segments.is_empty());
}

// spec: VIEW-076
#[test]
fn ruler_state_transitions_rubber_band_and_swaps_cleanly() {
    let mut r = RulerState::Idle;
    assert!(r.is_idle() && !r.swap());

    let a = eq_dir(0.0);
    let b = eq_dir(2_500.0);
    assert!(r.click(a, 100.0));
    assert!(matches!(r, RulerState::Anchored { start_height_m, .. } if (start_height_m - 100.0).abs() < 1e-9));
    // Duplicate click on A is ignored.
    assert!(!r.click(a, 100.0));

    // Light rubber-band works without terrain callback.
    let rb = TransectProfile::rubber_band(eq_pt(0.0), 100.0, eq_pt(2_500.0), 600.0, R, HEIGHTFIELD_SLOPE_LIMIT_DEG);
    let view = ViewState { camera: camera_above(eq_pt(1_250.0).dir(), 15_000.0), viewport: VIEWPORT, radius_m: R };
    let ov = build_overlay(&rb, &view, true, None);
    assert!(ov.is_rubber_band && ov.segments.len() == 32);

    // Second distinct click pins B; swap reverses A and B; third click starts new A; clear resets.
    assert!(r.click(b, 600.0));
    let RulerState::Measured { start, end } = r else { panic!("expected Measured") };
    assert!(r.swap());
    assert_eq!(r, RulerState::Measured { start: end, end: start });
    assert!(r.click(eq_dir(5_000.0), 50.0));
    assert!(matches!(r, RulerState::Anchored { .. }));
    r.clear();
    assert!(r.is_idle());
}
