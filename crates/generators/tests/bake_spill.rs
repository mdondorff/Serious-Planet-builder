//! Tier A, GEN-121 (m9 keep-pits-seams PR 1): the planet-wide spill level of a surface cut into tiles, against a global minimax flood of the whole surface as the oracle. Small tiles (10 to 12
//! cells) of synthetic terrain with basins that cross tile edges, a basin wider than a tile, a cube-edge-like gluing of two tiles (rotated, with the neighbour pairs named by the caller), a tile
//! of sea, nested basins and flats. The tiled result must equal the oracle bit for bit.

use planet_generators::bake::spill::{final_level, flood_labelled, perimeter_cell, perimeter_index, perimeter_len, SpillGraph, SEA};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A cell of a test world: the tile, the column and the row.
type Cell = (usize, usize, usize);

struct World {
    n: usize,
    z: Vec<Vec<f64>>,
    outlet: Vec<Vec<bool>>,
    /// Pairs of neighbouring border cells of two tiles (the caller names them; both directions are implied).
    cross: Vec<(Cell, Cell)>,
}

fn rng(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f64) / (1u64 << 31) as f64
}

impl World {
    fn node(&self, c: Cell) -> usize {
        c.0 * self.n * self.n + c.2 * self.n + c.1
    }

    fn height(&self, c: Cell) -> f64 {
        self.z[c.0][c.2 * self.n + c.1]
    }

    fn is_outlet(&self, c: Cell) -> bool {
        self.outlet[c.0][c.2 * self.n + c.1]
    }

    /// The oracle: the minimax level of every cell of the whole surface, a Dijkstra with `max` over the cells' own neighbour lists (8 neighbours in a tile, the named pairs across tiles).
    fn oracle(&self) -> Vec<f64> {
        let (n, tiles) = (self.n, self.z.len());
        let total = tiles * n * n;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); total];
        for t in 0..tiles {
            for j in 0..n {
                for i in 0..n {
                    for dj in -1i64..=1 {
                        for di in -1i64..=1 {
                            let (ii, jj) = (i as i64 + di, j as i64 + dj);
                            if (di, dj) != (0, 0) && ii >= 0 && jj >= 0 && ii < n as i64 && jj < n as i64 {
                                adj[self.node((t, i, j))].push(self.node((t, ii as usize, jj as usize)));
                            }
                        }
                    }
                }
            }
        }
        for &(a, b) in &self.cross {
            adj[self.node(a)].push(self.node(b));
            adj[self.node(b)].push(self.node(a));
        }
        let zs: Vec<f64> = (0..total).map(|k| self.z[k / (n * n)][k % (n * n)]).collect();
        let key = |v: f64| {
            let b = v.to_bits();
            if b & (1 << 63) != 0 {
                !b
            } else {
                b | (1 << 63)
            }
        };
        let mut level = vec![f64::INFINITY; total];
        let mut heap = BinaryHeap::new();
        for k in 0..total {
            if self.outlet[k / (n * n)][k % (n * n)] {
                level[k] = zs[k];
                heap.push(Reverse((key(zs[k]), k)));
            }
        }
        while let Some(Reverse((k, u))) = heap.pop() {
            if k != key(level[u]) {
                continue;
            }
            for &v in &adj[u] {
                let cand = level[u].max(zs[v]);
                if cand < level[v] {
                    level[v] = cand;
                    heap.push(Reverse((key(cand), v)));
                }
            }
        }
        level
    }

    /// The tiled result: every tile flooded alone, the graph solved, every tile's levels.
    fn tiled(&self) -> Vec<f64> {
        let n = self.n;
        let floods: Vec<_> = (0..self.z.len()).map(|t| flood_labelled(n, n, &self.z[t], &self.outlet[t])).collect();
        let mut graph = SpillGraph::new();
        let refs: Vec<_> = floods.iter().map(|f| graph.add_tile(perimeter_len(n, n), &f.edges)).collect();
        let seed =
            |c: Cell| if self.is_outlet(c) { SEA } else { perimeter_index(n, n, c.1, c.2).expect("a cross pair is on a tile's border") };
        for &(a, b) in &self.cross {
            graph.connect(refs[a.0], seed(a), refs[b.0], seed(b), self.height(a).max(self.height(b)));
        }
        let levels = graph.solve();
        let mut out = Vec::new();
        for (t, f) in floods.iter().enumerate() {
            out.extend(final_level(f, &|s| levels.seed(refs[t], s)));
        }
        out
    }
}

/// Neighbouring border cells of two tiles side by side (`a` left of `b`) or one above the other, 8-connected, as the caller of a planar tiling would name them.
fn planar_pairs(n: usize, a: usize, b: usize, horizontal: bool) -> Vec<(Cell, Cell)> {
    let mut v = Vec::new();
    for k in 0..n {
        for d in -1i64..=1 {
            let m = k as i64 + d;
            if m < 0 || m >= n as i64 {
                continue;
            }
            let m = m as usize;
            v.push(if horizontal { ((a, n - 1, k), (b, 0, m)) } else { ((a, k, n - 1), (b, m, 0)) });
        }
    }
    v
}

/// `cols x rows` tiles of `n x n` cells of one smooth, pitted terrain: the outer border of the whole grid and every cell under the sea level are outlets.
fn grid_world(cols: usize, rows: usize, n: usize, seed: u64) -> World {
    let mut s = seed;
    let (w, h) = (cols * n, rows * n);
    let (pa, pb, pc) = (rng(&mut s) * 6.0, rng(&mut s) * 6.0, rng(&mut s) * 6.0);
    let mut z = vec![vec![0.0; n * n]; cols * rows];
    let mut outlet = vec![vec![false; n * n]; cols * rows];
    for ty in 0..rows {
        for tx in 0..cols {
            for j in 0..n {
                for i in 0..n {
                    let (x, y) = ((tx * n + i) as f64 / w as f64, (ty * n + j) as f64 / h as f64);
                    let v = 60.0 * libm::sin(9.0 * x + pa) * libm::cos(8.0 * y + pb)
                        + 40.0 * libm::sin(17.0 * (x + y) + pc)
                        + 15.0
                        + 25.0 * libm::sin(37.0 * x + pc) * libm::sin(41.0 * y + pa)
                        + 12.0 * rng(&mut s);
                    let (gx, gy) = (tx * n + i, ty * n + j);
                    let edge = gx == 0 || gy == 0 || gx == w - 1 || gy == h - 1;
                    z[ty * cols + tx][j * n + i] = if edge { v.min(-1.0) } else { v };
                    outlet[ty * cols + tx][j * n + i] = edge || v < -20.0;
                }
            }
        }
    }
    let mut cross = Vec::new();
    for ty in 0..rows {
        for tx in 0..cols {
            let t = ty * cols + tx;
            if tx + 1 < cols {
                cross.extend(planar_pairs(n, t, t + 1, true));
            }
            if ty + 1 < rows {
                cross.extend(planar_pairs(n, t, t + cols, false));
                // the diagonal contact at the corner between four tiles
                if tx + 1 < cols {
                    cross.push(((t, n - 1, n - 1), (t + cols + 1, 0, 0)));
                }
                if tx > 0 {
                    cross.push(((t, 0, n - 1), (t + cols - 1, n - 1, 0)));
                }
            }
        }
    }
    World { n, z, outlet, cross }
}

fn assert_equal(w: &World, what: &str) -> (usize, usize) {
    let (oracle, tiled) = (w.oracle(), w.tiled());
    let mut raised = 0;
    let n2 = w.n * w.n;
    for (k, (a, b)) in oracle.iter().zip(&tiled).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "{what}: tile {} cell ({}, {}): oracle level {a}, tiled {b}",
            k / n2,
            k % n2 % w.n,
            k % n2 / w.n
        );
        if *a > w.z[k / n2][k % n2] {
            raised += 1;
        }
    }
    (raised, oracle.len())
}

// spec: GEN-121
#[test]
fn the_perimeter_index_round_trips_and_visits_every_border_cell_once() {
    for (w, h) in [(3, 3), (5, 3), (3, 7), (10, 12)] {
        let mut seen = vec![false; perimeter_len(w, h)];
        for j in 0..h {
            for i in 0..w {
                let k = perimeter_index(w, h, i, j);
                assert_eq!(k.is_some(), i == 0 || j == 0 || i == w - 1 || j == h - 1);
                if let Some(k) = k {
                    assert!(!seen[k as usize], "{w}x{h}: index {k} used twice");
                    seen[k as usize] = true;
                    assert_eq!(perimeter_cell(w, h, k), (i, j));
                }
            }
        }
        assert!(seen.iter().all(|s| *s), "{w}x{h}: an index is unused");
    }
}

// spec: GEN-121
#[test]
fn a_tiled_terrain_has_the_levels_of_a_global_flood_bit_for_bit() {
    let mut raised_total = 0;
    for seed in 1..=12u64 {
        let w = grid_world(3, 3, 10, seed);
        let (raised, cells) = assert_equal(&w, &format!("seed {seed}"));
        assert!(raised < cells, "seed {seed}: the whole terrain is under water");
        raised_total += raised;
    }
    assert!(raised_total > 100, "the synthetic terrain holds too few basins to test with: {raised_total} cells raised");
}

// spec: GEN-121
#[test]
fn a_basin_wider_than_a_tile_is_filled_to_its_one_planet_wide_spill_level() {
    // A bowl of 100 m over 3 x 3 tiles of 10 cells with a rim of 100 m at the edge of the bowl and a notch at 70 m in the east rim leading to the sea.
    let (n, cols) = (10usize, 3usize);
    let mut w = grid_world(cols, cols, n, 3);
    let c = (cols * n) as f64 / 2.0 - 0.5;
    for t in 0..cols * cols {
        let (tx, ty) = (t % cols, t / cols);
        for j in 0..n {
            for i in 0..n {
                let (gx, gy) = ((tx * n + i) as f64, (ty * n + j) as f64);
                let r = libm::hypot(gx - c, gy - c);
                let border = gx == 0.0 || gy == 0.0 || gx == (cols * n - 1) as f64 || gy == (cols * n - 1) as f64;
                let v = if r < 8.0 {
                    60.0
                } else if r < 11.0 {
                    60.0 + (r - 8.0) * 40.0 / 3.0
                } else {
                    130.0
                };
                w.z[t][j * n + i] = if border { -1.0 } else { v };
                w.outlet[t][j * n + i] = border;
            }
        }
    }
    // The notch: the rim cells at x = 24 to 29 of row 14 and 15 drop to 70 m, a channel to the sea.
    for gx in 20..cols * n - 1 {
        for gy in [14usize, 15] {
            let (t, i, j) = ((gy / n) * cols + gx / n, gx % n, gy % n);
            w.z[t][j * n + i] = w.z[t][j * n + i].min(70.0);
        }
    }
    let (raised, _) = assert_equal(&w, "one big bowl");
    assert!(raised > 100, "the bowl holds only {raised} raised cells");
    let level = w.tiled();
    // The flat floor (60 m) spans three tiles: a cell of each reads the one level of the notch, 70 m.
    for (t, i, j) in [(1 * cols, 8usize, 4usize), (1 * cols + 1, 4, 4), (1 * cols + 2, 2, 4)] {
        let l = level[t * n * n + j * n + i];
        assert!((l - 70.0).abs() < 1e-9, "tile {t} cell ({i}, {j}): the bowl's spill level is the notch's 70 m, not {l}");
    }
}

// spec: GEN-121
#[test]
fn two_tiles_glued_with_a_rotation_agree_with_the_flood_of_the_glued_surface() {
    // Tile 1's east column touches tile 2's top row (a cube edge: the axes turn); tile 1's west column and tile 2's bottom row are the sea.
    let n = 12usize;
    for seed in 1..=8u64 {
        let mut s = seed;
        let mut z = vec![vec![0.0; n * n]; 2];
        let mut outlet = vec![vec![false; n * n]; 2];
        for (t, tz) in z.iter_mut().enumerate() {
            for j in 0..n {
                for i in 0..n {
                    // A bowl centred on the glued corner (tile 1's (11, 11) and tile 2's (0, 0) are neighbours), 50 m deep, rim 80 to 100 m.
                    let (cx, cy) = if t == 0 { (n as f64 - 1.0, n as f64 - 1.0) } else { (0.0, 0.0) };
                    let r = libm::hypot(i as f64 - cx, j as f64 - cy);
                    let sea = (t == 0 && i == 0) || (t == 1 && j == n - 1);
                    tz[j * n + i] = if sea { -2.0 } else { 100.0 - (14.0 - r).max(0.0) * 3.5 + 8.0 * rng(&mut s) };
                    outlet[t][j * n + i] = sea;
                }
            }
        }
        // The gluing: tile 1's (11, j) is next to tile 2's (j, 0), and its 8 neighbours.
        let mut cross = Vec::new();
        for j in 0..n {
            for d in -1i64..=1 {
                let m = j as i64 + d;
                if (0..n as i64).contains(&m) {
                    cross.push(((0usize, n - 1, j), (1usize, m as usize, 0usize)));
                }
            }
        }
        let w = World { n, z, outlet, cross };
        let (raised, _) = assert_equal(&w, &format!("glued, seed {seed}"));
        assert!(raised > 20, "seed {seed}: only {raised} cells are raised, the bowl does not cross the edge");
    }
}

// spec: GEN-121
#[test]
fn nested_basins_a_flat_and_a_tile_of_sea_follow_the_oracle() {
    // A plateau of equal heights (ties), two nested bowls and a tile that is all sea next to them.
    let n = 10usize;
    let mut w = grid_world(2, 2, n, 5);
    for t in 0..4 {
        for p in 0..n * n {
            w.z[t][p] = 50.0;
            w.outlet[t][p] = false;
        }
    }
    for p in 0..n * n {
        w.z[3][p] = -5.0;
        w.outlet[3][p] = true;
    }
    // Tile 0: a bowl of 10 m inside a bowl of 30 m, the outer rim lower on the south side (towards the sea tile).
    for j in 2..9 {
        for i in 2..9 {
            w.z[0][j * n + i] = 30.0;
        }
    }
    for j in 4..7 {
        for i in 4..7 {
            w.z[0][j * n + i] = 10.0;
        }
    }
    w.z[0][8 * n + 5] = 45.0; // the outer bowl's lowest rim sits at 45 m after a notch through the plateau below
    w.z[2][0] = 40.0;
    let (raised, _) = assert_equal(&w, "nested");
    assert!(raised > 40, "only {raised} cells are raised");
    // A world with no outlet at all has an infinite level for every cell that is not an outlet.
    let none = World { n, z: vec![vec![5.0; n * n]], outlet: vec![vec![false; n * n]], cross: Vec::new() };
    assert!(none.tiled().iter().all(|v| *v == f64::INFINITY));
}

// spec: GEN-121
#[test]
fn the_same_input_gives_the_same_bits_and_a_bad_input_is_refused() {
    let w = grid_world(3, 2, 10, 9);
    let (a, b) = (w.tiled(), w.tiled());
    assert_eq!(a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let r = std::panic::catch_unwind(|| flood_labelled(3, 3, &[0.0, 1.0, f64::NAN, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0], &[false; 9]));
    assert!(r.is_err(), "a height that is not finite was accepted");
}
