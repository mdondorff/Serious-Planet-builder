//! GEN-022: ocean connectivity in the global bake (ADR 0041 point 2).

use std::collections::HashSet;

use planet_core::cube::TangentWarp;
use planet_core::hash::hash_u64s;
use planet_core::{Face, TileId};
use planet_generators::global::{bake, GlobalGrid, GLOBAL_VERSION, SEA_LEVEL_F32};
use planet_generators::ocean::{
    analyse_block, block_tiles, grid_block, is_sea, ocean_bits, reference_ocean, threshold_fp, tiled_ocean, vertex_area_fp, CubeIndex,
    OceanBits, OceanError, OceanGrid, OceanLattice, OceanMerger, EARTH_OCEAN_MIN_AREA_FRACTION, SPHERE_AREA_FP,
};

#[path = "support/ocean.rs"]
mod support;
use support::*;

// spec: GEN-022
#[test]
fn nodes_merge_the_copies_on_shared_face_edges() {
    for res in [1u32, 2, 4, 16] {
        let index = CubeIndex::new(res);
        let mut ids = HashSet::new();
        let (mut twos, mut threes) = (0, 0);
        for f in 0..6u8 {
            for j in 0..=res {
                for i in 0..=res {
                    let copies = index.copies(Face(f), i, j);
                    let id = index.node_id(Face(f), i, j);
                    ids.insert(id);
                    // Every copy names the same node, and the node id is the smallest raw id of its copies.
                    for &(g, a, b) in copies.as_slice() {
                        assert_eq!(index.node_id(g, a, b), id, "res {res} face {f} ({i}, {j}) copy of face {} ({a}, {b})", g.0);
                        assert!(index.copies(g, a, b).as_slice().iter().any(|&(h, c, d)| (h, c, d) == (Face(f), i, j)));
                    }
                    assert_eq!(id, copies.as_slice().iter().map(|&(g, a, b)| index.raw(g, a, b)).min().unwrap());
                    if index.raw(Face(f), i, j) == id {
                        match copies.as_slice().len() {
                            2 => twos += 1,
                            3 => threes += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
        let r = u64::from(res);
        // 6 (res - 1)^2 interior vertices, 12 (res - 1) on edges (2 copies), 8 corners (3 copies).
        assert_eq!(ids.len() as u64, 6 * (r - 1) * (r - 1) + 12 * (r - 1) + 8, "res {res}");
        assert_eq!(twos, 12 * (res - 1), "res {res}");
        assert_eq!(threes, 8, "res {res}");
    }
}

// spec: GEN-022
#[test]
fn vertex_areas_add_up_to_the_sphere_and_the_block_table_equals_the_vertex_formula() {
    for res in [1u32, 2, 8, 64] {
        let mut sum = 0u64;
        for j in 0..=res {
            for i in 0..=res {
                sum += vertex_area_fp(&TangentWarp, res, i, j);
            }
        }
        // Six faces; each vertex is rounded to the fixed point once.
        let vertices = 6.0 * f64::from(res + 1) * f64::from(res + 1);
        let sphere = SPHERE_AREA_FP;
        assert!((6.0 * sum as f64 - sphere).abs() <= vertices / 2.0 + 1.0, "res {res}: {} against {sphere}", 6.0 * sum as f64);
    }
    // A lone sea block hands its areas to the components: the area of an all-sea face is a sixth of the sphere.
    let res = 16;
    let all_sea = GlobalGrid { seed: 0, res, faces: std::array::from_fn(|_| vec![sample(SEA); 17 * 17]) };
    let (t, _) = tiled_ocean(&all_sea, &TangentWarp, 4, 3, 0.0).unwrap();
    assert_eq!(t.components.len(), 1);
    let direct: u64 = (0..6)
        .map(|_| (0..=res).flat_map(|j| (0..=res).map(move |i| (i, j))).map(|(i, j)| vertex_area_fp(&TangentWarp, res, i, j)).sum::<u64>())
        .sum();
    assert_eq!(t.components[0].area_fp, direct, "the block corner table gives each vertex the area of the vertex formula");
    // 6 (res - 1)^2 + 12 (res - 1) + 8 nodes.
    assert_eq!(t.components[0].nodes, 6 * 15 * 15 + 12 * 15 + 8);
    // A dual cell of the middle of a face is about (pi / 2 / res)^2 of the unit sphere (cell width pi/2 over res, a little less
    // at the corner of the face): within 1 percent, a check of the formula against geometry written out independently.
    let mid = vertex_area_fp(&TangentWarp, 64, 32, 32) as f64 / (SPHERE_AREA_FP / (4.0 * core::f64::consts::PI));
    let cell = core::f64::consts::FRAC_PI_2 / 64.0;
    let expected = cell * cell;
    assert!((mid / expected - 1.0).abs() < 0.01, "{mid} against {expected}");
}

// spec: GEN-022
#[test]
fn a_real_bake_gives_the_serial_reference_for_every_block_size_and_thread_count() {
    // The small variant (tier A): one seed at 64 cells, one thread against three; the full sweep (two seeds, six area
    // fractions, 128 cells) is `oracle_ocean_sweep.rs` (tier B).
    let (seed, res) = (1u64, 64u32);
    let grid = bake(seed, &TangentWarp, res, 4).unwrap();
    for fraction in [0.0, 0.02, EARTH_OCEAN_MIN_AREA_FRACTION, 1.0] {
        for cells in [4u32, 16, res] {
            for threads in [1usize, 3] {
                assert_equal_to_reference(&grid, cells, threads, fraction, &format!("seed {seed} res {res} fraction {fraction}"));
            }
        }
    }
}

// spec: GEN-022
#[test]
fn random_sea_masks_percolate_alike_in_the_serial_and_the_tiled_computation() {
    // Site percolation on the cube graph near its threshold: many components, big ones crossing blocks, faces and corners.
    // The small variant (tier A): fewer grids and thread counts 1, 2 and 3; the full sweep is `oracle_ocean_sweep.rs` (tier B).
    for (res, percent) in [(1u32, 70u64), (4, 55), (16, 60), (32, 62)] {
        for seed in 0..2u64 {
            let grid = random_grid(res, 1000 * seed + u64::from(res), percent);
            let mut sizes = vec![res, (res / 2).max(1), (res / 4).max(1), 1];
            sizes.sort_unstable();
            sizes.dedup();
            for cells in sizes {
                for (threads, fraction) in [(1usize, 0.0), (2, 0.01), (3, 0.1)] {
                    assert_equal_to_reference(&grid, cells, threads, fraction, &format!("res {res} {percent} % seed {seed}"));
                }
            }
        }
    }
}

// spec: GEN-022
#[test]
fn the_order_blocks_report_in_does_not_change_the_table() {
    let grid = random_grid(32, 5, 60);
    let res = 32;
    let tiles = block_tiles(res, 4).unwrap();
    let analyses: Vec<_> = tiles.iter().map(|t| analyse_block(&TangentWarp, res, *t, 4, &grid_block(&grid, *t, 4)).unwrap()).collect();
    let table = |order: &[usize]| {
        let mut m = OceanMerger::new(res, 0.0).unwrap();
        for &i in order {
            m.add(analyses[i].clone());
        }
        m.finish()
    };
    let forward: Vec<usize> = (0..tiles.len()).collect();
    let reverse: Vec<usize> = forward.iter().rev().copied().collect();
    let mut shuffled = forward.clone();
    shuffled.sort_by_key(|i| hash_u64s(9, &[*i as u64]));
    let t = table(&forward);
    assert_eq!(t, table(&reverse));
    assert_eq!(t, table(&shuffled));
    assert!(t.components.len() > 3, "the mask makes several components");
}

// spec: GEN-022
#[test]
fn a_dry_basin_behind_a_one_vertex_wall_stays_dry_and_the_ocean_is_one_component() {
    let res = 32;
    // Face 4 (+Z): the open sea is the band i <= 10; the wall i = 11 is land; the basin is 12 <= i <= 15 and 10 <= j <= 20; a far
    // pond is one vertex. The band touches the face edge i = 0, so the copies of those vertices on the neighbouring face
    // are sea too (one node), and the band is joined across faces.
    let set =
        node_set(res, &|f, i, j| f == Face(4) && (i <= 10 || ((12..=15).contains(&i) && (10..=20).contains(&j)) || (i, j) == (25, 25)));
    let grid = grid_from_nodes(res, &set);
    let band_nodes = set.len() as u64;
    let (probe, _) = reference_ocean(&grid, &TangentWarp, 0.0).unwrap();
    assert_eq!(probe.components.len(), 3, "the band, the basin and the pond");
    let mut by_area = probe.components.clone();
    by_area.sort_by_key(|c| std::cmp::Reverse(c.area_fp));
    assert!(by_area[0].area_fp > 3 * by_area[1].area_fp, "the band is much larger than the basin");
    assert_eq!(by_area.iter().map(|c| c.nodes).sum::<u64>(), band_nodes);
    // A threshold between the basin and the band.
    let fraction = (by_area[1].area_fp as f64 * 2.0) / SPHERE_AREA_FP;
    for (cells, threads) in [(4u32, 1usize), (8, 3), (32, 2)] {
        let (table, ocean) = tiled_ocean(&grid, &TangentWarp, cells, threads, fraction).unwrap();
        let oceans: Vec<_> = table.ocean_components().collect();
        assert_eq!(oceans.len(), 1);
        assert_eq!(oceans[0].label, by_area[0].label);
        for j in 10..=20 {
            for i in 12..=15 {
                assert!(!ocean.get(Face(4), i, j), "the basin vertex ({i}, {j}) stays dry");
            }
        }
        assert!(!ocean.get(Face(4), 25, 25), "the pond stays dry");
        assert!(!ocean.get(Face(4), 11, 15), "the wall is land");
        assert!(ocean.get(Face(4), 10, 15), "the vertex next to the wall is ocean");
        // The band reaches the face edge i = 0; its copies on the other face are ocean as well.
        let copies = CubeIndex::new(res).copies(Face(4), 0, 15);
        assert_eq!(copies.as_slice().len(), 2);
        for &(f, i, j) in copies.as_slice() {
            assert!(ocean.get(f, i, j), "copy on face {}", f.0);
        }
        // The band is 11 x 33 = 363 vertices of face 4 (363 nodes); its edge vertices have copies on the neighbouring faces: 31 + 10
        // + 10 on edges (one more copy each) and the two cube corners (0, 0) and (0, 32) (two more each): 363 + 51 + 4 copies.
        assert_eq!(table.ocean_components().map(|c| c.nodes).sum::<u64>(), 363);
        assert_eq!(ocean.count(), 418);
    }
    // Reference agrees.
    let (rt, rg) = reference_ocean(&grid, &TangentWarp, fraction).unwrap();
    let (tt, tg) = tiled_ocean(&grid, &TangentWarp, 8, 2, fraction).unwrap();
    assert_eq!((rt.components.clone(), rg.hash()), (tt.components.clone(), tg.hash()));
}

// spec: GEN-022
#[test]
fn a_sea_that_wraps_a_cube_corner_is_one_component_and_a_sea_vertex_needs_no_sea_neighbour() {
    let res = 8;
    // Around the corner (+x, +y, +z): the three faces' corner vertices are one node. Sea on each face's corner 3x3 block.
    let corner_nodes = node_set(res, &|f, i, j| match f.0 {
        0 => i >= res - 2 && j >= res - 2,
        2 => i <= 2 && j >= res - 2,
        4 => i >= res - 2 && j >= res - 2,
        _ => false,
    });
    let grid = grid_from_nodes(res, &corner_nodes);
    for cells in [1u32, 2, 8] {
        assert_equal_to_reference(&grid, cells, 2, 0.0, "corner");
    }
    let (reference, _) = reference_ocean(&grid, &TangentWarp, 0.0).unwrap();
    assert_eq!(reference.components.len(), 1, "the sea of the three faces around the corner is one component");
    // One isolated sea vertex is a component of its own (no neighbour needed), counted once.
    let lone = node_set(res, &|f, i, j| (f, i, j) == (Face(1), 4, 4));
    let g = grid_from_nodes(res, &lone);
    let (t, bits) = tiled_ocean(&g, &TangentWarp, 4, 1, 0.0).unwrap();
    assert_eq!(t.components.len(), 1);
    assert_eq!((t.components[0].nodes, bits.count()), (1, 1));
}

// spec: GEN-022
#[test]
fn the_threshold_is_a_fraction_of_the_surface_and_the_earth_default_is_a_million_square_kilometres() {
    let earth_km2 = 4.0 * core::f64::consts::PI * 6371.0 * 6371.0;
    assert!((EARTH_OCEAN_MIN_AREA_FRACTION * earth_km2 - 1.0e6).abs() < 1.0);
    assert_eq!(threshold_fp(0.0), Ok(0));
    assert!(threshold_fp(1.0).unwrap() as f64 <= SPHERE_AREA_FP);
    for bad in [-0.1, 1.5, f64::NAN, f64::INFINITY] {
        assert!(matches!(threshold_fp(bad), Err(OceanError::BadThreshold(_))), "{bad}");
    }
    // All sea: the one component is the whole surface (to the rounding of the vertex areas, which a threshold of 1 would see).
    let all_sea = GlobalGrid { seed: 0, res: 4, faces: std::array::from_fn(|_| vec![sample(SEA); 25]) };
    let (t, o) = tiled_ocean(&all_sea, &TangentWarp, 4, 1, 0.999).unwrap();
    assert!(t.components[0].ocean && o.count() == 6 * 25, "every vertex copy of an all-sea bake is ocean");
    // The threshold changes the answer without changing the components.
    let g = bake(1, &TangentWarp, 64, 2).unwrap();
    let (a, _) = tiled_ocean(&g, &TangentWarp, 16, 2, 0.0).unwrap();
    let (b, bg) = tiled_ocean(&g, &TangentWarp, 16, 2, 0.9).unwrap();
    assert_eq!(
        a.components.iter().map(|c| (c.label, c.nodes, c.area_fp)).collect::<Vec<_>>(),
        b.components.iter().map(|c| (c.label, c.nodes, c.area_fp)).collect::<Vec<_>>()
    );
    assert!(bg.count() == 0 && a.ocean_components().count() == a.components.len());
}

// spec: GEN-022
#[test]
fn ocean_bits_round_trip_and_refuse_a_bad_length_or_spare_bits() {
    for cells in [1u32, 2, 3 + 1, 8] {
        let mut b = OceanBits::zeros(cells);
        for (i, (k, l)) in (0..=cells).flat_map(|l| (0..=cells).map(move |k| (k, l))).enumerate() {
            if i % 3 == 0 {
                b.set(k, l);
            }
        }
        let bytes = b.to_bytes();
        assert_eq!(bytes.len() as u64, ((u64::from(cells) + 1).pow(2)).div_ceil(8));
        assert_eq!(OceanBits::from_bytes(cells, &bytes), Some(b.clone()), "cells {cells}");
        assert_eq!(OceanBits::from_bytes(cells, &bytes[1..]), None);
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(OceanBits::from_bytes(cells, &longer), None);
        let bits = (u64::from(cells) + 1).pow(2);
        if bits % 8 != 0 {
            let mut spare = bytes.clone();
            *spare.last_mut().unwrap() |= 0x80;
            assert_eq!(OceanBits::from_bytes(cells, &spare), None, "a bit beyond the last vertex");
        }
    }
}

// spec: GEN-022
#[test]
fn shapes_that_do_not_fit_are_errors_naming_what_is_wrong() {
    let g = random_grid(8, 1, 50);
    assert!(matches!(tiled_ocean(&g, &TangentWarp, 3, 1, 0.1), Err(OceanError::BadBlock(_))));
    assert!(matches!(tiled_ocean(&g, &TangentWarp, 16, 1, 0.1), Err(OceanError::BadBlock(_))));
    assert!(matches!(OceanMerger::new(6, 0.1), Err(OceanError::BadResolution(6))));
    let t = TileId::new(Face(0), 0, 0, 0).unwrap();
    let block = grid_block(&g, t, 8);
    // A tile of the wrong level, and a sample list of the wrong length.
    assert!(matches!(analyse_block(&TangentWarp, 8, t, 4, &block), Err(OceanError::BadBlock(_))));
    assert!(matches!(analyse_block(&TangentWarp, 8, t, 8, &block[1..]), Err(OceanError::BadBlock(_))));
    let mut m = OceanMerger::new(8, 0.1).unwrap();
    m.add(analyse_block(&TangentWarp, 8, t, 8, &block).unwrap());
    let table = m.finish();
    assert!(matches!(ocean_bits(16, t, 8, &block, &table), Err(OceanError::BadBlock(_))));
}

// spec: GEN-022
#[test]
fn the_lattice_trait_reads_the_grid_and_refuses_a_vertex_beyond_it() {
    let g = random_grid(4, 3, 60);
    let (_, bits) = tiled_ocean(&g, &TangentWarp, 4, 1, 0.0).unwrap();
    assert_eq!(OceanLattice::res(&bits), 4);
    assert_eq!(bits.is_ocean(Face(0), 2, 2), Ok(bits.get(Face(0), 2, 2)));
    assert!(bits.is_ocean(Face(0), 5, 0).is_err());
    let _: &OceanGrid = &bits;
}

// spec: GEN-022
#[test]
fn known_answers_of_a_small_bake_are_pinned() {
    // The serial reference and the tiled computation agree (asserted above for these inputs) and give these values on every
    // platform. If a value moves, the bake or the area formula changed: bump GLOBAL_VERSION and re-record.
    assert_eq!(GLOBAL_VERSION, 4, "new known answers belong with a new global version");
    assert_eq!(SEA_LEVEL_F32, 0.1);
    let g = bake(1, &TangentWarp, 128, 4).unwrap();
    let (rt, rg) = reference_ocean(&g, &TangentWarp, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    let (tt, tg) = tiled_ocean(&g, &TangentWarp, 32, 3, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    assert_eq!((rt.hash(), rg.hash()), (tt.hash(), tg.hash()));
    let sea_nodes: u64 = rt.components.iter().map(|c| c.nodes).sum();
    let sea_vertices = g.faces.iter().flatten().filter(|s| is_sea(s)).count();
    assert!(sea_nodes as usize <= sea_vertices);
    assert_eq!(rt.components.len(), KNOWN_COMPONENTS);
    assert_eq!(rt.hash(), KNOWN_TABLE_HASH);
    assert_eq!(rg.hash(), KNOWN_OCEAN_HASH);
}

const KNOWN_COMPONENTS: usize = 5;
const KNOWN_TABLE_HASH: u64 = 0x2471_1406_5250_894a;
const KNOWN_OCEAN_HASH: u64 = 0x46d4_7785_2d62_df8f;

// spec: GEN-022
#[test]
fn a_block_larger_than_the_bake_limit_is_refused_and_every_copy_of_a_node_is_one_direction() {
    let t = TileId::new(Face(0), 0, 0, 0).unwrap();
    assert!(matches!(analyse_block(&TangentWarp, 65536, t, 65536, &[]), Err(OceanError::BadBlock(m)) if m.contains("1024")));
    // The node identity is the COORD-007 identity: the copies of a vertex are the same direction, bit for bit.
    for res in [1u32, 2, 16] {
        let index = CubeIndex::new(res);
        for f in 0..6u8 {
            for j in 0..=res {
                for i in 0..=res {
                    let dir = |g: Face, a: u32, b: u32| TileId::new(g, 0, 0, 0).unwrap().sample_direction(&TangentWarp, res, a, b);
                    let d = dir(Face(f), i, j);
                    for &(g, a, b) in index.copies(Face(f), i, j).as_slice() {
                        let e = dir(g, a, b);
                        assert_eq!([d.x.to_bits(), d.y.to_bits(), d.z.to_bits()], [e.x.to_bits(), e.y.to_bits(), e.z.to_bits()]);
                    }
                }
            }
        }
    }
    // A merger that did not see every block cannot be told apart from a smaller world: the build refuses it (STRM-023); here the
    // count is observable.
    let mut m = OceanMerger::new(4, 0.0).unwrap();
    assert_eq!(m.blocks(), 0);
    m.add(analyse_block(&TangentWarp, 4, TileId::new(Face(0), 0, 0, 0).unwrap(), 4, &vec![sample(SEA); 25]).unwrap());
    assert_eq!(m.blocks(), 1);
}

// spec: GEN-022
#[test]
fn the_copies_of_an_edge_vertex_of_a_real_bake_agree_on_sea_or_land() {
    // The node model needs it: one node, one sea flag.
    for (seed, res) in [(1u64, 32u32), (7, 64), (3, 128)] {
        let g = bake(seed, &TangentWarp, res, 3).unwrap();
        let index = CubeIndex::new(res);
        for f in 0..6u8 {
            for j in 0..=res {
                for i in 0..=res {
                    let here = is_sea(&g.at(Face(f), i, j));
                    for &(h, a, b) in index.copies(Face(f), i, j).as_slice() {
                        assert_eq!(is_sea(&g.at(h, a, b)), here, "seed {seed} res {res} face {f} ({i}, {j})");
                    }
                }
            }
        }
    }
}
