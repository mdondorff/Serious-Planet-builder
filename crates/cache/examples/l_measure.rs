//! G-STORE (STRM-078, GEN-156): the stored local change L of the K7 chain, kind 13, measured on the new chain.
//! Reads `target/k7_l/<ti>_<tj>.l.f32` and `.land` (written by `generators/examples/k7_region 200 8 store`: the core of each 1,024-cell tile, f32 metres, and its land mask), encodes each core with the
//! layer-tile counts stream (0.25 m i16 counts, row deltas, brotli), checks the round trip against the quantisation step, and prints the bytes a land cell and the Earth-scale extrapolation.
//! Usage: l_measure
#![allow(clippy::chunks_exact_to_as_chunks)]
use planet_cache::layer_tiles::{decode_change_core, encode_change_core};

const T: usize = 1024;
/// Local land cells of an Earth-sized body at the 102 m local grid: 1.49e8 km2 over (0.102 km)^2.
const EARTH_LOCAL_LAND_CELLS: f64 = 1.49e8 / (0.102 * 0.102);

fn main() {
    let l_dir = std::env::var("PLANET_L_DIR").unwrap_or_else(|_| "target/k7_l".to_string());
    let mut tiles: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(&l_dir)
        .expect("run `k7_region 200 8 store` first (it writes target/k7_l, or PLANET_L_DIR)")
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            let n = p.file_name()?.to_str()?.to_string();
            n.strip_suffix(".l.f32").map(|id| (id.to_string(), p.clone()))
        })
        .collect();
    tiles.sort();
    let (mut bytes, mut land_cells, mut cells, mut worst) = (0usize, 0usize, 0usize, 0.0f64);
    let mut per_cell: Vec<f64> = Vec::new();
    for (id, path) in &tiles {
        let raw = std::fs::read(path).expect("a change file");
        let change: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        assert_eq!(change.len(), T * T, "tile {id}: a core of {T} x {T} cells");
        let land = std::fs::read(path.with_file_name(format!("{id}.land"))).expect("a land mask");
        let n_land = land.iter().filter(|&&l| l != 0).count();
        let enc = encode_change_core(&change, T);
        let dec = decode_change_core(&enc, T).expect("a stream that decodes");
        let err = change.iter().zip(&dec).map(|(a, b)| f64::from((a - b).abs())).fold(0.0, f64::max);
        worst = worst.max(err);
        assert!(err <= 0.125 + 1e-4, "tile {id}: the round trip is within half a step, not {err} m");
        bytes += enc.len();
        land_cells += n_land;
        cells += T * T;
        if n_land > 0 {
            per_cell.push(enc.len() as f64 / n_land as f64);
        }
        println!(
            "tile {id}: {} B for {n_land} land cells ({:.2} B a land cell), round trip max error {err:.3} m",
            enc.len(),
            enc.len() as f64 / n_land.max(1) as f64
        );
    }
    per_cell.sort_by(|a, b| a.total_cmp(b));
    let bpc = bytes as f64 / land_cells.max(1) as f64;
    println!("{} tiles, {cells} cells, {land_cells} land cells, {bytes} B encoded; worst round trip error {worst:.3} m", tiles.len());
    if let Some(&hi) = per_cell.last() {
        println!(
            "B a land cell: all tiles {bpc:.3}, lowest tile {:.3}, median {:.3}, highest tile {hi:.3}",
            per_cell[0],
            per_cell[per_cell.len() / 2]
        );
    }
    println!(
        "Earth-scale extrapolation ({:.2e} local land cells at 102 m): {:.1} GB at the pooled rate, {:.1} GB at the highest tile's rate (the prototype region is one mountain-and-coast area: a measurement of that area, not a global figure)",
        EARTH_LOCAL_LAND_CELLS,
        bpc * EARTH_LOCAL_LAND_CELLS / 1e9,
        per_cell.last().copied().unwrap_or(0.0) * EARTH_LOCAL_LAND_CELLS / 1e9
    );
}
