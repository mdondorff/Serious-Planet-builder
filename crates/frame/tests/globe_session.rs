//! Tier A: the overview as the editor window runs it (VIEW-013): the session over a camera's point, the click on a marker or on bare
//! surface, the flight it starts, and the hand-over to the free camera.

use planet_core::Vec3;
use planet_frame::globe::{
    landing, visible_markers, wheel_zoom_factor, Click, ClickTracker, GlobeSession, GlobeView, Marker, Viewport, CLICK_SLOP_PX,
    HIT_RADIUS_PX, LANDING_ALTITUDE_M, LANDING_PITCH_RAD, OVERVIEW_ALTITUDE_RADII,
};
use planet_frame::pick::PickLimits;

const R: f64 = 6_371_000.0;
const FOV: f64 = 0.8;
const VP: Viewport = Viewport { width_px: 1600.0, height_px: 900.0 };

fn dir(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

fn angle(a: Vec3, b: Vec3) -> f64 {
    libm::atan2(a.cross(b).length(), a.dot(b))
}

fn flat(_: Vec3) -> f64 {
    0.0
}

fn cam(s: &GlobeSession) -> planet_frame::camera::Camera {
    s.view.camera(R, FOV, VP.width_px / VP.height_px, 100.0)
}

// spec: VIEW-013
#[test]
fn the_overview_starts_over_the_cameras_point_at_one_and_a_half_radii() {
    let s = GlobeSession::enter(dir(30.0, 40.0) * (R + 500.0), R);
    assert!(angle(s.view.center, dir(30.0, 40.0)) < 1e-9);
    assert!((s.view.altitude_m - OVERVIEW_ALTITUDE_RADII * R).abs() < 1e-6);
    assert!(!s.flying());
}

// spec: VIEW-013
#[test]
fn a_click_on_a_marker_starts_a_flight_that_ends_over_it_and_hands_over_there() {
    let marker = Marker { id: 5, dir: dir(10.0, 12.0), radius_m: 3_000.0 };
    let mut s = GlobeSession::enter(dir(10.0, 10.0) * (R + 1.0), R);
    let c = cam(&s);
    let at = visible_markers(&c, &[marker], R, VP)[0].pixel;
    let limits = PickLimits::new(R, 9_000.0);
    let clicked = s.click([at[0] + 3.0, at[1] + 2.0], &c, VP, &[marker], &limits, &flat);
    assert_eq!(clicked, Some(Click::Marker(5)));
    assert!(s.flying());
    // While flying: no drag, no zoom, no second click.
    let before = s.view;
    s.drag(50.0, 50.0, FOV, VP.height_px);
    s.zoom(2.0);
    assert_eq!(s.view, before);
    assert_eq!(s.click([800.0, 450.0], &c, VP, &[marker], &limits, &flat), None);
    // The flight runs to its end and returns the closing view of the marker exactly once.
    let mut ended = None;
    let mut steps = 0;
    while ended.is_none() && steps < 10_000 {
        ended = s.step(1.0 / 60.0);
        steps += 1;
    }
    let end = ended.expect("the flight ends");
    assert_eq!(end, GlobeView::closing_on(&marker));
    assert!(!s.flying());
    assert_eq!(s.step(1.0), None, "nothing more after the end");
    assert_eq!(end.altitude_m, 12_000.0);
    let (position, _, pitch) = landing(&end, R);
    assert!((position.0.length() - (R + 12_000.0)).abs() < 1e-3 && angle(position.0, marker.dir) < 1e-9);
    assert_eq!(pitch, -LANDING_PITCH_RAD);
}

// spec: VIEW-013
#[test]
fn a_click_on_bare_surface_flies_to_a_close_look_over_the_spot_and_a_click_on_the_sky_does_nothing() {
    let mut s = GlobeSession::enter(dir(0.0, 0.0) * (R + 1.0), R);
    let c = cam(&s);
    let limits = PickLimits::new(R, 9_000.0);
    // The middle of the screen is the point under the camera.
    let Some(Click::Surface(spot)) = s.click([800.0, 450.0], &c, VP, &[], &limits, &flat) else { panic!("a surface click") };
    assert!(angle(spot, s.view.center) < 1e-9, "the middle pixel is the centre point");
    assert!(s.flying());
    let mut end = None;
    for _ in 0..10_000 {
        if let Some(v) = s.step(1.0 / 60.0) {
            end = Some(v);
            break;
        }
    }
    let end = end.expect("the flight ends");
    assert!(angle(end.center, spot) < 1e-9 && end.altitude_m == LANDING_ALTITUDE_M);
    // A corner of the screen beyond the planet's limb at 1.5 radii: the sky.
    let mut s = GlobeSession::enter(dir(0.0, 0.0) * (R + 1.0), R);
    let c = cam(&s);
    assert_eq!(s.click([2.0, 2.0], &c, VP, &[], &limits, &flat), None);
    assert!(!s.flying());
}

// spec: VIEW-013
#[test]
fn a_drag_and_the_wheel_move_the_view_when_nothing_flies_and_the_click_constants_are_what_the_spec_says() {
    let mut s = GlobeSession::enter(dir(5.0, 5.0) * (R + 1.0), R);
    let (c0, a0) = (s.view.center, s.view.altitude_m);
    s.drag(30.0, -20.0, FOV, VP.height_px);
    s.zoom(0.5);
    assert!(angle(s.view.center, c0) > 1e-4 && (s.view.altitude_m - 0.5 * a0).abs() < 1e-6);
    assert_eq!((HIT_RADIUS_PX, CLICK_SLOP_PX), (12.0, 4.0));
}

// spec: VIEW-013
#[test]
fn a_press_released_within_four_pixels_is_a_click_and_any_other_is_a_drag_and_a_stale_press_can_be_cancelled() {
    let mut t = ClickTracker::default();
    assert!(!t.pressed() && !t.moved_by(5.0), "nothing held: a motion is no drag");
    assert!(!t.release(), "a release without a press is no click");
    // A click: a press, a little motion, a release.
    t.press();
    assert!(t.pressed() && t.moved_by(1.5) && t.moved_by(1.5));
    assert!(t.release() && !t.pressed());
    // A drag: the motion adds up past the slop.
    t.press();
    assert!(t.moved_by(3.0) && t.moved_by(3.0));
    assert!(!t.release(), "6 px is a drag");
    // Exactly the slop is a drag; just under it is a click.
    t.press();
    t.moved_by(CLICK_SLOP_PX);
    assert!(!t.release());
    t.press();
    t.moved_by(CLICK_SLOP_PX - 0.01);
    assert!(t.release());
    // A cancelled press (the overview toggled, the focus lost, a release not seen): the next motion drags nothing and the next release is no click.
    t.press();
    t.cancel();
    assert!(!t.moved_by(10.0) && !t.release() && !t.pressed());
}

// spec: VIEW-013
#[test]
fn each_wheel_line_brings_the_camera_ten_percent_nearer() {
    assert_eq!(wheel_zoom_factor(0.0), 1.0);
    assert!((wheel_zoom_factor(1.0) - 0.9).abs() < 1e-15);
    assert!((wheel_zoom_factor(-1.0) - 1.0 / 0.9).abs() < 1e-12, "a line the other way goes farther");
    assert!((wheel_zoom_factor(3.0) - 0.729).abs() < 1e-12);
    let mut s = GlobeSession::enter(dir(0.0, 0.0) * (R + 1.0), R);
    let a0 = s.view.altitude_m;
    s.zoom(wheel_zoom_factor(2.0));
    assert!((s.view.altitude_m - 0.81 * a0).abs() < 1e-6);
}
