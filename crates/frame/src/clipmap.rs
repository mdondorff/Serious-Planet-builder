//! The height clipmap plan (REND-046, ADR 0085 points 2 and 3): GPU-free, f64 on the CPU, tile-local f32 to the shaders (CON-03, CON-11).
//!
//! `CLIPMAP_LEVELS` nested square levels of `texels`^2 texels lie around an anchor (the camera's ground point, in the azimuthal
//! equidistant projection of its tangent plane). A level's texel is twice the last's; each level's centre snaps to its own texel grid and
//! is addressed toroidally by the gather (a texel's slot is its world texel index modulo `texels`). All rectangles are in world texel
//! indices from the anchor: texel `(i, j)` covers the plane square `[i, i+1) x [j, j+1)` times the level's texel size.
//!
//! Per level the plan lists the resident tiles that cover it, the finest per region (a tile finer than the level's texel is replaced by its
//! resident ancestor), as blocks in coarse-to-fine order: the gather writes them in that order and keeps a texel only where the block's
//! `(u, v)` lies inside its tile, so a finer block overwrites a coarser one. A block is a rectangle of a tile's footprint on which an affine
//! map from texels to the tile's `(u, v)` is within [`AFFINE_TOLERANCE_TEXELS`] of the f64 projection and a quadratic fits the sphere's
//! drop within [`drop_tolerance_m`]; a tile whose footprint is too curved or too large for one map is split into several blocks.

use crate::preview::PreviewStamp;
use planet_core::cube::FaceMapping;
use planet_core::{PlanetFixed, TileId, Vec3};
use std::sync::Arc;

/// Levels of the clipmap (1 m to 1,024 m texels; 1 km to 1,048 km squares).
pub const CLIPMAP_LEVELS: usize = 11;
/// Texels per level edge.
pub const CLIPMAP_TEXELS: u32 = 1024;
/// The smallest texel of level 0, metres.
pub const CLIPMAP_FINEST_TEXEL_M: f64 = 1.0;
/// The most an affine block map may be off the f64 projection, in texels of its level (checked on a 5x5 grid including the corners and the centre).
pub const AFFINE_TOLERANCE_TEXELS: f64 = 0.125;
/// The drop fit's tolerance floor, metres.
pub const DROP_TOLERANCE_FLOOR_M: f64 = 1.0e-3;
/// The smallest block edge, texels: below it a block is kept whatever its error (the error is then reported by [`BlockMap::fit_error`]).
const MIN_BLOCK_TEXELS: i64 = 4;

/// The drop fit's tolerance at a level with the given texel: 1 mm up to a 1 m texel, then a thousandth of the texel (the shadow reads the
/// coarse levels at texel precision; fitting a kilometre texel to a millimetre would split every tile into thousands of blocks).
pub fn drop_tolerance_m(texel_m: f64) -> f64 {
    DROP_TOLERANCE_FLOOR_M.max(texel_m / 1024.0)
}

#[derive(Clone, Copy, Debug)]
pub struct ClipmapParams {
    pub radius_m: f64,
    /// Cells per tile edge (the sample spacing of a tile is its edge over this).
    pub tile_cells: u32,
    /// Texels per level edge; tests use 256.
    pub texels: u32,
    pub levels: usize,
}

impl ClipmapParams {
    pub fn new(radius_m: f64, tile_cells: u32) -> ClipmapParams {
        ClipmapParams { radius_m, tile_cells, texels: CLIPMAP_TEXELS, levels: CLIPMAP_LEVELS }
    }

    /// The sample spacing of a tile of `level` at the nominal quarter-turn face, metres.
    pub fn tile_spacing_m(&self, level: u8) -> f64 {
        self.radius_m * core::f64::consts::FRAC_PI_2 / (f64::from(1u32 << level.min(31)) * f64::from(self.tile_cells))
    }

    /// The deepest tile level whose sample spacing is at least `texel_m`.
    pub fn deepest_level_for(&self, texel_m: f64) -> u8 {
        let ratio = self.radius_m * core::f64::consts::FRAC_PI_2 / (f64::from(self.tile_cells) * texel_m);
        libm::floor(libm::log2(ratio.max(1.0))).clamp(0.0, f64::from(planet_core::tile::MAX_LEVEL)) as u8
    }
}

/// A half-open rectangle of world texel indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TexelRect {
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
}

impl TexelRect {
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
    pub fn width(&self) -> i64 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> i64 {
        self.y1 - self.y0
    }
    pub fn area(&self) -> i64 {
        if self.is_empty() {
            0
        } else {
            self.width() * self.height()
        }
    }
    pub fn intersect(&self, o: &TexelRect) -> TexelRect {
        TexelRect { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }
    pub fn contains(&self, i: i64, j: i64) -> bool {
        (self.x0..self.x1).contains(&i) && (self.y0..self.y1).contains(&j)
    }

    /// `self` without `old`, as at most four disjoint rectangles.
    pub fn minus(&self, old: &TexelRect) -> Vec<TexelRect> {
        let cut = self.intersect(old);
        if cut.is_empty() {
            return vec![*self];
        }
        [
            TexelRect { x0: self.x0, y0: self.y0, x1: cut.x0, y1: self.y1 },
            TexelRect { x0: cut.x1, y0: self.y0, x1: self.x1, y1: self.y1 },
            TexelRect { x0: cut.x0, y0: self.y0, x1: cut.x1, y1: cut.y0 },
            TexelRect { x0: cut.x0, y0: cut.y1, x1: cut.x1, y1: self.y1 },
        ]
        .into_iter()
        .filter(|r| !r.is_empty())
        .collect()
    }
}

/// One block of a tile on a level: the texels of `rect` read the tile at `uv_at`, minus `drop_at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockMap {
    pub tile: TileId,
    pub rect: TexelRect,
    /// `u = u[0] + u[1] (i - rect.x0) + u[2] (j - rect.y0)` at texel `(i, j)`'s centre; likewise `v`.
    pub u: [f32; 3],
    pub v: [f32; 3],
    /// `c0 + c1 x + c2 y + c3 (x^2 + y^2)`, the sphere's drop at the texel's centre, `(x, y)` in metres from the block's centre.
    pub drop: [f32; 4],
    /// The largest deviation of the affine map from the f64 projection on the check grid, in texels.
    pub fit_error_texels: f32,
    /// The largest deviation of the drop quadratic from the f64 sphere on the check grid, metres.
    pub drop_error_m: f32,
}

impl BlockMap {
    pub fn uv_at(&self, i: i64, j: i64) -> (f64, f64) {
        let (a, b) = ((i - self.rect.x0) as f64, (j - self.rect.y0) as f64);
        (
            f64::from(self.u[0]) + f64::from(self.u[1]) * a + f64::from(self.u[2]) * b,
            f64::from(self.v[0]) + f64::from(self.v[1]) * a + f64::from(self.v[2]) * b,
        )
    }

    /// Whether texel `(i, j)` takes this block's tile: its `(u, v)` lies inside the tile expanded by a quarter texel (twice the map's tolerance),
    /// so a texel on a seam between two tiles is taken by both rather than by neither; the gather clamps `(u, v)` to the tile.
    pub fn covers(&self, i: i64, j: i64) -> bool {
        let (u, v) = self.uv_at(i, j);
        let m = 0.25 * f64::from((self.u[1].abs() + self.u[2].abs()).max(self.v[1].abs() + self.v[2].abs()));
        (-m..=1.0 + m).contains(&u) && (-m..=1.0 + m).contains(&v)
    }

    pub fn drop_at(&self, i: i64, j: i64, texel_m: f64) -> f64 {
        let x = (i as f64 + 0.5 - 0.5 * (self.rect.x0 + self.rect.x1) as f64) * texel_m;
        let y = (j as f64 + 0.5 - 0.5 * (self.rect.y0 + self.rect.y1) as f64) * texel_m;
        let c = self.drop.map(f64::from);
        c[0] + c[1] * x + c[2] * y + c[3] * (x * x + y * y)
    }

    pub fn fit_error(&self) -> f32 {
        self.fit_error_texels
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LevelPlan {
    pub texel_m: f64,
    pub window: TexelRect,
    /// Coarse to fine.
    pub blocks: Arc<Vec<BlockMap>>,
    /// Rectangles to rewrite this frame, clipped to the window.
    pub dirty: Vec<TexelRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipmapPlan {
    /// The unit direction of the anchor; every level's rectangles are in its tangent frame.
    pub anchor: Vec3,
    /// Bumps with each re-anchor: a new generation means every level is dirty and all history is void.
    pub generation: u64,
    pub reanchored: bool,
    pub levels: Vec<LevelPlan>,
}

/// The anchor's tangent frame and the azimuthal equidistant projection around it.
#[derive(Clone, Copy, Debug)]
struct Frame {
    dir: Vec3,
    e1: Vec3,
    e2: Vec3,
    radius: f64,
}

impl Frame {
    fn new(dir: Vec3, radius: f64) -> Frame {
        let (e1, e2) = dir.tangent_basis();
        Frame { dir, e1, e2, radius }
    }

    fn to_plane(&self, d: Vec3) -> (f64, f64) {
        let (x, y) = (d.dot(self.e1), d.dot(self.e2));
        let s = libm::hypot(x, y);
        if s == 0.0 {
            // The anchor itself, or its antipode (a half turn away: far outside any window).
            return if d.dot(self.dir) < 0.0 { (core::f64::consts::PI * self.radius, 0.0) } else { (0.0, 0.0) };
        }
        let k = self.radius * libm::atan2(s, d.dot(self.dir)) / s;
        (x * k, y * k)
    }

    fn from_plane(&self, x: f64, y: f64) -> Vec3 {
        let rho = libm::hypot(x, y);
        if rho == 0.0 {
            return self.dir;
        }
        let th = rho / self.radius;
        (self.dir * libm::cos(th) + (self.e1 * x + self.e2 * y) * (libm::sin(th) / rho)).normalized()
    }

    fn drop_m(&self, x: f64, y: f64) -> f64 {
        let th = libm::hypot(x, y) / self.radius;
        // 2 R sin^2(theta / 2): exact and free of the cancellation in R (1 - cos theta).
        let s = libm::sin(0.5 * th);
        2.0 * self.radius * s * s
    }
}

/// The face coordinate of plane coordinate `u`, continued linearly beyond the face edge with the warp's slope there (the warp itself clamps
/// at the edge, which would flatten the fit of a block that crosses it; texels beyond the edge are read from the neighbouring face's tiles).
fn plane_to_face_ext(map: &dyn FaceMapping, u: f64) -> f64 {
    if u.abs() <= 1.0 {
        return map.plane_to_face(u);
    }
    const H: f64 = 1.0e-3;
    let slope = (1.0 - map.plane_to_face(1.0 - H)) / H;
    u.signum() * (1.0 + (u.abs() - 1.0) * slope)
}

/// The tile's `(u, v)` of a unit direction; beyond the tile's face edge the coordinates continue smoothly (see [`plane_to_face_ext`]).
pub fn tile_uv(map: &dyn FaceMapping, tile: TileId, d: Vec3) -> (f64, f64) {
    let f = tile.face();
    let dn = d.dot(f.normal());
    if dn <= 1.0e-9 {
        return (1.0e9, 1.0e9);
    }
    let s = plane_to_face_ext(map, d.dot(f.u_axis()) / dn);
    let t = plane_to_face_ext(map, d.dot(f.v_axis()) / dn);
    let (s0, s1, t0, t1) = tile.bounds();
    ((s - s0) / (s1 - s0), (t - t0) / (t1 - t0))
}

struct State {
    frame: Frame,
    generation: u64,
    texel0: f64,
    windows: Vec<TexelRect>,
    resident: Vec<TileId>,
    blocks: Vec<Arc<Vec<BlockMap>>>,
    stamps: Vec<(Vec3, f64)>,
}

pub struct ClipmapPlanner {
    params: ClipmapParams,
    state: Option<State>,
}

impl ClipmapPlanner {
    pub fn new(params: ClipmapParams) -> ClipmapPlanner {
        ClipmapPlanner { params, state: None }
    }

    pub fn params(&self) -> &ClipmapParams {
        &self.params
    }

    /// The plan of a frame. `resident` is the resident tiles of R in ascending tile-id order; `refreshed` the resident tiles whose
    /// content changed this frame (an edit's refresh) though they stayed in the set.
    pub fn plan(
        &mut self,
        camera: PlanetFixed,
        resident: &[TileId],
        refreshed: &[TileId],
        stamps: &[PreviewStamp],
        map: &dyn FaceMapping,
    ) -> ClipmapPlan {
        let p = self.params;
        let cam = camera.0.normalized();
        let texel0 = CLIPMAP_FINEST_TEXEL_M.max(finest_spacing_under(&p, resident, map, cam));
        let half = i64::from(p.texels / 2);
        let texel_of = |k: usize| texel0 * f64::from(1u32 << k);

        let (frame, generation, reanchored) = match &self.state {
            Some(s)
                if s.texel0 == texel0 && {
                    let (x, y) = s.frame.to_plane(cam);
                    x.abs().max(y.abs()) <= 0.25 * f64::from(p.texels) * texel0
                } =>
            {
                (s.frame, s.generation, false)
            }
            Some(s) => (Frame::new(cam, p.radius_m), s.generation + 1, true),
            None => (Frame::new(cam, p.radius_m), 1, true),
        };
        let (cx, cy) = frame.to_plane(cam);
        let windows: Vec<TexelRect> = (0..p.levels)
            .map(|k| {
                let (ix, iy) = (libm::round(cx / texel_of(k)) as i64, libm::round(cy / texel_of(k)) as i64);
                TexelRect { x0: ix - half, y0: iy - half, x1: ix + half, y1: iy + half }
            })
            .collect();

        let prev = if reanchored { None } else { self.state.as_ref() };
        let mut changed: Vec<TileId> = refreshed.to_vec();
        if let Some(s) = prev {
            changed.extend(symmetric_difference(&s.resident, resident));
        }

        let mut levels = Vec::with_capacity(p.levels);
        let mut all_blocks = Vec::with_capacity(p.levels);
        for k in 0..p.levels {
            let texel_m = texel_of(k);
            let window = windows[k];
            let mut dirty: Vec<TexelRect> = match prev {
                None => vec![window],
                Some(s) => window.minus(&s.windows[k]),
            };
            let deepest = p.deepest_level_for(texel_m);
            let mut rebuild = prev.is_none_or(|s| s.windows[k] != window);
            if prev.is_some() {
                for t in changed.iter().filter(|t| t.level() <= deepest) {
                    if let Some(r) = tile_rect(&frame, map, *t, texel_m).map(|r| r.intersect(&window)).filter(|r| !r.is_empty()) {
                        dirty.push(r);
                        rebuild = true;
                    }
                }
            }
            let blocks = match prev {
                Some(s) if !rebuild => Arc::clone(&s.blocks[k]),
                _ => Arc::new(self.level_blocks(&frame, map, resident, k, texel_m, window)),
            };
            let old_stamps = prev.map_or(&[][..], |s| &s.stamps[..]);
            for (dir, reach) in stamps.iter().map(|s| (s.dir, s.reach_m)).chain(old_stamps.iter().copied()) {
                if dir.dot(frame.dir) <= 0.0 {
                    continue;
                }
                let (x, y) = frame.to_plane(dir);
                let r = TexelRect {
                    x0: libm::floor((x - reach) / texel_m) as i64 - 1,
                    y0: libm::floor((y - reach) / texel_m) as i64 - 1,
                    x1: libm::ceil((x + reach) / texel_m) as i64 + 1,
                    y1: libm::ceil((y + reach) / texel_m) as i64 + 1,
                }
                .intersect(&window);
                if !r.is_empty() {
                    dirty.push(r);
                }
            }
            dirty.sort_by_key(|r| (r.y0, r.x0, r.y1, r.x1));
            dirty.dedup();
            all_blocks.push(Arc::clone(&blocks));
            levels.push(LevelPlan { texel_m, window, blocks, dirty });
        }

        self.state = Some(State {
            frame,
            generation,
            texel0,
            windows,
            resident: resident.to_vec(),
            blocks: all_blocks,
            stamps: stamps.iter().map(|s| (s.dir, s.reach_m)).collect(),
        });
        ClipmapPlan { anchor: frame.dir, generation, reanchored, levels }
    }

    /// The blocks of level `k`: for every region the finest resident tile no finer than the level's texel.
    fn level_blocks(
        &self,
        frame: &Frame,
        map: &dyn FaceMapping,
        resident: &[TileId],
        k: usize,
        texel_m: f64,
        window: TexelRect,
    ) -> Vec<BlockMap> {
        let p = &self.params;
        let deepest = p.deepest_level_for(texel_m);
        // The farthest corner of the window from the anchor (the window's centre is up to a quarter of level 0's extent off it).
        let half_diag_m = libm::hypot(window.x0.abs().max(window.x1.abs()) as f64, window.y0.abs().max(window.y1.abs()) as f64) * texel_m;
        let mut out: Vec<BlockMap> = Vec::new();
        for &tile in resident {
            let l = tile.level();
            if l > deepest {
                continue;
            }
            // A tile whose four children are resident, and no finer than the level, is covered by them. R is not a full quadtree (the
            // horizon cut drops children), so a tile with only some of its children stays as the backdrop of the others.
            if l < deepest && tile.children().is_some_and(|c| c.iter().all(|ch| resident.binary_search(ch).is_ok())) {
                continue;
            }
            // Cheap reject by the angle between the tile's centre and the anchor, before any projection.
            let tile_r_m = p.radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << l.min(31));
            let reach = (half_diag_m + tile_r_m) / p.radius_m;
            if reach < core::f64::consts::PI && tile.center_direction(map).dot(frame.dir) < libm::cos(reach) {
                continue;
            }
            let Some(rect) = tile_rect(frame, map, tile, texel_m).map(|r| r.intersect(&window)).filter(|r| !r.is_empty()) else { continue };
            split_blocks(frame, map, tile, rect, texel_m, &mut out);
        }
        let _ = k;
        out.sort_by_key(|b| (b.tile.level(), b.tile, b.rect.y0, b.rect.x0));
        out
    }
}

/// The sample spacing of the finest resident tile under the camera (0 if none).
fn finest_spacing_under(p: &ClipmapParams, resident: &[TileId], map: &dyn FaceMapping, dir: Vec3) -> f64 {
    for level in (0..=planet_core::tile::MAX_LEVEL).rev() {
        if resident.binary_search(&TileId::from_direction(map, dir, level)).is_ok() {
            return p.tile_spacing_m(level);
        }
    }
    0.0
}

fn symmetric_difference(a: &[TileId], b: &[TileId]) -> Vec<TileId> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        match (a.get(i), b.get(j)) {
            (Some(x), Some(y)) if x == y => {
                i += 1;
                j += 1;
            }
            (Some(x), Some(y)) if x < y => {
                out.push(*x);
                i += 1;
            }
            (Some(_), Some(y)) => {
                out.push(*y);
                j += 1;
            }
            (Some(x), None) => {
                out.push(*x);
                i += 1;
            }
            (None, Some(y)) => {
                out.push(*y);
                j += 1;
            }
            (None, None) => unreachable!(),
        }
    }
    out
}

/// The texel rectangle (world indices) bounding a tile's footprint, from a 5x5 grid of its samples; none if the tile lies behind the anchor's horizon.
fn tile_rect(frame: &Frame, map: &dyn FaceMapping, tile: TileId, texel_m: f64) -> Option<TexelRect> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut any = false;
    for l in 0..=4 {
        for k in 0..=4 {
            let d = tile.sample_direction(map, 4, k, l);
            if d.dot(frame.dir) <= 0.0 {
                continue;
            }
            any = true;
            let (x, y) = frame.to_plane(d);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    any.then(|| TexelRect {
        x0: libm::floor(x0 / texel_m) as i64 - 1,
        y0: libm::floor(y0 / texel_m) as i64 - 1,
        x1: libm::ceil(x1 / texel_m) as i64 + 1,
        y1: libm::ceil(y1 / texel_m) as i64 + 1,
    })
}

/// Fit one block; split it into quadrants while the fit is out of tolerance and the block is larger than the minimum.
fn split_blocks(frame: &Frame, map: &dyn FaceMapping, tile: TileId, rect: TexelRect, texel_m: f64, out: &mut Vec<BlockMap>) {
    let Some(b) = fit_block(frame, map, tile, rect, texel_m) else { return };
    let ok = f64::from(b.fit_error_texels) <= AFFINE_TOLERANCE_TEXELS && f64::from(b.drop_error_m) <= drop_tolerance_m(texel_m);
    if ok || (rect.width() <= MIN_BLOCK_TEXELS && rect.height() <= MIN_BLOCK_TEXELS) {
        out.push(b);
        return;
    }
    let (mx, my) = ((rect.x0 + rect.x1) / 2, (rect.y0 + rect.y1) / 2);
    let xs = if rect.width() > MIN_BLOCK_TEXELS { vec![(rect.x0, mx), (mx, rect.x1)] } else { vec![(rect.x0, rect.x1)] };
    let ys = if rect.height() > MIN_BLOCK_TEXELS { vec![(rect.y0, my), (my, rect.y1)] } else { vec![(rect.y0, rect.y1)] };
    for &(y0, y1) in &ys {
        for &(x0, x1) in &xs {
            split_blocks(frame, map, tile, TexelRect { x0, y0, x1, y1 }, texel_m, out);
        }
    }
}

/// The affine map and the drop quadratic of one block, fitted by least squares on a 3x3 and a 5x5 grid and checked on the 5x5 grid; none
/// when the block lies wholly outside its tile (beyond a margin of the fit's own texel step).
fn fit_block(frame: &Frame, map: &dyn FaceMapping, tile: TileId, rect: TexelRect, texel_m: f64) -> Option<BlockMap> {
    let (w, h) = (rect.width(), rect.height());
    let (hx, hy) = (0.5 * (w - 1) as f64, 0.5 * (h - 1) as f64);
    let at = |fa: f64, fb: f64| -> (f64, f64, f64, f64) {
        // `fa`, `fb` in [-1, 1] across the block's texel centres.
        let (a, b) = (hx * (1.0 + fa), hy * (1.0 + fb));
        let (px, py) = ((rect.x0 as f64 + a + 0.5) * texel_m, (rect.y0 as f64 + b + 0.5) * texel_m);
        let d = frame.from_plane(px, py);
        let (u, v) = tile_uv(map, tile, d);
        (u, v, px, py)
    };

    // Affine fit on the 3x3 grid (a regular grid: closed form).
    let (mut su, mut sv, mut sau, mut sbu, mut sav, mut sbv) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for fb in [-1.0, 0.0, 1.0] {
        for fa in [-1.0, 0.0, 1.0] {
            let (u, v, _, _) = at(fa, fb);
            su += u;
            sv += v;
            sau += fa * u;
            sav += fa * v;
            sbu += fb * u;
            sbv += fb * v;
        }
    }
    // Per unit of `fa`: sum(fa^2) = 6 over the nine points.
    let (gu, gv) = ((sau / 6.0, sbu / 6.0), (sav / 6.0, sbv / 6.0));
    let per = |g: f64, half: f64| if half > 0.0 { g / half } else { 0.0 };
    let u1 = [per(gu.0, hx), per(gu.1, hy)];
    let v1 = [per(gv.0, hx), per(gv.1, hy)];
    let u0 = su / 9.0 - u1[0] * hx - u1[1] * hy;
    let v0 = sv / 9.0 - v1[0] * hx - v1[1] * hy;

    // The drop quadratic about the block's centre, least squares on the 5x5 grid.
    let (ccx, ccy) = ((rect.x0 + rect.x1) as f64 * 0.5 * texel_m, (rect.y0 + rect.y1) as f64 * 0.5 * texel_m);
    let grid: Vec<f64> = (0..=8).map(|i| -1.0 + 0.25 * f64::from(i)).collect();
    let mut pts: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(81); // (u, v, a, b, x, y)
    let mut drops: Vec<f64> = Vec::with_capacity(81);
    for &fb in &grid {
        for &fa in &grid {
            let (u, v, px, py) = at(fa, fb);
            pts.push((u, v, hx * (1.0 + fa), hy * (1.0 + fb), px - ccx, py - ccy));
            drops.push(frame.drop_m(px, py));
        }
    }
    let sc = (0.5 * texel_m * (w.max(h)) as f64).max(1.0);
    let mut ata = [[0.0f64; 4]; 4];
    let mut atb = [0.0f64; 4];
    for (pt, &dr) in pts.iter().zip(&drops) {
        let (xn, yn) = (pt.4 / sc, pt.5 / sc);
        let basis = [1.0, xn, yn, xn * xn + yn * yn];
        for r in 0..4 {
            for c in 0..4 {
                ata[r][c] += basis[r] * basis[c];
            }
            atb[r] += basis[r] * dr;
        }
    }
    let cn = solve4(ata, atb);
    let coef = [cn[0], cn[1] / sc, cn[2] / sc, cn[3] / (sc * sc)];

    // Reject a block wholly outside its tile: every check point beyond the same edge by more than the map's own texel step.
    let m = 1.5 * (u1[0].abs() + u1[1].abs()).max(v1[0].abs() + v1[1].abs()).max(1.0e-12);
    let all = |f: &dyn Fn(&(f64, f64, f64, f64, f64, f64)) -> bool| pts.iter().all(f);
    if all(&|p| p.0 < -m) || all(&|p| p.0 > 1.0 + m) || all(&|p| p.1 < -m) || all(&|p| p.1 > 1.0 + m) {
        return None;
    }

    // The errors of the f32 values the plan hands over, against the f64 projection and sphere.
    let blk = BlockMap {
        tile,
        rect,
        u: [u0 as f32, u1[0] as f32, u1[1] as f32],
        v: [v0 as f32, v1[0] as f32, v1[1] as f32],
        drop: [coef[0] as f32, coef[1] as f32, coef[2] as f32, coef[3] as f32],
        fit_error_texels: 0.0,
        drop_error_m: 0.0,
    };
    let det = f64::from(blk.u[1]) * f64::from(blk.v[2]) - f64::from(blk.u[2]) * f64::from(blk.v[1]);
    let (mut fit_err, mut drop_err) = (0.0f64, 0.0f64);
    for (pt, &dr) in pts.iter().zip(&drops) {
        let (pu, pv) = (
            f64::from(blk.u[0]) + f64::from(blk.u[1]) * pt.2 + f64::from(blk.u[2]) * pt.3,
            f64::from(blk.v[0]) + f64::from(blk.v[1]) * pt.2 + f64::from(blk.v[2]) * pt.3,
        );
        let (du, dv) = (pt.0 - pu, pt.1 - pv);
        let (g_u, g_v) = ([f64::from(blk.u[1]), f64::from(blk.u[2])], [f64::from(blk.v[1]), f64::from(blk.v[2])]);
        let texels = if w == 1 && h == 1 {
            0.0
        } else if w == 1 || h == 1 {
            // One row or column: the map varies along one axis only, and the error is measured along it.
            let ax = usize::from(w == 1);
            let g2 = g_u[ax] * g_u[ax] + g_v[ax] * g_v[ax];
            if g2 < 1.0e-30 {
                f64::INFINITY
            } else {
                ((du * g_u[ax] + dv * g_v[ax]) / g2).abs()
            }
        } else if det.abs() < 1.0e-30 {
            f64::INFINITY
        } else {
            let ti = (du * g_v[1] - dv * g_u[1]) / det;
            let tj = (dv * g_u[0] - du * g_v[0]) / det;
            ti.abs().max(tj.abs())
        };
        // Points beyond the tile's face continue on the extended plane and are valid; only a vanishing `dn` yields the sentinel.
        fit_err = fit_err.max(if pt.0.abs() > 1.0e8 { 0.0 } else { texels });
        let c = blk.drop.map(f64::from);
        let fit = c[0] + c[1] * pt.4 + c[2] * pt.5 + c[3] * (pt.4 * pt.4 + pt.5 * pt.5);
        drop_err = drop_err.max((fit - dr).abs());
    }
    Some(BlockMap { fit_error_texels: fit_err as f32, drop_error_m: drop_err as f32, ..blk })
}

/// Gaussian elimination with partial pivoting on a 4x4 system.
fn solve4(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> [f64; 4] {
    for col in 0..4 {
        let piv = (col..4).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs())).unwrap_or(col);
        a.swap(col, piv);
        b.swap(col, piv);
        let d = a[col][col];
        if d.abs() < 1.0e-300 {
            continue;
        }
        for r in col + 1..4 {
            let f = a[r][col] / d;
            for c in col..4 {
                a[r][c] -= f * a[col][c];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; 4];
    for r in (0..4).rev() {
        let s: f64 = (r + 1..4).map(|c| a[r][c] * x[c]).sum();
        x[r] = if a[r][r].abs() < 1.0e-300 { 0.0 } else { (b[r] - s) / a[r][r] };
    }
    x
}
