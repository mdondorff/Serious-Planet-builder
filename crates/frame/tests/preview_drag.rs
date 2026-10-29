//! REND-033, the drag: a hill dragged in three steps (A0, A1, B). A tile refreshed at the middle state (it holds the hill at A1) is flagged while the
//! plan is older and gets the delta from the oldest state to the current one, `-hill(A0) + hill(B)`: the heights of such a tile are not the heights of
//! the current state. A tile still at the oldest state is.

use planet_core::Vec3;
use planet_frame::preview::{preview_delta, PreviewStamp};
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const REACH: f64 = 400.0;
const AMOUNT: f64 = 60.0;

fn east(m: f64) -> Vec3 {
    let a = m / R;
    Vec3::new(libm::cos(a), libm::sin(a), 0.0)
}

fn arc(a: Vec3, b: Vec3) -> f64 {
    R * libm::atan2(a.cross(b).length(), a.dot(b))
}

fn items_at(m: f64) -> Vec<FootprintItem> {
    let mut d = WorldDefinition::single_planet(R, 1);
    let spec = ItemSpec { id: 1, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir: east(m), content_hash: 0, reach_m: REACH, max_delta_m: 100.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [AMOUNT, 0.0, 0.0]));
    d.put_item_spec(BodyId::PLANET, spec).unwrap();
    d.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre: east(0.0), angle: core::f64::consts::PI }).unwrap()
}

/// The height of a world with the hill at `m` metres east, at `p` (the hill profile of GEN-026).
fn hill_at(m: f64, p: Vec3) -> f64 {
    let x = arc(east(m), p) / REACH;
    if x >= 1.0 { 0.0 } else { AMOUNT * (1.0 - x * x) * (1.0 - x * x) }
}

fn delta_at(stamps: &[PreviewStamp], p: Vec3) -> f64 {
    stamps
        .iter()
        .map(|s| {
            let x = arc(s.dir, p) / s.reach_m;
            if x >= 1.0 { 0.0 } else { f64::from(s.sign) * s.amount_m * (1.0 - x * x) * (1.0 - x * x) }
        })
        .sum()
}

// spec: REND-033
#[test]
fn a_tile_refreshed_at_the_middle_state_of_a_drag_plus_the_delta_from_the_oldest_state_is_the_current_state() {
    let (a0, a1, b) = (0.0, 300.0, 500.0);
    // The delta the editor sets: from the oldest state (hill at A0) to the current one (hill at B); the middle state is not in it.
    let delta = preview_delta(&items_at(a0), &items_at(b), east(0.0)).stamps;
    let probes = [("A0", a0), ("A1", a1), ("B", b)];
    let mut worst = 0.0f64;
    for (what, m) in probes {
        let p = east(m);
        let exact = hill_at(b, p);
        let oldest_tile = hill_at(a0, p) + delta_at(&delta, p);
        let middle_tile = hill_at(a1, p) + delta_at(&delta, p);
        println!("{what}: exact {exact:.2} m, tile at the oldest state + delta {oldest_tile:.2} m, tile at the middle state + delta {middle_tile:.2} m");
        assert!((oldest_tile - exact).abs() < 1e-6, "{what}: a tile at the oldest state plus the delta is the current state");
        worst = worst.max((middle_tile - exact).abs());
    }
    assert!(worst < 0.5, "a tile refreshed at the middle state plus the delta is off the current state by up to {worst:.2} m");
}
