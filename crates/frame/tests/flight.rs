//! TEST-013: the scripted flight used for the real-GPU jitter review (tier D) is deterministic and well-formed.

use planet_frame::flight_path;
use planet_frame::terrain::{FLIGHT_END_M, FLIGHT_GLIDE_STEP_M, FLIGHT_START_M};
use planet_generators::height::height_at;

const R: f64 = 6_371_000.0;

fn altitude(c: &planet_frame::Camera) -> f64 {
    let p = c.position.0;
    p.length() - R - height_at(1, p.normalized())
}

// spec: TEST-013
#[test]
fn flight_dives_from_orbit_to_one_metre_then_glides_in_small_steps() {
    let frames = 200;
    let path = flight_path(R, 16.0 / 9.0, 1, frames);
    assert_eq!(path.len(), frames);
    assert_eq!(path, flight_path(R, 16.0 / 9.0, 1, frames), "the path is deterministic");
    let dive = frames * 4 / 5;
    assert!((altitude(&path[0]) - FLIGHT_START_M).abs() < 1e-3 * FLIGHT_START_M, "start {}", altitude(&path[0]));
    for w in path[..dive].windows(2) {
        assert!(altitude(&w[1]) < altitude(&w[0]), "the dive descends every frame");
    }
    for (i, c) in path[dive - 1..].iter().enumerate() {
        assert!((altitude(c) - FLIGHT_END_M).abs() < 1e-6, "frame {}: altitude {} m", dive - 1 + i, altitude(c));
    }
    for w in path[dive..].windows(2) {
        let step = (w[1].position.0 - w[0].position.0).length();
        assert!(step < 2.0 * FLIGHT_GLIDE_STEP_M, "glide step {step} m");
        assert!(step > 0.5 * FLIGHT_GLIDE_STEP_M, "glide step {step} m");
    }
}

// spec: TEST-013
#[test]
#[should_panic(expected = "at least 10 frames")]
fn a_flight_needs_at_least_ten_frames() {
    flight_path(R, 1.0, 1, 9);
}
