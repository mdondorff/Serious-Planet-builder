//! Tier A: the globe overview (VIEW-013): the orbit camera, the markers on the screen, the click and the flight to a place.

use planet_core::Vec3;
use planet_frame::globe::{
    faces_camera, marker_at, project, visible_markers, GlobeFlight, GlobeView, Marker, ScreenMarker, Viewport, MAX_ALTITUDE_RADII,
    MIN_ALTITUDE_M,
};

const R: f64 = 6_371_000.0;
const FOV: f64 = 0.8;
const VP: Viewport = Viewport { width_px: 1600.0, height_px: 900.0 };

fn dir(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

fn camera(view: &GlobeView) -> planet_frame::camera::Camera {
    view.camera(R, FOV, VP.width_px / VP.height_px, 100.0)
}

fn angle(a: Vec3, b: Vec3) -> f64 {
    libm::atan2(a.cross(b).length(), a.dot(b))
}

// spec: VIEW-013
#[test]
fn the_view_looks_at_the_planet_from_over_its_centre_point_with_north_up() {
    for centre in [dir(0.0, 0.0), dir(48.0, 11.0), dir(-33.0, 151.0), dir(90.0, 0.0), dir(-90.0, 30.0)] {
        let view = GlobeView::new(centre, 3.0 * R);
        let cam = camera(&view);
        assert!((cam.position.0.length() - 4.0 * R).abs() < 1e-3);
        let px = project(&cam, centre * R, VP).expect("the centre point is on screen");
        assert!((px[0] - 800.0).abs() < 1e-6 && (px[1] - 450.0).abs() < 1e-6, "{px:?}");
    }
    // North is up on the screen: a point north of the centre is above it.
    let view = GlobeView::new(dir(0.0, 0.0), 3.0 * R);
    let cam = camera(&view);
    let north = project(&cam, dir(10.0, 0.0) * R, VP).unwrap();
    let east = project(&cam, dir(0.0, 10.0) * R, VP).unwrap();
    assert!(north[1] < 450.0 && (north[0] - 800.0).abs() < 1e-6, "{north:?}");
    assert!(east[0] > 800.0 && (east[1] - 450.0).abs() < 1e-6, "{east:?}");
}

// spec: VIEW-013
#[test]
fn a_marker_is_seen_only_on_the_near_side_of_the_horizon_and_inside_the_window() {
    let altitude = 1.0 * R;
    let view = GlobeView::new(dir(0.0, 0.0), altitude);
    let cam = camera(&view);
    // The horizon is at the angle acos(R / (R + h)) = 60 degrees from the centre point.
    let horizon_deg = libm::acos(R / (R + altitude)).to_degrees();
    assert!((horizon_deg - 60.0).abs() < 1e-9);
    assert!(faces_camera(cam.position.0, dir(0.0, 59.0), R));
    assert!(!faces_camera(cam.position.0, dir(0.0, 61.0), R));
    assert!(!faces_camera(cam.position.0, dir(0.0, 180.0), R), "the far side");
    let markers = [
        Marker { id: 1, dir: dir(0.0, 0.0), radius_m: 1000.0 },
        Marker { id: 2, dir: dir(0.0, 180.0), radius_m: 1000.0 },
        Marker { id: 3, dir: dir(20.0, 20.0), radius_m: 1000.0 },
        Marker { id: 4, dir: dir(0.0, 65.0), radius_m: 1000.0 },
    ];
    let seen = visible_markers(&cam, &markers, R, VP);
    let ids: Vec<u64> = seen.iter().map(|m| m.id).collect();
    assert_eq!(ids, [1, 3], "the far side and the one past the horizon are not shown");
    assert!((seen[0].pixel[0] - 800.0).abs() < 1e-6 && (seen[0].pixel[1] - 450.0).abs() < 1e-6);
}

// spec: VIEW-013
#[test]
fn the_marker_under_the_cursor_is_the_nearest_within_the_hit_radius_and_ties_go_to_the_lower_id() {
    let seen = [
        ScreenMarker { id: 9, pixel: [100.0, 100.0] },
        ScreenMarker { id: 4, pixel: [110.0, 100.0] },
        ScreenMarker { id: 2, pixel: [90.0, 100.0] },
        ScreenMarker { id: 7, pixel: [500.0, 500.0] },
    ];
    assert_eq!(marker_at(&seen, [108.0, 100.0], 12.0), Some(4));
    assert_eq!(marker_at(&seen, [100.0, 100.0], 12.0), Some(9));
    assert_eq!(marker_at(&seen, [100.0, 130.0], 12.0), None);
    assert_eq!(marker_at(&seen, [500.0, 512.0], 12.0), Some(7), "the radius is inclusive");
    assert_eq!(marker_at(&[], [0.0, 0.0], 12.0), None);
    // Two at the same distance: the lower id.
    let tie = [ScreenMarker { id: 6, pixel: [90.0, 100.0] }, ScreenMarker { id: 3, pixel: [110.0, 100.0] }];
    assert_eq!(marker_at(&tie, [100.0, 100.0], 12.0), Some(3));
}

// spec: VIEW-013
#[test]
fn dragging_moves_the_surface_with_the_cursor_and_zoom_stays_inside_its_limits() {
    let mut view = GlobeView::new(dir(10.0, 20.0), 100_000.0);
    let before = camera(&view);
    let marker = view.center * R;
    view.drag(50.0, -30.0, R, FOV, VP.height_px);
    let after = camera(&view);
    // The point that was under the middle of the screen is now about 50 px right and 30 px up from the middle (a flat-surface
    // approximation: within 1 px at 100 km altitude).
    let px = project(&after, marker, VP).expect("still on screen");
    assert!((px[0] - 850.0).abs() < 1.0 && (px[1] - 420.0).abs() < 1.0, "{px:?}");
    assert!((before.position.0.length() - after.position.0.length()).abs() < 1e-3, "a drag keeps the altitude");
    assert!((view.center.length() - 1.0).abs() < 1e-12);
    // Zoom out and in past the limits.
    view.zoom(1.0e6, R);
    assert_eq!(view.altitude_m, MAX_ALTITUDE_RADII * R);
    view.zoom(1.0e-12, R);
    assert_eq!(view.altitude_m, MIN_ALTITUDE_M);
    view.zoom(2.0, R);
    assert_eq!(view.altitude_m, 2.0 * MIN_ALTITUDE_M);
}

// spec: VIEW-013
#[test]
fn a_drag_at_the_pole_keeps_the_view_valid() {
    let mut view = GlobeView::new(dir(90.0, 0.0), 1.0 * R);
    view.drag(40.0, 25.0, R, FOV, VP.height_px);
    assert!((view.center.length() - 1.0).abs() < 1e-12 && view.center.x.is_finite() && view.center.z.is_finite());
    let cam = camera(&view);
    assert!(project(&cam, view.center * R, VP).is_some());
}

// spec: VIEW-013
#[test]
fn a_flight_starts_and_ends_exactly_and_the_centre_moves_along_the_great_circle() {
    let from = GlobeView::new(dir(48.0, 11.0), 2.0 * R);
    let to = GlobeView::new(dir(-33.0, 151.0), 5_000.0);
    let flight = GlobeFlight::new(from, to, R);
    assert_eq!(flight.at(0.0), from);
    let end = flight.at(flight.duration_s);
    assert_eq!(end.altitude_m, 5_000.0);
    assert!(angle(end.center, to.center) < 1e-9);
    assert_eq!(flight.at(flight.duration_s + 10.0), end, "clamped past the end");
    assert_eq!(flight.at(-1.0), from, "clamped before the start");
    // Along the way: unit centres, the angle from the start only grows, and the arc is the great circle (angles add up).
    let mut last = 0.0;
    for i in 0..=100 {
        let v = flight.at(flight.duration_s * f64::from(i) / 100.0);
        assert!((v.center.length() - 1.0).abs() < 1e-12);
        let travelled = angle(from.center, v.center);
        assert!(travelled >= last - 1e-12, "step {i}");
        assert!((travelled + angle(v.center, to.center) - flight.angle_rad).abs() < 1e-9, "on the great circle at step {i}");
        assert!(v.altitude_m.is_finite() && v.altitude_m > 0.0);
        last = travelled;
    }
    assert!(flight.is_done(flight.duration_s) && !flight.is_done(0.0));
}

// spec: VIEW-013
#[test]
fn a_long_flight_rises_to_show_the_way_and_a_short_one_is_quick() {
    let near_ground = 2_000.0;
    let long = GlobeFlight::new(GlobeView::new(dir(0.0, 0.0), near_ground), GlobeView::new(dir(0.0, 120.0), near_ground), R);
    let middle = long.at(long.duration_s / 2.0);
    assert!(middle.altitude_m > 100.0 * near_ground, "a trip of 120 degrees rises: {}", middle.altitude_m);
    let short = GlobeFlight::new(GlobeView::new(dir(0.0, 0.0), 50_000.0), GlobeView::new(dir(0.0, 0.01), 50_000.0), R);
    assert!(short.duration_s < 1.0, "{}", short.duration_s);
    assert!(long.duration_s > short.duration_s);
    // Flat altitudes and the same place: nothing moves, nothing breaks.
    let same = GlobeFlight::new(GlobeView::new(dir(5.0, 5.0), 1.0e6), GlobeView::new(dir(5.0, 5.0), 1.0e6), R);
    let v = same.at(same.duration_s / 3.0);
    assert!(angle(v.center, dir(5.0, 5.0)) < 1e-9 && (v.altitude_m - 1.0e6).abs() < 1e-3);
}

// spec: VIEW-013
#[test]
fn a_flight_to_the_opposite_side_goes_round_and_stays_valid() {
    let flight = GlobeFlight::new(GlobeView::new(dir(0.0, 0.0), 1.0e6), GlobeView::new(dir(0.0, 180.0), 1.0e6), R);
    for i in 0..=20 {
        let v = flight.at(flight.duration_s * f64::from(i) / 20.0);
        assert!((v.center.length() - 1.0).abs() < 1e-12 && v.center.x.is_finite());
    }
    assert!(angle(flight.at(flight.duration_s).center, dir(0.0, 180.0)) < 1e-9);
}

// spec: VIEW-013
#[test]
fn a_click_on_a_marker_flies_to_a_view_four_footprints_up_and_never_below_the_minimum() {
    let big = Marker { id: 1, dir: dir(10.0, 10.0), radius_m: 5_000.0 };
    let small = Marker { id: 2, dir: dir(11.0, 11.0), radius_m: 10.0 };
    assert_eq!(GlobeView::closing_on(&big).altitude_m, 20_000.0);
    assert_eq!(GlobeView::closing_on(&small).altitude_m, MIN_ALTITUDE_M);
    assert!(angle(GlobeView::closing_on(&big).center, big.dir) < 1e-12);
    // From the overview: the marker is on screen, the click on it finds it, the flight ends over it.
    let overview = GlobeView::new(dir(10.0, 10.0), 2.0 * R);
    let cam = camera(&overview);
    let seen = visible_markers(&cam, &[big, small], R, VP);
    let at = seen.iter().find(|m| m.id == 1).unwrap().pixel;
    assert_eq!(marker_at(&seen, [at[0] + 3.0, at[1] - 4.0], 12.0), Some(1));
    let flight = GlobeFlight::new(overview, GlobeView::closing_on(&big), R);
    assert_eq!(flight.at(flight.duration_s).altitude_m, 20_000.0);
}
