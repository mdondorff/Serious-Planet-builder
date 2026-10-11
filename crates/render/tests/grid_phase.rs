//! Tier A: the relief grid is anchored to the world, not to a tile (REND-006). Two tiles with different origins must
//! agree on where a grid line is for the same world point, at every decade the shader draws.

use planet_render::terrain::grid_phases;

// spec: REND-006
#[test]
fn tiles_with_different_origins_place_grid_lines_at_the_same_world_positions() {
    // Origins at planet-radius magnitude, with fractions that f32 could not hold.
    let a = [4_523_117.318_276_5_f64, -2_012_345.987_654_3, 3_141_592.653_589_8];
    let b = [a[0] + 12.34567891, a[1] - 0.00098765, a[2] + 5_000.5];
    let (fa, ca) = grid_phases(a);
    let (fb, cb) = grid_phases(b);
    let wrap = |d: f64, period: f64| d.abs().min(period - d.abs());
    for axis in 0..3 {
        // A world point 3.7 m from origin a, in each tile's local coordinates.
        let world = a[axis] + 3.7;
        let (la, lb) = ((world - a[axis]) as f32, (world - b[axis]) as f32);
        // Fine decades (1 cm .. 1 m): position modulo 1 m.
        let (xa, xb) = (f64::from(la + fa[axis]).rem_euclid(1.0), f64::from(lb + fb[axis]).rem_euclid(1.0));
        // The tile-local coordinate itself is an f32 (about 0.5 mm at 5 km from its origin), hence 1 mm.
        assert!(wrap(xa - xb, 1.0) < 1e-3, "axis {axis}: fine phase differs by {} m between tiles", xa - xb);
        assert!(wrap(xa - world.rem_euclid(1.0), 1.0) < 1e-3, "axis {axis}: fine phase is not the world coordinate");
        // Coarse decades (10 m .. 1,000 km): position modulo 1,000 km.
        let (ya, yb) = (f64::from(la + ca[axis]).rem_euclid(1.0e6), f64::from(lb + cb[axis]).rem_euclid(1.0e6));
        assert!(wrap(ya - yb, 1.0e6) < 0.1, "axis {axis}: coarse phase differs by {} m between tiles", ya - yb);
    }
}

// spec: REND-006
#[test]
fn phases_are_small_remainders_never_planet_radius_values() {
    let (f, c) = grid_phases([6_371_000.25, -6_371_000.75, 12.5]);
    assert!(f.iter().all(|v| (0.0..1.0).contains(v)), "{f:?}");
    assert!(c.iter().all(|v| (0.0..1.0e6).contains(v)), "{c:?}");
    assert_eq!(f[0], 0.25);
    assert_eq!(f[1], 0.25);
}
