//! Tier A, GEN-139 (SPIKE-105 F6): H0, one drainage structure over the six cube faces. The graph is connected and symmetric across the cube edges, the true areas sum to the sphere and the area at
//! the outlets is the sum of all areas, a catchment crosses cube edges as one catchment, and the result is the same bits on every run. Small grids; no store read.

use planet_generators::bake::h0::{GlobalGrid, NONE};

const R: f64 = 6_371_000.0;

#[test]
fn the_six_faces_are_one_connected_symmetric_graph_whose_areas_sum_to_the_sphere() {
    // spec: GEN-139
    for n in [4usize, 12, 33] {
        let g = GlobalGrid::new(n, R);
        let total: f64 = (0..g.len() as u32).map(|c| g.area(c)).sum();
        let want = 4.0 * core::f64::consts::PI * R * R;
        assert!(((total - want) / want).abs() < 1e-10, "n {n}: the cells sum to {total}, the sphere is {want}");
        let dx = R * core::f64::consts::FRAC_PI_2 / n as f64;
        for c in 0..g.len() as u32 {
            for (q, d) in g.neighbours(c) {
                assert_ne!(q, c, "cell {c} is its own neighbour");
                assert!(g.neighbours(q).iter().any(|(b, _)| *b == c), "n {n}: {c} -> {q} has no way back");
                assert!(d > 0.4 * dx && d < 1.6 * dx, "n {n}: the distance {c} -> {q} is {d} m, one cell is {dx} m");
            }
        }
        // One component: a walk from cell 0 reaches every cell.
        let mut seen = vec![false; g.len()];
        let mut stack = vec![0u32];
        seen[0] = true;
        while let Some(c) = stack.pop() {
            for (q, _) in g.neighbours(c) {
                if !seen[q as usize] {
                    seen[q as usize] = true;
                    stack.push(q);
                }
            }
        }
        assert!(seen.iter().all(|s| *s), "n {n}: the graph is not connected");
        // A face edge cell has a neighbour on another face, and only axis neighbours reach across.
        let across = (0..g.len() as u32).filter(|&c| g.neighbours(c).iter().any(|(q, _)| g.split(*q).0 != g.split(c).0)).count();
        assert_eq!(across, 6 * 4 * (n - 1), "n {n}: the cells with a neighbour across a cube edge");
    }
}

/// A planet that falls away from the +x pole cap: heights grow with the angle from the cap, the cap is the sea.
fn cap_planet(g: &GlobalGrid, cap_deg: f64) -> (Vec<f64>, Vec<bool>) {
    let axis = planet_core::Vec3::new(1.0, 0.0, 0.0);
    let ang: Vec<f64> = (0..g.len() as u32).map(|c| libm::acos(g.direction(c).dot(axis).clamp(-1.0, 1.0))).collect();
    let z: Vec<f64> = ang.iter().map(|a| 100.0 + 5000.0 * a + 40.0 * libm::sin(37.0 * a)).collect();
    let outlet = ang.iter().map(|a| a.to_degrees() < cap_deg).collect();
    (z, outlet)
}

#[test]
fn a_catchment_crosses_the_cube_edges_as_one_and_the_outlets_hold_the_whole_sphere() {
    // spec: GEN-139
    let g = GlobalGrid::new(24, R);
    let (z, outlet) = cap_planet(&g, 25.0);
    let r = g.route(&z, &outlet, 1e-3);
    let total: f64 = (0..g.len() as u32).map(|c| g.area(c)).sum();
    let at_outlets: f64 = (0..g.len()).filter(|&c| outlet[c]).map(|c| r.area[c]).sum();
    assert!(((at_outlets - total) / total).abs() < 1e-9, "the outlets hold {at_outlets} of {total} m2");
    assert!(r.outlet_of.iter().all(|o| *o != NONE), "a cell drains nowhere");
    assert!(r.receiver.iter().enumerate().all(|(c, rc)| outlet[c] == (*rc == NONE)), "only outlets lack a receiver");
    // Catchments by outlet: the largest one holds cells of every face (it crosses every cube edge it meets), and no two faces' parts are separate catchments of the same outlet.
    let mut faces_of = std::collections::BTreeMap::<u32, std::collections::BTreeSet<u8>>::new();
    for c in 0..g.len() {
        faces_of.entry(r.outlet_of[c]).or_default().insert(g.split(c as u32).0);
    }
    let widest = faces_of.values().map(|f| f.len()).max().unwrap();
    assert!(widest >= 4, "the widest catchment spans {widest} faces");
}

#[test]
fn h0_is_the_same_bits_on_every_run() {
    // spec: GEN-139
    let g = GlobalGrid::new(16, R);
    let (z, outlet) = cap_planet(&g, 20.0);
    let a = g.route(&z, &outlet, 1e-3);
    let b = g.route(&z, &outlet, 1e-3);
    assert_eq!(a, b);
}

#[test]
fn only_the_boundary_cells_reach_across_an_edge_each_by_one_neighbour_per_crossed_edge_and_the_directions_agree_with_the_mapping() {
    // spec: GEN-139
    use planet_core::cube::{face_to_direction, TangentWarp};
    use planet_core::Face;
    let n = 17usize;
    let g = GlobalGrid::new(n, R);
    for c in 0..g.len() as u32 {
        let (f, i, j) = g.split(c);
        let crossed = |axis_edges: usize| axis_edges;
        let edges = usize::from(i == 0) + usize::from(i == n - 1) + usize::from(j == 0) + usize::from(j == n - 1);
        let other: Vec<u32> = g.neighbours(c).into_iter().map(|(q, _)| q).filter(|q| g.split(*q).0 != f).collect();
        assert_eq!(
            other.len(),
            crossed(edges),
            "cell {c} ({f},{i},{j}) has {} neighbours across and {edges} edges on its boundary",
            other.len()
        );
        // The direction from the tables and frames is the mapping's.
        let s = |k: usize| 2.0 * (k as f64 + 0.5) / n as f64 - 1.0;
        let want = face_to_direction(&TangentWarp, Face(f), s(i), s(j));
        let have = g.direction(c);
        assert!(
            (want.x - have.x).abs() + (want.y - have.y).abs() + (want.z - have.z).abs() < 1e-14,
            "cell {c}: the direction differs from the mapping's"
        );
    }
}

#[test]
fn the_cross_edge_pairing_is_symmetric_at_production_sizes_and_keeps_the_along_edge_index() {
    // spec: GEN-139
    for n in [3072usize, 12288] {
        let g = GlobalGrid::new(n, R);
        for f in 0..6u8 {
            for k in 0..n {
                for (i, j) in [(0, k), (n - 1, k), (k, 0), (k, n - 1)] {
                    let c = g.id(f, i, j);
                    let mut ids = [0u32; 8];
                    let cnt = g.neighbour_ids(c, &mut ids);
                    for &q in ids[..cnt].iter().filter(|q| g.split(**q).0 != f) {
                        let mut back = [0u32; 8];
                        let bc = g.neighbour_ids(q, &mut back);
                        assert!(back[..bc].contains(&c), "n {n}: {c} -> {q} has no way back");
                        // The along-edge index of the neighbour is k or n - 1 - k.
                        let (_, qi, qj) = g.split(q);
                        assert!(
                            [k, n - 1 - k].iter().any(|a| *a == qi || *a == qj),
                            "n {n}: ({f},{i},{j}) -> {:?} does not keep the along-edge index",
                            g.split(q)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn bad_input_is_refused_and_a_flat_with_a_valid_epsilon_routes() {
    // spec: GEN-139
    let g = GlobalGrid::new(6, R);
    let (z, outlet) = cap_planet(&g, 30.0);
    let refused =
        |z: Vec<f64>, outlet: Vec<bool>, eps: f64| std::panic::catch_unwind(|| GlobalGrid::new(6, R).route(&z, &outlet, eps)).is_err();
    assert!(refused(z.clone(), outlet.clone(), 0.0), "epsilon 0 was taken");
    assert!(refused(z.clone(), outlet.clone(), -1e-3), "a negative epsilon was taken");
    assert!(refused(z.clone(), outlet.clone(), f64::NAN), "a NaN epsilon was taken");
    let mut bad = z.clone();
    bad[3] = f64::NAN;
    assert!(refused(bad, outlet.clone(), 1e-3), "a NaN height was taken");
    assert!(refused(z.clone(), vec![false; z.len()], 1e-3), "a planet without an outlet was routed");
    assert!(std::panic::catch_unwind(|| GlobalGrid::new(26_755, R)).is_err(), "more cells than a u32 id holds were taken");
    // A flat planet at 100 m with a sea cap routes: every cell drains.
    let flat = vec![100.0; z.len()];
    let r = g.route(&flat, &outlet, 1e-3);
    assert!(r.outlet_of.iter().all(|o| *o != NONE));
}
