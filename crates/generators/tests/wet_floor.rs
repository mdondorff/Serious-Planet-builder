//! SPIKE-21: the three passes of the wet floor equal the serial priority flood for every block size and thread count.

use planet_core::cube::TangentWarp;
use planet_core::Face;
use planet_generators::global::{bake, GlobalGrid, GlobalSample};
use planet_generators::ocean::{block_tiles, grid_block, parallel_map, tiled_ocean, CubeIndex, OceanBits, OceanGrid};
use planet_generators::wet_floor::{block_codes, pass_a, pass_b, pass_c, reference, INF};

fn ocean_bits_of(grid: &OceanGrid, tile: planet_core::TileId, cells: u32) -> OceanBits {
    let mut b = OceanBits::zeros(cells);
    for l in 0..=cells {
        for k in 0..=cells {
            if grid.get(tile.face(), tile.x() * cells + k, tile.y() * cells + l) {
                b.set(k, l);
            }
        }
    }
    b
}

/// The tiled floor of every vertex, compared with the reference; returns the number of vertices compared and of finite floors.
fn assert_equal_to_reference(grid: &GlobalGrid, ocean: &OceanGrid, cells: u32, threads: usize, what: &str) -> (usize, usize) {
    let res = grid.res;
    let index = CubeIndex::new(res);
    let want = reference(grid, ocean);
    let tiles = block_tiles(res, cells).unwrap();
    let blocks: Vec<(Vec<GlobalSample>, OceanBits)> =
        tiles.iter().map(|t| (grid_block(grid, *t, cells), ocean_bits_of(ocean, *t, cells))).collect();
    let codes: Vec<Vec<u16>> = blocks.iter().map(|(s, o)| block_codes(cells, s, o)).collect();
    let idx: Vec<usize> = (0..tiles.len()).collect();
    let edges = parallel_map(&idx, threads, &|&i| pass_a(index, tiles[i], cells, &codes[i]));
    let border = pass_b(&edges);
    let floors = parallel_map(&idx, threads, &|&i| pass_c(index, tiles[i], cells, &codes[i], &border));
    let (mut compared, mut finite) = (0, 0);
    for (i, t) in tiles.iter().enumerate() {
        let n = cells as usize + 1;
        for l in 0..n {
            for k in 0..n {
                let (gi, gj) = (t.x() * cells + k as u32, t.y() * cells + l as u32);
                let node = index.node_id(t.face(), gi, gj);
                let got = floors[i][l * n + k];
                assert_eq!(
                    got,
                    want[node as usize],
                    "{what}: face {:?} block {:?} vertex ({k}, {l}) = ({gi}, {gj}) cells {cells} threads {threads}",
                    t.face(),
                    (t.x(), t.y())
                );
                compared += 1;
                finite += usize::from(got != INF);
            }
        }
    }
    (compared, finite)
}

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
    // One value per node so the copies on shared face edges agree (the bake's function of the direction does).
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

// spec: GEN-022
#[test]
fn random_grids_give_the_serial_floor_for_every_block_size_and_thread_count() {
    let mut finite_total = 0;
    for (res, low) in [(4u32, 70u64), (8, 60), (16, 55), (16, 70), (32, 60)] {
        for seed in 0..3u64 {
            let (grid, ocean) = random_grid(res, 100 * seed + u64::from(res), low);
            let mut sizes = vec![res, (res / 2).max(1), (res / 4).max(1), 1];
            sizes.sort_unstable();
            sizes.dedup();
            for cells in sizes {
                for threads in [1usize, 3] {
                    let (_, finite) = assert_equal_to_reference(&grid, &ocean, cells, threads, &format!("res {res} low {low} seed {seed}"));
                    finite_total += finite;
                }
            }
        }
    }
    assert!(finite_total > 1000, "the grids have reachable floors ({finite_total})");
}

// spec: GEN-022
#[test]
fn a_real_bake_gives_the_serial_floor_for_every_block_size_and_thread_count() {
    for (seed, res) in [(1u64, 64u32), (7, 128)] {
        let grid = bake(seed, &TangentWarp, res, 4).unwrap();
        let (_, ocean) = tiled_ocean(&grid, &TangentWarp, res, 2, 0.02).unwrap();
        for cells in [4u32, 16, 32, res] {
            for threads in [1usize, 3] {
                let (n, finite) = assert_equal_to_reference(&grid, &ocean, cells, threads, &format!("seed {seed} res {res}"));
                assert!(finite > n / 10, "a real bake has plenty of reachable floors ({finite} of {n})");
            }
        }
    }
}
