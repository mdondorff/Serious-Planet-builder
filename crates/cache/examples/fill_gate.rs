//! The gate and preview of the fill depth (keep-pits-seams PR 3, planner inbox #67, #69, #70): on the production layer of `--seed` (pass 1 of face 2 and of the face across its +i edge from `--pass1 DIR`,
//! a store whose kind 12 records were made under the same carve), pass 2 and the post-pass over two regions with margins, and then the numbers the gate asks for:
//! - the region around the tile seam 2/12/10 | 2/13/10 (tiles 11..14 x 9..11 of face 2);
//! - the region around a mountain tile on the +i cube edge of face 2 and its neighbour across the edge (found by scanning the class lattice), each with its ring of tiles;
//! - the filled surface's (exact base + stored change + depth) height step across the tile seam and across the cube edge, against the step between interior neighbours;
//! - the flatness of the filled cells inside the basins, the share of cells clamped at 0 (outcrops), the pit share and depth per class on 2/12/10, the codes at their cap, and the inland cells at
//!   or below the sea level (the post-pass's outlets) in the seam region.
//! Writes only under `--out` (default target/bake/fill-gate). Not part of the product.

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_fill::{load_fill, load_heights};
use planet_cache::layer_fillpass::run_fill_pass;
use planet_cache::layer_store::load_face_change;
use planet_cache::layer_tiles::{bake_specs, face_change_reader, layer_tile_id, load_tile, run_pass2, BakeShape, Pass2Inputs};
use planet_cache::pack::TileStore;
use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};
use planet_generators::bake::face::{fill_m, FaceChange};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::{TileGrid, TileId as PassTile};
use planet_generators::classes::{CLASS_COUNT, CLASS_NAMES, HIGH_MOUNTAINS, MOUNTAINS};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;

fn quantile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

fn stats(name: &str, v: &mut Vec<f64>) {
    println!("{name}: n {} p50 {:.3} p99 {:.3} max {:.3}", v.len(), quantile(v, 0.5), quantile(v, 0.99), quantile(v, 1.0));
}

fn share_over(v: &[f64], m: f64) -> f64 {
    100.0 * v.iter().filter(|x| **x > m).count() as f64 / v.len().max(1) as f64
}

/// The heights of a cell pair across an edge: (unfilled, filled) on each side and the interior control pair of side 1.
type Pair = ((f64, f64), (f64, f64), (f64, f64), (f64, f64));

fn report(what: &str, pairs: &[Pair]) {
    let mut edge_f: Vec<f64> = pairs.iter().map(|p| (p.0 .1 - p.1 .1).abs()).collect();
    let mut edge_u: Vec<f64> = pairs.iter().map(|p| (p.0 .0 - p.1 .0).abs()).collect();
    let mut ctl_f: Vec<f64> = pairs.iter().map(|p| (p.2 .1 - p.3 .1).abs()).collect();
    let mut excess: Vec<f64> = pairs.iter().map(|p| (p.0 .1 - p.1 .1).abs() - (p.2 .1 - p.3 .1).abs()).collect();
    let basin: Vec<usize> = (0..pairs.len()).filter(|&k| pairs[k].0 .1 != pairs[k].0 .0 || pairs[k].1 .1 != pairs[k].1 .0).collect();
    println!("\n== {what}: {} edge pairs, {} with a pit on either side", pairs.len(), basin.len());
    println!(
        "share of edge pairs whose filled step is over 0.5 m: {:.2} % (interior control {:.2} %, unfilled {:.2} %)",
        share_over(&edge_f, 0.5),
        share_over(&ctl_f, 0.5),
        share_over(&edge_u, 0.5)
    );
    stats("filled edge step (m)", &mut edge_f);
    stats("unfilled edge step (m)", &mut edge_u);
    stats("filled interior control step (m)", &mut ctl_f);
    stats("filled edge step minus its interior control (m)", &mut excess);
    if !basin.is_empty() {
        let mut f: Vec<f64> = basin.iter().map(|&k| (pairs[k].0 .1 - pairs[k].1 .1).abs()).collect();
        let mut add: Vec<f64> = basin.iter().map(|&k| (pairs[k].0 .1 - pairs[k].1 .1).abs() - (pairs[k].0 .0 - pairs[k].1 .0).abs()).collect();
        println!("pairs with a pit: share filled step over 0.5 m {:.2} %", share_over(&f, 0.5));
        stats("  filled edge step at pits (m)", &mut f);
        stats("  the fill's addition to the step at pits (m)", &mut add);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.windows(2).find(|w| w[0] == k).map(|w| w[1].clone());
    let seed: u64 = arg("--seed").map_or(14, |v| v.parse().expect("seed"));
    let pass1_dir = PathBuf::from(arg("--pass1").expect("--pass1 DIR (a store with the pass-1 faces)"));
    let work = PathBuf::from(arg("--out").unwrap_or_else(|| "target/bake/fill-gate".into()));
    let _ = std::fs::remove_dir_all(&work);
    let (scratch_dir, tile_dir) = (work.join("scratch"), work.join("tiles"));
    std::fs::create_dir_all(&scratch_dir).expect("scratch dir");
    std::fs::create_dir_all(&tile_dir).expect("tile dir");
    let (threads, note) = planet_streaming::topology::bake_threads(0).unwrap_or_else(|e| panic!("{e}"));
    println!("fill gate: seed {seed}, {threads} threads ({note})");
    let def = WorldDefinition::single_planet(RADIUS_M, seed);
    let inputs = Arc::new(BakeInputs::new(seed, threads));
    let world = GlobalWorld::from_definition(&def).expect("world").with_class_source(inputs.clone());
    let (base, factor) = (|d: Vec3| inputs.base(d), |d: Vec3| inputs.carve_factor(d));
    let shape = BakeShape::production(seed, RADIUS_M);
    let (layer, spec) = bake_specs(&world, &shape).expect("specs");
    let g: TileGrid = spec.grid;
    let (n, t) = (g.cells as i64, g.tile as i64);
    let tpe = g.tiles_per_edge() as i64;
    let (res, rp) = (shape.face_res as u32, shape.planet_res as u32);
    let cell = |c: f64| ((c + 1.0) * 0.5 * n as f64).floor().clamp(0.0, n as f64 - 1.0) as i64;
    let across = |i: i64, j: i64| {
        let (f, s, u) = direction_to_face(&TangentWarp, g.direction(Face(2), i, j));
        (f, cell(s), cell(u))
    };
    // A mountain tile on the +i edge of face 2: the class lattice sampled 24 x 24 in each edge tile; its neighbour across the edge must be one tile for the whole row.
    let mut best: Option<(f64, i64)> = None;
    for tj in 2..tpe - 2 {
        let mut mountains = 0usize;
        for jj in 0..24 {
            for ii in 0..24 {
                let d = g.direction(Face(2), (tpe - 1) * t + ii * t / 24, tj * t + jj * t / 24);
                let w = inputs.weights_at(d);
                mountains += usize::from(w[MOUNTAINS] + w[HIGH_MOUNTAINS] > 0.5);
            }
        }
        let share = mountains as f64 / 576.0;
        let (a, b) = (across(n, tj * t), across(n, tj * t + t - 1));
        if a.0 == b.0 && (a.1 / t, a.2 / t) == (b.1 / t, b.2 / t) && best.is_none_or(|(s, _)| share > s) {
            best = Some((share, tj));
        }
    }
    let (share, cj) = best.expect("a mountain tile on the cube edge whose row maps to one neighbouring tile");
    let (nb0, nb1) = (across(n, cj * t), across(n, cj * t + t - 1));
    let (face_b, tile_b) = (nb0.0, (nb0.1 / t, nb0.2 / t));
    assert!(nb1.0 == face_b);
    println!("cube edge: face 2 tile ({}, {cj}) ({:.0} % mountain classes) against face {} tile {tile_b:?}", tpe - 1, 100.0 * share, face_b.0);
    // The regions.
    let mut region: BTreeSet<PassTile> = BTreeSet::new();
    for ti in 11..=14u32 {
        for tj in 9..=11u32 {
            region.insert(PassTile { face: Face(2), ti, tj });
        }
    }
    let seam_tiles: Vec<PassTile> = region.iter().copied().collect();
    for ti in [tpe - 2, tpe - 1] {
        for tj in cj - 1..=cj + 1 {
            region.insert(PassTile { face: Face(2), ti: ti as u32, tj: tj as u32 });
        }
    }
    for di in -1..=1i64 {
        for dj in -1..=1i64 {
            let (a, b) = (tile_b.0 + di, tile_b.1 + dj);
            if (0..tpe).contains(&a) && (0..tpe).contains(&b) {
                region.insert(PassTile { face: face_b, ti: a as u32, tj: b as u32 });
            }
        }
    }
    let tiles: Vec<PassTile> = region.iter().copied().collect();
    println!("{} tiles in the run (seam region {}, the rest around the cube edge)", tiles.len(), seam_tiles.len());
    // Pass 1 faces from the store (read only: the kind 12 records).
    let p1 = TileStore::open(&pass1_dir, world.params()).expect("pass-1 store");
    let changes: Vec<FaceChange> = (0..6u8)
        .map(|f| {
            if f == 2 || Face(f) == face_b {
                load_face_change(&p1, &world, res, &layer, Face(f)).expect("readable").expect("a pass-1 face in the store")
            } else {
                let m = layer.cells + 2 * layer.halo;
                FaceChange { face: Face(f), cells: layer.cells, halo: layer.halo, change: vec![0.0; m * m] }
            }
        })
        .collect();
    let changes: [FaceChange; 6] = changes.try_into().expect("six faces");
    let reader = face_change_reader(&changes);
    let clock = Instant::now();
    run_pass2(
        &tile_dir,
        &world,
        rp,
        &spec,
        &Pass2Inputs { base: &base, face_change: &reader, factor: Some(&factor), scratch: Some(&scratch_dir) },
        &tiles,
        threads,
        &mut |r| println!("pass 2 tile {}/{}/{}: {:.0} s so far", r.id.face.0, r.id.ti, r.id.tj, clock.elapsed().as_secs_f64()),
    )
    .expect("pass 2");
    let clock = Instant::now();
    let fp = run_fill_pass(&tile_dir, &scratch_dir, &world, rp, &spec, &tiles, threads, &base, &mut |_, _| {}).expect("fill pass");
    println!(
        "\nfill pass {:.0} s: {} tiles written, {} pit cells, deepest {:.2} m, {} cells clamped at 0 ({:.2} % of pit-surface cells), {} cells no path reaches, {} pairs dropped",
        clock.elapsed().as_secs_f64(),
        fp.written.len(),
        fp.pit_cells,
        fp.deepest_m,
        fp.clamped_cells,
        100.0 * fp.clamped_cells as f64 / (fp.pit_cells + fp.clamped_cells).max(1) as f64,
        fp.unreached_cells,
        fp.dropped_pairs
    );
    let tstore = TileStore::open(&tile_dir, world.params()).expect("tile store");
    let sstore = TileStore::open(&scratch_dir, world.params()).expect("scratch store");
    let _ = layer_tile_id;
    let dir = |id: PassTile, i: i64, j: i64| g.direction(id.face, i64::from(id.ti) * t + i, i64::from(id.tj) * t + j);
    let load = |id: PassTile| (load_tile(&tstore, &world, rp, &spec, id).unwrap().unwrap().core, load_fill(&tstore, &world, rp, &spec, id).unwrap().unwrap());
    // The (unfilled, filled) height of a core cell on the surface the runtime shows.
    let heights = |id: PassTile, data: &(Vec<f32>, Vec<u16>), i: i64, j: i64| -> (f64, f64) {
        let q = (j * t + i) as usize;
        let u = base(dir(id, i, j)) + f64::from(data.0[q]);
        (u, u + fill_m(data.1[q]))
    };
    // 1. The tile seam and the cube edge.
    let (a_id, b_id) = (PassTile { face: Face(2), ti: 12, tj: 10 }, PassTile { face: Face(2), ti: 13, tj: 10 });
    let (a, b) = (load(a_id), load(b_id));
    let pairs: Vec<Pair> = (0..t).map(|j| (heights(a_id, &a, t - 1, j), heights(b_id, &b, 0, j), heights(a_id, &a, t - 2, j), heights(a_id, &a, t - 1, j))).collect();
    report("tile seam 2/12/10 | 2/13/10", &pairs);
    let (c_id, d_id) = (PassTile { face: Face(2), ti: (tpe - 1) as u32, tj: cj as u32 }, PassTile { face: face_b, ti: tile_b.0 as u32, tj: tile_b.1 as u32 });
    let (c, d) = (load(c_id), load(d_id));
    let pairs: Vec<Pair> = (0..t)
        .map(|j| {
            let (_, ci, cj2) = across(n, cj * t + j);
            let other = heights(d_id, &d, ci - i64::from(d_id.ti) * t, cj2 - i64::from(d_id.tj) * t);
            (heights(c_id, &c, t - 1, j), other, heights(c_id, &c, t - 2, j), heights(c_id, &c, t - 1, j))
        })
        .collect();
    report("cube edge, mountain tile on face 2 | neighbour", &pairs);
    // 2. The flatness of the filled cells (the unclamped ones: a clamped cell holds depth 0) inside the basins, on the two preview tiles.
    for (name, id, data) in [("2/12/10", a_id, &a), ("cube-edge tile", c_id, &c)] {
        let tn = t as usize;
        let mask: Vec<bool> = data.1.iter().map(|c| *c > 0).collect();
        let mut label = vec![false; tn * tn];
        let mut dev: Vec<f64> = Vec::new();
        let (mut comps, mut cells_in) = (0usize, 0usize);
        for start in 0..tn * tn {
            if !mask[start] || label[start] {
                continue;
            }
            let mut members = vec![start];
            label[start] = true;
            let mut queue = VecDeque::from([start]);
            while let Some(p) = queue.pop_front() {
                let (i, j) = (p % tn, p / tn);
                for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let (ii, jj) = (i as i64 + di, j as i64 + dj);
                    if ii < 0 || jj < 0 || ii >= t || jj >= t {
                        continue;
                    }
                    let q = jj as usize * tn + ii as usize;
                    if mask[q] && !label[q] {
                        label[q] = true;
                        members.push(q);
                        queue.push_back(q);
                    }
                }
            }
            if members.len() < 20 {
                continue;
            }
            comps += 1;
            cells_in += members.len();
            let mut v: Vec<f64> = members.iter().map(|&q| heights(id, data, (q % tn) as i64, (q / tn) as i64).1).collect();
            let med = quantile(&mut v, 0.5);
            dev.extend(v.iter().map(|x| (x - med).abs()));
        }
        println!("\n== flatness of the filled floor, tile {name}: {comps} basins of 20 cells or more, {cells_in} cells (deviation from the basin's median surface, m)");
        stats("filled surface", &mut dev);
        println!("codes at the cap: {}", data.1.iter().filter(|c| **c == u16::MAX).count());
    }
    // 3. The pit share and depth per class on 2/12/10 (fill-probe2: plain 4.6 %, mountains 5.3 %, high mountains 13.5 %).
    {
        let tn = t as usize;
        let par: &dyn Par = &threads;
        let rows: Vec<Vec<(u8, bool)>> = par::map(par, tn, &|j| {
            (0..tn)
                .map(|i| {
                    let dd = dir(a_id, i as i64, j as i64);
                    let w = inputs.weights_at(dd);
                    let c = (0..CLASS_COUNT).fold(0, |bst, k| if w[k] > w[bst] { k } else { bst });
                    (c as u8, base(dd) > 0.0)
                })
                .collect()
        });
        let mut per: Vec<(usize, usize, Vec<f64>)> = vec![(0, 0, Vec::new()); CLASS_COUNT];
        for (p, (c, land)) in rows.iter().flatten().enumerate() {
            if !*land {
                continue;
            }
            let e = &mut per[*c as usize];
            e.0 += 1;
            if a.1[p] > 0 {
                e.1 += 1;
                e.2.push(fill_m(a.1[p]));
            }
        }
        println!("\n== pit share and depth per class, tile 2/12/10");
        for (k, (land, pits, dep)) in per.iter_mut().enumerate() {
            if *land > 0 {
                println!(
                    "{:<16} land {:>9} pits {:>8} ({:>5.1} %) depth p50 {:>6.1} p95 {:>6.1} max {:>6.1} m",
                    CLASS_NAMES[k],
                    land,
                    pits,
                    100.0 * *pits as f64 / *land as f64,
                    quantile(dep, 0.5),
                    quantile(dep, 0.95),
                    quantile(dep, 1.0)
                );
            }
        }
    }
    // 4. Inland cells at or below the sea level in the seam region (the post-pass's outlets): the mosaic of the region's scratch heights, components of cells at or below 0 that do not touch the
    // mosaic's border (a component that does may be the sea or a basin cut by the region).
    {
        let (cols, rows_n) = (4usize, 3usize);
        let (w, h) = (cols * t as usize, rows_n * t as usize);
        let mut low = vec![false; w * h];
        for (ci, ti) in (11..=14u32).enumerate() {
            for (rj, tj) in (9..=11u32).enumerate() {
                let id = PassTile { face: Face(2), ti, tj };
                let z = load_heights(&sstore, &world, rp, &spec, id).unwrap().unwrap();
                for (p, v) in z.iter().enumerate() {
                    let (x, y) = (ci * t as usize + p % t as usize, rj * t as usize + p / t as usize);
                    low[y * w + x] = *v <= 0.0;
                }
            }
        }
        let total_low = low.iter().filter(|b| **b).count();
        let mut seen = vec![false; w * h];
        let (mut inland_cells, mut inland_comps, mut border_cells, mut largest) = (0usize, 0usize, 0usize, 0usize);
        for s in 0..w * h {
            if !low[s] || seen[s] {
                continue;
            }
            let (mut size, mut touches) = (0usize, false);
            let mut stack = vec![s];
            seen[s] = true;
            while let Some(p) = stack.pop() {
                size += 1;
                let (x, y) = (p % w, p / w);
                touches |= x == 0 || y == 0 || x == w - 1 || y == h - 1;
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                        if xx >= 0 && yy >= 0 && xx < w as i64 && yy < h as i64 {
                            let q = yy as usize * w + xx as usize;
                            if low[q] && !seen[q] {
                                seen[q] = true;
                                stack.push(q);
                            }
                        }
                    }
                }
            }
            if touches {
                border_cells += size;
            } else {
                inland_cells += size;
                inland_comps += 1;
                largest = largest.max(size);
            }
        }
        println!(
            "\n== cells at or below the sea level in the seam region ({} x {} cells): {total_low} in all; inland (not touching the region's border): {inland_cells} cells in {inland_comps} basins (largest {largest}); touching it: {border_cells} cells",
            w, h
        );
    }
}
