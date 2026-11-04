//! The latitude and longitude of the centre of tiles of the erosion layer's grid (98,304 cells a face edge, tiles of 3,072): for the poses of a still over a layer tile.
//!
//! `cargo run --release -p planet --example tile_latlon -- FACE/TI/TJ ...` (for example `2/12/10 0/8/24 0/14/3`).

use planet_core::Face;
use planet_generators::bake::tile::TileGrid;

fn main() {
    let grid = TileGrid { cells: 98_304, tile: 3072, halo: 128, radius_m: 6_371_000.0 };
    for arg in std::env::args().skip(1) {
        let p: Vec<usize> = arg.split('/').map(|v| v.parse().expect("FACE/TI/TJ")).collect();
        let d = grid.direction(Face(p[0] as u8), (p[1] * 3072 + 1536) as i64, (p[2] * 3072 + 1536) as i64);
        // The planet's +z is the pole (COORD): latitude from z, longitude from x and y.
        println!("{arg}: lat {:.4} lon {:.4}", libm::asin(d.z.clamp(-1.0, 1.0)).to_degrees(), libm::atan2(d.y, d.x).to_degrees());
    }
}
