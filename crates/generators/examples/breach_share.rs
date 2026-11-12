#![allow(clippy::needless_range_loop, clippy::too_many_arguments, dead_code)]
//! GEN-174: what the round-0 breach does to the start surface of a window: basins drained and kept, cells cut, the share of land in closed basins before and after. `breach_share [threads]`; env `PLANET_WIN`, `PLANET_MARGIN`.
use planet_core::{Face, Vec3};
use planet_generators::bake::breach::{breach_basins, PreBreach};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::{lifted_flats, priority_flood_ungraded};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::Grid;
use std::time::Instant;

const R: f64 = 6_371_000.0;
const EPS: f64 = 1e-3;

struct GenWin {
    grid: Grid,
    z0: Vec<f64>,
    outlet: Vec<bool>,
    w: usize,
    h: usize,
    origin: (usize, usize),
    boxx: (usize, usize, usize, usize),
    stream: u64,
}

fn gen_setup(threads: usize) -> GenWin {
    let win = std::env::var("PLANET_WIN").expect("PLANET_WIN=face,cells,i0,i1,j0,j1,seed");
    let v: Vec<u64> = win.split(',').map(|x| x.trim().parse().expect("integers")).collect();
    let (face, cells, i0b, i1b, j0b, j1b, seed) =
        (v[0] as u8, v[1] as usize, v[2] as usize, v[3] as usize, v[4] as usize, v[5] as usize, v[6]);
    let margin: usize = std::env::var("PLANET_MARGIN").ok().map_or(200, |s| s.parse().unwrap());
    let (i0, i1) = (i0b.saturating_sub(margin), (i1b + margin).min(cells - 1));
    let (j0, j1) = (j0b.saturating_sub(margin), (j1b + margin).min(cells - 1));
    let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
    let spec = FaceSpec { face: Face(face), cells, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let t = Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    let mut z0 = vec![0.0f64; w * h];
    let rows = h.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in z0.chunks_mut(rows * w).enumerate() {
            let (inputs, spec) = (&inputs, &spec);
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * w + c;
                    let d: Vec3 = spec.halo_direction((i0 + p % w) as i64, (j0 + p / w) as i64);
                    *v = inputs.base(d);
                }
            });
        }
    });
    let grid = Grid::new(w, h, spec.dx()).with_origin((i0 as u32, j0 as u32)).with_metric(Metric::cube_face(
        cells,
        R,
        (i0 as i64, j0 as i64),
        w,
        h,
    ));
    let outlet = grid.outlets(&z0, 0.0);
    println!(
        "== window face {face} cells {cells} seed {seed}: i {i0}-{i1} j {j0}-{j1} = {w} x {h}; box i {i0b}-{i1b} j {j0b}-{j1b}; land {:.1} %; base sampled in {:.1} s",
        100.0 * outlet.iter().filter(|o| !**o).count() as f64 / (w * h) as f64,
        t.elapsed().as_secs_f64()
    );
    GenWin { grid, z0, outlet, w, h, origin: (i0, j0), boxx: (i0b - i0, i1b - i0, j0b - j0, j1b - j0), stream: spec.stream() }
}

fn closed_share(g: &GenWin, z: &[f64], min_cells: usize) -> (usize, usize, usize) {
    let filled = priority_flood_ungraded(&g.grid, z, &g.outlet, EPS, true);
    let flats = lifted_flats(&g.grid, z, &g.outlet, EPS, &filled);
    let big: Vec<_> = flats.iter().filter(|f| f.cells.len() > min_cells && f.max_lift_m >= 0.5).collect();
    (flats.iter().filter(|f| f.max_lift_m >= 0.5).count(), big.len(), big.iter().map(|f| f.cells.len()).sum())
}

fn main() {
    let threads: usize = std::env::args().nth(1).map_or(8, |s| s.parse().unwrap());
    let g = gen_setup(threads);
    let land = g.outlet.iter().filter(|o| !**o).count();
    let (all, big, cells) = closed_share(&g, &g.z0, 30);
    println!(
        "start surface: land {land}; closed basins (lift >= 0.5 m) {all}; over 30 cells {big} holding {cells} cells = {:.1} % of land",
        100.0 * cells as f64 / land as f64
    );
    {
        let mut z = g.z0.clone();
        for pass in 1..=6 {
            let rep =
                breach_basins(&g.grid, &mut z, &g.outlet, EPS, &PreBreach { passes: 1, halo: 128, max_depth_m: 500.0, min_lift_m: 0.5 });
            let (all, big, cells) = closed_share(&g, &z, 30);
            println!("pass {pass}: basins {}, drained {}, kept {} (+{} bound); after: closed basins {all}, over 30 cells {big} = {:.1} % of land", rep.basins, rep.drained, rep.kept_lake, rep.kept_bound, 100.0 * cells as f64 / land as f64);
        }
    }
    for depth in [20.0, 50.0, 100.0, 500.0] {
        let mut z = g.z0.clone();
        let t = Instant::now();
        let rep = breach_basins(&g.grid, &mut z, &g.outlet, EPS, &PreBreach { passes: 1, halo: 128, max_depth_m: depth, min_lift_m: 0.5 });
        let (all, big, cells) = closed_share(&g, &z, 30);
        let moved = z.iter().zip(&g.z0).filter(|(a, b)| a != b).count();
        println!(
            "max depth {depth:>5} m: {} basins, drained {}, kept {} lakes ({} cells) + {} bound; cut {} cells, {:.1} m summed ({:.2} s); after: closed basins {all}, over 30 cells {big} = {:.1} % of land; cells changed {moved}",
            rep.basins, rep.drained, rep.kept_lake, rep.cells_in_kept_lakes, rep.kept_bound, rep.cells_cut, rep.cut_m_cells, t.elapsed().as_secs_f64(), 100.0 * cells as f64 / land as f64
        );
    }
}
