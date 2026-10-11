//! Tier A: the relief grid is anchored to the world, not to a tile, and its line coordinates stay small at every decade
//! (REND-006). Two tiles with different origins must agree on where a grid line is for the same world point, and the
//! shader's f32 arithmetic (tile-local offset in cells plus the node's phase) must resolve a line to a fraction of a pixel.

use planet_render::terrain::{grid_phases, GRID_SPACINGS_M};

/// Signed offset (in cells, [-0.5, 0.5)) from the nearest grid line that the shader computes for a point `local` metres
/// from a tile origin, in f32 like the shader: the local offset, the spacing built by repeated `* 10` from 0.01, the
/// division, the phase, and `fract(x + 0.5) - 0.5` as `line()` does it.
fn shader_offset(local: f64, phase: f32, decade: usize) -> f64 {
    let mut s = 0.01f32;
    for _ in 0..decade {
        s *= 10.0;
    }
    let x = (local as f32) / s + phase;
    let y = x + 0.5;
    f64::from(y - y.floor() - 0.5)
}

/// The exact signed offset from the nearest line, in cells, for a world coordinate.
fn exact_offset(world: f64, s: f64) -> f64 {
    (world / s + 0.5).rem_euclid(1.0) - 0.5
}

/// Distance between two signed offsets, wrapped to one cell.
fn wrap(d: f64) -> f64 {
    let d = d.rem_euclid(1.0);
    d.min(1.0 - d)
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
            let (xa, xb) = (shader_offset(la, pa[k][axis], k), shader_offset(lb, pb[k][axis], k));
            let exact = exact_offset(world, *s);
            // 1 mm, or 1e-7 cells where an f32 fraction cannot do better (at 1,000 km a cell is that wide).
            let tol = 1e-3 / s + 1e-7;
            assert!(wrap(xa - exact) < tol, "axis {axis}, {s} m: tile a is {} cells off the world coordinate", wrap(xa - exact));
            assert!(wrap(xb - exact) < tol, "axis {axis}, {s} m: tile b is {} cells off the world coordinate", wrap(xb - exact));
            assert!(wrap(xa - xb) < 2.0 * tol, "axis {axis}, {s} m: tiles disagree by {} cells", wrap(xa - xb));
        }
    }
}

// spec: REND-006
#[test]
fn a_ground_tile_resolves_every_visible_decade_to_a_fraction_of_a_pixel() {
    // At 1 m altitude a pixel is about 1.5 mm. A tile near the camera is a few metres across, its origin at planet-radius
    // magnitude. At full strength (a pixel of at least 8e-7 cells; the shader fades decades below that) every line must be
    // placed to a sixth of a pixel (0.25 mm). This is what the 10 m decade failed when its phase was a 1,000 km remainder
    // in metres added in f32 (6 cm of rounding, plateaus 40 px wide).
    let origin = [4_523_117.318_276_5_f64, 2_012_345.987_654_3, 3_141_592.653_589_8];
    let phases = grid_phases(origin);
    let pixel_m = 1.5e-3;
    let mut checked = 0usize;
    for axis in 0..3 {
        for step in 0..4000 {
            let local = -2.0 + f64::from(step) * pixel_m / 3.0; // three samples per pixel across 2 m
            let world = origin[axis] + local;
            for (k, s) in GRID_SPACINGS_M.iter().enumerate().filter(|(_, s)| pixel_m / **s >= 8.0e-7) {
                let err_m = wrap(shader_offset(local, phases[k][axis], k) - exact_offset(world, *s)) * s;
                assert!(
                    err_m < pixel_m / 6.0,
                    "axis {axis}, decade {s} m, local {local}: line coordinate is {err_m} m off (pixel {pixel_m})"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 3 * 4000 * 5, "too few decade samples checked: {checked}");
}

// spec: REND-006
#[test]
fn the_old_scheme_of_a_metre_phase_added_in_f32_fails_the_ground_resolution() {
    // Negative control: the first relief grid added the origin modulo 1,000 km, in metres, to the local offset in f32. At
    // the 10 m decade that rounds to 6 cm, forty ground pixels, so this test shows the check above can fail.
    let origin = 4_523_117.318_276_5_f64;
    let phase_m = (origin.rem_euclid(1.0e6)) as f32;
    let (s, pixel_m) = (10.0f64, 1.5e-3);
    let worst = (0..4000)
        .map(|step| {
            let local = -2.0 + f64::from(step) * pixel_m / 3.0;
            let x = (local as f32 + phase_m) / 10.0;
            let y = x + 0.5;
            let off = f64::from(y - y.floor() - 0.5);
            wrap(off - exact_offset(origin + local, s)) * s
        })
        .fold(0.0f64, f64::max);
    assert!(worst > 10.0 * pixel_m, "the old scheme should be tens of pixels off at the 10 m decade, got {worst} m");
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
    // A fraction just under 1 rounds to 1.0 in f32 and a tiny negative origin wraps to 1.0 in f64: both must read 0.
    let edge = grid_phases([-1e-20, 1.0 - 1e-9, 0.0]);
    assert!(edge.iter().flatten().all(|v| (0.0..1.0).contains(v)), "{edge:?}");
}
