//! Helpers of the ocean tests (GEN-022), shared by `ocean.rs` (tier A) and `oracle_ocean_sweep.rs` (tier B).
#![allow(dead_code)]

use std::collections::HashSet;

use planet_core::cube::TangentWarp;
use planet_core::hash::hash_u64s;
use planet_core::Face;
use planet_generators::global::{GlobalGrid, GlobalSample};
use planet_generators::ocean::{reference_ocean, tiled_ocean, CubeIndex};

pub const SEA: f32 = -1.0;
pub const LAND: f32 = 1.0;

pub fn sample(continent: f32) -> GlobalSample {
    GlobalSample { continent, elevation_m: 0.0, temperature_c: 0.0, precipitation_mm: 0.0 }
}

/// A bake whose sea is a set of nodes: every copy of a node gets the same value, as a real bake's shared vertices do.
pub fn grid_from_nodes(res: u32, sea_nodes: &HashSet<u64>) -> GlobalGrid {
    let index = CubeIndex::new(res);
    let n = (res + 1) as usize;
    let faces = std::array::from_fn(|f| {
        let mut v = Vec::with_capacity(n * n);
        for j in 0..=res {
            for i in 0..=res {
                v.push(sample(if sea_nodes.contains(&index.node_id(Face(f as u8), i, j)) { SEA } else { LAND }));
            }
        }
        v
    });
    GlobalGrid { seed: 0, res, faces }
}

pub fn node_set(res: u32, pred: &dyn Fn(Face, u32, u32) -> bool) -> HashSet<u64> {
    let index = CubeIndex::new(res);
    let mut set = HashSet::new();
    for f in 0..6u8 {
        for j in 0..=res {
            for i in 0..=res {
                if pred(Face(f), i, j) {
                    set.insert(index.node_id(Face(f), i, j));
                }
            }
        }
    }
    set
}

pub fn random_grid(res: u32, seed: u64, percent: u64) -> GlobalGrid {
    let index = CubeIndex::new(res);
    let set = node_set(res, &|f, i, j| hash_u64s(seed, &[index.node_id(f, i, j)]) % 100 < percent);
    grid_from_nodes(res, &set)
}

/// The tiled result equals the serial reference for the table's components and every bit.
pub fn assert_equal_to_reference(grid: &GlobalGrid, cells: u32, threads: usize, fraction: f64, what: &str) {
    let (rt, rg) = reference_ocean(grid, &TangentWarp, fraction).unwrap();
    let (tt, tg) = tiled_ocean(grid, &TangentWarp, cells, threads, fraction).unwrap();
    assert_eq!(rt.components, tt.components, "{what}: components, blocks of {cells}, {threads} threads");
    assert_eq!(rt.hash(), tt.hash(), "{what}");
    assert_eq!(rg, tg, "{what}: ocean bits, blocks of {cells}, {threads} threads");
    assert_eq!(rg.hash(), tg.hash());
}
