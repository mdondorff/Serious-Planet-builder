//! The wet floor of the global bake (GEN-031, ADR 0071): per vertex the lowest conduction bound at which the sea reaches it.
//!
//! The floor of a vertex is the minimum, over 8-neighbour paths from an ocean vertex to it, of the largest elevation code along the
//! path (an ocean vertex has code 0, an unreadable elevation [`INF`]); a vertex no path reaches has [`INF`]. A consumer compares it with
//! the code of its bound ([`bound_code`]): the vertex is **wet** at that bound when `floor <= bound_code`. Nothing here depends on the
//! sea level, so one channel serves every level and every conduction rule.
//!
//! Three passes over the blocks of a bake, all in integers (so ties, order and the stored and in-memory paths are exact):
//! 1. per block, [`analyse_floor_block`]: a union-find in increasing code that emits an edge whenever two components holding terminals
//!    (the block's border vertices, which blocks share, and one virtual ocean node) merge: at most about `8 cells` edges per block (the merges, and one per border ocean vertex);
//! 2. [`merge_floor_edges`]: one serial Kruskal-style pass over every block's edges, giving the floor of each border vertex the ocean reaches;
//! 3. per block, [`floor_block`]: a bucketed priority flood seeded with those border floors and the block's ocean vertices.
//!
//! The floors are unique minimums, so the result is the same for every block size, thread count and block order and equals
//! [`reference_floors`], a serial bucketed flood over the whole cube graph (the oracle of the tests).

use crate::global::{GlobalGrid, GlobalSample, ReadError};
use crate::ocean::{parallel_map, CubeIndex, OceanBits, OceanGrid};
use planet_core::{Face, TileId};
use std::collections::HashMap;

/// The implementation id of the wet-floor channel in cache keys (CON-07): a change of the rule renames it.
pub const WET_FLOOR_IMPLEMENTATION_ID: &str = "cpu-wet-floor-v1";

/// The code of a vertex no path can cross (an unreadable elevation) and the floor of a vertex no path reaches.
pub const INF: u16 = 0xFFFF;

/// The code of an elevation of 0 m: the codes are half-metre steps, `floor(2 e) + CODE_ZERO`.
pub const CODE_ZERO: i32 = 24_000;

/// The virtual ocean node every ocean vertex belongs to.
const OCEAN_NODE: u64 = u64::MAX;
const NO_TERM: u64 = u64::MAX - 1;

/// The code of an elevation: half-metre steps from -12,000 m to +20,766 m, clamped to 1 ..= 0xFFFE (so a clamp keeps the wet set a
/// superset at both ends); NaN is [`INF`].
pub fn code_of(e: f32) -> u16 {
    if e.is_nan() {
        return INF;
    }
    let c = libm::floor(f64::from(e) * 2.0) + f64::from(CODE_ZERO);
    c.clamp(1.0, 65_534.0) as u16
}

/// The code of the conduction bound `bound_m` (metres), rounded down like [`code_of`]: a vertex is wet at the bound when its floor is at most
/// this. Monotone in the bound (a larger bound never makes a vertex dry). A NaN bound gives 0 (nothing but the ocean is wet); a bound below the lowest code gives that code
/// (1, as [`code_of`] clamps), so the wet set stays a superset; a bound above the range gives the highest finite code.
pub fn bound_code(bound_m: f64) -> u16 {
    if bound_m.is_nan() {
        return 0;
    }
    let c = libm::floor(bound_m * 2.0) + f64::from(CODE_ZERO);
    c.clamp(1.0, 65_534.0) as u16
}

/// The floors of one block (`(cells + 1)^2`, `k` fastest) or of a whole grid, read as a lattice by the water code.
pub trait FloorLattice: Sync {
    /// Cells per face edge of the bake.
    fn res(&self) -> u32;
    /// The floor of vertex `(i, j)` of `face`.
    fn floor(&self, face: Face, i: u32, j: u32) -> Result<u16, ReadError>;
}

/// The floors of a whole in-memory bake: one `u16` per raw vertex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorGrid {
    pub res: u32,
    pub faces: [Vec<u16>; 6],
}

impl FloorGrid {
    pub fn zeros(res: u32) -> FloorGrid {
        let n = (res as usize + 1) * (res as usize + 1);
        FloorGrid { res, faces: std::array::from_fn(|_| vec![INF; n]) }
    }

    pub fn at(&self, face: Face, i: u32, j: u32) -> u16 {
        self.faces[face.0 as usize][j as usize * (self.res as usize + 1) + i as usize]
    }

    pub fn set(&mut self, face: Face, i: u32, j: u32, v: u16) {
        self.faces[face.0 as usize][j as usize * (self.res as usize + 1) + i as usize] = v;
    }
}

impl FloorLattice for FloorGrid {
    fn res(&self) -> u32 {
        self.res
    }

    fn floor(&self, face: Face, i: u32, j: u32) -> Result<u16, ReadError> {
        if i > self.res || j > self.res {
            return Err(ReadError::Source(format!("vertex ({i}, {j}) of a {}-cell grid", self.res)));
        }
        Ok(self.at(face, i, j))
    }
}

/// The edges one block emits (pass 1): `(terminal, terminal, weight)` with the terminals node ids or the ocean node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FloorEdges {
    pub edges: Vec<(u64, u64, u16)>,
}

fn find(parent: &mut [u32], mut x: u32) -> u32 {
    let mut root = x;
    while parent[root as usize] != root {
        root = parent[root as usize];
    }
    while parent[x as usize] != root {
        let next = parent[x as usize];
        parent[x as usize] = root;
        x = next;
    }
    root
}

/// The 8 neighbours of local vertex `(k, l)` of a block of `n x n` vertices.
fn neighbours(n: usize, k: usize, l: usize) -> impl Iterator<Item = usize> {
    (-1i64..=1).flat_map(move |dl| (-1i64..=1).map(move |dk| (dk, dl))).filter_map(move |(dk, dl)| {
        if dk == 0 && dl == 0 {
            return None;
        }
        let (nk, nl) = (k as i64 + dk, l as i64 + dl);
        (nk >= 0 && nl >= 0 && nk < n as i64 && nl < n as i64).then(|| nl as usize * n + nk as usize)
    })
}

/// The codes of a block: 0 for an ocean vertex, else the elevation's code. `samples` and `ocean` are the block's `(cells + 1)^2` vertices.
pub fn block_codes(cells: u32, samples: &[GlobalSample], ocean: &OceanBits) -> Vec<u16> {
    let n = cells as usize + 1;
    (0..n * n).map(|i| if ocean.get((i % n) as u32, (i / n) as u32) { 0 } else { code_of(samples[i].elevation_m) }).collect()
}

/// Pass 1: the edges of the block `tile` of `cells` cells, from its [`block_codes`].
pub fn analyse_floor_block(index: CubeIndex, tile: TileId, cells: u32, codes: &[u16]) -> FloorEdges {
    let n = cells as usize + 1;
    let nv = n * n;
    let (x0, y0) = (tile.x() * cells, tile.y() * cells);
    let mut order: Vec<u32> = (0..nv as u32).filter(|&i| codes[i as usize] != INF).collect();
    order.sort_unstable_by_key(|&i| (codes[i as usize], i));
    let mut parent: Vec<u32> = (0..nv as u32).collect();
    let mut term = vec![NO_TERM; nv];
    let mut active = vec![false; nv];
    let mut edges: Vec<(u64, u64, u16)> = Vec::new();
    let is_border = |k: usize, l: usize| k == 0 || l == 0 || k == n - 1 || l == n - 1;
    for &v in &order {
        let (k, l) = (v as usize % n, v as usize / n);
        let w = codes[v as usize];
        active[v as usize] = true;
        // The terminals the vertex itself is: an ocean vertex is the ocean node, a border vertex is its global node.
        let node = is_border(k, l).then(|| index.node_id(tile.face(), x0 + k as u32, y0 + l as u32));
        term[v as usize] = if w == 0 { OCEAN_NODE } else { node.unwrap_or(NO_TERM) };
        if w == 0 {
            if let Some(id) = node {
                edges.push((id, OCEAN_NODE, 0));
            }
        }
        for u in neighbours(n, k, l) {
            if !active[u] {
                continue;
            }
            let (a, b) = (find(&mut parent, v), find(&mut parent, u as u32));
            if a == b {
                continue;
            }
            let (ta, tb) = (term[a as usize], term[b as usize]);
            if ta != NO_TERM && tb != NO_TERM && ta != tb {
                edges.push((ta, tb, w));
            }
            let keep = if ta == OCEAN_NODE || tb == NO_TERM {
                ta
            } else if tb == OCEAN_NODE || ta == NO_TERM {
                tb
            } else {
                ta.min(tb)
            };
            parent[b as usize] = a;
            term[a as usize] = keep;
        }
    }
    FloorEdges { edges }
}

/// Pass 2: the floor of every border node the ocean reaches, by node id (a node in no edge, or whose component the ocean does not reach, is
/// absent: its floor is [`INF`]).
pub fn merge_floor_edges(blocks: &[FloorEdges]) -> HashMap<u64, u16> {
    let mut all: Vec<(u16, u64, u64)> = blocks.iter().flat_map(|b| b.edges.iter().map(|&(a, b, w)| (w, a, b))).collect();
    all.sort_unstable();
    let mut dense: HashMap<u64, u32> = HashMap::new();
    let mut ids: Vec<u64> = Vec::new();
    let idx = |id: u64, dense: &mut HashMap<u64, u32>, ids: &mut Vec<u64>| -> u32 {
        *dense.entry(id).or_insert_with(|| {
            ids.push(id);
            (ids.len() - 1) as u32
        })
    };
    let mut parent: Vec<u32> = Vec::new();
    let mut has_ocean: Vec<bool> = Vec::new();
    let mut pending: Vec<Vec<u32>> = Vec::new();
    let mut out: HashMap<u64, u16> = HashMap::new();
    for (w, a, b) in all {
        let (ia, ib) = (idx(a, &mut dense, &mut ids), idx(b, &mut dense, &mut ids));
        while parent.len() < ids.len() {
            let i = parent.len() as u32;
            parent.push(i);
            has_ocean.push(ids[i as usize] == OCEAN_NODE);
            pending.push(if ids[i as usize] == OCEAN_NODE { Vec::new() } else { vec![i] });
        }
        let (ra, rb) = (find(&mut parent, ia), find(&mut parent, ib));
        if ra == rb {
            continue;
        }
        let (oa, ob) = (has_ocean[ra as usize], has_ocean[rb as usize]);
        if oa != ob {
            let dry = if oa { rb } else { ra };
            for t in std::mem::take(&mut pending[dry as usize]) {
                out.insert(ids[t as usize], w);
            }
        }
        // Merge the smaller pending list into the larger.
        let (big, small) = if pending[ra as usize].len() >= pending[rb as usize].len() { (ra, rb) } else { (rb, ra) };
        let moved = std::mem::take(&mut pending[small as usize]);
        pending[big as usize].extend(moved);
        parent[small as usize] = big;
        has_ocean[big as usize] = oa || ob;
        if has_ocean[big as usize] {
            pending[big as usize].clear();
        }
    }
    out
}

/// Pass 3: the floor of every vertex of the block, from its [`block_codes`] and the border floors of [`merge_floor_edges`].
pub fn floor_block(index: CubeIndex, tile: TileId, cells: u32, codes: &[u16], border: &HashMap<u64, u16>) -> Vec<u16> {
    let n = cells as usize + 1;
    let nv = n * n;
    let (x0, y0) = (tile.x() * cells, tile.y() * cells);
    let mut val = vec![INF; nv];
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); 65_536];
    for v in 0..nv {
        let (k, l) = (v % n, v / n);
        let seed = if codes[v] == 0 {
            Some(0)
        } else if k == 0 || l == 0 || k == n - 1 || l == n - 1 {
            border.get(&index.node_id(tile.face(), x0 + k as u32, y0 + l as u32)).copied()
        } else {
            None
        };
        if let Some(s) = seed {
            val[v] = s;
            buckets[s as usize].push(v as u32);
        }
    }
    for b in 0..65_535usize {
        while let Some(u) = buckets[b].pop() {
            if val[u as usize] as usize != b {
                continue;
            }
            let (k, l) = (u as usize % n, u as usize / n);
            for w in neighbours(n, k, l) {
                if codes[w] == INF {
                    continue;
                }
                let nvl = (codes[w] as usize).max(b);
                if nvl < val[w] as usize {
                    val[w] = nvl as u16;
                    buckets[nvl].push(w as u32);
                }
            }
        }
    }
    val
}

/// The serial reference over a whole in-memory bake: a bucketed priority flood on the nodes, copies merged. The floor of vertex
/// `(face, i, j)` is `out[raw id of its node]`; use [`reference_floor_grid`] for a grid.
pub fn reference_floors(grid: &GlobalGrid, ocean: &OceanGrid) -> Vec<u16> {
    let res = grid.res;
    let index = CubeIndex::new(res);
    let n = index.n();
    let total = (6 * n * n) as usize;
    let code_at = |f: Face, i: u32, j: u32| -> u16 {
        if ocean.get(f, i, j) {
            0
        } else {
            code_of(grid.at(f, i, j).elevation_m)
        }
    };
    let mut val = vec![INF; total];
    let mut buckets: Vec<Vec<u64>> = vec![Vec::new(); 65_536];
    for raw in 0..total as u64 {
        let (f, i, j) = index.unraw(raw);
        if index.node_id(f, i, j) == raw && code_at(f, i, j) == 0 {
            val[raw as usize] = 0;
            buckets[0].push(raw);
        }
    }
    for b in 0..65_535usize {
        while let Some(r) = buckets[b].pop() {
            if val[r as usize] as usize != b {
                continue;
            }
            let (f, i, j) = index.unraw(r);
            for &(cf, ci, cj) in index.copies(f, i, j).as_slice() {
                for dj in -1i64..=1 {
                    for di in -1i64..=1 {
                        if di == 0 && dj == 0 {
                            continue;
                        }
                        let (ni, nj) = (i64::from(ci) + di, i64::from(cj) + dj);
                        if ni < 0 || nj < 0 || ni > i64::from(res) || nj > i64::from(res) {
                            continue;
                        }
                        let (ni, nj) = (ni as u32, nj as u32);
                        let c = code_at(cf, ni, nj);
                        if c == INF {
                            continue;
                        }
                        let id = index.node_id(cf, ni, nj);
                        let nvl = (c as usize).max(b);
                        if nvl < val[id as usize] as usize {
                            val[id as usize] = nvl as u16;
                            buckets[nvl].push(id);
                        }
                    }
                }
            }
        }
    }
    val
}

/// [`reference_floors`] as a [`FloorGrid`].
pub fn reference_floor_grid(grid: &GlobalGrid, ocean: &OceanGrid) -> FloorGrid {
    let index = CubeIndex::new(grid.res);
    let by_node = reference_floors(grid, ocean);
    let mut out = FloorGrid::zeros(grid.res);
    for f in 0..6u8 {
        for j in 0..=grid.res {
            for i in 0..=grid.res {
                out.set(Face(f), i, j, by_node[index.node_id(Face(f), i, j) as usize]);
            }
        }
    }
    out
}

/// The tiled computation over an in-memory bake: blocks of `cells` cells on `threads` threads, the three passes, the floors of every
/// block written into a [`FloorGrid`]. Equal to [`reference_floor_grid`] for every block size and thread count.
pub fn tiled_floors(grid: &GlobalGrid, ocean: &OceanGrid, cells: u32, threads: usize) -> Result<FloorGrid, crate::ocean::OceanError> {
    let res = grid.res;
    let index = CubeIndex::new(res);
    let tiles = crate::ocean::block_tiles(res, cells)?;
    let codes_of = |t: &TileId| -> Vec<u16> {
        let samples = crate::ocean::grid_block(grid, *t, cells);
        let mut bits = OceanBits::zeros(cells);
        for l in 0..=cells {
            for k in 0..=cells {
                if ocean.get(t.face(), t.x() * cells + k, t.y() * cells + l) {
                    bits.set(k, l);
                }
            }
        }
        block_codes(cells, &samples, &bits)
    };
    let edges = parallel_map(&tiles, threads, &|t| analyse_floor_block(index, *t, cells, &codes_of(t)));
    let border = merge_floor_edges(&edges);
    let floors = parallel_map(&tiles, threads, &|t| floor_block(index, *t, cells, &codes_of(t), &border));
    let mut out = FloorGrid::zeros(res);
    let n = cells as usize + 1;
    for (t, block) in tiles.iter().zip(floors) {
        for l in 0..n {
            for k in 0..n {
                out.set(t.face(), t.x() * cells + k as u32, t.y() * cells + l as u32, block[l * n + k]);
            }
        }
    }
    Ok(out)
}
