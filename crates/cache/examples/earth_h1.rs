//! SPIKE-134 #134b (spike branch only): the Earth reference for the kept H1 basins. `earth_h1 <grid.f64> <w> <h> <sea_m> [<name>]` reads a prepared GLO-30 grid
//! (L cells of 101.8 m, row-major f64, SPIKE-119's prep), reduces it over 8 x 8 cells (814 m) by the mean, the minimum or the median (`[name] [mean|min|median]`), runs H1 on it (the canonical flood, the clipped MFD routing and
//! `hydrology` with GEN-145's defaults: initiation 75 km2, pit limit 0.5 m, artefact floor 4 cells or 2 m; no runoff weights, no open-water term) and prints the
//! share of channel cells that lie in kept basins, counted inside a one-cell border.
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (path, w, h, sea) = (&a[0], a[1].parse::<usize>().map_err(|_| "w")?, a[2].parse::<usize>().map_err(|_| "h")?, a[3].parse::<f64>().map_err(|_| "sea")?);
    let name = a.get(4).cloned().unwrap_or_default();
    // the block statistic: mean (default), min (K6''s node height), median
    let stat = a.get(5).cloned().unwrap_or_else(|| "mean".into());
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let z: Vec<f64> = bytes.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().expect("8 bytes"))).collect();
    if z.len() != w * h {
        return Err(format!("{path}: {} values, not {w} x {h}", z.len()));
    }
    let (cw, ch) = (w / 8, h / 8);
    let mut zc = vec![0f64; cw * ch];
    for j in 0..ch {
        for i in 0..cw {
            let mut v: Vec<f64> = (0..64).map(|k| z[(j * 8 + k / 8) * w + i * 8 + k % 8]).collect();
            zc[j * cw + i] = match stat.as_str() {
                "min" => v.iter().copied().fold(f64::INFINITY, f64::min),
                "median" => {
                    v.sort_by(f64::total_cmp);
                    v[32]
                }
                _ => v.iter().sum::<f64>() / 64.0,
            };
        }
    }
    let grid = Grid::new(cw, ch, 8.0 * 101.75);
    let outlet = grid.outlets(&zc, sea);
    let eps = 1e-6;
    let routing = route_rules(&grid, &zc, &outlet, eps, 0x6731_E0, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&grid, &zc, &outlet, eps, true);
    let hy = hydrology(&grid, &zc, &filled, &routing, &outlet, None, None, &HydroParams::default());
    let inner = |c: usize| {
        let (x, y) = (c % cw, c / cw);
        x >= 1 && y >= 1 && x + 1 < cw && y + 1 < ch
    };
    let chan = (0..cw * ch).filter(|&c| inner(c) && hy.channel[c] && !outlet[c]).count();
    let in_b = (0..cw * ch).filter(|&c| inner(c) && hy.channel[c] && !outlet[c] && hy.label[c] != u32::MAX).count();
    let mut depths: Vec<f64> = hy.basins.iter().map(|b| b.spill_m - b.floor_m).collect();
    depths.sort_by(f64::total_cmp);
    let basin_cells = (0..cw * ch).filter(|&c| inner(c) && hy.label[c] != u32::MAX).count();
    println!(
        "EARTH {name} {stat}: {cw} x {ch} cells of 814 m; channel cells {chan}, in kept basins {in_b} ({:.2} %); kept basins {} (depth p50 {:.1} max {:.1} m), basin cells {basin_cells} of {} ({:.2} %)",
        100.0 * in_b as f64 / chan.max(1) as f64,
        hy.basins.len(),
        depths.get(depths.len() / 2).copied().unwrap_or(0.0),
        depths.last().copied().unwrap_or(0.0),
        (cw - 2) * (ch - 2),
        100.0 * basin_cells as f64 / ((cw - 2) * (ch - 2)) as f64
    );
    Ok(())
}
