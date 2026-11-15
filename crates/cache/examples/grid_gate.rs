//! SPIKE-119 probe (spike code on spike/river-suite, never merged): main's river gate (`planet_cache::river_gate::measure`, BUILD-055, unchanged) on a raw height grid, plus the drained area.
//! `grid_gate <heights> <w> <h> <cell_m> <sea_level_m> <border_cells> <area_out.f32|-> [start heights]`
//! - heights: raw little-endian f32 or f64 (by the file's size), row-major, w x h;
//! - the outlets are the grid's edge and every cell at or below `sea_level_m` (Grid::outlets);
//! - counted inside a border of `border_cells` (the edge cells drain out by construction);
//! - the start surface is the heights themselves unless a start file is given (the erosion depth is then against it);
//! - area_out: the drained area (m2, f32) of `route_rules` on the plain flood (epsilon 1 mm, seed 7, default round rules), the same routing as the river review's canyon split; "-" writes none.
//! Prints the gate's report and a COUNTS line: cells, land, sea, river cells, river cells by drained area class.
use planet_cache::river_gate::{measure, GateInput, GateParams};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

fn read(path: &str, n: usize) -> Vec<f64> {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    if b.len() == n * 8 {
        b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
    } else if b.len() == n * 4 {
        b.chunks_exact(4).map(|c| f64::from(f32::from_le_bytes(c.try_into().unwrap()))).collect()
    } else {
        panic!("{path}: {} bytes, expected {} (f32) or {} (f64)", b.len(), n * 4, n * 8)
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 7 {
        panic!("usage: grid_gate <heights> <w> <h> <cell_m> <sea_level_m> <border_cells> <area_out.f32|-> [start heights]");
    }
    let (w, h): (usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap());
    let cell: f64 = a[3].parse().unwrap();
    let sea: f64 = a[4].parse().unwrap();
    let border: usize = a[5].parse().unwrap();
    let surface = read(&a[0], w * h);
    let start = a.get(7).map_or_else(|| surface.clone(), |p| read(p, w * h));
    let g = Grid::new(w, h, cell);
    let outlet = g.outlets(&surface, sea);
    let inside: Vec<bool> = (0..w * h).map(|c| (border..w - border).contains(&(c % w)) && (border..h - border).contains(&(c / w))).collect();
    let region = vec![true; w * h];
    let params = GateParams::default();
    let report = measure(
        &GateInput {
            grid: &g,
            start: &start,
            surface: &surface,
            outlet: &outlet,
            inside: &inside,
            region: &region,
            h1: &[],
            seed: 7,
            sea_level_m: sea,
            outside_drains: false,
        },
        &params,
    );
    println!("GRID input {} ({w} x {h} cells of {cell} m, sea level {sea} m, border {border})", a[0]);
    for l in report.lines() {
        println!("GRID gate {l}");
    }
    let r = route_rules(&g, &surface, &outlet, 1.0e-3, 7, 0.0, None, RoundRules::default());
    let mut area = vec![cell * cell; w * h];
    for &c in r.order.iter().rev() {
        let c = c as usize;
        if r.receiver[c] != NONE {
            area[r.receiver[c] as usize] += area[c];
        }
    }
    let land = (0..w * h).filter(|&c| inside[c] && surface[c] > sea).count();
    let total = inside.iter().filter(|&&b| b).count();
    let classes = [75e6, 300e6, 1e9, 1e10, 1e11];
    let mut by = vec![0usize; classes.len()];
    for c in 0..w * h {
        if inside[c] && surface[c] > sea {
            for (k, t) in classes.iter().enumerate() {
                if area[c] >= *t {
                    by[k] += 1;
                }
            }
        }
    }
    let amax = (0..w * h).filter(|&c| inside[c]).map(|c| area[c]).fold(0.0, f64::max);
    println!(
        "GRID counts: {total} cells inside, {land} land, {} sea or outlet; river cells (area >= 75 km2) {}; >= 300 km2 {}; >= 1,000 km2 {}; >= 10,000 km2 {}; >= 100,000 km2 {}; largest drained area inside {:.0} km2",
        total - land,
        by[0],
        by[1],
        by[2],
        by[3],
        by[4],
        amax / 1e6
    );
    if a[6] != "-" {
        let bytes: Vec<u8> = area.iter().flat_map(|v| (*v as f32).to_le_bytes()).collect();
        std::fs::write(&a[6], bytes).expect("area written");
    }
}
