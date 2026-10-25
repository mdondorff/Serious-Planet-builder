//! Ocean connectivity in the global bake (ADR 0041 point 2, GEN-022).
//!
//! A bake vertex is *sea* when its stored continentalness is at or below [`SEA_LEVEL_C`] (the complement of the land rule
//! of ADR 0026 point 3: `f64::from(continent) <= SEA_LEVEL_C`). The sea vertices form a graph under 4-connectivity on the six
//! cube-face grids in which the copies of a vertex on a shared face edge are **one node** (two copies on an edge, three at
//! each of the 8 cube corners; found by exact integer cube points, never by floating point). Its connected components are
//! labelled by their smallest node id, a node's id being the smallest raw vertex id (`face (res+1)^2 + j (res+1) + i`) among
//! its copies. A component whose area, the sum of the solid-angle areas of the dual cells of its vertices, reaches the
//! **area threshold** (body data, a fraction of the body's surface) is **ocean**; every other component, an inland basin below
//! sea level, stays dry. The channel is categorical: one bit per bake vertex.
//!
//! The components are found by a **tiled union-find**, one tile per bake block (the unit the bake holds in memory):
//!
//! 1. [`analyse_block`] runs while a block is in memory. It labels the sea vertices of the block with a scanline union-find whose
//!    roots are minimal indices, sums the fixed-point area and the node count of every local component over the vertices the
//!    block owns (a partition of every face: a block owns its vertices except the far border it shares with the next block,
//!    which the last block of a row owns too), and reports the unions that cross the block's border: for every maximal run of sea
//!    vertices along a side of the block, the local component's label joined with the run's smallest node id, and the same for
//!    each sea corner. Two blocks (or two faces) that share a run, or a corner, report the same node, so their components meet.
//! 2. [`OceanMerger`] joins the reported pairs in a global union-find over node ids with minimal roots (sorted first, so the
//!    order the blocks arrive in is irrelevant), sums areas and node counts per root and applies the threshold: an
//!    [`OceanTable`] of components with their labels, node counts, areas and ocean flags, and the list of local component labels
//!    that are ocean.
//! 3. [`ocean_bits`] labels a block again and sets one bit per vertex whose local component's label is in that list.
//!
//! Areas are integers (fixed point, `2^-48` of the unit sphere's area), so the sums do not depend on the order or the number
//! of threads; the vertex areas are IEEE `+ - * /`, `sqrt` and `libm::atan`. Everything is a set function of the bake: the
//! result is bit-identical for every block size, thread count and block order (CON-14). [`reference_ocean`] is the serial
//! queue-fill reference that the tiled result is tested against.
//!
//! Memory: a block needs about 10 bytes per vertex beyond its samples while it is analysed (the sea mask, the union-find and the
//! labels; the area corners are two rows), the merger keeps one record per local component and per border run (a few per block), never the labels; the
//! number of components is the number of connected sea regions, a handful at bake resolution (SPIKE-08: 5 at 16384 cells).

use planet_core::cube::{Face, FaceMapping};
use planet_core::hash::hash_u64s;
use planet_core::TileId;

use crate::global::{GlobalGrid, GlobalSample, ReadError, GLOBAL_VERSION, SEA_LEVEL_C};

/// Fixed-point scale of an area: `2^-48` of the unit sphere's solid angle (4 pi), so a face-sized area is about `2^50`.
pub const AREA_FP_BITS: u32 = 48;
const AREA_FP_SCALE: f64 = (1u64 << AREA_FP_BITS) as f64;

/// The unit sphere's area in fixed point (the sum of the areas of all vertex copies, up to the rounding of the vertices).
pub const SPHERE_AREA_FP: f64 = 4.0 * core::f64::consts::PI * AREA_FP_SCALE;

/// The Earth default of the area threshold (ADR 0041 point 2): 1e6 km^2, about 0.196 % of the surface. It is body data in the
/// world definition (`body.<n>.sea.min_area_fraction`), so the constant lives there.
pub use planet_world_def::EARTH_OCEAN_MIN_AREA_FRACTION;

/// Names the ocean channel in cache keys (CON-07): the connectivity of the sea over the global bake.
pub const OCEAN_IMPLEMENTATION_ID: &str = "cpu-ocean-connectivity-v1";

/// Why an ocean computation was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum OceanError {
    /// The area threshold must be a finite fraction from 0 to 1.
    BadThreshold(f64),
    /// A bake resolution must be a power of two up to the tiled limit.
    BadResolution(u32),
    /// A block's shape does not match what the call says.
    BadBlock(String),
}

impl std::fmt::Display for OceanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OceanError::BadThreshold(t) => write!(f, "the ocean area threshold is a fraction of the body's surface from 0 to 1, got {t}"),
            OceanError::BadResolution(r) => write!(f, "the ocean channel needs a power-of-two bake resolution up to 65536, got {r}"),
            OceanError::BadBlock(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for OceanError {}

/// The area threshold in fixed point for a fraction of the body's surface (a component is ocean when its area is at least this).
/// A fraction of 0 makes every sea component ocean; a fraction above what any component reaches makes none.
pub fn threshold_fp(fraction: f64) -> Result<u64, OceanError> {
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return Err(OceanError::BadThreshold(fraction));
    }
    // `fraction * 4 pi 2^48` is below 2^52: the cast is exact in range, and rounds towards zero.
    Ok((fraction * SPHERE_AREA_FP) as u64)
}

/// Whether a sample is sea for the connectivity: the complement of the land rule of ADR 0026 point 3.
pub fn is_sea(s: &GlobalSample) -> bool {
    f64::from(s.continent) <= SEA_LEVEL_C
}

// ------------------------------------------------------------------------------------------------------------------------
// Nodes: vertices with the copies on shared face edges merged

/// The integer indexing of the vertices of a bake of `res` cells per face edge: raw ids, and the copies of a vertex on a shared
/// face edge as one node.
#[derive(Clone, Copy, Debug)]
pub struct CubeIndex {
    res: u32,
}

/// The vertex copies of one node: at most three (a cube corner).
#[derive(Clone, Copy, Debug)]
pub struct Copies {
    items: [(Face, u32, u32); 3],
    len: usize,
}

impl Copies {
    pub fn as_slice(&self) -> &[(Face, u32, u32)] {
        &self.items[..self.len]
    }
}

fn axes(face: Face) -> ([i64; 3], [i64; 3], [i64; 3]) {
    let r = |v: planet_core::Vec3| [v.x as i64, v.y as i64, v.z as i64];
    (r(face.normal()), r(face.u_axis()), r(face.v_axis()))
}

impl CubeIndex {
    pub fn new(res: u32) -> CubeIndex {
        CubeIndex { res }
    }

    pub fn res(self) -> u32 {
        self.res
    }

    /// Vertices per face edge.
    pub fn n(self) -> u64 {
        u64::from(self.res) + 1
    }

    /// The raw id of vertex `(i, j)` of `face`: unique per copy.
    pub fn raw(self, face: Face, i: u32, j: u32) -> u64 {
        let n = self.n();
        u64::from(face.0) * n * n + u64::from(j) * n + u64::from(i)
    }

    /// Whether `(i, j)` lies on the border of its face, where copies exist.
    pub fn on_edge(self, i: u32, j: u32) -> bool {
        i == 0 || j == 0 || i == self.res || j == self.res
    }

    /// Every copy of the node of vertex `(i, j)` of `face`, itself included, by exact integer cube points: in units of half a
    /// cell the point is `res n + (2 i - res) U + (2 j - res) V`, and a face holds it when its normal coordinate is `res`.
    pub fn copies(self, face: Face, i: u32, j: u32) -> Copies {
        let mut out = Copies { items: [(face, i, j); 3], len: 1 };
        if !self.on_edge(i, j) {
            return out;
        }
        let r = i64::from(self.res);
        let (n, u, v) = axes(face);
        let p: [i64; 3] = [0, 1, 2].map(|k| n[k] * r + (2 * i64::from(i) - r) * u[k] + (2 * i64::from(j) - r) * v[k]);
        out.len = 0;
        for g in Face::ALL {
            let (gn, gu, gv) = axes(g);
            let dot = |a: [i64; 3]| p[0] * a[0] + p[1] * a[1] + p[2] * a[2];
            if dot(gn) == r {
                let (gi, gj) = ((dot(gu) + r) / 2, (dot(gv) + r) / 2);
                out.items[out.len] = (g, gi as u32, gj as u32);
                out.len += 1;
            }
        }
        out
    }

    /// The node id: the smallest raw id among the copies.
    pub fn node_id(self, face: Face, i: u32, j: u32) -> u64 {
        if !self.on_edge(i, j) {
            return self.raw(face, i, j);
        }
        self.copies(face, i, j).as_slice().iter().map(|&(f, a, b)| self.raw(f, a, b)).min().expect("a node has a copy")
    }

    /// The vertex a raw id names.
    pub fn unraw(self, raw: u64) -> (Face, u32, u32) {
        let n = self.n();
        let (f, rem) = (raw / (n * n), raw % (n * n));
        (Face(f as u8), (rem % n) as u32, (rem / n) as u32)
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// Areas

fn corner_f(u: f64, v: f64) -> f64 {
    libm::atan(u * v / libm::sqrt(1.0 + u * u + v * v))
}

/// The solid angle of the rectangle `[u0, u1] x [v0, v1]` of the tangent plane from its corner values `F(u, v)`, `F = atan(u v /
/// sqrt(1 + u^2 + v^2))`: the one expression every area in this module uses.
fn rect(f00: f64, f10: f64, f01: f64, f11: f64) -> f64 {
    ((f11 - f10) - f01) + f00
}

fn to_fp(omega: f64) -> u64 {
    libm::floor(omega.max(0.0) * AREA_FP_SCALE + 0.5) as u64
}

/// The tangent-plane coordinate at half-cell position `h` (0 to `2 res`; vertex `i` is at `2 i`).
fn plane_at(map: &dyn FaceMapping, res: u32, h: i64) -> f64 {
    // `h / res - 1` is exact for a power of two `res`, so the edge values are exactly +-1.
    map.face_to_plane(h as f64 / f64::from(res) - 1.0)
}

fn clip_h(h: i64, res: u32) -> i64 {
    h.clamp(0, 2 * i64::from(res))
}

/// The area of the dual cell of vertex `(i, j)` on its own face (a half cell on an edge, a quarter at a corner), in fixed
/// point: the solid angle of the cell `[i - 1/2, i + 1/2] x [j - 1/2, j + 1/2]` clipped to the face. The six faces' vertices
/// sum to the sphere (up to the rounding of each vertex).
pub fn vertex_area_fp(map: &dyn FaceMapping, res: u32, i: u32, j: u32) -> u64 {
    let (hi, hj) = (2 * i64::from(i), 2 * i64::from(j));
    let (u0, u1) = (plane_at(map, res, clip_h(hi - 1, res)), plane_at(map, res, clip_h(hi + 1, res)));
    let (v0, v1) = (plane_at(map, res, clip_h(hj - 1, res)), plane_at(map, res, clip_h(hj + 1, res)));
    to_fp(rect(corner_f(u0, v0), corner_f(u1, v0), corner_f(u0, v1), corner_f(u1, v1)))
}

/// The tangent-plane coordinates of the corners of the dual cells along one axis of the block whose lower corner vertex is `o`:
/// corner `k` is the lower corner of vertex `o + k` (`cells + 2` values), so vertex `o + k` spans corners `k` and `k + 1`. Equal,
/// bit for bit, to the values [`vertex_area_fp`] computes for each vertex.
fn corner_axis(map: &dyn FaceMapping, res: u32, o: u32, cells: u32) -> Vec<f64> {
    (0..=i64::from(cells) + 1).map(|k| plane_at(map, res, clip_h(2 * (i64::from(o) + k) - 1, res))).collect()
}

// ------------------------------------------------------------------------------------------------------------------------
// Local labelling of one block

const NO_COMP: u32 = u32::MAX;

/// The sea vertices of a block labelled by 4-connected component, components numbered in the order of their smallest index.
struct Labelled {
    /// Component of every vertex, [`NO_COMP`] on land.
    comp: Vec<u32>,
    /// The smallest vertex index of every component.
    root: Vec<u32>,
}

fn find(parent: &mut [u32], mut a: u32) -> u32 {
    while parent[a as usize] != a {
        let up = parent[parent[a as usize] as usize];
        parent[a as usize] = up;
        a = up;
    }
    a
}

/// Scanline union-find over an `n x n` row-major mask with minimal-index roots.
fn label_mask(sea: &[bool], n: usize) -> Labelled {
    let mut parent: Vec<u32> = (0..sea.len() as u32).collect();
    for l in 0..n {
        for k in 0..n {
            let idx = l * n + k;
            if !sea[idx] {
                continue;
            }
            if k > 0 && sea[idx - 1] {
                let r = find(&mut parent, (idx - 1) as u32);
                parent[idx] = r;
            }
            if l > 0 && sea[idx - n] {
                let up = find(&mut parent, (idx - n) as u32);
                let me = find(&mut parent, idx as u32);
                match up.cmp(&me) {
                    std::cmp::Ordering::Less => parent[me as usize] = up,
                    std::cmp::Ordering::Greater => parent[up as usize] = me,
                    std::cmp::Ordering::Equal => {}
                }
            }
        }
    }
    let mut comp = vec![NO_COMP; sea.len()];
    let mut root = Vec::new();
    for idx in 0..sea.len() {
        if !sea[idx] {
            continue;
        }
        // Roots are minimal indices and the scan is ascending, so a vertex's root has been numbered already, unless it is one.
        let r = find(&mut parent, idx as u32);
        if r as usize == idx {
            comp[idx] = root.len() as u32;
            root.push(idx as u32);
        } else {
            comp[idx] = comp[r as usize];
        }
    }
    Labelled { comp, root }
}

/// The geometry of one block of a bake.
#[derive(Clone, Copy)]
struct BlockGeom {
    index: CubeIndex,
    face: Face,
    /// The lattice indices of the block's lower corner vertex.
    i0: u32,
    j0: u32,
    cells: u32,
    /// Whether the block is the last of its row / column on the face (it owns the far border).
    last_x: bool,
    last_y: bool,
}

impl BlockGeom {
    fn new(res: u32, tile: TileId, cells: u32) -> Result<BlockGeom, OceanError> {
        if res == 0 || !res.is_power_of_two() || u64::from(res) > crate::global::MAX_TILED_RES {
            return Err(OceanError::BadResolution(res));
        }
        if cells == 0 || !cells.is_power_of_two() || cells > res || cells > crate::global::MAX_BLOCK_CELLS {
            return Err(OceanError::BadBlock(format!(
                "{cells} cells per block do not tile a grid of {res} cells in blocks of at most {}",
                crate::global::MAX_BLOCK_CELLS
            )));
        }
        let level = (res / cells).trailing_zeros();
        if u32::from(tile.level()) != level {
            return Err(OceanError::BadBlock(format!(
                "tile {} is at level {}, a grid of {res} cells in blocks of {cells} is at level {level}",
                tile.raw(),
                tile.level()
            )));
        }
        let side = 1u32 << level;
        Ok(BlockGeom {
            index: CubeIndex::new(res),
            face: tile.face(),
            i0: tile.x() * cells,
            j0: tile.y() * cells,
            cells,
            last_x: tile.x() == side - 1,
            last_y: tile.y() == side - 1,
        })
    }

    fn n(&self) -> usize {
        self.cells as usize + 1
    }

    fn node_id(&self, k: usize, l: usize) -> u64 {
        self.index.node_id(self.face, self.i0 + k as u32, self.j0 + l as u32)
    }

    fn owned(&self, k: usize, l: usize) -> bool {
        (k < self.cells as usize || self.last_x) && (l < self.cells as usize || self.last_y)
    }

    fn on_border(&self, k: usize, l: usize) -> bool {
        k == 0 || l == 0 || k == self.cells as usize || l == self.cells as usize
    }
}

/// A block labelled, with the label (smallest node id) of every local component.
struct Block {
    lab: Labelled,
    /// The label of each component: the smallest node id of its vertices.
    labels: Vec<u64>,
}

fn label_block(geom: BlockGeom, samples: &[GlobalSample]) -> Result<Block, OceanError> {
    let n = geom.n();
    if samples.len() != n * n {
        return Err(OceanError::BadBlock(format!("a block of {} cells has {} samples, {} are needed", geom.cells, samples.len(), n * n)));
    }
    let sea: Vec<bool> = samples.iter().map(is_sea).collect();
    let lab = label_mask(&sea, n);
    // The root is the smallest index, so its raw id is the smallest raw id of the component; only a vertex on a face edge can
    // have a copy with a smaller id.
    let mut labels: Vec<u64> =
        lab.root.iter().map(|&r| geom.index.raw(geom.face, geom.i0 + r % n as u32, geom.j0 + r / n as u32)).collect();
    for l in 0..n {
        for k in 0..n {
            if !geom.on_border(k, l) {
                continue;
            }
            let c = lab.comp[l * n + k];
            if c == NO_COMP || !geom.index.on_edge(geom.i0 + k as u32, geom.j0 + l as u32) {
                continue;
            }
            let id = geom.node_id(k, l);
            if id < labels[c as usize] {
                labels[c as usize] = id;
            }
        }
    }
    Ok(Block { lab, labels })
}

/// One local component of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalComponent {
    /// The smallest node id of its vertices.
    pub label: u64,
    /// Nodes it owns (a node counts once, at its smallest copy).
    pub nodes: u64,
    /// The fixed-point area of the dual cells of the vertices the block owns.
    pub area_fp: u64,
    /// Whether it contains a vertex of the block's border (it may continue in another block).
    pub border: bool,
}

/// What one block contributes to the global union-find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockAnalysis {
    pub tile: TileId,
    /// The local components, by label.
    pub components: Vec<LocalComponent>,
    /// Unions `(label, node id)` across the block's border: runs of sea along its sides, and its sea corners.
    pub links: Vec<(u64, u64)>,
}

/// Label the sea of one block and report what the global union-find needs (step 1 of the module's algorithm). `samples` is the
/// block's `(cells + 1)^2` samples, `res` the grid's cells per face edge.
pub fn analyse_block(
    map: &dyn FaceMapping,
    res: u32,
    tile: TileId,
    cells: u32,
    samples: &[GlobalSample],
) -> Result<BlockAnalysis, OceanError> {
    let geom = BlockGeom::new(res, tile, cells)?;
    let block = label_block(geom, samples)?;
    let n = geom.n();
    let mut comps: Vec<LocalComponent> =
        block.labels.iter().map(|&label| LocalComponent { label, nodes: 0, area_fp: 0, border: false }).collect();
    // Areas and nodes over the owned vertices. The corner values `F` of the dual cells are evaluated one row at a time (two rows
    // alive), once per vertex instead of four times, and only when the block has sea at all.
    if !comps.is_empty() {
        let (pu, pv) = (corner_axis(map, res, geom.i0, cells), corner_axis(map, res, geom.j0, cells));
        let row = |l: usize, out: &mut Vec<f64>| {
            out.clear();
            out.extend(pu.iter().map(|&u| corner_f(u, pv[l])));
        };
        let (mut lower, mut upper) = (Vec::with_capacity(n + 1), Vec::with_capacity(n + 1));
        row(0, &mut lower);
        for l in 0..n {
            row(l + 1, &mut upper);
            for k in 0..n {
                let c = block.lab.comp[l * n + k];
                if c == NO_COMP {
                    continue;
                }
                let comp = &mut comps[c as usize];
                if geom.on_border(k, l) {
                    comp.border = true;
                }
                if geom.owned(k, l) {
                    comp.area_fp += to_fp(rect(lower[k], lower[k + 1], upper[k], upper[k + 1]));
                    if geom.index.raw(geom.face, geom.i0 + k as u32, geom.j0 + l as u32) == geom.node_id(k, l) {
                        comp.nodes += 1;
                    }
                }
            }
            std::mem::swap(&mut lower, &mut upper);
        }
    }
    let mut links: Vec<(u64, u64)> = Vec::new();
    let last = n - 1;
    // The four sides, each as a list of vertex indices in order.
    let sides: [Vec<(usize, usize)>; 4] = [
        (0..n).map(|k| (k, 0)).collect(),
        (0..n).map(|k| (k, last)).collect(),
        (0..n).map(|l| (0, l)).collect(),
        (0..n).map(|l| (last, l)).collect(),
    ];
    for side in &sides {
        let mut run: Option<(u32, u64)> = None;
        for &(k, l) in side.iter().chain(std::iter::once(&(usize::MAX, usize::MAX))) {
            let c = if k == usize::MAX { NO_COMP } else { block.lab.comp[l * n + k] };
            match (c, run) {
                (NO_COMP, Some((rc, rep))) => {
                    links.push((block.labels[rc as usize], rep));
                    run = None;
                }
                (NO_COMP, None) => {}
                (c, None) => run = Some((c, geom.node_id(k, l))),
                (_, Some((rc, rep))) => run = Some((rc, rep.min(geom.node_id(k, l)))),
            }
        }
    }
    for (k, l) in [(0, 0), (last, 0), (0, last), (last, last)] {
        let c = block.lab.comp[l * n + k];
        if c != NO_COMP {
            links.push((block.labels[c as usize], geom.node_id(k, l)));
        }
    }
    links.sort_unstable();
    links.dedup();
    comps.sort_by_key(|c| c.label);
    Ok(BlockAnalysis { tile, components: comps, links })
}

/// One connected component of the sea of a bake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Component {
    /// The smallest node id: the component's identity (future lake ids are derived from it).
    pub label: u64,
    pub nodes: u64,
    pub area_fp: u64,
    pub ocean: bool,
}

/// The components of a bake and which of them are ocean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OceanTable {
    pub res: u32,
    pub threshold_fp: u64,
    /// By label.
    pub components: Vec<Component>,
    /// The labels of the local components (per block) that belong to ocean components, sorted: what [`ocean_bits`] looks up.
    ocean_labels: Vec<u64>,
}

impl OceanTable {
    pub fn ocean_components(&self) -> impl Iterator<Item = &Component> {
        self.components.iter().filter(|c| c.ocean)
    }

    /// Whether the local component with this label (any block) is part of the ocean.
    pub fn is_ocean_label(&self, label: u64) -> bool {
        self.ocean_labels.binary_search(&label).is_ok()
    }

    /// The area of a component as a fraction of the body's surface.
    pub fn fraction(&self, c: &Component) -> f64 {
        c.area_fp as f64 / SPHERE_AREA_FP
    }

    /// A 64-bit hash of the table (the known answer of a bake): resolution, version, threshold and every component.
    pub fn hash(&self) -> u64 {
        let mut words = vec![u64::from(self.res), u64::from(GLOBAL_VERSION), self.threshold_fp, self.components.len() as u64];
        for c in &self.components {
            words.extend([c.label, c.nodes, c.area_fp, u64::from(c.ocean)]);
        }
        hash_u64s(0x6f63_6e74, &words)
    }
}

/// Step 2: the global union-find over the reports of the blocks.
#[derive(Debug)]
pub struct OceanMerger {
    res: u32,
    threshold_fp: u64,
    components: Vec<LocalComponent>,
    links: Vec<(u64, u64)>,
    blocks: usize,
}

impl OceanMerger {
    pub fn new(res: u32, area_fraction: f64) -> Result<OceanMerger, OceanError> {
        if res == 0 || !res.is_power_of_two() || u64::from(res) > crate::global::MAX_TILED_RES {
            return Err(OceanError::BadResolution(res));
        }
        Ok(OceanMerger { res, threshold_fp: threshold_fp(area_fraction)?, components: Vec::new(), links: Vec::new(), blocks: 0 })
    }

    /// Take a block's report, in any order.
    pub fn add(&mut self, a: BlockAnalysis) {
        self.blocks += 1;
        self.components.extend(a.components);
        self.links.extend(a.links);
    }

    /// Blocks added so far.
    pub fn blocks(&self) -> usize {
        self.blocks
    }

    /// Bytes this merger holds (the records, not counting the allocator's slack).
    pub fn held_bytes(&self) -> usize {
        self.components.len() * std::mem::size_of::<LocalComponent>() + self.links.len() * std::mem::size_of::<(u64, u64)>()
    }

    pub fn finish(mut self) -> OceanTable {
        // Union-find over the sorted distinct keys (labels of border components and link nodes); the index order is the key
        // order, so the root of a set (its smallest index) is its smallest key.
        let mut keys: Vec<u64> = Vec::with_capacity(self.links.len() * 2 + self.components.len());
        keys.extend(self.links.iter().flat_map(|&(a, b)| [a, b]));
        keys.extend(self.components.iter().filter(|c| c.border).map(|c| c.label));
        keys.sort_unstable();
        keys.dedup();
        let at = |k: u64| keys.binary_search(&k).expect("a key was collected from the same lists");
        let mut parent: Vec<u32> = (0..keys.len() as u32).collect();
        for &(a, b) in &self.links {
            let (ra, rb) = (find(&mut parent, at(a) as u32), find(&mut parent, at(b) as u32));
            match ra.cmp(&rb) {
                std::cmp::Ordering::Less => parent[rb as usize] = ra,
                std::cmp::Ordering::Greater => parent[ra as usize] = rb,
                std::cmp::Ordering::Equal => {}
            }
        }
        // Sum the pieces per root.
        self.components.sort_by_key(|c| c.label);
        let mut sums: std::collections::BTreeMap<u64, (u64, u64)> = std::collections::BTreeMap::new();
        for c in &self.components {
            let root = if c.border { keys[find(&mut parent, at(c.label) as u32) as usize] } else { c.label };
            let e = sums.entry(root).or_default();
            e.0 += c.nodes;
            e.1 += c.area_fp;
        }
        let components: Vec<Component> = sums
            .into_iter()
            .map(|(label, (nodes, area_fp))| Component { label, nodes, area_fp, ocean: area_fp >= self.threshold_fp })
            .collect();
        let ocean_root = |label: u64| components.binary_search_by_key(&label, |c| c.label).map(|i| components[i].ocean).unwrap_or(false);
        let mut ocean_labels: Vec<u64> = self
            .components
            .iter()
            .filter(|c| {
                let root = if c.border { keys[find(&mut parent, at(c.label) as u32) as usize] } else { c.label };
                ocean_root(root)
            })
            .map(|c| c.label)
            .collect();
        ocean_labels.sort_unstable();
        ocean_labels.dedup();
        OceanTable { res: self.res, threshold_fp: self.threshold_fp, components, ocean_labels }
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// The channel: one bit per vertex

/// The ocean bits of one block: `(cells + 1)^2` bits row-major (`k` fastest), bit `i` at word `i / 64`, position `i % 64`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OceanBits {
    pub cells: u32,
    pub words: Vec<u64>,
}

impl OceanBits {
    pub fn zeros(cells: u32) -> OceanBits {
        let bits = (u64::from(cells) + 1) * (u64::from(cells) + 1);
        OceanBits { cells, words: vec![0; bits.div_ceil(64) as usize] }
    }

    pub fn get(&self, k: u32, l: u32) -> bool {
        let i = l as usize * (self.cells as usize + 1) + k as usize;
        self.words[i / 64] >> (i % 64) & 1 == 1
    }

    pub fn set(&mut self, k: u32, l: u32) {
        let i = l as usize * (self.cells as usize + 1) + k as usize;
        self.words[i / 64] |= 1 << (i % 64);
    }

    /// The bits as bytes, little endian, `ceil(bits / 8)` of them.
    pub fn to_bytes(&self) -> Vec<u8> {
        let bits = (u64::from(self.cells) + 1) * (u64::from(self.cells) + 1);
        let mut out: Vec<u8> = self.words.iter().flat_map(|w| w.to_le_bytes()).collect();
        out.truncate(bits.div_ceil(8) as usize);
        out
    }

    /// The inverse of [`to_bytes`](OceanBits::to_bytes); `None` when the length does not fit the cells or a bit beyond the
    /// last vertex is set.
    pub fn from_bytes(cells: u32, bytes: &[u8]) -> Option<OceanBits> {
        let mut b = OceanBits::zeros(cells);
        let bits = (u64::from(cells) + 1) * (u64::from(cells) + 1);
        if bytes.len() as u64 != bits.div_ceil(8) {
            return None;
        }
        for (i, chunk) in bytes.chunks(8).enumerate() {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            b.words[i] = u64::from_le_bytes(w);
        }
        let spare = b.words.len() as u64 * 64 - bits;
        if spare > 0 && b.words[b.words.len() - 1] >> (64 - spare) != 0 {
            return None;
        }
        Some(b)
    }

    pub fn count(&self) -> u64 {
        self.words.iter().map(|w| u64::from(w.count_ones())).sum()
    }
}

/// Step 3: the ocean bits of a block, given the table of the whole bake.
pub fn ocean_bits(res: u32, tile: TileId, cells: u32, samples: &[GlobalSample], table: &OceanTable) -> Result<OceanBits, OceanError> {
    if table.res != res {
        return Err(OceanError::BadBlock(format!("the table is of a {}-cell bake, the block of a {res}-cell bake", table.res)));
    }
    let geom = BlockGeom::new(res, tile, cells)?;
    let block = label_block(geom, samples)?;
    let n = geom.n();
    let ocean: Vec<bool> = block.labels.iter().map(|&l| table.is_ocean_label(l)).collect();
    let mut bits = OceanBits::zeros(cells);
    for l in 0..n {
        for k in 0..n {
            let c = block.lab.comp[l * n + k];
            if c != NO_COMP && ocean[c as usize] {
                bits.set(k as u32, l as u32);
            }
        }
    }
    Ok(bits)
}

/// Where a reader gets the ocean flag of bake vertices: vertex `(i, j)` of `face` of a grid of [`OceanLattice::res`] cells.
/// Implemented by an in-memory [`OceanGrid`] and by the tile store (`planet_cache::global_store::StoreOceanSource`).
pub trait OceanLattice: Sync {
    fn res(&self) -> u32;
    fn is_ocean(&self, face: Face, i: u32, j: u32) -> Result<bool, ReadError>;
}

/// The ocean channel of a whole in-memory bake (`res` at most the in-memory limit): one bit per raw vertex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OceanGrid {
    pub res: u32,
    pub faces: [Vec<u64>; 6],
}

impl OceanGrid {
    pub fn zeros(res: u32) -> OceanGrid {
        let bits = (u64::from(res) + 1) * (u64::from(res) + 1);
        let words = bits.div_ceil(64) as usize;
        OceanGrid { res, faces: std::array::from_fn(|_| vec![0; words]) }
    }

    pub fn get(&self, face: Face, i: u32, j: u32) -> bool {
        let idx = j as usize * (self.res as usize + 1) + i as usize;
        self.faces[face.0 as usize][idx / 64] >> (idx % 64) & 1 == 1
    }

    pub fn set(&mut self, face: Face, i: u32, j: u32) {
        let idx = j as usize * (self.res as usize + 1) + i as usize;
        self.faces[face.0 as usize][idx / 64] |= 1 << (idx % 64);
    }

    pub fn count(&self) -> u64 {
        self.faces.iter().flatten().map(|w| u64::from(w.count_ones())).sum()
    }

    /// A 64-bit hash of every bit: the known answer of the channel.
    pub fn hash(&self) -> u64 {
        let mut words = vec![u64::from(self.res), u64::from(GLOBAL_VERSION)];
        words.extend(self.faces.iter().flatten().copied());
        hash_u64s(0x6f63_6269, &words)
    }
}

impl OceanLattice for OceanGrid {
    fn res(&self) -> u32 {
        self.res
    }

    fn is_ocean(&self, face: Face, i: u32, j: u32) -> Result<bool, ReadError> {
        if i > self.res || j > self.res {
            return Err(ReadError::Source(format!("vertex ({i}, {j}) of a {}-cell grid", self.res)));
        }
        Ok(self.get(face, i, j))
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// In-memory drivers: the serial reference and the tiled computation over a `GlobalGrid`

/// The serial reference: components by an explicit-queue fill over the node graph (no recursion, a visited array), the ocean
/// flags of the nodes set on every copy. Independent of the tiled code except for the vertex areas.
pub fn reference_ocean(grid: &GlobalGrid, map: &dyn FaceMapping, area_fraction: f64) -> Result<(OceanTable, OceanGrid), OceanError> {
    let res = grid.res;
    let thr = threshold_fp(area_fraction)?;
    let index = CubeIndex::new(res);
    let n = index.n();
    let total = 6 * n * n;
    let mut visited = vec![false; total as usize];
    let sea = |f: Face, i: u32, j: u32| is_sea(&grid.at(f, i, j));
    let mut components: Vec<Component> = Vec::new();
    let mut members: Vec<Vec<u64>> = Vec::new();
    for raw in 0..total {
        if visited[raw as usize] {
            continue;
        }
        let (f, i, j) = index.unraw(raw);
        // Only the smallest copy of a node starts a component, so the start is the smallest node id of its component.
        if index.node_id(f, i, j) != raw || !sea(f, i, j) {
            continue;
        }
        let (mut nodes, mut area) = (0u64, 0u64);
        let mut queue = vec![raw];
        let mut mine = Vec::new();
        visited[raw as usize] = true;
        while let Some(r) = queue.pop() {
            let (f, i, j) = index.unraw(r);
            nodes += 1;
            mine.push(r);
            for &(cf, ci, cj) in index.copies(f, i, j).as_slice() {
                area += vertex_area_fp(map, res, ci, cj);
                let steps: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                for (di, dj) in steps {
                    let (ni, nj) = (i64::from(ci) + di, i64::from(cj) + dj);
                    if ni < 0 || nj < 0 || ni > i64::from(res) || nj > i64::from(res) || !sea(cf, ni as u32, nj as u32) {
                        continue;
                    }
                    let id = index.node_id(cf, ni as u32, nj as u32);
                    if !visited[id as usize] {
                        visited[id as usize] = true;
                        queue.push(id);
                    }
                }
            }
        }
        components.push(Component { label: raw, nodes, area_fp: area, ocean: area >= thr });
        members.push(mine);
    }
    let mut ocean = OceanGrid::zeros(res);
    for (c, nodes) in components.iter().zip(&members) {
        if !c.ocean {
            continue;
        }
        for &r in nodes {
            let (f, i, j) = index.unraw(r);
            for &(cf, ci, cj) in index.copies(f, i, j).as_slice() {
                ocean.set(cf, ci, cj);
            }
        }
    }
    let table = OceanTable { res, threshold_fp: thr, components, ocean_labels: Vec::new() };
    Ok((table, ocean))
}

/// The block of `grid` for `tile` (cells per block `cells`), as the bake would hand it out.
pub fn grid_block(grid: &GlobalGrid, tile: TileId, cells: u32) -> Vec<GlobalSample> {
    let n = cells + 1;
    let (x0, y0) = (tile.x() * cells, tile.y() * cells);
    let mut v = Vec::with_capacity((n * n) as usize);
    for l in 0..n {
        for k in 0..n {
            v.push(grid.at(tile.face(), x0 + k, y0 + l));
        }
    }
    v
}

/// The tiled computation over an in-memory bake: blocks of `cells` cells analysed on `threads` threads, merged, and the bits of
/// every block written into an [`OceanGrid`]. The table's `ocean_labels` are filled, so [`ocean_bits`] works with it.
pub fn tiled_ocean(
    grid: &GlobalGrid,
    map: &(dyn FaceMapping + Sync),
    cells: u32,
    threads: usize,
    area_fraction: f64,
) -> Result<(OceanTable, OceanGrid), OceanError> {
    let res = grid.res;
    let mut merger = OceanMerger::new(res, area_fraction)?;
    let tiles = block_tiles(res, cells)?;
    let analyses = parallel_map(&tiles, threads, &|t| analyse_block(map, res, *t, cells, &grid_block(grid, *t, cells)));
    for a in analyses {
        merger.add(a?);
    }
    let table = merger.finish();
    let blocks = parallel_map(&tiles, threads, &|t| ocean_bits(res, *t, cells, &grid_block(grid, *t, cells), &table));
    let mut out = OceanGrid::zeros(res);
    for (t, b) in tiles.iter().zip(blocks) {
        let b = b?;
        for l in 0..=cells {
            for k in 0..=cells {
                if b.get(k, l) {
                    out.set(t.face(), t.x() * cells + k, t.y() * cells + l);
                }
            }
        }
    }
    Ok((table, out))
}

/// Every block tile of a grid of `res` cells in blocks of `cells`, face by face, row by row.
pub fn block_tiles(res: u32, cells: u32) -> Result<Vec<TileId>, OceanError> {
    if res == 0 || !res.is_power_of_two() || u64::from(res) > crate::global::MAX_TILED_RES {
        return Err(OceanError::BadResolution(res));
    }
    if cells == 0 || !cells.is_power_of_two() || cells > res {
        return Err(OceanError::BadBlock(format!("{cells} cells per block do not tile a grid of {res} cells")));
    }
    let level = (res / cells).trailing_zeros() as u8;
    let side = 1u32 << level;
    let mut out = Vec::with_capacity(6 * (side as usize) * (side as usize));
    for f in 0..6u8 {
        for y in 0..side {
            for x in 0..side {
                out.push(TileId::new(Face(f), level, x, y).expect("in range"));
            }
        }
    }
    Ok(out)
}

/// `f` over `items` on up to `threads` scoped threads, results in item order.
pub fn parallel_map<T: Sync, R: Send>(items: &[T], threads: usize, f: &(dyn Fn(&T) -> R + Sync)) -> Vec<R> {
    let threads = threads.max(1).min(items.len().max(1));
    if threads == 1 {
        return items.iter().map(f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let parts: Vec<Vec<(usize, R)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                planet_core::thread::spawn_named(s, "ocean-pass", || {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= items.len() {
                            return mine;
                        }
                        mine.push((i, f(&items[i])));
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("ocean worker panicked")).collect()
    });
    let mut flat: Vec<(usize, R)> = parts.into_iter().flatten().collect();
    flat.sort_unstable_by_key(|(i, _)| *i);
    flat.into_iter().map(|(_, r)| r).collect()
}
