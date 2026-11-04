//! The cost of `ErosionLayer::range_over_cap` and `change_at` at production geometry (98,304 cells a face edge, tiles of 3,072): every tile is the same decoded core, so the numbers are the
//! descent's, not a store's. A dev tool for the runtime layer read (GEN-102, GEN-112); not a perf measurement of the viewer.
//!
//! `cargo run --release -p planet --example layer_cap_cost`

use std::sync::Arc;
use std::time::Instant;

use planet_core::hash::SplitMix64;
use planet_core::{Face, Vec3};
use planet_generators::bake::tile::{TileGrid, TileId, TileResult, TileStats};
use planet_generators::erosion_layer::{ErosionLayer, LayerSource, LayerTile};

struct One {
    grid: TileGrid,
    tile: Arc<LayerTile>,
}

impl LayerSource for One {
    fn grid(&self) -> &TileGrid {
        &self.grid
    }
    fn tile(&self, _: Face, _: u32, _: u32) -> Result<Option<Arc<LayerTile>>, String> {
        Ok(Some(self.tile.clone()))
    }
    fn global_range(&self) -> (f64, f64) {
        (-500.0, 500.0)
    }
    fn id(&self) -> u64 {
        1
    }
}

fn main() {
    let t = 3072;
    let core: Vec<f32> = (0..t * t).map(|k| (libm::sinf((k % t) as f32 * 0.01) + libm::cosf((k / t) as f32 * 0.013)) * 100.0).collect();
    let stats = TileStats {
        land_cells: 0,
        thermal_passes: 0,
        thermal_moving_at_cap: 0,
        eroded_m3: 0.0,
        deposited_m3: 0.0,
        exported_m3: 0.0,
        clamped_cells: 0,
    };
    let tile = Arc::new(
        LayerTile::new(
            Arc::new(TileResult { id: TileId { face: Face(0), ti: 0, tj: 0 }, core, eroded: Vec::new(), deposited: Vec::new(), stats }),
            t,
        )
        .unwrap(),
    );
    let grid = TileGrid { cells: 98_304, tile: t, halo: 256, radius_m: 6_371_000.0 };
    let layer = ErosionLayer::new(Arc::new(One { grid, tile })).unwrap();
    let mut rng = SplitMix64(1);
    let dirs: Vec<Vec3> = (0..200)
        .map(|_| Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized())
        .collect();
    let start = Instant::now();
    for d in &dirs {
        layer.change_at(*d).unwrap();
    }
    println!("change_at: {:.2} us a read", start.elapsed().as_secs_f64() * 1e6 / dirs.len() as f64);
    for r in [1e-6, 1e-5, 1e-4, 1e-3, 3e-3, 1e-2, 3e-2, 6e-2, 0.12, 0.3] {
        let start = Instant::now();
        let mut taken = 0;
        for d in &dirs {
            taken += usize::from(layer.range_over_cap(*d, r).unwrap().is_some());
        }
        println!(
            "range_over_cap radius {r:e} rad: {:.1} us a call ({taken} of {} read the pyramids)",
            start.elapsed().as_secs_f64() * 1e6 / dirs.len() as f64,
            dirs.len()
        );
    }
}
