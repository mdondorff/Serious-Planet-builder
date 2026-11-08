//! SPIKE-105 F1 (spike branch only, never merged): cost and basin sizes of a coarse planet-wide routing. `f1_coarse_routing CELLS THREADS` samples face 2 of seed 14 at CELLS cells per
//! edge (halo 4), floods and routes it, prints times and the catchments.

use planet_core::Face;
use planet_generators::bake::face::{sample_base, FaceSpec};
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::route::route_weighted;
use planet_generators::bake::Grid;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cells: usize = a[1].parse().unwrap();
    let threads: usize = a[2].parse().unwrap();
    let (seed, radius, sea) = (14u64, 6_371_000.0f64, 0.0f64);
    let t0 = std::time::Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    println!("BakeInputs::new: {:.1} s", t0.elapsed().as_secs_f64());
    let spec = FaceSpec { face: Face(2), cells, radius_m: radius, sea_level: sea, seed, halo: 4 };
    let t1 = std::time::Instant::now();
    let z = sample_base(&spec, &|d| inputs.base(d), threads);
    let t_base = t1.elapsed().as_secs_f64();
    let m = spec.grid_cells();
    let dx = spec.dx();
    println!("cells {cells}, grid {m} x {m} = {} cells, dx {:.0} m", m * m, dx);
    println!("sample_base: {t_base:.2} s");
    let grid = Grid::new(m, m, dx);
    let outlet = grid.outlets(&z, sea);
    let t2 = std::time::Instant::now();
    let f = priority_flood(&grid, &z, &outlet, 1e-3);
    let t_flood = t2.elapsed().as_secs_f64();
    drop(f);
    println!("priority_flood alone: {t_flood:.2} s");
    let t3 = std::time::Instant::now();
    let r = route_weighted(&grid, &z, &outlet, 1e-3, seed, 0.0, None);
    let t_route = t3.elapsed().as_secs_f64();
    println!("route_weighted (flood+receivers+MFD+order): {t_route:.2} s");
    let own = dx * dx;
    let land = z.iter().filter(|v| **v > sea).count();
    let land_area = land as f64 * own;
    let (mut sea_basins, mut edge_basins) = (0usize, 0usize);
    let mut list: Vec<(f64, bool)> = Vec::new();
    for p in 0..grid.len() {
        if outlet[p] && r.area[p] > 1.5 * own {
            let is_sea = z[p] <= sea;
            if is_sea { sea_basins += 1 } else { edge_basins += 1 }
            list.push((r.area[p], is_sea));
        }
    }
    list.sort_by(|x, y| y.0.total_cmp(&x.0));
    println!("land cells {land} ({:.3e} km2 of grid); outlets with >= 1 land donor: {} (sea outlets {sea_basins}, grid-edge land outlets {edge_basins})", land_area / 1e6, list.len());
    for (i, (ar, s)) in list.iter().take(10).enumerate() {
        println!("  #{}: {:.3e} km2 ({})", i + 1, ar / 1e6, if *s { "sea outlet" } else { "grid edge" });
    }
    for thr in [1e5, 1e6, 3e6] {
        let s: f64 = list.iter().filter(|(ar, _)| ar / 1e6 > thr).map(|(ar, _)| ar - own).sum();
        let n = list.iter().filter(|(ar, _)| ar / 1e6 > thr).count();
        println!("catchments > {thr:.0e} km2: {n} basins, {:.1} % of land area", 100.0 * s / land_area);
    }
    println!("total {:.1} s", t0.elapsed().as_secs_f64());
}
