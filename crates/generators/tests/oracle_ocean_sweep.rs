//! GEN-022: the full thread-count and block-size sweeps of the tiled ocean against the serial reference (tier B).
//! The small variants of the same two tests run in `ocean.rs` (tier A, `cargo xtask test fast`).

use planet_core::cube::TangentWarp;
use planet_generators::global::bake;
use planet_generators::ocean::EARTH_OCEAN_MIN_AREA_FRACTION;

#[path = "support/ocean.rs"]
mod support;
use support::*;

// spec: GEN-022
#[test]
fn a_real_bake_gives_the_serial_reference_for_every_block_size_and_thread_count() {
    for (seed, res) in [(1u64, 64u32), (7, 128)] {
        let grid = bake(seed, &TangentWarp, res, 4).unwrap();
        for fraction in [0.0, 0.0005, 0.02, EARTH_OCEAN_MIN_AREA_FRACTION, 0.5, 1.0] {
            for cells in [4u32, 16, 32, res] {
                for threads in [1usize, 3] {
                    assert_equal_to_reference(&grid, cells, threads, fraction, &format!("seed {seed} res {res} fraction {fraction}"));
                }
            }
        }
    }
}

// spec: GEN-022
#[test]
fn random_sea_masks_percolate_alike_in_the_serial_and_the_tiled_computation() {
    // Site percolation on the cube graph near its threshold: many components, big ones crossing blocks, faces and corners.
    for (res, percent) in [(1u32, 70u64), (2, 60), (4, 55), (16, 60), (16, 70), (32, 55), (32, 62)] {
        for seed in 0..3u64 {
            let grid = random_grid(res, 1000 * seed + u64::from(res), percent);
            let mut sizes = vec![res, (res / 2).max(1), (res / 4).max(1), 1];
            sizes.sort_unstable();
            sizes.dedup();
            for cells in sizes {
                for (threads, fraction) in [(1usize, 0.0), (4, 0.0), (2, 0.01), (3, 0.1)] {
                    assert_equal_to_reference(&grid, cells, threads, fraction, &format!("res {res} {percent} % seed {seed}"));
                }
            }
        }
    }
}
