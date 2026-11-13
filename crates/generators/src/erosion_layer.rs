//! The erosion layer read at runtime (GEN-112 to GEN-114, GEN-102; m9-bake PR 6): the stored change of pass 2 against the base, as a C1 height term and as a range over a cap.
//!
//! The layer is a grid of tiles, each the change of its core's cells (`TileGrid`, the cell centres being the samples), held by a [`LayerSource`] that decodes a tile once while it stays
//! cached (the cache crate implements it over the store, CON-02). A decoded tile ([`LayerTile`]) carries a min/max pyramid built when it is decoded: blocks of 32 cells, then 2 x 2
//! blocks up to the whole tile. [`ErosionLayer::change_at`] reads a monotone C1 cubic of the cells around the point, one per axis (GEN-188), which stays within the range of the four cells that surround it,
//! and [`ErosionLayer::range_over_cap`] the range of the cells a cap's reads can touch, from the pyramids of the tiles the cap reaches.

use crate::bake::tile::{TileGrid, TileId as PassTile, TileResult};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::hash::hash_u64s;
use planet_core::{smoothstep, Face, Vec3};
use std::sync::Arc;

/// Version of the runtime read (the C1 read, the fade and the range): part of the keys of a field that reads a layer.
pub const LAYER_READ_VERSION: u32 = 2;
/// The height above the sea level, in metres, within which the layer's change is faded to 0 (the owner's coast remedy: a coastal plain keeps its sea level).
pub const COAST_FADE_M: f64 = 3.0;
/// Cells a side of a block of the pyramid's finest level.
pub const BLOCK_CELLS: usize = 32;
/// The smallest node, in cells, a range is taken from below the blocks (2 cells).
const MIN_NODE_CELLS: usize = 2;
/// The width, in cells, over which a face's change is cross-faded with the neighbouring face's at a cube edge (the layer holds no halo: at the edge each face reads its own border cell, and the
/// blend makes the two reads meet).
pub const EDGE_BLEND_CELLS: f64 = 2.0;
/// A cap whose angular radius is at most this many cells takes its range from the cells below the blocks; a larger one from the blocks (its rim would be tens of thousands of nodes).
const FINE_CAP_CELLS: f64 = 8.0;
/// A cap that reaches more tiles than this takes the layer's global range (a coarse render tile; the pyramids of a hundred tiles are not read for it).
pub const MAX_CAP_TILES: usize = 36;

/// A cell coordinate within this many cells of a cell centre is the centre (GEN-188): a direction made from a cell's indices reads its stored sample bit for bit.
pub const CENTRE_SNAP_CELLS: f64 = 1e-9;

/// `x` moved onto the nearest cell centre (an integer) when it lies within [`CENTRE_SNAP_CELLS`] of it.
fn snap(x: f64) -> f64 {
    let r = libm::round(x);
    if (x - r).abs() <= CENTRE_SNAP_CELLS {
        r
    } else {
        x
    }
}

/// The slope at a sample between the differences `a` (before it) and `b` (after it) (GEN-188): when they have the same sign, their harmonic mean `2ab / (a + b)` times
/// `2ab / (a² + b²)` (1 when `a = b`, so a uniform slope is kept; tending to 0 as either difference does), else 0. It lies between 0 and twice the smaller difference, so the cubic
/// below stays monotone between its two samples. It vanishes to second order where one difference changes sign, so it is C1 there: the cross pass, whose differences move with the
/// position along the row, has no crease. The harmonic mean alone (Fritsch-Butland) kinks where a difference changes sign (a slope jump of up to 20 m per cell on a noisy layer).
fn harmonic_slope(a: f64, b: f64) -> f64 {
    let ab = a * b;
    if ab > 0.0 {
        // `ab > 0` makes `a + b` non-zero and `a² + b² >= 2ab > 0`.
        (2.0 * ab / (a + b)) * (2.0 * ab / (a * a + b * b))
    } else {
        0.0
    }
}

/// The monotone cubic (GEN-188) through `p[1]` (at 0) and `p[2]` (at 1) on unit spacing, evaluated at `t` in `[0, 1]`: the Hermite cubic with the slopes [`harmonic_slope`] at both
/// samples. It never leaves `[min(p1, p2), max(p1, p2)]` (no overshoot, so no clamp), is exactly `p[1]` at 0 and `p[2]` at 1, and joins its neighbouring segment with the same value and
/// slope (C1), so a read made of it has no step and no kink at a cell-centre line.
pub fn monotone_cubic(p: [f64; 4], t: f64) -> f64 {
    let (m1, m2) = (harmonic_slope(p[1] - p[0], p[2] - p[1]), harmonic_slope(p[2] - p[1], p[3] - p[2]));
    let (t2, t3) = (t * t, t * t * t);
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * p[1] + h10 * m1 + h01 * p[2] + h11 * m2
}

/// The fade of the change at a point whose bake elevation is `e` over a sea at `sea_level_m`: 0 at the sea level and below, 1 from [`COAST_FADE_M`] above it, a smoothstep between.
pub fn coast_fade(e: f64, sea_level_m: f64) -> f64 {
    smoothstep(0.0, COAST_FADE_M, e - sea_level_m)
}

/// One decoded tile of the layer: its core's change in metres and the pyramid of its ranges.
pub struct LayerTile {
    result: Arc<TileResult>,
    cells: usize,
    block: usize,
    /// The ranges `[min, max]` per level, row-major: level 0 the blocks, each next level `2 x 2` of the level below (the last row and column of an odd side take what is left), up to one node.
    levels: Vec<(usize, Vec<[f32; 2]>)>,
}

impl LayerTile {
    /// The tile of `result` whose core holds `cells x cells` values: an error naming the tile when the core has another length or holds a value that is not finite.
    pub fn new(result: Arc<TileResult>, cells: usize) -> Result<LayerTile, String> {
        let id = result.id;
        let what = |why: String| format!("layer tile {}/{}/{}: {why}", id.face.0, id.ti, id.tj);
        if cells == 0 || result.core.len() != cells * cells {
            return Err(what(format!("a core of {} values is not {cells} x {cells}", result.core.len())));
        }
        if result.core.iter().any(|v| !v.is_finite()) {
            return Err(what("the change holds a value that is not finite".into()));
        }
        let block = BLOCK_CELLS.min(cells);
        let side = cells.div_ceil(block);
        let mut level0 = vec![[f32::INFINITY, f32::NEG_INFINITY]; side * side];
        for j in 0..cells {
            for i in 0..cells {
                let (v, b) = (result.core[j * cells + i], &mut level0[(j / block) * side + i / block]);
                b[0] = b[0].min(v);
                b[1] = b[1].max(v);
            }
        }
        let mut levels = vec![(side, level0)];
        while levels.last().expect("a level").0 > 1 {
            let (s, prev) = levels.last().expect("a level");
            let half = s.div_ceil(2);
            let mut next = vec![[f32::INFINITY, f32::NEG_INFINITY]; half * half];
            for y in 0..*s {
                for x in 0..*s {
                    let (a, b) = (prev[y * s + x], &mut next[(y / 2) * half + x / 2]);
                    b[0] = b[0].min(a[0]);
                    b[1] = b[1].max(a[1]);
                }
            }
            levels.push((half, next));
        }
        Ok(LayerTile { result, cells, block, levels })
    }

    /// The change at cell `(i, j)` of the core, in metres.
    pub fn cell(&self, i: usize, j: usize) -> f32 {
        self.result.core[j * self.cells + i]
    }

    /// The tile's result.
    pub fn result(&self) -> &TileResult {
        &self.result
    }

    /// The number of levels of the pyramid (the last holds one node).
    pub fn levels(&self) -> usize {
        self.levels.len()
    }

    /// The range of node `(x, y)` of `level` (level 0: the blocks of [`LayerTile::block_cells`] cells, each next level 2 x 2 of the one below).
    pub fn node_range(&self, level: usize, x: usize, y: usize) -> [f32; 2] {
        let (side, nodes) = &self.levels[level];
        nodes[y * side + x]
    }

    /// Cells a side of a block of level 0.
    pub fn block_cells(&self) -> usize {
        self.block
    }

    /// The range of the whole tile.
    pub fn range(&self) -> [f32; 2] {
        self.levels.last().expect("a level").1[0]
    }

    /// The range of the cells `[i0, i1) x [j0, j1)` of the core, from the cells themselves.
    fn cells_range(&self, i0: usize, j0: usize, i1: usize, j1: usize) -> [f32; 2] {
        let mut r = [f32::INFINITY, f32::NEG_INFINITY];
        for j in j0..j1.min(self.cells) {
            for i in i0..i1.min(self.cells) {
                let v = self.cell(i, j);
                r[0] = r[0].min(v);
                r[1] = r[1].max(v);
            }
        }
        r
    }
}

/// Where the tiles of a layer come from: a store with a cache of decoded tiles (the cache crate), or a synthetic layer in a test.
pub trait LayerSource: Send + Sync {
    /// The geometry of the layer's tiles.
    fn grid(&self) -> &TileGrid;
    /// The decoded tile `(face, ti, tj)`: `Ok(None)` when the layer holds none (a tile with no land: its change is 0), an error naming the tile when it cannot be read.
    fn tile(&self, face: Face, ti: u32, tj: u32) -> Result<Option<Arc<LayerTile>>, String>;
    /// The range `(lowest, highest)` of the change over the whole planet, which a cap too wide for the pyramids takes.
    fn global_range(&self) -> (f64, f64);
    /// An identity of the layer's content (its key in the store, or a hash of a synthetic layer): the keys of a field that reads it follow it.
    fn id(&self) -> u64;
}

/// The layer read at directions and over caps.
pub struct ErosionLayer {
    src: Arc<dyn LayerSource>,
    id: u64,
}

/// A tile of a stencil, looked up once: its index in the face and the tile (`None`: the layer holds none).
type HeldTile = ((u32, u32), Option<Arc<LayerTile>>);

fn merge(acc: &mut [f64; 2], r: [f32; 2]) {
    acc[0] = acc[0].min(f64::from(r[0]));
    acc[1] = acc[1].max(f64::from(r[1]));
}

impl ErosionLayer {
    /// The layer of `src`: an error naming the problem when its geometry does not tile the face or its range is not finite.
    pub fn new(src: Arc<dyn LayerSource>) -> Result<ErosionLayer, String> {
        let g = src.grid();
        g.check().map_err(|e| format!("the erosion layer's tiles: {e}"))?;
        let (lo, hi) = src.global_range();
        if !(lo.is_finite() && hi.is_finite() && lo <= 0.0 && hi >= 0.0) {
            return Err(format!("the erosion layer's global range [{lo}, {hi}] must be finite and contain 0"));
        }
        let id = hash_u64s(0x6c61_7972, &[src.id(), g.cells as u64, g.tile as u64, g.radius_m.to_bits(), u64::from(LAYER_READ_VERSION)]);
        Ok(ErosionLayer { src, id })
    }

    /// The identity of the layer as a field reads it.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The range of the change over the whole planet.
    pub fn global_range(&self) -> (f64, f64) {
        self.src.global_range()
    }

    /// The layer's change at the unit direction `d`: the monotone C1 cubic of the cells around the point, first along each row, then across the rows (GEN-188; the cell centres being the
    /// samples, read bit for bit there). It stays within the range of the four cells that surround it without a clamp (a bilinear read leaves a crease along every cell line; Catmull-Rom
    /// overshoots beside a sharp feature, and a clamp to the four cells steps where the four change, at every cell-centre line). A cell of a tile the
    /// layer does not hold is 0; the stencil reads the neighbouring tile's cells and clamps at the edge of the face. Within [`EDGE_BLEND_CELLS`] of a cube edge the face's read is
    /// cross-faded with the neighbouring face's read of the same direction (weights as the bake's `change_at`: the own face 1, a neighbour `1 - smoothstep` of the distance to the edge, the
    /// sum normalised), so the two faces meet at the edge and around a corner.
    pub fn change_at(&self, d: Vec3) -> Result<f64, String> {
        let n = self.src.grid().cells as f64;
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let own = self.read_face(face, d)?;
        let (x, y) = ((s + 1.0) * 0.5 * n, (t + 1.0) * 0.5 * n);
        let beyond = 1.0 + 2.0 / n;
        let neighbour = |so: f64, to: f64| direction_to_face(&TangentWarp, face_to_direction(&TangentWarp, face, so, to)).0;
        let weight = |dist: f64| {
            let u = (dist / EDGE_BLEND_CELLS).min(1.0);
            1.0 - u * u * (3.0 - 2.0 * u)
        };
        let (mut sum, mut total) = (own, 1.0);
        let (dx, dy) = (x.min(n - x), y.min(n - y));
        if dx < EDGE_BLEND_CELLS {
            let w = weight(dx);
            sum += w * self.read_face(neighbour(if x < n - x { -beyond } else { beyond }, t), d)?;
            total += w;
        }
        if dy < EDGE_BLEND_CELLS {
            let w = weight(dy);
            sum += w * self.read_face(neighbour(s, if y < n - y { -beyond } else { beyond }), d)?;
            total += w;
        }
        Ok(sum / total)
    }

    /// The change of one face read at a direction on the plane of that face (a direction of a neighbouring face reads at the plane's continuation, clamped to the face's cells).
    fn read_face(&self, face: Face, d: Vec3) -> Result<f64, String> {
        let g = self.src.grid();
        let w = d.dot(face.normal());
        if w <= 0.0 || !w.is_finite() {
            return Err(format!("direction {d:?} is not in front of face {}", face.0));
        }
        let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
        let (s, t) = (plane(d.dot(face.u_axis()) / w), plane(d.dot(face.v_axis()) / w));
        let n = g.cells;
        let top = (n - 1) as f64;
        let (x, y) = (snap(((s + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, top)), snap(((t + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, top)));
        let (i, j) = ((x as usize).min(n - 2), (y as usize).min(n - 2));
        let (fx, fy) = (x - i as f64, y - j as f64);
        // The tiles of the 4 x 4 stencil, each looked up once.
        let mut held: Vec<HeldTile> = Vec::with_capacity(4);
        let mut cell = |a: i64, b: i64| -> Result<f64, String> {
            let (a, b) = (a.clamp(0, n as i64 - 1) as usize, b.clamp(0, n as i64 - 1) as usize);
            let key = ((a / g.tile) as u32, (b / g.tile) as u32);
            let pos = match held.iter().position(|(k, _)| *k == key) {
                Some(p) => p,
                None => {
                    held.push((key, self.src.tile(face, key.0, key.1)?));
                    held.len() - 1
                }
            };
            Ok(held[pos].1.as_ref().map_or(0.0, |tile| f64::from(tile.cell(a % g.tile, b % g.tile))))
        };
        let (ii, jj) = (i as i64, j as i64);
        let mut rows = [0.0; 4];
        for (r, dj) in rows.iter_mut().zip(-1..=2) {
            let p = [cell(ii - 1, jj + dj)?, cell(ii, jj + dj)?, cell(ii + 1, jj + dj)?, cell(ii + 2, jj + dj)?];
            *r = monotone_cubic(p, fx);
        }
        Ok(monotone_cubic(rows, fy))
    }

    /// The range `(lowest, highest)` that holds the change at every direction within `angular_radius` of `centre` (GEN-102): the union of the ranges of the pyramid nodes (and of the
    /// cells below the blocks) that the cap, widened by two cells, reaches, on every face it touches. `None` when the cap reaches more than [`MAX_CAP_TILES`] tiles: the caller takes the
    /// global range. The nodes taken only grow with the radius, so the ranges of nested caps nest (LOD-012).
    pub fn range_over_cap(&self, centre: Vec3, angular_radius: f64) -> Result<Option<(f64, f64)>, String> {
        let g = self.src.grid();
        let cell_angle = core::f64::consts::FRAC_PI_2 / g.cells as f64;
        // Two cells for the stencil, and the blend's width for a point near a cube edge that also reads the neighbouring face's border cells.
        let margin = (EDGE_BLEND_CELLS + 2.0) * cell_angle;
        let fine = angular_radius <= FINE_CAP_CELLS * cell_angle;
        let radius = angular_radius * (1.0 + 1e-9) + 1e-12 + margin;
        let mut acc = [f64::INFINITY, f64::NEG_INFINITY];
        let mut visited = 0usize;
        let per_edge = g.tiles_per_edge();
        for f in 0..6u8 {
            if !self.face_reached(Face(f), centre, radius) {
                continue;
            }
            if !self.descend_tiles(Face(f), (0, 0, per_edge, per_edge), centre, (radius, fine), &mut acc, &mut visited)? {
                return Ok(None);
            }
        }
        if acc[0] > acc[1] {
            return Ok(Some((0.0, 0.0)));
        }
        Ok(Some((acc[0], acc[1])))
    }

    /// Whether a cap of `radius` around `centre` can reach face `face` (the face's corners are 54.74 degrees from its normal).
    fn face_reached(&self, face: Face, centre: Vec3, radius: f64) -> bool {
        centre.angle_to(face.normal()) <= 0.9553166181245093 + radius + 1e-9
    }

    fn dir(face: Face, s: f64, t: f64) -> Vec3 {
        face_to_direction(&TangentWarp, face, s.clamp(-1.0, 1.0), t.clamp(-1.0, 1.0))
    }

    /// The angular distance of the centre of the square `[s0, s1] x [t0, t1]` of `face` from `centre`, and the radius of the cap around that centre that holds the square.
    fn square_against(face: Face, (s0, t0, s1, t1): (f64, f64, f64, f64), centre: Vec3) -> (f64, f64) {
        let c = Self::dir(face, 0.5 * (s0 + s1), 0.5 * (t0 + t1));
        let corner_radius =
            [(s0, t0), (s1, t0), (s0, t1), (s1, t1)].iter().map(|&(s, t)| c.angle_to(Self::dir(face, s, t))).fold(0.0, f64::max)
                * (1.0 + 1e-9)
                + 1e-12;
        (c.angle_to(centre), corner_radius)
    }

    /// The tiles `[x0, x1) x [y0, y1)` of `face` against the cap, halved until one tile is left; false when the cap reaches too many tiles.
    #[allow(clippy::too_many_arguments)]
    fn descend_tiles(
        &self,
        face: Face,
        rect: (usize, usize, usize, usize),
        centre: Vec3,
        cap: (f64, bool),
        acc: &mut [f64; 2],
        visited: &mut usize,
    ) -> Result<bool, String> {
        let (radius, fine) = cap;
        let g = self.src.grid();
        let (x0, y0, x1, y1) = rect;
        let per_edge = g.tiles_per_edge() as f64;
        let to_face = |a: usize| -1.0 + 2.0 * a as f64 / per_edge;
        let (apart, corner_radius) = Self::square_against(face, (to_face(x0), to_face(y0), to_face(x1), to_face(y1)), centre);
        if apart > radius + corner_radius {
            return Ok(true);
        }
        if x1 - x0 == 1 && y1 - y0 == 1 {
            *visited += 1;
            if *visited > MAX_CAP_TILES {
                return Ok(false);
            }
            match self.src.tile(face, x0 as u32, y0 as u32)? {
                None => {
                    acc[0] = acc[0].min(0.0);
                    acc[1] = acc[1].max(0.0);
                }
                Some(tile) => {
                    let top = tile.levels.len() - 1;
                    self.descend_tile(face, (x0, y0), &tile, top, (0, 0), centre, (radius, fine), acc);
                }
            }
            return Ok(true);
        }
        let (mx, my) = (x0 + (x1 - x0).div_ceil(2), y0 + (y1 - y0).div_ceil(2));
        let xs = if x1 - x0 == 1 { vec![(x0, x1)] } else { vec![(x0, mx), (mx, x1)] };
        let ys = if y1 - y0 == 1 { vec![(y0, y1)] } else { vec![(y0, my), (my, y1)] };
        for &(a, b) in &xs {
            for &(c, d) in &ys {
                if !self.descend_tiles(face, (a, c, b, d), centre, cap, acc, visited)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// The node `(x, y)` of level `level` of `tile` against the cap: skipped when apart, taken whole when inside, split down to the blocks and then to nodes of [`MIN_NODE_CELLS`] cells.
    #[allow(clippy::too_many_arguments)]
    fn descend_tile(
        &self,
        face: Face,
        origin: (usize, usize),
        tile: &LayerTile,
        level: usize,
        node: (usize, usize),
        centre: Vec3,
        cap: (f64, bool),
        acc: &mut [f64; 2],
    ) {
        let (radius, fine) = cap;
        let g = self.src.grid();
        let size = tile.block << level;
        let (c0, r0) = (node.0 * size, node.1 * size);
        let (c1, r1) = ((c0 + size).min(tile.cells), (r0 + size).min(tile.cells));
        let n = g.cells as f64;
        let (oi, oj) = (origin.0 * g.tile, origin.1 * g.tile);
        let to_face = |a: usize| -1.0 + 2.0 * a as f64 / n;
        let (apart, corner_radius) =
            Self::square_against(face, (to_face(oi + c0), to_face(oj + r0), to_face(oi + c1), to_face(oj + r1)), centre);
        if apart > radius + corner_radius {
            return;
        }
        if apart + corner_radius <= radius {
            merge(acc, tile.levels[level].1[node.1 * tile.levels[level].0 + node.0]);
            return;
        }
        if level == 0 && !fine {
            // A cap of more than a few cells takes the whole block it reaches: its rim would be tens of thousands of small nodes (a coarser range, nested all the same).
            merge(acc, tile.levels[0].1[node.1 * tile.levels[0].0 + node.0]);
            return;
        }
        if level == 0 {
            self.descend_cells(face, origin, tile, (c0, r0, c1, r1), centre, radius, acc);
            return;
        }
        let below = tile.levels[level - 1].0;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (x, y) = (2 * node.0 + dx, 2 * node.1 + dy);
            if x < below && y < below {
                self.descend_tile(face, origin, tile, level - 1, (x, y), centre, cap, acc);
            }
        }
    }

    /// The cells `[c0, c1) x [r0, r1)` of `tile` against the cap, halved down to [`MIN_NODE_CELLS`]: the range of the cells of a node it reaches.
    #[allow(clippy::too_many_arguments)]
    fn descend_cells(
        &self,
        face: Face,
        origin: (usize, usize),
        tile: &LayerTile,
        rect: (usize, usize, usize, usize),
        centre: Vec3,
        radius: f64,
        acc: &mut [f64; 2],
    ) {
        let g = self.src.grid();
        let (c0, r0, c1, r1) = rect;
        let n = g.cells as f64;
        let (oi, oj) = (origin.0 * g.tile, origin.1 * g.tile);
        let to_face = |a: usize| -1.0 + 2.0 * a as f64 / n;
        let (apart, corner_radius) =
            Self::square_against(face, (to_face(oi + c0), to_face(oj + r0), to_face(oi + c1), to_face(oj + r1)), centre);
        if apart > radius + corner_radius {
            return;
        }
        if apart + corner_radius <= radius || (c1 - c0 <= MIN_NODE_CELLS && r1 - r0 <= MIN_NODE_CELLS) {
            merge(acc, tile.cells_range(c0, r0, c1, r1));
            return;
        }
        let (mc, mr) = (c0 + (c1 - c0).div_ceil(2), r0 + (r1 - r0).div_ceil(2));
        let cols = if c1 - c0 <= 1 { vec![(c0, c1)] } else { vec![(c0, mc), (mc, c1)] };
        let rows = if r1 - r0 <= 1 { vec![(r0, r1)] } else { vec![(r0, mr), (mr, r1)] };
        for &(a, b) in &cols {
            for &(c, d) in &rows {
                self.descend_cells(face, origin, tile, (a, c, b, d), centre, radius, acc);
            }
        }
    }
}

/// The tile of a layer as the generators name it, for sources that map a record to a tile.
pub type LayerTileId = PassTile;
