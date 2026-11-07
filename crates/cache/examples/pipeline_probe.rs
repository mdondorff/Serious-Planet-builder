//! SPIKE-104 probe (spike branch only, never merged): where the closed basins of pass 2's grid come from. `pipeline_probe G0_DIR TI TJ THREADS` reads the face-2 pass-1 store
//! (`G0_DIR/layer`) and the pass-2 tile store (`G0_DIR/tiles`) of a `bake layer --class-source` run (seed 14, defaults, G = 0) and, on the core of pass-2 tile (2, TI, TJ) at 101.8 m,
//! floods several surfaces (open at the core's border and the sea) and prints for each: pits (cells more than 0.5 m under the flood) as share of land, depth p50/p95/max,
//! volume, and the 3 km relief p50 (non-overlapping 30-cell blocks, land only). It also prints the differences between the base as sampled by each stage.

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::load_face_change;
use planet_cache::layer_tiles::{bake_specs, load_tile, BakeShape};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::map::change_in_face;
use planet_generators::bake::tile::{sample_window_coarse, TileId};
use planet_generators::bake::Grid;
use planet_world_def::WorldDefinition;

fn q(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p) as usize]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let (ti, tj, threads): (u32, u32, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap());
    let seed = 14u64;
    let t0 = std::time::Instant::now();
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let inputs = std::sync::Arc::new(BakeInputs::new(seed, threads));
    let world = GlobalWorld::from_definition(&def).unwrap().with_class_source(inputs.clone());
    let shape = BakeShape { seed, radius_m: 6_371_000.0, face_res: 12_288, planet_res: 98_304, tile: None, halo: None };
    let (layer, spec) = bake_specs(&world, &shape).unwrap();
    let g = spec.grid;
    let id = TileId { face: Face(2), ti, tj };
    let base = |d: Vec3| inputs.base(d);
    // The base as each stage sees it, over the window: exact (stride 1), pass 2 (every 4 cells, 407 m), the store's lattice spacing (every 6 cells, 611 m: the runtime's
    // global16384 lattice, here anchored at the window, not at the store's vertices), pass 1's spacing (every 8 cells, 814 m).
    let mut bases = Vec::new();
    for s in [1usize, 4, 6, 8] {
        bases.push(sample_window_coarse(&g, id, &base, threads, s));
        eprintln!("base stride {s}: {:.0} s", t0.elapsed().as_secs_f64());
    }
    let p1 = TileStore::open(&dir.join("layer"), world.params()).unwrap();
    let fc = load_face_change(&p1, &world, 12_288, &layer, Face(2)).unwrap().expect("a pass-1 face");
    let fcw = sample_window_coarse(&g, id, &|d: Vec3| change_in_face(&fc, d), threads, 1);
    let ts = TileStore::open(&dir.join("tiles"), world.params()).unwrap();
    let r = load_tile(&ts, &world, 98_304, &spec, id).unwrap().expect("the pass-2 tile");
    let (w, t, h) = (g.window(), g.tile, g.halo);
    let core = |v: &[f64]| -> Vec<f64> { (0..t * t).map(|c| v[(c / t + h) * w + c % t + h]).collect() };
    let cb: Vec<Vec<f64>> = bases.iter().map(|b| core(b)).collect();
    let cfc = core(&fcw);
    let ch: Vec<f64> = r.core.iter().map(|v| f64::from(*v)).collect();
    let sea = layer.sea_level;
    // Differences between the stage samplings, on land of the exact base.
    let land: Vec<bool> = cb[0].iter().map(|v| *v > sea).collect();
    for (name, x, y) in [
        ("exact - pass 1's 814 m view", 0usize, 3usize),
        ("exact - pass 2's 407 m base", 0, 1),
        ("611 m lattice - pass 2's 407 m base (runtime base against the base the change was taken on)", 2, 1),
    ] {
        let mut d: Vec<f64> = (0..t * t).filter(|c| land[*c]).map(|c| cb[x][c] - cb[y][c]).collect();
        let rms = (d.iter().map(|v| v * v).sum::<f64>() / d.len().max(1) as f64).sqrt();
        let mut ad: Vec<f64> = d.iter().map(|v| v.abs()).collect();
        println!("{name}: rms {rms:.1} m, |d| p50 {:.1} p99 {:.1} max {:.1} m, d p1 {:.1} p99 {:.1} m", q(&mut ad, 0.5), q(&mut ad, 0.99), q(&mut ad, 1.0), q(&mut d, 0.01), q(&mut d, 0.99));
    }
    // Pass 1's carved surface on its own grid (base at the cell centres plus the change) over the tile's footprint and a margin, read at the 102 m cells by the C1 read (as pass 2
    // reads the change) and by the nearest pass-1 cell (no interpolation): if the C1 read makes the pits, the nearest read keeps pass 1's drainage.
    let fspec = planet_generators::bake::face::FaceSpec { face: Face(2), cells: 12_288, radius_m: layer.radius_m, sea_level: sea, seed, halo: fc.halo };
    let ratio = 98_304 / 12_288;
    let (m1, h1) = (fc.cells + 2 * fc.halo, fc.halo as i64);
    let (c0i, c0j) = ((ti as usize * t / ratio) as i64, (tj as usize * t / ratio) as i64);
    let mut carved = fc.clone();
    for j1 in c0j - 8..c0j + (t / ratio) as i64 + 8 {
        for i1 in c0i - 8..c0i + (t / ratio) as i64 + 8 {
            let idx = ((j1 + h1) as usize) * m1 + (i1 + h1) as usize;
            carved.change[idx] = (inputs.base(fspec.halo_direction(i1, j1)) + f64::from(fc.change[idx])) as f32;
        }
    }
    let (gi0, gj0) = (ti as i64 * t as i64, tj as i64 * t as i64);
    let core_dir = |c: usize| g.direction(Face(2), gi0 + (c % t) as i64, gj0 + (c / t) as i64);
    let nearest = |f: &planet_generators::bake::face::FaceChange, c: usize| -> f64 {
        let (i1, j1) = ((gi0 + (c % t) as i64).div_euclid(ratio as i64), (gj0 + (c / t) as i64).div_euclid(ratio as i64));
        f64::from(f.change[((j1 + h1) as usize) * m1 + (i1 + h1) as usize])
    };
    let s_i: Vec<f64> = (0..t * t).map(|c| change_in_face(&carved, core_dir(c))).collect();
    let s_j: Vec<f64> = (0..t * t).map(|c| nearest(&carved, c)).collect();
    let fc_near: Vec<f64> = (0..t * t).map(|c| nearest(&fc, c)).collect();
    let grid = Grid::new(t, t, g.dx());
    let add = |x: &[f64], y: &[f64]| -> Vec<f64> { x.iter().zip(y).map(|(p, q)| p + q).collect() };
    // Pass 2's input breached at 102 m (pass 1's breach rule, no incision, no uplift, 4 routes): does a breach at this grid reopen what the interpolation dams?
    let breached = |limit: f64| -> Vec<f64> {
        let mut z = add(&cb[1], &cfc);
        let outlet = grid.outlets(&z, sea);
        let p = planet_generators::bake::carve::CarveParams {
            u_over_k: 0.0,
            k_dt: 0.0,
            steps: 4,
            reroute_every: 1,
            breach_max_m: limit,
            uplift_taper_m: 0.0,
            sea_level: sea,
            ..Default::default()
        };
        let t1 = std::time::Instant::now();
        planet_generators::bake::carve::carve(&grid, &mut z, &outlet, &p, None, 7);
        eprintln!("breach {limit} m: {:.1} s", t1.elapsed().as_secs_f64());
        z
    };
    let (b50, b150, b500) = (breached(50.0), breached(150.0), breached(500.0));
    let lowered = |z: &[f64]| -> String {
        let d0 = add(&cb[1], &cfc);
        let mut l: Vec<f64> = z.iter().zip(&d0).map(|(a, b)| b - a).filter(|v| *v > 0.01).collect();
        let n = l.len();
        format!("{:.2} % of cells lowered, p50 {:.1} m, p95 {:.1} m", 100.0 * n as f64 / (t * t) as f64, q(&mut l.clone(), 0.5), q(&mut l, 0.95))
    };
    for (n, z) in [("50", &b50), ("150", &b150), ("500", &b500)] {
        println!("breach {n} m at 102 m: {}", lowered(z));
    }
    let surfaces: Vec<(&str, Vec<f64>)> = vec![
        ("L D breached at 102 m, limit 50 m", b50),
        ("M D breached at 102 m, limit 150 m", b150),
        ("N D breached at 102 m, limit 500 m", b500),
        ("A exact base (102 m)", cb[0].clone()),
        ("B pass 2's base (407 m nodes)", cb[1].clone()),
        ("C pass 1's view (814 m nodes)", cb[3].clone()),
        ("D pass 2's input: B + pass-1 change (C1)", add(&cb[1], &cfc)),
        ("E pass 2's output: B + stored change", add(&cb[1], &ch)),
        ("F runtime-like: 611 m base + stored change", add(&cb[2], &ch)),
        ("G exact base + stored change", add(&cb[0], &ch)),
        ("H exact base + pass-1 change (no pass 2)", add(&cb[0], &cfc)),
        ("I pass 1's carved surface, C1 read at 102 m", s_i),
        ("J pass 1's carved surface, nearest pass-1 cell", s_j),
        ("K pass 2's base + pass-1 change, nearest cell", add(&cb[1], &fc_near)),
    ];
    println!("surface | land in pits | depth p50 / p95 / max (m) | volume (m3) | 3 km relief p50 (m)");
    for (name, z) in &surfaces {
        let outlet = grid.outlets(z, sea);
        let filled = priority_flood(&grid, z, &outlet, 1e-3);
        let (mut nland, mut depths, mut vol) = (0usize, Vec::new(), 0.0f64);
        for c in 0..t * t {
            if z[c] > sea {
                nland += 1;
            }
            let d = filled[c] - z[c];
            if d > 0.5 {
                depths.push(d);
                vol += d * g.dx() * g.dx();
            }
        }
        let share = 100.0 * depths.len() as f64 / nland.max(1) as f64;
        let mut rel = Vec::new();
        let b = 30;
        for bj in 0..t / b {
            for bi in 0..t / b {
                let (mut lo, mut hi, mut l) = (f64::INFINITY, f64::NEG_INFINITY, 0);
                for j in bj * b..bj * b + b {
                    for i in bi * b..bi * b + b {
                        let v = z[j * t + i];
                        lo = lo.min(v);
                        hi = hi.max(v);
                        if v > sea {
                            l += 1;
                        }
                    }
                }
                if l * 2 > b * b {
                    rel.push(hi - lo);
                }
            }
        }
        println!(
            "{name} | {share:.1} % | {:.1} / {:.1} / {:.1} | {vol:.3e} | {:.0}",
            q(&mut depths.clone(), 0.5),
            q(&mut depths.clone(), 0.95),
            q(&mut depths, 1.0),
            q(&mut rel, 0.5)
        );
    }
    eprintln!("done in {:.0} s", t0.elapsed().as_secs_f64());
}
