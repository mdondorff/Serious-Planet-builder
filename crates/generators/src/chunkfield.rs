//! The chunk field (ADR 0077, GEN-036 to GEN-038): a regional stage computed per chunk on a global cell graph, its box-filtered pyramid and the
//! level-aware height that reads it.
//!
//! The cells are the cells of a cube-sphere lattice of `cells_per_chunk x 2^11` per face edge, so a chunk (the area of a level-11 tile) is
//! `cells_per_chunk` cells wide. A cell's neighbours are the eight cells around it, across a face edge through the direction (as in
//! [`coupled::Lattice`](crate::coupled::Lattice)), so the neighbour relation is a function of the cell and not of the chunk asking: a chunk
//! computes on the ball of cells within `halo` steps of its own around the graph, the region shrinks by one ring per step, and the cells of
//! the chunk (and one ring of apron) come out exact whatever the chunk's position, which is what makes neighbouring chunks agree to the
//! bit across a face edge and at a cube corner (GEN-038).

use crate::chunk::{ChunkId, CHUNK_GRID_LEVEL, TILE_CELLS};
use crate::height::HeightField;
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{FaceMapping, TileId, Vec3};
use planet_world_def::{BodyId, ReadRecord};
use std::collections::HashMap;
use std::sync::Arc;

/// The coarsest tile level whose tiles read the chunk pyramid (GEN-035): tiles above it are the base, where the global run carries the large
/// scale. The chunk grid level, so a tile reads one chunk at most.
pub const PYRAMID_TOP_LEVEL: u8 = 14;

/// The eight neighbours of a cell, in order: the four edge neighbours, then the four diagonals.
const OFFS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// "No cell" in a region's neighbour table (cells at the outer ring have no table).
const NONE: u32 = u32::MAX;

/// A cell of the lattice: the face and the position on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub face: u8,
    pub i: u32,
    pub j: u32,
}

impl Cell {
    /// The cell packed into 64 bits: face, then `i`, then `j` (ordered the way cells are compared).
    pub fn key(self) -> u64 {
        u64::from(self.face) << 56 | u64::from(self.i) << 28 | u64::from(self.j)
    }
}

/// The cube-sphere lattice the chunks are cut from (see the module comment).
#[derive(Clone, Copy, Debug)]
pub struct ChunkLattice {
    per_chunk: u32,
    n: i64,
}

impl ChunkLattice {
    /// `cells_per_chunk` cells per chunk edge; `None` for zero or for a lattice whose indices would not fit the cell key.
    pub fn new(cells_per_chunk: u32) -> Option<ChunkLattice> {
        let n = i64::from(cells_per_chunk) << CHUNK_GRID_LEVEL;
        (cells_per_chunk > 0 && n < (1 << 27)).then_some(ChunkLattice { per_chunk: cells_per_chunk, n })
    }

    pub fn cells_per_chunk(&self) -> u32 {
        self.per_chunk
    }

    /// Cells per face edge.
    pub fn n(&self) -> i64 {
        self.n
    }

    /// The cell size in metres on a body of radius `radius_m`, at the face centre.
    pub fn cell_m(&self, radius_m: f64) -> f64 {
        radius_m * core::f64::consts::FRAC_PI_2 / self.n as f64
    }

    /// The unit direction of the centre of the cell at (`i`, `j`) of `face`; a position beyond the face edge is read on the extended cube plane.
    fn direction_at(&self, face: u8, i: i64, j: i64) -> Vec3 {
        direction_at_n(self.n, face, i, j)
    }

    /// [`ChunkLattice::direction`] for a position given as signed indices, beyond the face edge read on the extended cube plane: the cell
    /// directions of a raster that reaches over a face edge (the horizon bake's halo, M8-5).
    pub fn direction_ext(&self, face: u8, i: i64, j: i64) -> Vec3 {
        self.direction_at(face, i, j)
    }

    /// The unit direction of the centre of `c`.
    pub fn direction(&self, c: Cell) -> Vec3 {
        self.direction_at(c.face, i64::from(c.i), i64::from(c.j))
    }

    /// The cell holding the direction `d`.
    pub fn cell_at(&self, d: Vec3) -> Cell {
        cell_at_n(self.n, d)
    }

    /// The cell at (`i`, `j`) of `face`, where a position beyond the face edge is the cell holding the direction beyond it.
    fn canon(&self, face: u8, i: i64, j: i64) -> Cell {
        canon_n(self.n, face, i, j)
    }

    /// The eight neighbours of `c` in the order of [`OFFS`].
    pub fn neighbours(&self, c: Cell) -> [Cell; 8] {
        let mut out = [c; 8];
        for (k, &(di, dj)) in OFFS.iter().enumerate() {
            out[k] = self.canon(c.face, i64::from(c.i) + di, i64::from(c.j) + dj);
        }
        out
    }
}

/// The direction of the centre of cell (`i`, `j`) of `face` on a lattice of `n` cells per face edge (beyond the edge: the extended cube plane).
fn direction_at_n(n: i64, face: u8, i: i64, j: i64) -> Vec3 {
    let n = n as f64;
    face_to_direction(&TangentWarp, planet_core::Face(face), -1.0 + (2.0 * i as f64 + 1.0) / n, -1.0 + (2.0 * j as f64 + 1.0) / n)
}

/// How far past a cell boundary a position must lie to count as inside the next cell, in cells. Across a cube-face edge near a corner the centre
/// of the cell beyond the edge lies within about 1e-9 cells of a boundary of the other face's lattice (8 m cells: 1e-12), where the rounding of
/// the direction maths would pick the neighbour; the bias picks the upper cell whatever the rounding.
const TIE_BIAS_CELLS: f64 = 1e-6;

/// The cell of a lattice of `n` cells per face edge holding the direction `d` (a position within [`TIE_BIAS_CELLS`] below a boundary is read as
/// above it).
fn cell_at_n(n: i64, d: Vec3) -> Cell {
    let (f, s, t) = direction_to_face(&TangentWarp, d);
    let i = (((s + 1.0) * 0.5 * n as f64 + TIE_BIAS_CELLS) as i64).clamp(0, n - 1);
    let j = (((t + 1.0) * 0.5 * n as f64 + TIE_BIAS_CELLS) as i64).clamp(0, n - 1);
    Cell { face: f.0, i: i as u32, j: j as u32 }
}

/// The cell at (`i`, `j`) of `face` on a lattice of `n` cells per face edge, a position beyond the face edge being the cell holding the
/// direction beyond it.
fn canon_n(n: i64, face: u8, i: i64, j: i64) -> Cell {
    if (0..n).contains(&i) && (0..n).contains(&j) {
        Cell { face, i: i as u32, j: j as u32 }
    } else {
        cell_at_n(n, direction_at_n(n, face, i, j))
    }
}

/// The cells a chunk computes on: its own `cells_per_chunk^2` cells first (row-major, `j` outer), then the ball around them ring by ring.
pub struct Region {
    pub cells: Vec<Cell>,
    /// Steps from the chunk's own cells (0 for those).
    pub depth: Vec<u16>,
    /// The neighbours of the cells inside the outermost ring, as indices into `cells` (`NONE` for the outermost ring).
    pub neighbours: Vec<[u32; 8]>,
    /// The number of cells of the chunk itself.
    pub core: usize,
    /// The outermost ring's depth.
    pub halo: u16,
    /// The position of every cell in `cells`, by the cell's key.
    index: HashMap<u64, u32>,
}

impl Region {
    /// The position of `cell` in [`Region::cells`], if it is in the region.
    pub fn index_of(&self, cell: Cell) -> Option<usize> {
        self.index.get(&cell.key()).map(|&i| i as usize)
    }
}

/// The region of the chunk over `tile` (a tile of the chunk grid level) with `halo` rings around it.
pub fn region(lat: &ChunkLattice, tile: TileId, halo: u16) -> Region {
    let n = lat.per_chunk;
    let mut cells = Vec::with_capacity((n as usize + 2 * halo as usize).pow(2));
    let mut depth = Vec::with_capacity(cells.capacity());
    let mut index: HashMap<u64, u32> = HashMap::with_capacity(cells.capacity());
    for j in 0..n {
        for i in 0..n {
            let c = Cell { face: tile.face().0, i: tile.x() * n + i, j: tile.y() * n + j };
            index.insert(c.key(), cells.len() as u32);
            cells.push(c);
            depth.push(0u16);
        }
    }
    let core = cells.len();
    let mut neighbours: Vec<[u32; 8]> = Vec::with_capacity(cells.capacity());
    let mut at = 0;
    while at < cells.len() && depth[at] < halo {
        let d = depth[at];
        let mut row = [NONE; 8];
        for (k, nb) in lat.neighbours(cells[at]).iter().enumerate() {
            let idx = *index.entry(nb.key()).or_insert_with(|| {
                cells.push(*nb);
                depth.push(d + 1);
                (cells.len() - 1) as u32
            });
            row[k] = idx;
        }
        neighbours.push(row);
        at += 1;
    }
    neighbours.resize(cells.len(), [NONE; 8]);
    Region { cells, depth, neighbours, core, halo, index }
}

/// A chunk stage: a process over a region of the cell graph that yields a change to the base height (ADR 0077 point 3). It states its step
/// cap and its bound; the halo it needs is `step_cap x reach + 1` rings (reach: one ring per step), one ring more than the steps need so the
/// chunk's own cells come out with one exact ring of apron around them.
pub trait ChunkStage: Send + Sync {
    /// The most steps the stage takes.
    fn step_cap(&self) -> u32;
    /// A bound on `|change|` in metres, for the plan's bounds (GEN-039's neighbours).
    fn bound_m(&self) -> f64;
    /// The change in metres at every cell of `region`; the values at depth at most `region.halo - step_cap` are exact (the cells further out
    /// are what the shrinking region left behind and are not to be read).
    fn compute(&self, lat: &ChunkLattice, region: &Region) -> Vec<f32>;
    /// A hash of everything that defines the process (seed, parameters, version): a change re-keys every tile a chunk of it can reach.
    fn definition_hash(&self) -> u64;
}

/// The halo of a stage in rings (a chunk is refused below it): its step cap, each step reaching one ring, plus the apron ring.
pub fn halo_rings(stage: &dyn ChunkStage) -> u16 {
    (stage.step_cap() + 1).min(u32::from(u16::MAX)) as u16
}

/// The change raster of one chunk and its box-filtered pyramid (GEN-037): `levels[k]` is `(n >> k)^2` cells, row-major with `j` outer, each
/// cell the mean of the 2 x 2 cells below it in `levels[k - 1]`.
pub struct ChunkResult {
    tile: TileId,
    n: usize,
    levels: Vec<Vec<f32>>,
}

impl ChunkResult {
    /// The pyramid of the change `values` of the chunk's own cells (`n x n`, row-major) with `k_max` levels above the raster; `None` if `n` is
    /// not divisible by `2^k_max` or the values are not `n x n`.
    pub fn new(tile: TileId, n: usize, values: &[f32], k_max: usize) -> Option<ChunkResult> {
        if values.len() != n * n || n == 0 || !n.is_multiple_of(1usize << k_max) {
            return None;
        }
        let mut levels = vec![values.to_vec()];
        for k in 1..=k_max {
            let (m, below) = (n >> k, &levels[k - 1]);
            let w = n >> (k - 1);
            let mut level = Vec::with_capacity(m * m);
            for j in 0..m {
                for i in 0..m {
                    let (a, b) = (2 * j * w + 2 * i, (2 * j + 1) * w + 2 * i);
                    level.push(0.25 * (below[a] + below[a + 1] + below[b] + below[b + 1]));
                }
            }
            levels.push(level);
        }
        Some(ChunkResult { tile, n, levels })
    }

    /// Bytes of the raster and its levels: `n (u32) | levels above the raster (u32) | every level's values, f32 little endian`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.size_bytes());
        out.extend_from_slice(&(self.n as u32).to_le_bytes());
        out.extend_from_slice(&(self.k_max() as u32).to_le_bytes());
        for level in &self.levels {
            for v in level {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }

    /// The result of `tile` from [`ChunkResult::to_bytes`]; `None` for bytes that are not exactly a pyramid of that form (a short or damaged
    /// payload is a miss, never a partial result).
    pub fn from_bytes(tile: TileId, bytes: &[u8]) -> Option<ChunkResult> {
        let word = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        let (n, k_max) = (word(0)? as usize, word(4)? as usize);
        if n == 0 || n > 1 << 16 || k_max > 16 || !n.is_multiple_of(1usize << k_max) {
            return None;
        }
        let count: usize = (0..=k_max).map(|k| (n >> k) * (n >> k)).sum();
        if bytes.len() != 8 + 4 * count {
            return None;
        }
        let mut at = 8;
        let mut levels = Vec::with_capacity(k_max + 1);
        for k in 0..=k_max {
            let m = (n >> k) * (n >> k);
            levels.push((0..m).map(|i| f32::from_le_bytes(bytes[at + 4 * i..at + 4 * i + 4].try_into().expect("4 bytes"))).collect());
            at += 4 * m;
        }
        Some(ChunkResult { tile, n, levels })
    }

    /// The bytes the result holds in memory (counted toward the chunk pool of the RAM caps, STRM-041).
    pub fn size_bytes(&self) -> usize {
        self.levels.iter().map(|l| 4 * l.len()).sum()
    }
    pub fn tile(&self) -> TileId {
        self.tile
    }

    /// Levels above the raster.
    pub fn k_max(&self) -> usize {
        self.levels.len() - 1
    }

    /// The change at cell (`i`, `j`) of pyramid level `k`.
    pub fn at(&self, k: usize, i: usize, j: usize) -> f32 {
        self.levels[k][j * (self.n >> k) + i]
    }

    /// A hash of every value of every level (FNV-1a over the bits): the known answer CI compares on Windows and Linux.
    /// The state word of this result in a tile key (STRM-038): its hash with the low bit set, so it is never the 0 of a chunk that is not ready.
    pub fn state_word(&self) -> u64 {
        self.hash() | 1
    }

    pub fn hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for level in &self.levels {
            for v in level {
                for b in v.to_bits().to_le_bytes() {
                    h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        h
    }
}

/// The pyramid levels above the raster a chunk of `lat` carries on a body of radius `radius_m`: enough for the tile level `PYRAMID_TOP_LEVEL`,
/// which reads the coarsest (between two levels, so the next one up is carried).
pub fn pyramid_levels(lat: &ChunkLattice, radius_m: f64, top: u8) -> usize {
    pyramid_position(lat, radius_m, top, top).map_or(0, |x| libm::ceil(x) as usize)
}

/// Where tile level `level` reads the pyramid, as a continuous level: `log2` of the tile's vertex spacing over the cell size (0 for the
/// raster, 1 for the first filtered level, and a blend of the two levels around it in between, as a mip map does, so a step in level is no
/// step in the filter); `None` for a tile level above the pyramid's top, which is the base.
pub fn pyramid_position(lat: &ChunkLattice, radius_m: f64, level: u8, top: u8) -> Option<f64> {
    if level < top {
        return None;
    }
    let spacing = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level.min(30)) / f64::from(TILE_CELLS);
    Some(libm::log2(spacing / lat.cell_m(radius_m)).max(0.0))
}

/// The chunk over `tile`, computed by `stage`: the region, the change, the pyramid. One thread, fixed order (CON-14). `halo` is
/// [`halo_rings`] unless a test asks for a smaller one to see what the rule protects.
pub fn compute_chunk(lat: &ChunkLattice, stage: &dyn ChunkStage, radius_m: f64, top: u8, tile: TileId, halo: u16) -> Option<ChunkResult> {
    if halo < halo_rings(stage) {
        return None;
    }
    let reg = region(lat, tile, halo);
    let change = stage.compute(lat, &reg);
    ChunkResult::new(tile, lat.per_chunk as usize, &change[..reg.core], pyramid_levels(lat, radius_m, top))
}

/// A height field with chunk detail over a base field (GEN-036): the base plus, for a tile level at or below the pyramid's top, the change of
/// the ready chunks read at the pyramid level of that tile level, interpolated across the cells. A chunk that is not ready, and a tile level
/// above the pyramid's top, read the base. The ready set is immutable: a landing builds a new field.
///
/// The keys are the base's (the chaining of a chunk's content hash into a tile's key is the next step).
pub struct ChunkField<F: HeightField> {
    base: F,
    lat: ChunkLattice,
    radius_m: f64,
    bound_m: f64,
    top: u8,
    definition: u64,
    ready: HashMap<(u8, u32, u32), Arc<ChunkResult>>,
}

impl<F: HeightField> ChunkField<F> {
    pub fn new(base: F, lat: ChunkLattice, radius_m: f64, stage: &dyn ChunkStage) -> ChunkField<F> {
        ChunkField {
            base,
            lat,
            radius_m,
            bound_m: stage.bound_m(),
            top: PYRAMID_TOP_LEVEL,
            definition: stage.definition_hash(),
            ready: HashMap::new(),
        }
    }

    /// The field whose pyramid reaches up to tile level `top` instead of the default [`PYRAMID_TOP_LEVEL`]; the chunks it is given must have been
    /// computed with the same top.
    pub fn with_top_level(mut self, top: u8) -> ChunkField<F> {
        self.top = top;
        self
    }

    /// The field with `chunk` ready as well.
    pub fn with_ready(mut self, chunk: Arc<ChunkResult>) -> ChunkField<F> {
        let t = chunk.tile();
        assert!(
            chunk.n == self.lat.per_chunk as usize && chunk.k_max() == pyramid_levels(&self.lat, self.radius_m, self.top),
            "a ready chunk must have the lattice and the pyramid top of the field (call with_top_level before with_ready)"
        );
        self.ready.insert((t.face().0, t.x(), t.y()), chunk);
        self
    }

    pub fn base(&self) -> &F {
        &self.base
    }

    /// The change at `dir` as tile level `level` reads it: zero where no chunk is ready.
    pub fn change_at_level(&self, dir: Vec3, level: u8) -> f64 {
        let Some(x) = pyramid_position(&self.lat, self.radius_m, level, self.top) else { return 0.0 };
        let own_tile = TileId::from_direction(&TangentWarp, dir, CHUNK_GRID_LEVEL);
        let Some(own) = self.ready.get(&(own_tile.face().0, own_tile.x(), own_tile.y())) else { return 0.0 };
        let (face, s, t) = direction_to_face(&TangentWarp, dir);
        let k0 = (libm::floor(x) as usize).min(own.k_max());
        let k1 = (k0 + 1).min(own.k_max());
        let frac = if k1 == k0 { 0.0 } else { (x - k0 as f64).min(1.0) };
        let a = self.read_level(own, k0, face.0, s, t);
        if frac == 0.0 {
            a
        } else {
            a * (1.0 - frac) + self.read_level(own, k1, face.0, s, t) * frac
        }
    }

    /// Pyramid level `k` at the face position (`s`, `t`), interpolated with a Catmull-Rom spline across the cells (a linear read of a coarse
    /// level errs by far more than 0.25 m on the stand-in's relief). A cell outside the chunk is the cell of the chunk holding it if that is
    /// ready, else the nearest cell of this one; past the face edge it is the edge cell.
    fn read_level(&self, own: &ChunkResult, k: usize, face: u8, s: f64, t: f64) -> f64 {
        let nk = self.lat.n() >> k;
        let per = i64::from(self.lat.per_chunk) >> k;
        let (u, v) = ((s + 1.0) * 0.5 * nk as f64 - 0.5, (t + 1.0) * 0.5 * nk as f64 - 0.5);
        let (fu, fv) = (libm::floor(u), libm::floor(v));
        let (i0, j0, wx, wy) = (fu as i64, fv as i64, u - fu, v - fv);
        let (ox, oy) = (i64::from(own.tile().x()) * per, i64::from(own.tile().y()) * per);
        let sample = |i: i64, j: i64| -> f64 {
            // Past a face edge the cell is the one holding the direction beyond it, on the other face.
            let c = canon_n(nk, face, i, j);
            let (i, j) = (i64::from(c.i), i64::from(c.j));
            let (chunk, li, lj) = match self.ready.get(&(c.face, (i / per) as u32, (j / per) as u32)) {
                Some(c) => (&**c, (i % per) as usize, (j % per) as usize),
                None => (own, (i.clamp(ox, ox + per - 1) - ox) as usize, (j.clamp(oy, oy + per - 1) - oy) as usize),
            };
            f64::from(chunk.at(k, li, lj))
        };
        let (cx, cy) = (catmull_rom_weights(wx), catmull_rom_weights(wy));
        let mut sum = 0.0;
        for (b, wb) in cy.iter().enumerate() {
            let mut row = 0.0;
            for (a, wa) in cx.iter().enumerate() {
                row += wa * sample(i0 - 1 + a as i64, j0 - 1 + b as i64);
            }
            sum += wb * row;
        }
        sum
    }
}

/// The weights of four equally spaced samples for a point a fraction `f` past the second (Catmull-Rom).
fn catmull_rom_weights(f: f64) -> [f64; 4] {
    let (f2, f3) = (f * f, f * f * f);
    [0.5 * (-f3 + 2.0 * f2 - f), 0.5 * (3.0 * f3 - 5.0 * f2 + 2.0), 0.5 * (-3.0 * f3 + 4.0 * f2 + f), 0.5 * (f3 - f2)]
}

/// The implementation id and version of the chunk stage in a tile key (STRM-038).
pub const CHUNK_STAGE_ID: &str = "chunk-stage";
pub const CHUNK_STAGE_VERSION: u32 = 1;

/// A chunk's place on the grid: face and tile position at the chunk grid level.
pub type ChunkKey = (u8, u32, u32);

/// The chunks a tile reads, own chunk first and the neighbours that its reads can reach after it, in a fixed order; empty for a tile above the
/// pyramid's top (the base). A tile reads cells of the neighbouring chunk within `reach_cells` of its edge (the spline and the blend of two
/// pyramid levels); a neighbour out of that reach is not listed, so a landing re-keys only the tiles it can change.
pub fn chunks_read_by(lat: &ChunkLattice, top: u8, reach_cells: i64, tile: TileId) -> Vec<ChunkKey> {
    if tile.level() < top || tile.level() < CHUNK_GRID_LEVEL {
        return Vec::new();
    }
    let (n, per) = (lat.n(), i64::from(lat.cells_per_chunk()));
    let shift = u32::from(tile.level());
    let range = |index: u32| -> (i64, i64) {
        let lo = ((u64::from(index) * n as u64) >> shift) as i64;
        let hi = (((u64::from(index) + 1) * n as u64 + (1u64 << shift) - 1) >> shift) as i64 - 1;
        (lo, hi)
    };
    let ((i0, i1), (j0, j1)) = (range(tile.x()), range(tile.y()));
    let own = (tile.face().0, (i0 / per) as u32, (j0 / per) as u32);
    let mut out = vec![own];
    let mut others = std::collections::BTreeSet::new();
    for a in [i0 - reach_cells, (i0 + i1) / 2, i1 + reach_cells] {
        for b in [j0 - reach_cells, (j0 + j1) / 2, j1 + reach_cells] {
            let c = canon_n(n, tile.face().0, a, b);
            let key = (c.face, (i64::from(c.i) / per) as u32, (i64::from(c.j) / per) as u32);
            if key != own {
                others.insert(key);
            }
        }
    }
    out.extend(others);
    out
}

/// How far into a neighbouring chunk a read can reach, in raster cells, with margin: the spline and the blend reach two cells of the coarsest
/// pyramid level, and a vertex on a face edge reads through the neighbouring face, whose lattice is shifted along the edge up to about the
/// stencil depth near a cube corner (so three times that, and the vertex spacing).
pub fn reach_cells(lat: &ChunkLattice, radius_m: f64, top: u8) -> i64 {
    3 * (2 << pyramid_levels(lat, radius_m, top)) + 4
}

impl<F: HeightField> ChunkField<F> {
    /// How far into a neighbouring chunk a read of this field can reach, in raster cells, with margin: the spline and the blend reach two cells of
    /// the coarsest level it blends, and a vertex on a face edge reads through the neighbouring face, whose lattice is shifted along the edge up to
    /// about the stencil depth near a cube corner (so three times that, and the vertex spacing).
    pub fn reach_cells(&self) -> i64 {
        reach_cells(&self.lat, self.radius_m, self.top)
    }

    /// The chunks tile `tile` reads (see [`chunks_read_by`]).
    pub fn chunks_read_by(&self, tile: TileId) -> Vec<ChunkKey> {
        chunks_read_by(&self.lat, self.top, self.reach_cells(), tile)
    }

    /// The state word of a chunk: 0 while it is not ready, else the content hash of its result (never 0).
    pub fn chunk_state(&self, key: ChunkKey) -> u64 {
        self.ready.get(&key).map_or(0, |c| c.state_word())
    }

    /// The records of the chunks `tile` reads, with their state words (STRM-038).
    fn chunk_records(&self, tile: TileId) -> Vec<ReadRecord> {
        self.chunks_read_by(tile)
            .into_iter()
            .map(|k| ReadRecord::Chunk {
                body: self.base.body(),
                chunk: TileId::new(planet_core::Face(k.0), CHUNK_GRID_LEVEL, k.1, k.2).expect("a chunk of the grid is a tile"),
                state: self.chunk_state(k),
            })
            .collect()
    }
}

impl<F: HeightField> HeightField for ChunkField<F> {
    fn height(&self, dir: Vec3) -> f64 {
        self.height_at_level(dir, planet_core::tile::MAX_LEVEL)
    }
    fn height_at_level(&self, dir: Vec3, level: u8) -> f64 {
        self.base.height_at_level(dir, level) + self.change_at_level(dir, level)
    }
    fn level_aware(&self) -> bool {
        true
    }
    /// The tile's heights as drawn at its own level (GEN-036): the base's batch plus the change of the pyramid level the tile reads. The base is
    /// read as the field below the chunk stage reads it (level-independent: the global run and the layers under the chunks).
    fn heights_and_climate(
        &self,
        map: &dyn FaceMapping,
        id: TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, crate::climate::ClimateTexture) {
        let (mut heights, climate) = self.base.heights_and_climate(map, id, dirs, stop);
        for (h, &d) in heights.iter_mut().zip(dirs) {
            if stop() {
                break;
            }
            *h += self.change_at_level(d, id.level());
        }
        (heights, climate)
    }
    fn seed(&self) -> u64 {
        self.base.seed()
    }
    fn implementation_id(&self) -> &'static str {
        CHUNK_STAGE_ID
    }
    fn version(&self) -> u32 {
        CHUNK_STAGE_VERSION
    }
    fn max_abs_height_m(&self) -> f64 {
        self.base.max_abs_height_m() + self.bound_m
    }
    /// The stage's own definition (the process, the grid, the top level and its bound): a change re-keys every tile it reads.
    fn inputs_hash(&self) -> u64 {
        planet_core::hash::hash_u64s(
            0x0063_6866,
            &[self.definition, u64::from(self.lat.cells_per_chunk()), u64::from(self.top), self.bound_m.to_bits(), self.radius_m.to_bits()],
        )
    }
    fn read_records(&self) -> Vec<ReadRecord> {
        self.base.read_records()
    }
    fn read_records_for(&self, tile: TileId, map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let mut reads = self.base.read_records_for(tile, map);
        reads.extend(self.chunk_records(tile));
        reads
    }
    fn reads_cells(&self) -> bool {
        true
    }
    /// The base's hash for the tile, its version, implementation and bake key and the state of the chunks the tile reads (a chunk's state is the hash of its result, so another process gives another state once it has landed; while none has, the tile is the base)
    /// (STRM-038): the 64-bit in-memory keys chain the stage below as the 128-bit ones do.
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        let source = self
            .base
            .source_key()
            .map_or([0u64; 2], |k| [u64::from_le_bytes(k[..8].try_into().unwrap()), u64::from_le_bytes(k[8..].try_into().unwrap())]);
        let id: Vec<u64> = self.base.implementation_id().bytes().map(u64::from).collect();
        planet_core::hash::hash_u64s(
            0x0063_6873,
            &[
                self.base.inputs_hash_for(tile, map),
                u64::from(self.base.version()),
                planet_core::hash::hash_u64s(4, &id),
                source[0],
                source[1],
                planet_world_def::records_hash64(&self.chunk_records(tile)),
            ],
        )
    }
    fn body(&self) -> BodyId {
        self.base.body()
    }
    fn upstream(&self) -> Option<&dyn HeightField> {
        Some(&self.base)
    }
    fn stage_params(&self) -> Option<(&str, u32)> {
        Some(("chunk", 0))
    }
    fn item_revision(&self) -> u64 {
        self.base.item_revision()
    }
    /// A tile is the base's where the chunk detail cannot reach it: above the pyramid's top (STRM-029).
    fn passes_through(&self, tile: TileId, _map: &dyn FaceMapping) -> bool {
        tile.level() < self.top
    }
    fn surface_id(&self) -> &'static str {
        self.base.surface_id()
    }
    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> crate::climate::ClimateTexture {
        self.base.climate(map, id)
    }
    fn climate_version_word(&self) -> u64 {
        self.base.climate_version_word()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.base.height_bounds(centre, angular_radius);
        (lo - self.bound_m, hi + self.bound_m)
    }
    fn bounds_monotone(&self) -> bool {
        self.base.bounds_monotone()
    }
}

/// The chunk stage of a chain (GEN-039, `on-top`): the lattice and process of the stage, the pyramid's top level and the chunks ready now. A
/// chain wraps its base in it before the stamp stages (base, chunk, stamps); the ready set is a snapshot, so a field keeps what it was built from.
#[derive(Clone)]
pub struct ChunkLayer {
    pub lat: ChunkLattice,
    pub radius_m: f64,
    pub top: u8,
    pub stage: Arc<dyn ChunkStage>,
    pub ready: Vec<Arc<ChunkResult>>,
}

impl ChunkLayer {
    /// `base` under the chunk stage with the ready chunks.
    pub fn wrap<'a>(&self, base: Box<dyn HeightField + 'a>) -> Box<dyn HeightField + 'a> {
        let mut field = ChunkField::new(base, self.lat, self.radius_m, &*self.stage).with_top_level(self.top);
        for r in &self.ready {
            field = field.with_ready(r.clone());
        }
        Box::new(field)
    }
}

/// Which chunk of the grid holds `dir` (with the halo and cap the stage states), for tests and plan requests.
pub fn chunk_id_of(dir: Vec3, halo_rings: u16, lat: &ChunkLattice, radius_m: f64, stage: &dyn ChunkStage) -> Option<ChunkId> {
    ChunkId::of_direction(&TangentWarp, dir, f64::from(halo_rings) * lat.cell_m(radius_m), stage.step_cap())
}

/// The stand-in process (SPIKE-34): ridged relief of 800, 200 and 50 m wavelength added up to the amplitude, then `steps` relaxations (each
/// cell becomes the mean of itself and its eight neighbours, reaching one ring per step). It exists to test the chunk infrastructure; M8-3's
/// A-net implements the same [`ChunkStage`].
#[cfg(feature = "stress")]
pub struct StandIn {
    pub seed: u64,
    pub radius_m: f64,
    pub amplitude_m: f64,
    pub steps: u32,
}

#[cfg(feature = "stress")]
impl StandIn {
    fn relief(&self, dir: Vec3) -> f32 {
        let mut sum = 0.0;
        let mut weight = 0.0;
        for (o, (wavelength_m, w)) in [(800.0, 1.0), (200.0, 0.5), (50.0, 0.25)].into_iter().enumerate() {
            let n = crate::height::layer_noise(self.seed, 0x5ca1e, o as u32, dir * (self.radius_m / wavelength_m));
            sum += w * (1.0 - libm::fabs(n));
            weight += w;
        }
        (self.amplitude_m * (2.0 * sum / weight - 1.0)) as f32
    }
}

#[cfg(feature = "stress")]
impl ChunkStage for StandIn {
    fn step_cap(&self) -> u32 {
        self.steps
    }
    fn bound_m(&self) -> f64 {
        2.0 * self.amplitude_m
    }
    fn definition_hash(&self) -> u64 {
        planet_core::hash::hash_u64s(0x0073_7461, &[self.seed, self.radius_m.to_bits(), self.amplitude_m.to_bits(), u64::from(self.steps)])
    }
    fn compute(&self, lat: &ChunkLattice, region: &Region) -> Vec<f32> {
        let mut cur: Vec<f32> = region.cells.iter().map(|&c| self.relief(lat.direction(c))).collect();
        let mut next = cur.clone();
        assert!(u32::from(region.halo) >= self.steps, "a halo of {} rings is below the {} steps of the stand-in", region.halo, self.steps);
        for step in 1..=self.steps {
            // Cells within `halo - step` rings have their neighbours' previous values; the rest are left behind.
            let reach = region.halo - step as u16;
            let end = region.depth.partition_point(|&d| d <= reach);
            for c in 0..end {
                let mut sum = cur[c];
                for &nb in &region.neighbours[c] {
                    sum += cur[nb as usize];
                }
                next[c] = sum / 9.0;
            }
            core::mem::swap(&mut cur, &mut next);
        }
        cur
    }
}
