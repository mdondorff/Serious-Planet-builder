//! GEN-031: the wet floor. The three passes equal the serial priority flood for every block size and thread count; the codes, the bound and
//! the sentinels; a lagoon behind a low barrier, a bay behind a ridge, a path that leaves and re-enters a block, a diagonal-only link.

use planet_core::cube::TangentWarp;
use planet_core::Face;
use planet_generators::global::{bake, GlobalGrid, GlobalSample};
use planet_generators::ocean::{tiled_ocean, CubeIndex, OceanGrid};
use planet_generators::wet_floor::{bound_code, code_of, reference_floor_grid, tiled_floors, FloorLattice, CODE_ZERO, INF};

fn assert_equal_to_reference(grid: &GlobalGrid, ocean: &OceanGrid, cells: u32, threads: usize, what: &str) -> usize {
    let want = reference_floor_grid(grid, ocean);
    let got = tiled_floors(grid, ocean, cells, threads).unwrap();
    for f in 0..6u8 {
        for j in 0..=grid.res {
            for i in 0..=grid.res {
                assert_eq!(
                    got.at(Face(f), i, j),
                    want.at(Face(f), i, j),
                    "{what}: face {f} vertex ({i}, {j}) cells {cells} threads {threads}"
                );
            }
        }
    }
    got.faces.iter().flatten().filter(|v| **v != INF).count()
}

/// A grid of `res` cells whose vertices (one value per node, so the copies on shared face edges agree) are low (`low_percent` of them,
/// -40 to 80 m) or high (100 to 500 m), with some NaN elevations and ocean vertices.
fn random_grid(res: u32, seed: u64, low_percent: u64) -> (GlobalGrid, OceanGrid) {
    let n = res + 1;
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut rnd = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut faces: [Vec<GlobalSample>; 6] = Default::default();
    let mut ocean = OceanGrid::zeros(res);
    let index = CubeIndex::new(res);
    let mut by_node: std::collections::HashMap<u64, (f32, bool)> = std::collections::HashMap::new();
    for f in 0..6u8 {
        for l in 0..n {
            for k in 0..n {
                let id = index.node_id(Face(f), k, l);
                let v = *by_node.entry(id).or_insert_with(|| {
                    let low = rnd() % 100 < low_percent;
                    let e = if low { (rnd() % 120) as f32 - 40.0 } else { 100.0 + (rnd() % 400) as f32 };
                    let nan = rnd() % 500 == 0;
                    let inf = rnd() % 300 == 0;
                    let e = if inf { [f32::INFINITY, f32::NEG_INFINITY][(rnd() % 2) as usize] } else { e };
                    (if nan { f32::NAN } else { e }, rnd() % 100 < 4)
                });
                faces[f as usize].push(GlobalSample { continent: 0.2, elevation_m: v.0, temperature_c: 0.0, precipitation_mm: 0.0 });
                if v.1 {
                    ocean.set(Face(f), k, l);
                }
            }
        }
    }
    (GlobalGrid { seed, res, faces }, ocean)
}

// spec: GEN-031
#[test]
fn random_grids_give_the_serial_floor_for_every_block_size_and_thread_count() {
    let mut finite_total = 0;
    for (res, low) in [(4u32, 70u64), (8, 60), (16, 55), (16, 70), (32, 60)] {
        for seed in 0..if res >= 32 { 1u64 } else { 3 } {
            let (grid, ocean) = random_grid(res, 100 * seed + u64::from(res), low);
            let mut sizes = vec![res, (res / 2).max(1), (res / 4).max(1), 1];
            sizes.sort_unstable();
            sizes.dedup();
            if res >= 32 {
                sizes.retain(|c| *c >= 4);
            }
            for cells in sizes {
                for threads in [1usize, 3] {
                    finite_total += assert_equal_to_reference(&grid, &ocean, cells, threads, &format!("res {res} low {low} seed {seed}"));
                }
            }
        }
    }
    assert!(finite_total > 1000, "the grids have reachable floors ({finite_total})");
}

// spec: GEN-031
#[test]
fn a_real_bake_gives_the_serial_floor_for_every_block_size_and_thread_count() {
    for (seed, res) in [(1u64, 64u32), (7, 128)] {
        let grid = bake(seed, &TangentWarp, res, 4).unwrap();
        let (_, ocean) = tiled_ocean(&grid, &TangentWarp, res, 2, 0.02).unwrap();
        for cells in [4u32, 16, 32, res] {
            for threads in [1usize, 3] {
                let finite = assert_equal_to_reference(&grid, &ocean, cells, threads, &format!("seed {seed} res {res}"));
                assert!(
                    finite > (6 * (res as usize + 1) * (res as usize + 1)) / 10,
                    "a real bake has plenty of reachable floors ({finite})"
                );
            }
        }
    }
}

/// A grid of `res` cells that is flat at `base` metres except the vertices `edits` (face 0, i, j, elevation), with an ocean vertex at
/// face 0 (0, 0) only unless `ocean_at` names others.
fn flat_grid(res: u32, base: f32, edits: &[(u32, u32, f32)], ocean_at: &[(u32, u32)]) -> (GlobalGrid, OceanGrid) {
    let n = (res + 1) as usize;
    let mut faces: [Vec<GlobalSample>; 6] = Default::default();
    // Every face but 0, and the border ring of face 0 (where the copies are shared), is a 500 m wall; the interior of face 0 is `base`.
    for (fi, f) in faces.iter_mut().enumerate() {
        *f = (0..n * n)
            .map(|v| {
                let (i, j) = (v % n, v / n);
                let wall = fi != 0 || i == 0 || j == 0 || i == n - 1 || j == n - 1;
                GlobalSample { continent: 0.2, elevation_m: if wall { 500.0 } else { base }, temperature_c: 0.0, precipitation_mm: 0.0 }
            })
            .collect();
    }
    for &(i, j, e) in edits {
        faces[0][j as usize * n + i as usize].elevation_m = e;
    }
    let mut ocean = OceanGrid::zeros(res);
    for &(i, j) in ocean_at {
        ocean.set(Face(0), i, j);
    }
    (GlobalGrid { seed: 0, res, faces }, ocean)
}

// spec: GEN-031
#[test]
fn the_floor_is_the_largest_code_on_the_best_path_and_a_ridge_decides_it() {
    // A 16-cell face: a ridge of 90 m along column 8, the ocean at vertex (2, 2). Left of the ridge the floor is the flat 10 m code;
    // right of it the ridge's 90 m code.
    let ridge: Vec<(u32, u32, f32)> = (0..=16).map(|j| (8, j, 90.0)).collect();
    let (grid, ocean) = flat_grid(16, 10.0, &ridge, &[(2, 2)]);
    for cells in [16u32, 8, 4] {
        let floors = tiled_floors(&grid, &ocean, cells, 2).unwrap();
        assert_eq!(floors.at(Face(0), 2, 2), 0, "the ocean vertex");
        assert_eq!(floors.at(Face(0), 5, 5), code_of(10.0), "left of the ridge, cells {cells}");
        assert_eq!(floors.at(Face(0), 12, 12), code_of(90.0), "right of the ridge: over it, cells {cells}");
        assert_eq!(floors.at(Face(0), 8, 8), code_of(90.0), "on it");
    }
    // Wet at a bound above the ridge, dry behind it at a bound below.
    let floors = tiled_floors(&grid, &ocean, 4, 1).unwrap();
    assert!(floors.at(Face(0), 12, 12) <= bound_code(100.0) && floors.at(Face(0), 12, 12) > bound_code(64.0));
    assert!(floors.at(Face(0), 5, 5) <= bound_code(64.0));
}

// spec: GEN-031
#[test]
fn a_lagoon_behind_a_low_barrier_and_a_diagonal_only_link_are_wet() {
    // The ocean at (1, 1); a barrier of 40 m at (4, 4); a lagoon of -10 m beyond it, joined to the barrier only diagonally.
    let (grid, ocean) = flat_grid(16, 500.0, &[(2, 2, 10.0), (3, 3, 20.0), (4, 4, 40.0), (5, 5, -10.0), (6, 6, -10.0)], &[(1, 1)]);
    let floors = tiled_floors(&grid, &ocean, 4, 2).unwrap();
    // The path (1,1) -> (2,2) -> (3,3) -> (4,4) -> (5,5) -> (6,6) is diagonal throughout: the lagoon's floor is the barrier's 40 m.
    assert_eq!(floors.at(Face(0), 6, 6), code_of(40.0));
    assert_eq!(floors.at(Face(0), 3, 3), code_of(20.0));
    // A vertex elsewhere on the 500 m plain is reached over the 500 m plain only.
    assert_eq!(floors.at(Face(0), 12, 3), code_of(500.0));
}

// spec: GEN-031
#[test]
fn an_enclosed_pit_is_unreached_and_an_unreadable_elevation_blocks_a_path() {
    // A ring of NaN around vertex (8, 8) on an otherwise low plain: the pit inside is not reached. The ocean at (2, 2).
    let mut ring = Vec::new();
    for d in -1i32..=1 {
        for e in -1i32..=1 {
            if d != 0 || e != 0 {
                ring.push(((8 + d) as u32, (8 + e) as u32, f32::NAN));
            }
        }
    }
    ring.push((8, 8, -50.0));
    let (grid, ocean) = flat_grid(16, 5.0, &ring, &[(2, 2)]);
    let floors = tiled_floors(&grid, &ocean, 8, 1).unwrap();
    assert_eq!(floors.at(Face(0), 8, 8), INF, "the pit inside the ring");
    assert_eq!(floors.at(Face(0), 7, 7), INF, "an unreadable elevation is never crossed");
    assert_eq!(floors.at(Face(0), 12, 12), code_of(5.0));
}

// spec: GEN-031
#[test]
fn the_codes_round_down_clamp_and_keep_the_sentinels() {
    assert_eq!(code_of(0.0) as i32, CODE_ZERO);
    assert_eq!(code_of(-0.0) as i32, CODE_ZERO);
    assert_eq!(code_of(0.4) as i32, CODE_ZERO, "rounded down to half a metre");
    assert_eq!(code_of(0.5) as i32, CODE_ZERO + 1);
    assert_eq!(code_of(-0.1) as i32, CODE_ZERO - 1, "down, not towards zero");
    assert_eq!(code_of(f32::NAN), INF);
    assert_eq!(code_of(f32::NEG_INFINITY), 1, "below the range clamps to the lowest finite code");
    assert_eq!(code_of(-100_000.0), 1);
    assert_eq!(code_of(f32::INFINITY), 0xFFFE, "above the range clamps to the highest finite code, never the sentinel");
    assert_eq!(code_of(1.0e9), 0xFFFE);
    // The bound: rounded down like the codes, monotone, a NaN bound dries everything but the ocean.
    assert_eq!(bound_code(64.0) as i32, CODE_ZERO + 128);
    assert_eq!(bound_code(f64::NAN), 0);
    assert_eq!(bound_code(-20_000.0), 1, "below the range: the lowest finite code, as code_of clamps, so the wet set stays a superset");
    assert!(code_of(-20_000.0) <= bound_code(-15_000.0));
    let mut prev = 0u16;
    for k in -2000..2000 {
        let b = bound_code(f64::from(k) * 0.37);
        assert!(b >= prev, "monotone at {k}");
        prev = b;
    }
    // Superset: a vertex whose elevation is at most the bound has a code at most the bound's code.
    for k in -300..300 {
        let (e, b) = (f64::from(k) * 0.23, f64::from(k % 17) * 3.1);
        if e <= b {
            assert!(code_of(e as f32) <= bound_code(b), "elevation {e} bound {b}");
        }
    }
}

// spec: GEN-031
#[test]
fn a_floor_lattice_reads_a_grid_and_refuses_a_vertex_outside_it() {
    let (grid, ocean) = flat_grid(4, 5.0, &[], &[(1, 1)]);
    let floors = tiled_floors(&grid, &ocean, 4, 1).unwrap();
    assert_eq!(floors.res(), 4);
    assert_eq!(floors.floor(Face(0), 1, 1).unwrap(), 0);
    assert!(floors.floor(Face(0), 5, 0).is_err());
}
