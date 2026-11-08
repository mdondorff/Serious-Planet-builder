//! SPIKE-105 F9 (spike branch only, never merged): how many 256-cell tiles does an E2 edit (a ridge raised across a divide) dirty in the routing, and what does the border-graph solve cost?
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::route::{route_weighted, Routing, NONE};
use planet_generators::bake::spill::{final_level, flood_labelled, perimeter_len, SpillGraph, TileFlood, TileRef};
use planet_generators::bake::Grid;
use std::time::Instant;

const THREADS: usize = 4;
const N: usize = 3072;
const T: usize = 256;
const NT: usize = N / T;

fn sample(inputs: &BakeInputs, spec: &FaceSpec, i0: i64, j0: i64, n: usize) -> Vec<f64> {
    let mut z = vec![0.0f64; n * n];
    let rows_per = n.div_ceil(THREADS);
    std::thread::scope(|s| {
        for (k, chunk) in z.chunks_mut(rows_per * n).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows_per * n + c;
                    let d: Vec3 = spec.halo_direction(i0 + (p % n) as i64, j0 + (p / n) as i64);
                    *v = inputs.base(d);
                }
            });
        }
    });
    z
}

fn tile_of(p: usize) -> usize {
    (p / N / T) * NT + (p % N) / T
}

/// Reports the share of tiles and cells flagged by `mask`, and the bounding box.
fn report(name: &str, mask: &[bool]) -> usize {
    let mut tiles = vec![false; NT * NT];
    let (mut cells, mut x0, mut y0, mut x1, mut y1) = (0usize, usize::MAX, usize::MAX, 0usize, 0usize);
    for (p, m) in mask.iter().enumerate() {
        if *m {
            cells += 1;
            tiles[tile_of(p)] = true;
            let (x, y) = (p % N, p / N);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let nt = tiles.iter().filter(|t| **t).count();
    let bb = if cells > 0 { format!("x {x0}..{x1}, y {y0}..{y1}") } else { "-".into() };
    println!("  {name:<34} tiles {nt:>3}/{} ({:>5.1} %), cells {cells:>9} ({:>6.3} %), bbox {bb}", NT * NT, 100.0 * nt as f64 / (NT * NT) as f64, 100.0 * cells as f64 / (N * N) as f64);
    nt
}

fn roots(r: &Routing) -> Vec<u32> {
    let mut root = vec![0u32; r.receiver.len()];
    for &c in &r.order {
        let c = c as usize;
        root[c] = if r.receiver[c] == NONE { c as u32 } else { root[r.receiver[c] as usize] };
    }
    root
}

/// Diffs routing `b` against `a` (surfaces `za`, `zb`).
fn diff(label: &str, grid: &Grid, eps: f64, za: &[f64], zb: &[f64], oa: &[bool], ob: &[bool], ra: &Routing, rb: &Routing) {
    println!("{label}");
    let fa = priority_flood(grid, za, oa, eps);
    let fb = priority_flood(grid, zb, ob, eps);
    let n = za.len();
    let lvl: Vec<bool> = (0..n).map(|p| fa[p] != fb[p]).collect();
    let rec: Vec<bool> = (0..n).map(|p| ra.receiver[p] != rb.receiver[p]).collect();
    let area: Vec<bool> = (0..n).map(|p| (ra.area[p] - rb.area[p]).abs() > 1e-9 * ra.area[p].abs().max(rb.area[p].abs())).collect();
    let height: Vec<bool> = (0..n).map(|p| za[p] != zb[p]).collect();
    let any: Vec<bool> = (0..n).map(|p| lvl[p] || rec[p] || area[p]).collect();
    report("surface height changed", &height);
    report("filled level changed", &lvl);
    report("receiver changed", &rec);
    report("area changed > 1e-9 rel", &area);
    report("any of level/receiver/area", &any);
    let (xa, xb) = (roots(ra), roots(rb));
    let mut changed = std::collections::HashSet::new();
    let mut cells = 0usize;
    for p in 0..n {
        if xa[p] != xb[p] {
            cells += 1;
            changed.insert(xa[p]);
            changed.insert(xb[p]);
        }
    }
    let own = grid.dx() * grid.dx();
    let big = |r: &Routing, set: &std::collections::HashSet<u32>| set.iter().filter(|c| r.area[**c as usize] > 1.5 * own).count();
    let donors_a = (0..n).filter(|&p| ra.receiver[p] == NONE && ra.area[p] > 1.5 * own).count();
    println!("  catchments (D8 roots with a donor): A has {donors_a}; roots whose membership changed {} (with a donor in A {}, in B {}); cells that changed root {cells}", changed.len(), big(ra, &changed), big(rb, &changed));
}

/// Tile floods of a surface (4 threads), tile-local outlets.
fn tile_floods(z: &[f64], outlet: &[bool]) -> Vec<TileFlood> {
    let one = |t: usize| {
        let (tx, ty) = ((t % NT) * T, (t / NT) * T);
        let mut zt = Vec::with_capacity(T * T);
        let mut ot = Vec::with_capacity(T * T);
        for y in 0..T {
            let o = (ty + y) * N + tx;
            zt.extend_from_slice(&z[o..o + T]);
            ot.extend_from_slice(&outlet[o..o + T]);
        }
        flood_labelled(T, T, &zt, &ot)
    };
    let mut out: Vec<Option<TileFlood>> = (0..NT * NT).map(|_| None).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let res = std::sync::Mutex::new(&mut out);
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| loop {
                let t = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if t >= NT * NT {
                    break;
                }
                let f = one(t);
                res.lock().unwrap()[t] = Some(f);
            });
        }
    });
    out.into_iter().map(|f| f.unwrap()).collect()
}

/// Joins the tiles and solves; returns (planet-wide levels per cell, nodes, edges, build s, solve s).
fn graph_levels(grid: &Grid, z: &[f64], tiles: &[TileFlood]) -> (Vec<f64>, usize, usize, f64, f64) {
    let t0 = Instant::now();
    let mut g = SpillGraph::new();
    let mut refs: Vec<TileRef> = Vec::new();
    let mut edges = 0usize;
    for tf in tiles {
        refs.push(g.add_tile(perimeter_len(T, T), &tf.edges));
        edges += tf.edges.len();
    }
    let mut nodes = 1usize;
    nodes += tiles.len() * perimeter_len(T, T);
    let seed_at = |p: usize| tiles[tile_of(p)].seed_of(p % N % T, p / N % T).expect("border cell");
    for p in 0..N * N {
        let (x, y) = (p % N, p / N);
        let on_border = x % T == 0 || x % T == T - 1 || y % T == 0 || y % T == T - 1;
        if !on_border {
            continue;
        }
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            if q < p || tile_of(q) == tile_of(p) {
                continue;
            }
            g.connect(refs[tile_of(p)], seed_at(p), refs[tile_of(q)], seed_at(q), z[p].max(z[q]));
            edges += 1;
        }
    }
    let build = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let lv = g.solve();
    let solve = t1.elapsed().as_secs_f64();
    let mut out = vec![0.0f64; N * N];
    for (t, tf) in tiles.iter().enumerate() {
        let fl = final_level(tf, &|l| lv.seed(refs[t], l));
        let (tx, ty) = ((t % NT) * T, (t / NT) * T);
        for y in 0..T {
            out[(ty + y) * N + tx..(ty + y) * N + tx + T].copy_from_slice(&fl[y * T..(y + 1) * T]);
        }
    }
    (out, nodes, edges, build, solve)
}

fn main() {
    let seed = 14u64;
    let inputs = BakeInputs::new(seed, THREADS);
    let sp = FaceSpec { face: Face(1), cells: 12_288, radius_m: 6_371_000.0, sea_level: 0.0, seed: 14, halo: 0 };
    // Window centred on (8096, 1228) would start at j = -308, off the face; it is moved to j0 = 0 (the edit stays at its face coordinates).
    let (i0, j0) = (8096 - (N as i64) / 2, 0i64);
    let grid = Grid::new(N, N, sp.dx()).with_origin((i0 as u32, j0 as u32));
    let t = Instant::now();
    let base = sample(&inputs, &sp, i0, j0, N);
    println!("window {N}x{N} at face 1 ({i0},{j0}), dx {:.1} m; base {:.1} s; land {:.1} %", sp.dx(), t.elapsed().as_secs_f64(), 100.0 * base.iter().filter(|v| **v > 0.0).count() as f64 / (N * N) as f64);

    // E2: ridge (2012,298)-(2037,316) x4, +200 m, smoothstep over 5 km half-width.
    let (ax, ay, bx, by) = (8048.0 - i0 as f64, 1192.0 - j0 as f64, 8148.0 - i0 as f64, 1264.0 - j0 as f64);
    let hw = 5000.0;
    let mut edited = base.clone();
    for p in 0..N * N {
        let (x, y) = ((p % N) as f64, (p / N) as f64);
        let (dx, dy) = (bx - ax, by - ay);
        let u = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        let d = ((x - ax - u * dx).powi(2) + (y - ay - u * dy).powi(2)).sqrt() * grid.dx();
        edited[p] += 200.0 * planet_core::smoothstep(0.0, 1.0, 1.0 - d / hw);
    }
    let oa = grid.outlets(&base, 0.0);
    let ob = grid.outlets(&edited, 0.0);
    println!("outlets differ: {}", (0..N * N).filter(|&p| oa[p] != ob[p]).count());
    let mut cp = CarveParams::default();
    cp.steps = 5;
    cp.reroute_every = 5;
    let eps = cp.epsilon;

    let t = Instant::now();
    let ra = route_weighted(&grid, &base, &oa, eps, seed, 0.0, None);
    println!("route_weighted A {:.1} s", t.elapsed().as_secs_f64());
    let rb = route_weighted(&grid, &edited, &ob, eps, seed, 0.0, None);
    diff("== first routing, B (edited) against A (base)", &grid, eps, &base, &edited, &oa, &ob, &ra, &rb);

    // Tile-local floods: which tiles' flood output (label, level, edges) changes, and the final planet-wide level.
    let t = Instant::now();
    let fa = tile_floods(&base, &oa);
    println!("tile floods A (144 tiles, 4 threads): {:.2} s", t.elapsed().as_secs_f64());
    let fb = tile_floods(&edited, &ob);
    let tile_changed = |a: &[TileFlood], b: &[TileFlood]| (0..a.len()).filter(|&t| a[t] != b[t]).count();
    let in_h = (0..NT * NT).filter(|&t| (0..T * T).any(|c| base[(t / NT * T + c / T) * N + (t % NT) * T + c % T] != edited[(t / NT * T + c / T) * N + (t % NT) * T + c % T])).count();
    println!("  tiles with changed heights {in_h}; tiles whose flood_labelled output (label, level, edges) differs {}", tile_changed(&fa, &fb));
    let (la, nodes, edges, build, solve) = graph_levels(&grid, &base, &fa);
    println!("  graph A: nodes {nodes}, edges {edges}, build {:.3} s, solve (Dijkstra) {:.3} s", build, solve);
    let (lb, nodes, edges, build, solve) = graph_levels(&grid, &edited, &fb);
    println!("  graph B: nodes {nodes}, edges {edges}, build {:.3} s, solve (Dijkstra) {:.3} s", build, solve);
    let t = Instant::now();
    let wa = flood_labelled(N, N, &base, &oa);
    println!("  whole-window flood_labelled A {:.2} s", t.elapsed().as_secs_f64());
    let wb = flood_labelled(N, N, &edited, &ob);
    let dif = |a: &[f64], b: &[f64]| (0..a.len()).filter(|&p| a[p] != b[p]).count();
    println!("  final_level (tiles + graph) vs whole-window flood: A differs in {} cells, B in {} cells", dif(&la, &wa.level), dif(&lb, &wb.level));
    let lvl: Vec<bool> = (0..N * N).map(|p| la[p] != lb[p]).collect();
    report("final_level changed (A->B)", &lvl);
    drop((wa, wb, fa, fb, la, lb));

    // One carve round (5 steps, one routing) on each.
    let (mut ca, mut cb) = (base.clone(), edited.clone());
    let t = Instant::now();
    carve(&grid, &mut ca, &oa, &cp, None, seed);
    carve(&grid, &mut cb, &ob, &cp, None, seed);
    println!("carve 5 steps x2: {:.1} s", t.elapsed().as_secs_f64());
    let rca = route_weighted(&grid, &ca, &oa, eps, seed, 0.0, None);
    let rcb = route_weighted(&grid, &cb, &ob, eps, seed, 0.0, None);
    diff("== routing after one carve round (5 steps), B against A", &grid, eps, &ca, &cb, &oa, &ob, &rca, &rcb);
    let ta = tile_floods(&ca, &oa);
    let tb = tile_floods(&cb, &ob);
    println!("  tiles whose flood_labelled output differs after the round: {}", tile_changed(&ta, &tb));
    let (la, _, e, _, s) = graph_levels(&grid, &ca, &ta);
    let (lb, _, _, _, _) = graph_levels(&grid, &cb, &tb);
    println!("  graph (carved A): edges {e}, solve {s:.3} s");
    let wa = flood_labelled(N, N, &ca, &oa);
    let wb = flood_labelled(N, N, &cb, &ob);
    println!("  final_level vs whole-window flood: A differs in {} cells, B in {} cells", dif(&la, &wa.level), dif(&lb, &wb.level));
    let lvl: Vec<bool> = (0..N * N).map(|p| la[p] != lb[p]).collect();
    report("final_level changed (A->B)", &lvl);
}
