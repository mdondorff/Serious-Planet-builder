//! The prefetch source without a loop (STRM-018): the travel estimator, the prediction and the capped set, as pure functions
//! of the camera path on a fake clock. The loop tests (rotation, travel, share, direction change, teleport) are in
//! `managed.rs`.

use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::prefetch::{cap_to_free, prefetch_set, PrefetchConfig, TravelEstimator};
use planet_frame::ResidencyEntry;
use planet_frame::ResidencySet;

const R: f64 = 6_371_000.0;

fn at(dir: Vec3, height: f64) -> PlanetFixed {
    PlanetFixed(dir.normalized() * (R + height))
}

/// A straight track along the tangent at (0.3, 0.5, 0.8), metres from its start.
fn track(metres: f64, height: f64) -> PlanetFixed {
    let start = Vec3::new(0.3, 0.5, 0.8).normalized();
    let east = start.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
    at(start + east * (metres / R), height)
}

fn est() -> TravelEstimator {
    TravelEstimator::new(PrefetchConfig::default(), R)
}

// spec: STRM-018
#[test]
fn a_stationary_camera_has_no_velocity_and_no_prediction_whatever_its_view_does() {
    let mut e = est();
    for k in 0..50 {
        // The position is the only input; a turning view cannot reach the estimator at all.
        e.observe(k * 33, track(0.0, 800.0));
    }
    assert_eq!(e.velocity().map(|v| v.length()), Some(0.0));
    assert!(e.predict().is_none());
}

// spec: STRM-018
#[test]
fn straight_travel_is_extrapolated_by_the_look_ahead() {
    let mut e = est();
    // 300 m/s along the track.
    for k in 0..30 {
        e.observe(k * 33, track(k as f64 * 9.9, 800.0));
    }
    let v = e.velocity().unwrap().length();
    assert!((v - 300.0).abs() < 2.0, "velocity {v}");
    let p = e.predict().unwrap();
    let here = track(29.0 * 9.9, 800.0);
    let ahead = (p.0 - here.0).length();
    assert!((ahead - 600.0).abs() < 15.0, "2 s at 300 m/s is 600 m, got {ahead}");
    // Deterministic: the same path gives the same prediction.
    let mut e2 = est();
    for k in 0..30 {
        e2.observe(k * 33, track(k as f64 * 9.9, 800.0));
    }
    assert_eq!(e2.predict(), Some(p));
}

// spec: STRM-018
#[test]
fn the_prediction_is_capped_in_distance_and_ignores_slow_drift() {
    let mut fast = est();
    for k in 0..30 {
        fast.observe(k * 33, track(k as f64 * 330.0, 800.0)); // 10 km/s, below the teleport step (500 m + 10 % of altitude)... 330 m a frame
    }
    // 330 m a frame is not a teleport at 800 m altitude (limit 580 m); 2 s at 10 km/s would be 20 km, capped at 50 km: no cap yet.
    assert_eq!(fast.teleports(), 0);
    let mut capped = TravelEstimator::new(PrefetchConfig { max_distance_m: 5_000.0, ..PrefetchConfig::default() }, R);
    for k in 0..30 {
        capped.observe(k * 33, track(k as f64 * 330.0, 800.0));
    }
    let here = track(29.0 * 330.0, 800.0);
    let d = (capped.predict().unwrap().0 - here.0).length();
    assert!((d - 5_000.0).abs() < 1.0, "capped to 5 km, got {d}");
    let mut slow = est();
    for k in 0..30 {
        slow.observe(k * 33, track(k as f64 * 0.01, 800.0)); // 0.3 m/s
    }
    assert!(slow.predict().is_none(), "below the minimum speed");
}

// spec: STRM-018
#[test]
fn a_descent_is_not_extrapolated_below_half_its_altitude() {
    let mut e = est();
    // Geometric descent, 7 % a frame (under the teleport step of 10 % of the altitude).
    for k in 0..20 {
        e.observe(k * 33, track(0.0, 20_000.0 * libm::pow(0.93, k as f64)));
    }
    assert_eq!(e.teleports(), 0);
    let h_now = 20_000.0 * libm::pow(0.93, 19.0);
    let p = e.predict().unwrap();
    let h_pred = p.0.length() - R;
    assert!(h_pred >= 0.5 * h_now - 1e-6 && h_pred > 0.0, "predicted altitude {h_pred} from {h_now}");
}

// spec: STRM-018
#[test]
fn a_teleport_clears_the_history_and_travel_resumes_from_the_new_place() {
    let mut e = est();
    for k in 0..20 {
        e.observe(k * 33, track(k as f64 * 9.9, 800.0));
    }
    assert!(e.predict().is_some());
    let far = at(Vec3::new(-0.6, -0.2, -0.5), 300.0);
    assert!(e.observe(20 * 33, far), "the jump is a teleport");
    assert_eq!(e.teleports(), 1);
    assert!(e.velocity().is_none() && e.predict().is_none(), "nothing is extrapolated from the old direction");
    // A few frames standing: zero velocity. Moving again: a prediction from the new place only.
    for k in 21..30 {
        e.observe(k * 33, far);
    }
    assert!(e.predict().is_none());
    let dir = Vec3::new(-0.6, -0.2, -0.5).normalized();
    let east = dir.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
    for k in 30..60 {
        e.observe(k * 33, at(dir + east * ((k - 30) as f64 * 9.9 / R), 300.0));
    }
    let p = e.predict().unwrap();
    assert!((p.0 - far.0).length() < 2_000.0, "the prediction is near the new place");
}

// spec: STRM-018
#[test]
fn old_samples_leave_the_window_so_a_stopped_camera_stops_predicting() {
    let mut e = est();
    for k in 0..20 {
        e.observe(k * 33, track(k as f64 * 9.9, 800.0));
    }
    assert!(e.predict().is_some());
    let stop = track(19.0 * 9.9, 800.0);
    for k in 20..50 {
        e.observe(k * 33, stop);
    }
    assert!(e.predict().is_none(), "400 ms after stopping");
}

fn set(entries: &[(u32, u64, f64)]) -> ResidencySet {
    // (level, index, distance): TileId::new(face 0, level, x, y) is not needed: tiles are built from raw ids.
    let mut tiles: Vec<ResidencyEntry> =
        entries.iter().map(|&(level, i, d)| ResidencyEntry { tile: tile(level, i), leaf: true, distance_m: d }).collect();
    tiles.sort_by_key(|e| e.tile);
    ResidencySet { tiles }
}

fn tile(level: u32, i: u64) -> TileId {
    TileId::new(planet_core::Face(0), level as u8, i as u32, i as u32).unwrap()
}

// spec: STRM-018
#[test]
fn the_set_is_ahead_minus_here_coarse_before_fine_then_nearer() {
    let here = set(&[(1, 0, 5.0), (3, 0, 5.0)]);
    let ahead = set(&[(1, 0, 6.0), (2, 1, 90.0), (2, 2, 40.0), (3, 0, 7.0), (4, 3, 10.0), (4, 4, 5.0)]);
    let out = prefetch_set(&here, &ahead);
    let got: Vec<(u8, f64)> = out.iter().map(|(t, d)| (t.level(), *d)).collect();
    assert_eq!(got, vec![(2, 40.0), (2, 90.0), (4, 5.0), (4, 10.0)], "tiles R already holds are not prefetch; order level then distance");
}

// spec: STRM-018
#[test]
fn the_cap_takes_resident_tiles_free_and_missing_ones_only_into_free_slots() {
    let s: Vec<(TileId, f64)> = (0..6).map(|i| (tile(3, i), i as f64)).collect();
    let resident = |t: TileId| t == tile(3, 1) || t == tile(3, 4);
    let got: Vec<TileId> = cap_to_free(&s, 2, resident).iter().map(|x| x.0).collect();
    assert_eq!(got, vec![tile(3, 0), tile(3, 1), tile(3, 2), tile(3, 4)], "two missing tiles (the nearest) plus the resident ones");
    let none: Vec<TileId> = cap_to_free(&s, 0, resident).iter().map(|x| x.0).collect();
    assert_eq!(none, vec![tile(3, 1), tile(3, 4)], "a full share asks for no missing tile");
}
