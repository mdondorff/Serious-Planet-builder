//! LOD-003, tier B: complete coverage over the ten scripted views and the review's hard cameras. Moved out of `lod.rs`
//! unchanged (each samples 3,000 directions per camera, about 9-11 s under load) so that `cargo xtask test fast` stays quick;
//! it still runs in `cargo xtask test` and in CI. The cheap LOD-003 tests (balancing, coverage of hand-built trees) stay in
//! `lod.rs`.

use planet_core::PlanetFixed;

#[path = "support/lod.rs"]
mod support;
use support::*;

// spec: LOD-003
#[test]
fn leaves_cover_the_visible_surface_exactly_once() {
    for (name, c) in views() {
        assert_covers(name, c);
        let area: u128 = select(c).nodes.iter().map(|n| 1u128 << (2 * (28 - u32::from(n.tile.level())))).sum();
        assert!(area < 6u128 << 56, "{name}: horizon culling must drop something");
    }
}

// spec: LOD-003
#[test]
fn culling_stays_conservative_for_known_hard_cameras() {
    // Regression cameras from review: a tile whose chord radius underestimated its angular radius was culled while still visible.
    assert_covers("review-random", camera(-86.427, 127.912, 673.0));
    for angle in [60.0f64, 70.0, 80.0, 89.0] {
        // On the great circle from the +Z face centre through its (+X,+Y) corner, beyond the corner.
        let a = angle.to_radians();
        let corner = planet_core::Vec3::new(1.0, 1.0, 1.0).normalized();
        let dir = planet_core::Vec3::new(0.0, 0.0, 1.0) * libm::cos(a)
            + (corner - planet_core::Vec3::new(0.0, 0.0, corner.z)).normalized() * libm::sin(a);
        for h in [5_000.0, 100_000.0, 1_000_000.0] {
            assert_covers(&format!("beyond-corner {angle} deg {h} m"), PlanetFixed(dir * (6_371_000.0 + h)));
        }
    }
    // A camera below the reference sphere is treated as sitting on it: still covered, and within the node budget.
    for h in [-1.0, -400.0, -10_000.0] {
        let c = camera(20.0, 30.0, h);
        assert_covers("below-surface", c);
        assert!(select(c).nodes.len() < 8000, "below-surface camera at {h} m selects {} nodes", select(c).nodes.len());
    }
}
