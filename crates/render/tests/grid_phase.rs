//! Tier A: the relief grid is anchored to the world, not to a tile, and its line coordinates stay small at every decade
//! (REND-006). Two tiles with different origins must agree on where a grid line is for the same world point, and the
//! shader's f32 arithmetic (tile-local offset in cells plus the node's phase) must resolve a line to well under a pixel.

use planet_render::terrain::{grid_phases, GRID_SPACINGS_M};

/// The shader's line coordinate for a point `local` metres from a tile whose origin is `origin`, in cells of `s`: the f32
/// offset in cells plus the node's f32 phase, wrapped to [0, 1).
fn shader_cells(local: f64, phase: f32, s: f64) -> f64 {
    f64::from((local / s) as f32 + phase).rem_euclid(1.0)
}

fn wrap(d: f64) -> f64 {
    d.abs().min(1.0 - d.abs())
}

// spec: REND-006
#[test]
fn tiles_with_different_origins_place_grid_lines_at_the_same_world_positions() {
    // Origins at planet-radius magnitude, with fractions that f32 could not hold.
    let a = [4_523_117.318_276_5_f64, -2_012_345.987_654_3, 3_141_592.653_589_8];
    let b = [a[0] + 12.34567891, a[1] - 0.00098765, a[2] + 5_000.5];
    let (pa, pb) = (grid_phases(a), grid_phases(b));
    for axis in 0..3 {
        // A world point 3.7 m from origin a, in each tile's local offsets.
        let world = a[axis] + 3.7;
        let (la, lb) = (world - a[axis], world - b[axis]);
        for (k, s) in GRID_SPACINGS_M.iter().enumerate() {
            let (xa, xb) = (shader_cells(la, pa[k][axis], *s), shader_cells(lb, pb[k][axis], *s));
            let exact = (world / s).rem_euclid(1.0);
            // The tile-local offset is itself an f32 (about 0.5 mm at 5 km from its origin): a 1 mm tolerance in cells.
            let tol = 1e-3 / s + 1e-4;
            assert!(wrap(xa - exact) < tol, "axis {axis}, {s} m: tile a is {} cells off the world coordinate", wrap(xa - exact));
            assert!(wrap(xb - exact) < tol, "axis {axis}, {s} m: tile b is {} cells off the world coordinate", wrap(xb - exact));
            assert!(wrap(xa - xb) < 2.0 * tol, "axis {axis}, {s} m: tiles disagree by {} cells", wrap(xa - xb));
        }
    }
}

// spec: REND-006
#[test]
fn a_ground_tile_resolves_every_decade_to_a_fraction_of_a_pixel() {
    // At 1 m altitude a pixel is about 1.5 mm. A tile near the camera is a few metres across, its origin at planet-radius
    // magnitude. The line coordinate must be exact to a fraction of a pixel at every decade (f32 holds a fraction to about
    // 6e-8 cells; decades whose pixel is under 4e-7 cells fade out in the shader), so the 10 m decade's old failure (a 1,000 km phase added in
    // metres, rounding to 6 cm = 40 px) cannot come back.
    let origin = [4_523_117.318_276_5_f64, 2_012_345.987_654_3, 3_141_592.653_589_8];
    let phases = grid_phases(origin);
    let pixel_m = 1.5e-3;
    for axis in 0..3 {
        for step in 0..400 {
            let local = -2.0 + f64::from(step) * pixel_m / 3.0; // three samples per pixel across 2 m
            let world = origin[axis] + local;
            // Decades whose pixel is under 4e-7 cells are faded out by the shader (an f32 fraction holds ~6e-8 cells).
            for (k, s) in GRID_SPACINGS_M.iter().enumerate().filter(|(_, s)| pixel_m / **s >= 4.0e-7) {
                let got = shader_cells(local, phases[k][axis], *s);
                let err_m = wrap(got - (world / s).rem_euclid(1.0)) * s;
                assert!(
                    err_m < pixel_m / 6.0,
                    "axis {axis}, decade {s} m, local {local}: line coordinate is {err_m} m off (pixel {pixel_m})"
                );
            }
        }
    }
}

// spec: REND-006
#[test]
fn phases_are_fractions_never_planet_radius_values() {
    let p = grid_phases([6_371_000.25, -6_371_000.75, 12.5]);
    assert!(p.iter().flatten().all(|v| (0.0..1.0).contains(v)), "{p:?}");
    // 6,371,000.25 m is a quarter of the way into a metre, and 0.25 m into a kilometre (0.00025 of it).
    assert_eq!(p[2][0], 0.25);
    assert_eq!(p[2][1], 0.25);
    assert!((f64::from(p[5][0]) - 0.00025).abs() < 1e-7, "{}", p[5][0]);
}
