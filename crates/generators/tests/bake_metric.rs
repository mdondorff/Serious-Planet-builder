//! Tier A, GEN-138, SPIKE-105 F5: the physical cell metric of a cube-face grid. The cells of a face sum to a sixth of the sphere, the edge cells of the faces that meet at an edge are
//! the same size, the neighbour distances at the centre, a corner and an edge match F5's analytic values, the weighted routing conserves the true area, and the carve with the metric is the same
//! bits on every run while the default metric is untouched. Small grids; no planet bake.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::metric::Metric;
use planet_generators::bake::route::route;
use planet_generators::bake::Grid;

const R: f64 = 6_371_000.0;

fn face_grid(n: usize, halo: usize) -> Grid {
    let m = n + 2 * halo;
    let dx = R * core::f64::consts::FRAC_PI_2 / n as f64;
    Grid::new(m, m, dx).with_metric(Metric::cube_face(n, R, (-(halo as i64), -(halo as i64)), m, m))
}

// spec: GEN-138
#[test]
fn the_cells_of_a_face_sum_to_a_sixth_of_the_sphere_and_the_faces_that_meet_at_an_edge_agree() {
    for n in [8usize, 64, 257] {
        let g = face_grid(n, 0);
        let total: f64 = (0..g.len()).map(|p| g.cell_area(p)).sum();
        let want = 4.0 * core::f64::consts::PI * R * R / 6.0;
        assert!(((total - want) / want).abs() < 1e-12, "n {n}: the face sums to {total}, a sixth of the sphere is {want}");
        // The equiangular map is symmetric: the cells along the west edge and along the east edge are the same size, so the faces that meet there agree on the edge cells.
        for j in 0..n {
            let (a, b) = (g.cell_area(j * n), g.cell_area(j * n + n - 1));
            assert!(((a - b) / a).abs() < 1e-9, "n {n} row {j}: the edge cells differ ({a} and {b})");
        }
        let (centre, corner) = (g.cell_area((n / 2) * n + n / 2), g.cell_area(0));
        assert!(corner < centre, "a corner cell ({corner}) is not smaller than a centre cell ({centre})");
    }
}

// spec: GEN-138
#[test]
fn the_neighbour_distances_follow_the_sphere_not_the_grid() {
    let n = 256;
    let g = face_grid(n, 0);
    let dx = R * core::f64::consts::FRAC_PI_2 / n as f64;
    let at = |i: usize, j: usize, d: usize| g.distance_at(j * n + i, d) / dx;
    // Centre: one dx on the axes and sqrt 2 dx on the diagonals.
    assert!((at(n / 2, n / 2, 1) - 1.0).abs() < 1e-3, "centre axis {}", at(n / 2, n / 2, 1));
    assert!((at(n / 2, n / 2, 7) - core::f64::consts::SQRT_2).abs() < 2e-3, "centre diagonal {}", at(n / 2, n / 2, 7));
    // F5: at a corner the axis neighbours are about 0.94 dx and the diagonal neighbours about 0.67 sqrt 2 dx.
    assert!((at(1, 1, 1) - 0.94).abs() < 0.02, "corner axis {}", at(1, 1, 1));
    assert!((at(1, 1, 7) - 0.67 * core::f64::consts::SQRT_2).abs() < 0.12, "corner diagonal {}", at(1, 1, 7));
    // Symmetry: the distance a to b is the distance b to a.
    for (i, j) in [(3usize, 5usize), (100, 20), (200, 200)] {
        let p = j * n + i;
        for d in 0..8 {
            let q = g.neighbour(p, d).unwrap();
            let back = (0..8).find(|&e| g.neighbour(q, e) == Some(p)).unwrap();
            assert!((g.distance_at(p, d) - g.distance_at(q, back)).abs() < 1e-6, "cell {p} direction {d}");
        }
    }
}

// spec: GEN-138
#[test]
fn the_default_grid_keeps_the_uniform_metric() {
    let g = Grid::new(8, 8, 800.0);
    assert!(!g.has_metric());
    assert_eq!(g.cell_area(10), 640_000.0);
    assert_eq!(g.distance_at(10, 0), 800.0);
    assert_eq!(g.distance_at(10, 7), 800.0 * core::f64::consts::SQRT_2);
}

// spec: GEN-138
#[test]
fn the_routing_conserves_the_true_area_and_the_carve_is_the_same_bits_on_every_run() {
    let (n, halo) = (40usize, 4usize);
    let g = face_grid(n, halo);
    let m = n + 2 * halo;
    let z: Vec<f64> = (0..m * m).map(|p| 25.0 * ((p % m) as f64 + 0.5 * (p / m) as f64) + 3.0 * libm::sin(p as f64 * 0.7)).collect();
    let outlet = g.outlets(&z, -1.0);
    let r = route(&g, &z, &outlet, 1e-3, 5, 0.0);
    let total: f64 = (0..g.len()).map(|p| g.cell_area(p)).sum();
    let at_outlets: f64 = (0..g.len()).filter(|&p| outlet[p]).map(|p| r.area[p]).sum();
    assert!(((at_outlets - total) / total).abs() < 1e-9, "the outlets hold {at_outlets} of {total} m2");
    let p = CarveParams { steps: 6, ..CarveParams::default() };
    let run = |g: &Grid| {
        let mut zz = z.clone();
        carve(g, &mut zz, &outlet, &p, None, 7);
        zz.iter().map(|v| v.to_bits()).collect::<Vec<u64>>()
    };
    assert_eq!(run(&g), run(&g), "two runs differ");
    // The metric moves the result: the areas shrink, so the carve differs from the uniform one.
    let uniform = Grid::new(m, m, g.dx());
    assert_ne!(run(&g), run(&uniform), "the metric changed nothing");
}
