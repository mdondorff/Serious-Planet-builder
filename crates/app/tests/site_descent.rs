//! VIEW-022, tier A: `editor --edit --site LAT,LON` descends over the site first (the owner's bug of 2026-10-30: the window opened at the
//! default view with no descent). The scripted flight starts high above the site and lands over it; the editor mode, which edits from
//! the free camera, turns on where the descent ends.

use planet::window::descent_finished;
use planet_core::Vec3;
use planet_frame::scenes::{build_script_at, StreamScene, FRAME_STEP_S};
use planet_frame::Camera;
use planet_generators::height::Reference;

const R: f64 = 6_371_000.0;

// spec: VIEW-022
#[test]
fn a_site_descent_starts_high_over_the_site_and_the_editor_mode_waits_for_its_end() {
    let site = (45.0f64, -100.0f64);
    let script = build_script_at(site, StreamScene::Dive, R, 16.0 / 9.0, &Reference(1), 7);
    let (first, last) = (script.cameras[0], script.cameras[script.cameras.len() - 1]);
    let site_dir = Vec3::new(
        libm::cos(site.0.to_radians()) * libm::cos(site.1.to_radians()),
        libm::cos(site.0.to_radians()) * libm::sin(site.1.to_radians()),
        libm::sin(site.0.to_radians()),
    );
    let height = |c: &Camera| c.position.0.length() - R;
    assert!(
        height(&first) > 100.0 * height(&last),
        "the flight starts far above where it ends: {} m and {} m",
        height(&first),
        height(&last)
    );
    assert!(height(&last) < 2_000.0, "it ends near the ground: {} m", height(&last));
    let over_site = |c: &Camera| c.position.0.normalized().dot(site_dir);
    assert!(over_site(&last) > 0.999_99, "it lands over the site (cosine {})", over_site(&last));
    // The editor mode waits: not at the start, not while the descent plays, yes from its last camera.
    let n = script.cameras.len();
    assert!(!descent_finished(0.0, n));
    assert!(!descent_finished((n as f64 - 2.0) * FRAME_STEP_S, n));
    assert!(descent_finished((n as f64 - 1.0) * FRAME_STEP_S, n));
    assert!(descent_finished(1.0e6, n));
    assert!(descent_finished(0.0, 1), "a one-camera script has nothing to wait for");
}
