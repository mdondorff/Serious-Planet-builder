//! The planet-wide spill level of a surface cut into tiles (GEN-121; m9 keep-pits-seams PR 1): Barnes et al.'s parallel priority-flood (2016) in its minimax form, with no store and no base.
//!
//! The spill level of a cell is the lowest value of the highest height along a path from the cell to an outlet (the sea): `L(c) = min over paths c -> outlet of max z along the path`. The
//! fill depth of a cell is `L(c) - z(c)`. A tile alone cannot know it: a basin wider than the tile's reach drains through the tile's border in one tile and is closed in another (SPIKE-102 / keep-pits
//! preview: 152 m of disagreement across a tile seam). The planet-wide level is computed in three steps, none of which needs more than one tile or the graph of the tiles' borders:
//! 1. [`flood_labelled`] floods one tile from its outlets and from every cell of its border (each border cell is a seed with its own label), in order of height: every cell gets the label of the
//!    seed that reaches it first and `level`, the highest height on its path from that seed; the tile also returns the territories' adjacency, `(label a, label b, w)` with `w = max(level, level)`
//!    of two neighbouring cells of different labels;
//! 2. [`SpillGraph`] joins the tiles: the territory edges of every tile and the pairs of neighbouring border cells of two tiles (the caller names them: a cube edge is a pair list like any other)
//!    with `w = max(z, z)`; `solve` gives each seed its planet-wide level `G` (Dijkstra with `max` for `+`, from the sea);
//! 3. [`final_level`] gives every cell `max(level(c), G(label(c)))`.
//!
//! A cell whose territory belongs to an outlet of its own tile keeps `level`. The result is the minimax level of the whole surface (the proof: a cell's best seed in its tile is never worse than another
//! seed of the tile, because the two seeds connect through the cell). The kernels are single-threaded with a fixed visiting order (CON-14): the values are the same bits on any machine.

use super::Grid;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// The label of a cell that drains to an outlet inside its own tile (the sea).
pub const SEA: u32 = u32::MAX;

/// The order key of a height: monotone under the total order, so a heap compares it as an integer.
fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b & (1 << 63) != 0 {
        !b
    } else {
        b | (1 << 63)
    }
}

/// Cells on the border of a `w x h` tile (its seeds).
pub fn perimeter_len(w: usize, h: usize) -> usize {
    2 * (w + h) - 4
}

/// The index of border cell `(i, j)` along the perimeter (top row left to right, right column down, bottom row right to left, left column up), `None` for an inner cell.
pub fn perimeter_index(w: usize, h: usize, i: usize, j: usize) -> Option<u32> {
    let k = if j == 0 {
        i
    } else if i == w - 1 {
        w - 1 + j
    } else if j == h - 1 {
        (w - 1) + (h - 1) + (w - 1 - i)
    } else if i == 0 {
        2 * (w - 1) + (h - 1) + (h - 1 - j)
    } else {
        return None;
    };
    Some(k as u32)
}

/// The cell `(i, j)` of perimeter index `k` (the inverse of [`perimeter_index`]).
pub fn perimeter_cell(w: usize, h: usize, k: u32) -> (usize, usize) {
    let k = k as usize;
    assert!(k < perimeter_len(w, h), "perimeter index {k} is outside a {w} x {h} tile");
    if k < w - 1 {
        (k, 0)
    } else if k < w - 1 + h - 1 {
        (w - 1, k - (w - 1))
    } else if k < 2 * (w - 1) + (h - 1) {
        (w - 1 - (k - (w - 1) - (h - 1)), h - 1)
    } else {
        (0, h - 1 - (k - 2 * (w - 1) - (h - 1)))
    }
}

/// One tile after its own flood.
#[derive(Clone, Debug, PartialEq)]
pub struct TileFlood {
    pub w: usize,
    pub h: usize,
    /// The seed that reached each cell: a perimeter index, or [`SEA`] for a cell reached from an outlet of the tile.
    pub label: Vec<u32>,
    /// The highest height on the cell's path from its seed (at least its own height).
    pub level: Vec<f64>,
    /// The adjacency of the territories: `(a, b, w)` with `a < b` (labels, [`SEA`] last), the lowest `w` over the neighbouring cell pairs of two territories, sorted.
    /// A tile of 3072 x 3072 cells holds about 113 MB here (labels and levels): a caller with many tiles keeps only `edges` and runs [`flood_labelled`] again for [`final_level`] (the flood is deterministic).
    pub edges: Vec<(u32, u32, f64)>,
}

/// Floods one tile (`w x h` cells, row-major, at least 3 x 3) from its outlets and from every border cell that is not an outlet. Heights must be finite.
pub fn flood_labelled(w: usize, h: usize, z: &[f64], outlet: &[bool]) -> TileFlood {
    let grid = Grid::new(w, h, 1.0);
    let n = grid.len();
    assert!(z.len() == n && outlet.len() == n, "one height and one outlet flag per cell");
    super::assert_finite("the heights", z);
    // -0.0 and 0.0 are one height (f64::max returns either of them, which would make the bits depend on the operand order): every height is normalised to +0.0.
    let z: Vec<f64> = z.iter().map(|v| v + 0.0).collect();
    let z = z.as_slice();
    let mut label = vec![SEA; n];
    let mut level = vec![0.0f64; n];
    let mut seen = vec![false; n];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    for p in 0..n {
        let (i, j) = (p % w, p / w);
        if outlet[p] {
            seen[p] = true;
            level[p] = z[p];
            // An outlet that touches only outlets expands nothing.
            if (0..8).filter_map(|d| grid.neighbour(p, d)).any(|q| !outlet[q]) {
                heap.push(Reverse((key(z[p]), p as u32)));
            }
        } else if let Some(k) = perimeter_index(w, h, i, j) {
            seen[p] = true;
            label[p] = k;
            level[p] = z[p];
            heap.push(Reverse((key(z[p]), p as u32)));
        }
    }
    while let Some(Reverse((_, p))) = heap.pop() {
        let p = p as usize;
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            if seen[q] {
                continue;
            }
            seen[q] = true;
            label[q] = label[p];
            level[q] = level[p].max(z[q]);
            heap.push(Reverse((key(level[q]), q as u32)));
        }
    }
    debug_assert!(seen.iter().all(|s| *s), "a cell was not reached");
    let mut lowest: HashMap<(u32, u32), f64> = HashMap::new();
    for p in 0..n {
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            if q < p || label[p] == label[q] {
                continue;
            }
            let (a, b) = if label[p] < label[q] { (label[p], label[q]) } else { (label[q], label[p]) };
            let w = level[p].max(level[q]);
            let e = lowest.entry((a, b)).or_insert(f64::INFINITY);
            *e = e.min(w);
        }
    }
    let mut edges: Vec<(u32, u32, f64)> = lowest.into_iter().map(|((a, b), w)| (a, b, w)).collect();
    edges.sort_by_key(|x| (x.0, x.1));
    TileFlood { w, h, label, level, edges }
}

/// The seed a cell of the tile's border stands for in a [`SpillGraph`]: [`SEA`] when the cell drains to an outlet (it is one, or it was reached from one), its perimeter index when it is a seed. `None` for an inner cell.
impl TileFlood {
    pub fn seed_of(&self, i: usize, j: usize) -> Option<u32> {
        perimeter_index(self.w, self.h, i, j)?;
        Some(self.label[j * self.w + i])
    }
}

/// The level of every cell of a tile given the planet-wide level `g` of its seeds: `max(level, g(label))`, and `level` for a cell that drains to the sea inside its own tile.
pub fn final_level(tile: &TileFlood, g: &dyn Fn(u32) -> f64) -> Vec<f64> {
    tile.label.iter().zip(&tile.level).map(|(&l, &v)| if l == SEA { v } else { v.max(g(l)) }).collect()
}

/// A tile in a [`SpillGraph`]: its index and the perimeter length it was added with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRef(pub usize);

/// The graph of the seeds of many tiles: the territory edges of each tile and the pairs of neighbouring border cells of two tiles.
#[derive(Clone, Debug, Default)]
pub struct SpillGraph {
    /// First node of each tile (node 0 is the sea).
    base: Vec<u32>,
    seeds: Vec<u32>,
    edges: Vec<(u32, u32, f64)>,
}

/// The planet-wide levels of the seeds ([`SpillGraph::solve`]).
#[derive(Clone, Debug, PartialEq)]
pub struct SpillLevels {
    base: Vec<u32>,
    seeds: Vec<u32>,
    g: Vec<f64>,
}

impl SpillGraph {
    pub fn new() -> SpillGraph {
        SpillGraph::default()
    }

    fn next_base(&self) -> u32 {
        match (self.base.last(), self.seeds.last()) {
            (Some(b), Some(s)) => b.checked_add(*s).expect("the spill graph has more nodes than a u32 numbers"),
            _ => 1,
        }
    }

    /// Adds a tile of `seeds` border cells with the territory edges its flood returned.
    pub fn add_tile(&mut self, seeds: usize, edges: &[(u32, u32, f64)]) -> TileRef {
        let t = TileRef(self.base.len());
        self.base.push(self.next_base());
        self.seeds.push(seeds as u32);
        for &(a, b, w) in edges {
            let (na, nb) = (self.node(t, a), self.node(t, b));
            self.edges.push((na, nb, w));
        }
        t
    }

    /// The node of seed `seed` of tile `t` ([`SEA`] is the sea's node 0).
    fn node(&self, t: TileRef, seed: u32) -> u32 {
        if seed == SEA {
            0
        } else {
            assert!(seed < self.seeds[t.0], "seed {seed} is outside tile {} of {} seeds", t.0, self.seeds[t.0]);
            self.base[t.0] + seed
        }
    }

    /// Joins border cell `a` of tile `ta` (a seed, or [`SEA`] when the cell is an outlet) to border cell `b` of tile `tb` with `w = max(z(a), z(b))`: two cells of different tiles that neighbour each other.
    pub fn connect(&mut self, ta: TileRef, a: u32, tb: TileRef, b: u32, w: f64) {
        assert!(w.is_finite(), "a spill edge needs a finite weight, not {w}");
        let w = w + 0.0;
        let (na, nb) = (self.node(ta, a), self.node(tb, b));
        self.edges.push((na, nb, w));
    }

    /// The planet-wide level of every seed: the lowest highest-edge over the paths from the sea (`-inf` for the sea, `+inf` for a seed no path from the sea reaches).
    pub fn solve(&self) -> SpillLevels {
        let n = self.next_base() as usize;
        let mut start = vec![0u32; n + 1];
        for &(a, b, _) in &self.edges {
            start[a as usize + 1] += 1;
            start[b as usize + 1] += 1;
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut adj = vec![(0u32, 0.0f64); self.edges.len() * 2];
        for &(a, b, w) in &self.edges {
            adj[fill[a as usize] as usize] = (b, w);
            fill[a as usize] += 1;
            adj[fill[b as usize] as usize] = (a, w);
            fill[b as usize] += 1;
        }
        let mut g = vec![f64::INFINITY; n];
        g[0] = f64::NEG_INFINITY;
        let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
        heap.push(Reverse((key(g[0]), 0)));
        while let Some(Reverse((k, u))) = heap.pop() {
            let u = u as usize;
            if k != key(g[u]) {
                continue;
            }
            for &(v, w) in &adj[start[u] as usize..start[u + 1] as usize] {
                let cand = g[u].max(w);
                if cand < g[v as usize] {
                    g[v as usize] = cand;
                    heap.push(Reverse((key(cand), v)));
                }
            }
        }
        SpillLevels { base: self.base.clone(), seeds: self.seeds.clone(), g }
    }
}

impl SpillLevels {
    /// The planet-wide level of seed `seed` of tile `t` (`-inf` for [`SEA`]).
    pub fn seed(&self, t: TileRef, seed: u32) -> f64 {
        if seed == SEA {
            return f64::NEG_INFINITY;
        }
        assert!(seed < self.seeds[t.0], "seed {seed} is outside tile {} of {} seeds", t.0, self.seeds[t.0]);
        self.g[(self.base[t.0] + seed) as usize]
    }
}
