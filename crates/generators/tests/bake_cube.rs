//! Tier A, GEN-144 (A3 sections 4.1, 6.1, 6.2; the cube-edge fixture of results.md): the six faces as one grid. A catchment that crosses a cube edge is one catchment; the area is conserved
//! across the edges; the resident route, the partitioned route and the round solver run on it and agree bit for bit. Small cubes; no store read.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::{Grid, NEIGHBOURS};

const N: usize = 24;
const R: f64 = 6_371_000.0;
const EPS: f64 = 1e-3;

/// A land that slopes down toward a sea cap on face 5, with ridges, so that water crosses several cube edges.
fn globe(g: &Grid) -> Vec<f64> {
    let cube = planet_generators::bake::h0::GlobalGrid::new(N, R);
    (0..g.len())
        .map(|p| {
            let d = cube.direction(p as u32);
            let sea = planet_core::Vec3::new(0.2, -0.3, -0.93);
            let s = (d.x * sea.x + d.y * sea.y + d.z * sea.z).clamp(-1.0, 1.0);
            let ridge = 120.0 * libm::sin(9.0 * d.x + 4.0 * d.y) * libm::cos(7.0 * d.z);
            let jitter = 3.0 * planet_generators::bake::route::jitter(5, p as u64, 0);
            1800.0 * (1.0 - s) + ridge + jitter - 400.0 * (s - 0.85).max(0.0) * 8.0 - 20.0
        })
        .collect()
}

// spec: GEN-144
#[test]
fn the_cube_grid_joins_the_faces_symmetrically_and_covers_the_sphere() {
    let g = Grid::cube(N, R);
    assert_eq!(g.len(), 6 * N * N);
    let mut total = 0.0;
    for p in 0..g.len() {
        total += g.cell_area(p);
        for d in 0..4 {
            let q = g.neighbour(p, d).expect("every cell has its four axis neighbours");
            let back = (0..4).filter_map(|e| g.neighbour(q, e)).any(|x| x == p);
            assert!(back, "the axis neighbour of {p} in {d} ({q}) does not point back");
            let (a, b) = (g.distance_at(p, d), (0..4).find(|&e| g.neighbour(q, e) == Some(p)).map(|e| g.distance_at(q, e)).unwrap());
            assert_eq!(a.to_bits(), b.to_bits(), "the distance {p} to {q} is not symmetric: {a} against {b}");
        }
        for d in 4..8 {
            if let Some(q) = g.neighbour(p, d) {
                assert_eq!(p / (N * N), q / (N * N), "a diagonal stays inside its face");
            }
        }
    }
    let sphere = 4.0 * core::f64::consts::PI * R * R;
    assert!((total - sphere).abs() <= 1e-9 * sphere, "the cells cover the sphere: {total} against {sphere}");
    assert_eq!(NEIGHBOURS.len(), 8);
}

// spec: GEN-144
#[test]
fn a_catchment_crosses_the_cube_edges_and_the_area_is_conserved() {
    let g = Grid::cube(N, R);
    let z = globe(&g);
    let outlet = g.outlets(&z, 0.0);
    assert!(outlet.iter().any(|&o| o) && outlet.iter().any(|&o| !o));
    for rules in [RoundRules::ROUND, RoundRules { clip_mfd: planet_generators::bake::route::MfdClip::Off, canonical_flood: true }] {
        let r = route_rules(&g, &z, &outlet, EPS, 5, 0.0, None, rules);
        let crossings = (0..g.len()).filter(|&p| r.receiver[p] != NONE && p / (N * N) != r.receiver[p] as usize / (N * N)).count();
        assert!(crossings > 0, "some receiver crosses a cube edge");
        // Every cell drains to an outlet; with the clip the outlet holds its members' areas, so the outlets hold the sphere.
        let held: f64 = (0..g.len()).filter(|&p| outlet[p]).map(|p| r.area[p]).sum();
        let sphere = 4.0 * core::f64::consts::PI * R * R;
        assert!(
            (held - sphere).abs() <= 1e-9 * sphere,
            "A_d at the outlets {held} against the sphere {sphere} (clip {:?})",
            rules.clip_mfd
        );
    }
}

// spec: GEN-144
#[test]
fn the_partitioned_route_and_the_round_solver_run_on_the_cube() {
    let g = Grid::cube(N, R);
    let z = globe(&g);
    let outlet = g.outlets(&z, 0.0);
    let want = route_rules(&g, &z, &outlet, EPS, 5, 0.0, None, RoundRules::ROUND);
    for tile in [6usize, 8, 12] {
        let got = partitioned_route(&g, &z, &outlet, EPS, 5, tile, RoundRules::ROUND, &mut PartCache::default());
        assert_eq!(got.routing.receiver, want.receiver, "G-PART receivers (tile {tile})");
        assert!(got.routing.area.iter().zip(&want.area).all(|(a, b)| a.to_bits() == b.to_bits()), "G-PART A_d (tile {tile})");
    }
    let p = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let mut a = z.clone();
    carve(&g, &mut a, &outlet, &p, None, 5);
    for threads in [1usize, 4] {
        let mut b = z.clone();
        solve_rounds(&g, &mut b, &outlet, &p, 2, &threads, 5);
        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "G-ROUND on the cube ({threads} threads)");
    }
}

// spec: GEN-144
#[test]
fn a_landmass_of_the_cube_is_routed_alone_with_its_coast_as_outlets() {
    let full = Grid::cube(N, R);
    let z = globe(&full);
    // The landmass: the land cells and the sea cells that touch them (the coast ring); the open sea is not on the grid.
    let land: Vec<bool> = z.iter().map(|&v| v > 0.0).collect();
    let active: Vec<bool> = (0..z.len()).map(|p| land[p] || (0..8).filter_map(|d| full.neighbour(p, d)).any(|q| land[q])).collect();
    assert!(active.iter().any(|&a| !a) && land.iter().any(|&l| l), "a landmass with open sea around it");
    let g = Grid::cube(N, R).with_active(active.clone());
    let outlet: Vec<bool> = (0..z.len()).map(|p| active[p] && z[p] <= 0.0).collect();
    let want = route_rules(&g, &z, &outlet, EPS, 5, 0.0, None, RoundRules::ROUND);
    let (mut held, mut cells) = (0.0, 0.0);
    for p in (0..z.len()).filter(|&p| active[p]) {
        cells += g.cell_area(p);
        if outlet[p] {
            held += want.area[p];
        }
    }
    assert!((held - cells).abs() <= 1e-9 * cells, "the outlets hold the area of the active cells: {held} against {cells}");
    for tile in [6usize, 8, 12] {
        let got = partitioned_route(&g, &z, &outlet, EPS, 5, tile, RoundRules::ROUND, &mut PartCache::default());
        let all = priority_flood_with(&g, &z, &outlet, EPS, true);
        assert!(
            (0..z.len()).all(|p| got.filled[p].to_bits() == all[p].to_bits()),
            "the filled surface, inactive cells included (tile {tile})"
        );
        for p in (0..z.len()).filter(|&p| active[p]) {
            assert_eq!(got.routing.receiver[p], want.receiver[p], "receiver {p} (tile {tile})");
            assert_eq!(got.routing.area[p].to_bits(), want.area[p].to_bits(), "A_d {p} (tile {tile})");
        }
    }
    let p = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let mut a = z.clone();
    carve(&g, &mut a, &outlet, &p, None, 5);
    let mut b = z.clone();
    solve_rounds(&g, &mut b, &outlet, &p, 2, &3usize, 5);
    assert!((0..z.len()).all(|c| a[c].to_bits() == b[c].to_bits()), "G-ROUND on a masked cube");
}

// spec: GEN-144
#[test]
fn the_cube_grid_is_the_h0_graph() {
    let g = Grid::cube(N, R);
    let h0 = planet_generators::bake::h0::GlobalGrid::new(N, R);
    for p in 0..g.len() {
        let mut ids = [0u32; 8];
        h0.neighbour_ids(p as u32, &mut ids);
        let with_d = h0.neighbours(p as u32);
        for d in 0..4 {
            assert_eq!(g.neighbour(p, d), Some(ids[d] as usize), "axis neighbour {d} of {p}");
            assert_eq!(g.distance_at(p, d).to_bits(), with_d[d].1.to_bits(), "distance {d} of {p}");
        }
        assert_eq!(g.cell_area(p).to_bits(), h0.area(p as u32).to_bits(), "area of {p}");
    }
}

// spec: GEN-144
#[test]
#[should_panic(expected = "divide a face edge")]
fn tiles_of_a_cube_grid_divide_a_face_edge() {
    let g = Grid::cube(N, R);
    let z = globe(&g);
    let outlet = g.outlets(&z, 0.0);
    partitioned_route(&g, &z, &outlet, EPS, 5, 7, RoundRules::ROUND, &mut PartCache::default());
}

// spec: GEN-144
#[test]
fn a_cache_is_not_shared_between_a_plane_and_a_cube_of_the_same_size() {
    let cube = Grid::cube(8, R);
    let plane = Grid::new(8, 48, 800.0);
    let z: Vec<f64> = (0..cube.len()).map(|p| 50.0 + (p % 37) as f64 * 3.0 - (p / 64) as f64).collect();
    let mut cache = PartCache::default();
    for g in [&cube, &plane, &cube] {
        let outlet: Vec<bool> = (0..z.len()).map(|p| z[p] <= 52.0 || (g.neighbour(p, 3).is_none() && !g.is_cube())).collect();
        let want = route_rules(g, &z, &outlet, EPS, 5, 0.0, None, RoundRules::ROUND);
        let got = partitioned_route(g, &z, &outlet, EPS, 5, 4, RoundRules::ROUND, &mut cache);
        assert_eq!(got.routing.receiver, want.receiver);
        assert!(got.routing.area.iter().zip(&want.area).all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}

// spec: GEN-144
#[test]
fn outlets_off_the_grid_cost_no_flood_and_a_masked_plane_is_the_resident_route() {
    // A plane with a mask: the cells right of column 9 do not exist; the sea flag stays set on some of them (outlets off the grid).
    let (w, h) = (16usize, 12usize);
    let active: Vec<bool> = (0..w * h).map(|p| p % w < 10).collect();
    let g = Grid::new(w, h, 800.0).with_active(active.clone());
    let z: Vec<f64> = (0..w * h).map(|p| 30.0 + 7.0 * ((p % w) as f64) - 2.0 * ((p / w) as f64) + (p % 5) as f64).collect();
    let outlet: Vec<bool> = (0..w * h).map(|p| (p % w == 0) || p % w == 12).collect();
    let want = route_rules(&g, &z, &outlet, EPS, 5, 0.0, None, RoundRules::ROUND);
    let got = partitioned_route(&g, &z, &outlet, EPS, 5, 4, RoundRules::ROUND, &mut PartCache::default());
    assert_eq!(got.routing.receiver, want.receiver);
    assert!(got.routing.area.iter().zip(&want.area).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(got.filled.iter().zip(&priority_flood_with(&g, &z, &outlet, EPS, true)).all(|(a, b)| a.to_bits() == b.to_bits()));
    // The outlets off the grid change nothing: the same route and the same floods as with the flags cleared.
    let on_grid: Vec<bool> = (0..w * h).map(|p| outlet[p] && active[p]).collect();
    let clean = partitioned_route(&g, &z, &on_grid, EPS, 5, 4, RoundRules::ROUND, &mut PartCache::default());
    assert_eq!(got.stats.floods, clean.stats.floods, "no tile is flooded for an outlet off the grid");
    assert_eq!(got.routing.receiver, clean.routing.receiver);
}
