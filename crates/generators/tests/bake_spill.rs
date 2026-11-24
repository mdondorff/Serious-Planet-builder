//! Tier A, GEN-121 (m9 keep-pits-seams PR 1): the planet-wide spill level of a surface cut into tiles, against a global minimax flood of the whole surface as the oracle. Small tiles (10 to 12
//! cells) of synthetic terrain with basins that cross tile edges, a basin wider than a tile, a cube-edge-like gluing of two tiles (rotated, with the neighbour pairs named by the caller), a tile
//! of sea, nested basins and flats. The tiled result must equal the oracle bit for bit.

use planet_core::{Face, GlobalCell};
use planet_generators::bake::fillgeom::border_neighbours;
use planet_generators::bake::spill::{
    final_level, flood_labelled, perimeter_cell, perimeter_index, perimeter_len, Seam, SeamKind, SpillGraph, SEA,
};
use planet_generators::bake::tile::{TileGrid, TileId};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

/// A cell of a test world: the tile, the column and the row.
type Cell = (usize, usize, usize);

/// A seam side as the engine takes it: (seed, height) per cell.
type SeamRow = [(u32, f64)];

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
        let zs: Vec<f64> = (0..total).map(|k| self.z[k / (n * n)][k % (n * n)] + 0.0).collect();
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
    for (t, i, j) in [(cols, 8usize, 4usize), (cols + 1, 4, 4), (cols + 2, 2, 4)] {
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

// spec: GEN-121
#[test]
fn a_border_outlet_is_the_sea_a_weight_must_be_finite_and_a_signed_zero_is_one_height() {
    // Tile A's only drainage is an outlet cell on its border: the cell next to it in tile B reads the sea through `seed_of`, never through an isolated perimeter node.
    let n = 6usize;
    let mut z = vec![vec![10.0; n * n]; 2];
    let mut outlet = vec![vec![false; n * n]; 2];
    z[0][2 * n + n - 1] = -3.0;
    outlet[0][2 * n + n - 1] = true;
    let f = flood_labelled(n, n, &z[0], &outlet[0]);
    assert_eq!(f.seed_of(n - 1, 2), Some(SEA), "an outlet on the border is the sea");
    assert_eq!(f.seed_of(0, 0), perimeter_index(n, n, 0, 0), "a border cell that is not an outlet is its own seed");
    assert_eq!(f.seed_of(2, 2), None, "an inner cell is no seed");
    let w =
        World { n, z: z.clone(), outlet, cross: vec![((0, n - 1, 2), (1, 0, 2)), ((0, n - 1, 1), (1, 0, 1)), ((0, n - 1, 3), (1, 0, 3))] };
    let (raised, _) = assert_equal(&w, "an outlet on the border");
    assert_eq!(raised, 0, "a flat of 10 m beside the sea is not raised");
    let weight = std::panic::catch_unwind(|| {
        let mut g = SpillGraph::new();
        let (a, b) = (g.add_tile(perimeter_len(3, 3), &[]), g.add_tile(perimeter_len(3, 3), &[]));
        g.connect(a, 0, b, 0, f64::NAN);
    });
    assert!(weight.is_err(), "a weight that is not finite was accepted");
    // A floor of -0.0 under a rim of 0.0: one height, the same bits as the oracle (which normalises too).
    let m = 5usize;
    let mut zz = vec![vec![0.0f64; m * m]; 1];
    let mut oo = vec![vec![false; m * m]; 1];
    for j in 0..m {
        for i in 0..m {
            let edge = i == 0 || j == 0 || i == m - 1 || j == m - 1;
            zz[0][j * m + i] = if edge {
                5.0
            } else if i == 2 && j == 2 {
                -0.0
            } else {
                0.0
            };
        }
    }
    zz[0][2] = -4.0;
    oo[0][2] = true;
    let sign = World { n: m, z: zz, outlet: oo, cross: Vec::new() };
    assert_equal(&sign, "signed zero");
}

/// A seam of a test world: its kind, its tie key and the two rows of border cells (`a[k]` faces `b[k]`).
struct SeamCells {
    kind: SeamKind,
    key: u64,
    a: Vec<Cell>,
    b: Vec<Cell>,
}

impl SeamCells {
    /// The cell pairs a seam of its kind stands for (GEN-191's table): intra-face `k` with `k - 1`, `k`, `k + 1`; cube edge and corner `k` with `k`.
    fn pairs(&self) -> Vec<(Cell, Cell)> {
        let m = self.a.len() as i64;
        let reach: &[i64] = if self.kind == SeamKind::IntraFace { &[-1, 0, 1] } else { &[0] };
        let mut out = Vec::new();
        for k in 0..m {
            for &d in reach {
                if (0..m).contains(&(k + d)) {
                    out.push((self.a[k as usize], self.b[(k + d) as usize]));
                }
            }
        }
        out
    }
}

impl World {
    /// The tiled result through `connect_seams` (GEN-191): every tile flooded alone, the seams' cell pairs added by the engine, the graph solved; with the number
    /// of edges the engine added.
    fn tiled_seams(&self, seams: &[SeamCells]) -> (Vec<f64>, usize) {
        let n = self.n;
        let floods: Vec<_> = (0..self.z.len()).map(|t| flood_labelled(n, n, &self.z[t], &self.outlet[t])).collect();
        let mut graph = SpillGraph::new();
        let refs: Vec<_> = floods.iter().map(|f| graph.add_tile(perimeter_len(n, n), &f.edges)).collect();
        let seed =
            |c: Cell| if self.is_outlet(c) { SEA } else { perimeter_index(n, n, c.1, c.2).expect("a seam cell is on a tile's border") };
        let rows: Vec<(Box<SeamRow>, Box<SeamRow>)> = seams
            .iter()
            .map(|sm| {
                let row = |cs: &[Cell]| cs.iter().map(|&c| (seed(c), self.height(c))).collect::<Vec<_>>();
                (row(&sm.a).into_boxed_slice(), row(&sm.b).into_boxed_slice())
            })
            .collect();
        let list: Vec<Seam<'_>> = seams
            .iter()
            .zip(&rows)
            .filter(|(sm, _)| !sm.a.is_empty())
            .map(|(sm, (ra, rb))| Seam { a: refs[sm.a[0].0], b: refs[sm.b[0].0], kind: sm.kind, key: sm.key, a_cells: ra, b_cells: rb })
            .collect();
        let added = graph.connect_seams(&list);
        let levels = graph.solve();
        let mut out = Vec::new();
        for (t, f) in floods.iter().enumerate() {
            out.extend(final_level(f, &|s| levels.seed(refs[t], s)));
        }
        (out, added)
    }

    /// The cross pairs that are not two outlet cells: the edges a correct engine adds for them.
    fn joined_pairs(&self) -> usize {
        self.cross.iter().filter(|(a, b)| !(self.is_outlet(*a) && self.is_outlet(*b))).count()
    }
}

/// The seams of `grid_world`'s layout: an intra-face seam between tiles side by side and one above the other, and a corner seam where four tiles meet.
fn grid_seams(cols: usize, rows: usize, n: usize) -> Vec<SeamCells> {
    let mut v = Vec::new();
    let mut key = 0u64;
    let mut push = |kind, a: Vec<Cell>, b: Vec<Cell>| {
        key += 1;
        v.push(SeamCells { kind, key, a, b });
    };
    for ty in 0..rows {
        for tx in 0..cols {
            let t = ty * cols + tx;
            if tx + 1 < cols {
                push(SeamKind::IntraFace, (0..n).map(|k| (t, n - 1, k)).collect(), (0..n).map(|k| (t + 1, 0, k)).collect());
            }
            if ty + 1 < rows {
                push(SeamKind::IntraFace, (0..n).map(|k| (t, k, n - 1)).collect(), (0..n).map(|k| (t + cols, k, 0)).collect());
                if tx + 1 < cols {
                    push(SeamKind::Corner, vec![(t, n - 1, n - 1)], vec![(t + cols + 1, 0, 0)]);
                }
                if tx > 0 {
                    push(SeamKind::Corner, vec![(t, 0, n - 1)], vec![(t + cols - 1, n - 1, 0)]);
                }
            }
        }
    }
    v
}

/// A pair of cells as an unordered key.
fn unordered(p: (Cell, Cell)) -> (Cell, Cell) {
    if p.0 <= p.1 {
        p
    } else {
        (p.1, p.0)
    }
}

// spec: GEN-191
#[test]
fn the_seam_engine_gives_todays_planar_spill_levels_bit_for_bit() {
    // lead #193: given a face's tiles as intra-face and corner seams, the engine adds the pairs today's caller names (layer_fillpass through border_neighbours,
    // inside a face: the 8 neighbours of a border cell in other tiles), so the levels are the same bits as that pair list's and the oracle's (output-neutral)
    for (cols, rows, n, seed) in [(3, 3, 10, 7u64), (4, 2, 12, 11), (2, 3, 11, 5)] {
        let w = grid_world(cols, rows, n, seed);
        let seams = grid_seams(cols, rows, n);
        let want: BTreeSet<(Cell, Cell)> = w.cross.iter().copied().map(unordered).collect();
        let got: BTreeSet<(Cell, Cell)> = seams.iter().flat_map(|sm| sm.pairs()).map(unordered).collect();
        assert_eq!(got, want, "{cols} x {rows} tiles of {n}: the seams stand for the caller's pairs, no more, no fewer");
        let (today, oracle) = (w.tiled(), w.oracle());
        let (engine, added) = w.tiled_seams(&seams);
        assert_eq!(added, w.joined_pairs(), "{cols} x {rows} tiles of {n}: one edge per pair that is not two outlet cells");
        assert_eq!(today.len(), engine.len());
        for k in 0..today.len() {
            assert_eq!(today[k].to_bits(), engine[k].to_bits(), "{cols} x {rows} tiles of {n}: cell {k}");
            assert_eq!(engine[k].to_bits(), oracle[k].to_bits());
        }
        // the tie key orders the edges, never the levels: the seams in reverse order of keys give the same bits
        let mut reversed = grid_seams(cols, rows, n);
        let m = reversed.len() as u64;
        for (k, sm) in reversed.iter_mut().enumerate() {
            sm.key = m - k as u64;
        }
        let (again, _) = w.tiled_seams(&reversed);
        assert!(again.iter().zip(&engine).all(|(a, b)| a.to_bits() == b.to_bits()), "the key order changes no level");
    }
    // today's caller, cell by cell: inside a face, border_neighbours of a 3 x 3 grid of tiles names exactly the pairs of grid_seams (tile t at ti = t % 3, tj = t / 3)
    let t = 6usize;
    let g = TileGrid { cells: 3 * t, tile: t, halo: 2, radius_m: 100.0 * (3 * t) as f64 / core::f64::consts::FRAC_PI_2 };
    let tile_of = |id: TileId| -> usize { id.tj as usize * 3 + id.ti as usize };
    let mut today: BTreeSet<(Cell, Cell)> = BTreeSet::new();
    for tj in 0..3u32 {
        for ti in 0..3u32 {
            let id = TileId { face: Face(0), ti, tj };
            for j in 0..t {
                for i in 0..t {
                    if i != 0 && j != 0 && i != t - 1 && j != t - 1 {
                        continue;
                    }
                    for (o, bi, bj) in border_neighbours(&g, id, i, j) {
                        if o.face == Face(0) {
                            today.insert(unordered(((tile_of(id), i, j), (tile_of(o), bi, bj))));
                        }
                    }
                }
            }
        }
    }
    let engine: BTreeSet<(Cell, Cell)> = grid_seams(3, 3, t).iter().flat_map(|sm| sm.pairs()).map(unordered).collect();
    assert_eq!(engine, today, "the seams of one face stand for today's in-face pairs");
}

// spec: GEN-191
#[test]
fn a_cube_edge_seam_joins_two_faces_basins_axis_only() {
    // two tiles on two faces glued across a cube edge, rotated: tile 0's east column meets tile 1's south row, cell k against cell n - 1 - k, axis-only (GEN-139). A
    // basin across the edge drains only through tile 1's far side; the engine with a cube-edge seam equals the oracle with those axis pairs and no diagonals
    let n = 10usize;
    let mut z = vec![vec![30.0; n * n]; 2];
    let mut outlet = vec![vec![false; n * n]; 2];
    for j in 3..7 {
        for i in 6..n {
            z[0][j * n + i] = 5.0;
        }
    }
    for j in 0..4 {
        for i in 3..7 {
            z[1][j * n + i] = 5.0;
        }
    }
    // the only outlet: tile 1's north row, behind a 12 m rim; a 5 m channel leads from tile 1's basin to the rim
    for i in 0..n {
        z[1][(n - 1) * n + i] = -2.0;
        outlet[1][(n - 1) * n + i] = true;
        z[1][(n - 2) * n + i] = 12.0;
    }
    for j in 4..n - 2 {
        z[1][j * n + 5] = 5.0;
    }
    let cross: Vec<(Cell, Cell)> = (0..n).map(|k| ((0, n - 1, k), (1, n - 1 - k, 0))).collect();
    let w = World { n, z, outlet, cross };
    let seams = vec![SeamCells {
        kind: SeamKind::CubeEdge,
        key: 1,
        a: (0..n).map(|k| (0, n - 1, k)).collect(),
        b: (0..n).map(|k| (1, n - 1 - k, 0)).collect(),
    }];
    let ((engine, added), oracle) = (w.tiled_seams(&seams), w.oracle());
    assert_eq!(added, n);
    assert!(engine.iter().zip(&oracle).all(|(a, b)| a.to_bits() == b.to_bits()), "the cube-edge seam equals the oracle");
    // tile 0's basin has no outlet of its own: joined across the edge, it fills to tile 1's 12 m rim, as tile 1's basin does
    assert_eq!((engine[5 * n + 8], engine[n * n + 2 * n + 4]), (12.0, 12.0), "the basin across the edge spills at the rim");
    // an intra-face seam joins diagonals too (3 n - 2 pairs); across a cube edge the engine joins the facing cells only (n pairs)
    let mut g = SpillGraph::new();
    let r = (g.add_tile(perimeter_len(n, n), &[]), g.add_tile(perimeter_len(n, n), &[]));
    let rows: Vec<(u32, f64)> = (0..n as u32).map(|k| (k, 1.0)).collect();
    let count = |kind| g.clone().connect_seams(&[Seam { a: r.0, b: r.1, kind, key: 1, a_cells: &rows, b_cells: &rows }]);
    assert_eq!((count(SeamKind::CubeEdge), count(SeamKind::IntraFace)), (n, 3 * n - 2), "axis-only across a cube edge");
}

// spec: GEN-191
#[test]
fn three_faces_at_a_cube_corner_join_through_their_corner_cells_without_a_diagonal() {
    // lead #193: a cube corner joins 3 tiles on 3 faces. The world is three whole faces of a level of n cells (one tile a face) around a real cube corner; its
    // cross pairs are every axis neighbour relation between them by GlobalCell (COORD-011, the fold); the seams are one cube-edge seam per pair of faces
    let n = 6i64;
    let nu = n as usize;
    let c0 = GlobalCell::new(Face(0), n, 0, 0).unwrap();
    let across: Vec<GlobalCell> = c0.neighbours4(n).into_iter().filter(|b| b.face() != c0.face()).collect();
    let corners = [c0, across[0], across[1]];
    let faces: Vec<Face> = corners.iter().map(|c| c.face()).collect();
    let tile = |f: Face| faces.iter().position(|&g| g == f);
    // the three corner cells are corner cells of their faces and pairwise axis neighbours (no diagonal at a cube corner)
    for (k, c) in corners.iter().enumerate() {
        assert!((c.i() == 0 || c.i() == n - 1) && (c.j() == 0 || c.j() == n - 1), "{c:?}");
        for d in &corners[k + 1..] {
            assert!(c.neighbours4(n).contains(d), "{c:?} and {d:?}: an axis pair");
        }
    }
    // the cross pairs and the seams, both from the fold: the a side the lower tile; seam t + u - 1 joins tiles t and u
    let mut seam_pairs: Vec<Vec<(Cell, Cell)>> = vec![Vec::new(); 3];
    for (t, &f) in faces.iter().enumerate() {
        for j in 0..n {
            for i in 0..n {
                let c = GlobalCell::new(f, n, i, j).unwrap();
                for b in c.neighbours4(n) {
                    if let Some(u) = tile(b.face()).filter(|&u| u > t) {
                        seam_pairs[t + u - 1].push(((t, i as usize, j as usize), (u, b.i() as usize, b.j() as usize)));
                    }
                }
                // no diagonal of the 8-neighbourhood leaves the face (GEN-139)
                assert!(c.neighbours8(n).iter().all(|b| b.face() == f || c.neighbours4(n).contains(b)));
            }
        }
    }
    assert!(seam_pairs.iter().all(|s| s.len() == nu), "each pair of faces shares one edge of n axis pairs");
    let cross: Vec<(Cell, Cell)> = seam_pairs.iter().flatten().copied().collect();
    let corner_cell = |c: GlobalCell| -> Cell { (tile(c.face()).unwrap(), c.i() as usize, c.j() as usize) };
    for (k, c) in corners.iter().enumerate() {
        for d in &corners[k + 1..] {
            let p = unordered((corner_cell(*c), corner_cell(*d)));
            assert!(cross.iter().any(|&q| unordered(q) == p), "the corner cells are joined pairwise");
        }
    }
    // terrain: 50 m everywhere; each corner cell a pit walled in on its own face (tile 0 at 5 m, tile 1 at 7 m); tile 2's corner cell (6 m) leads by a 6 m
    // channel along its face's diagonal to the face's only outlet. The pits of tiles 0 and 1 drain only through the corner cells' pairs
    let mut z = vec![vec![50.0; nu * nu]; 3];
    let mut outlet = vec![vec![false; nu * nu]; 3];
    let at = |c: GlobalCell| c.j() as usize * nu + c.i() as usize;
    let cz = [5.0, 7.0, 6.0];
    for t in 0..3 {
        z[t][at(corners[t])] = cz[t];
    }
    let oz = corners[2];
    let (si, sj) = (if oz.i() == 0 { 1 } else { -1 }, if oz.j() == 0 { 1 } else { -1 });
    for s in 1..n {
        let k = (oz.j() + sj * s) as usize * nu + (oz.i() + si * s) as usize;
        z[2][k] = if s == n - 1 { -1.0 } else { 6.0 };
        outlet[2][k] = s == n - 1;
    }
    let w = World { n: nu, z, outlet, cross };
    let seams: Vec<SeamCells> = seam_pairs
        .iter()
        .enumerate()
        .map(|(k, ps)| SeamCells {
            kind: SeamKind::CubeEdge,
            key: k as u64,
            a: ps.iter().map(|p| p.0).collect(),
            b: ps.iter().map(|p| p.1).collect(),
        })
        .collect();
    let ((engine, added), oracle) = (w.tiled_seams(&seams), w.oracle());
    assert_eq!(added, w.joined_pairs());
    assert!(engine.iter().zip(&oracle).all(|(a, b)| a.to_bits() == b.to_bits()), "three cube-edge seams at a corner equal the oracle");
    // tile 0's pit (5 m) fills to tile 2's 6 m through its pair with tile 2's corner cell; tile 1's corner (7 m) stays at 7 m
    let level = |e: &[f64], t: usize| e[t * nu * nu + at(corners[t])];
    assert_eq!((level(&engine, 0), level(&engine, 1), level(&engine, 2)), (6.0, 7.0, 6.0), "the corner cells carry the drainage");
    // without the corner cells' pairs the pits of tiles 0 and 1 fill to their 50 m walls
    let mut cut = seams;
    let is_corner = |c: Cell| corners.iter().any(|&g| corner_cell(g) == c);
    for sm in cut.iter_mut() {
        let keep: Vec<usize> = (0..sm.a.len()).filter(|&k| !is_corner(sm.a[k]) && !is_corner(sm.b[k])).collect();
        sm.a = keep.iter().map(|&k| sm.a[k]).collect();
        sm.b = keep.iter().map(|&k| sm.b[k]).collect();
    }
    let (without, _) = w.tiled_seams(&cut);
    assert_eq!((level(&without, 0), level(&without, 1)), (50.0, 50.0), "the drainage hinges on the corner cells");
}

// spec: GEN-191
#[test]
fn a_bad_seam_is_refused_before_any_edge_is_added() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let n = 4usize;
    let row = |z: f64| -> Vec<(u32, f64)> { (0..n as u32).map(|k| (k, z)).collect() };
    let good = row(1.0);
    let short = good[..3].to_vec();
    let mut nan = row(1.0);
    nan[2].1 = f64::NAN;
    let two = good[..2].to_vec();
    let inf = vec![(0u32, f64::NEG_INFINITY)];
    let one = good[..1].to_vec();
    let none: Vec<(u32, f64)> = Vec::new();
    let cases: [(SeamKind, &SeamRow, &SeamRow, &str); 5] = [
        (SeamKind::IntraFace, &good, &short, "rows of different lengths"),
        (SeamKind::CubeEdge, &good, &nan, "a NaN height on one side"),
        (SeamKind::Corner, &two, &two, "a corner seam of two cells"),
        (SeamKind::Corner, &none, &none, "a corner seam of no cell"),
        (SeamKind::CubeEdge, &inf, &one, "an infinite height"),
    ];
    for (kind, a, b, what) in cases {
        let mut g = SpillGraph::new();
        let r = (g.add_tile(perimeter_len(n, n), &[]), g.add_tile(perimeter_len(n, n), &[]));
        let before = format!("{g:?}");
        // a good seam keyed first, then the bad one: refused as a whole, the graph unchanged
        let res = catch_unwind(AssertUnwindSafe(|| {
            g.connect_seams(&[
                Seam { a: r.0, b: r.1, kind: SeamKind::CubeEdge, key: 0, a_cells: &good, b_cells: &good },
                Seam { a: r.0, b: r.1, kind, key: 1, a_cells: a, b_cells: b },
            ])
        }));
        assert!(res.is_err(), "{what} is refused");
        assert_eq!(format!("{g:?}"), before, "{what}: no edge added");
    }
}
