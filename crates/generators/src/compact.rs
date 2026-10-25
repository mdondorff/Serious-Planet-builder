//! The compact tile format (ADR 0033 point 4, SPIKE-06, REND-016): one texel per grid vertex plus a 128-byte record instead of 44
//! bytes per vertex, for levels 3 and finer, with the exact morph target.
//!
//! No vertex buffer exists on the GPU for such a tile: the vertex shader derives `(i, j)` (or the skirt vertex) from the vertex
//! index, reads the texel of the vertex and of the neighbours the morph needs, and places the vertex on a biquadratic base
//! surface through nine tile-local control points plus the tile-relative height along the base normal. This module is the
//! GPU-free builder (CON-02/CON-03: decisions live here, the renderer executes them) and the CPU mirror of the shader.
//!
//! A tile is one slot of contiguous bytes (GEN-025, ADR 0062): a 128-byte header (nine control points, `hr_min`, `hr_step`,
//! the origin height, the skirt depth, the flags), the 324-byte climate block (GEN-021: 81 words, temperature `f16` low, precipitation
//! `f16` high), 4 bytes of padding and the texels. A texel is 8 bytes, two words (GEN-019): word 0 is the 32-bit height step count
//! (`hr_min + f32(q) * hr_step`, relative to the tile origin's height), word 1 holds two signed 15-bit octahedral normal components
//! in bits 0-14 and 15-29 (0.0072 degrees at worst, measured; CON-14: 0.01 degrees), bits 30-31 zero. The slot is the same bytes in
//! the CPU tile, the pack payload, the block of the pool and the shader's reads.
//!
//! CON-11: everything the shader sees is tile-local (control points, heights relative to the tile origin) or a unit vector; the
//! control points are `shell * dir - origin` rounded once from f64.
//!
//! CON-13: the height step is `relief / 16,777,215` (2^24 - 1 levels in the 32-bit field): every level is an integer an f32 holds
//! exactly, so CPU and GPU form the same `f32(q)`, and the step is at most 1 mm up to [`WIDE_RELIEF_1MM_M`] (16.7 km) of relief in one tile; the reconstruction error stays within 1 mm up to 8,192 m (the f32 sum, GEN-019). There is no relief guard and no fallback format: a tile with more relief has a larger step, documented in GEN-019.

use crate::climate::{ClimateTexture, CLIMATE_BYTES, CLIMATE_MIN_COMPACT_CELLS, CLIMATE_TEXELS};
use crate::height::HeightField;
use crate::mesh::{mesh_key_field, skirt_depth, vertex_count, MeshVertex};
use planet_core::cube::FaceMapping;
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};

/// Tiles of level 0 to this level less one keep the vertex format: the compact format starts at level 3 (the biquadratic base
/// surface errs by about 0.018 theta^2 of the tile edge, too much for levels 0-2; ADR 0033 point 4).
pub const COMPACT_MIN_LEVEL: u8 = 3;

/// Bytes of one texel: two words (GEN-019).
pub const TEXEL_BYTES: u64 = 8;

/// Bytes of the slot header: nine control points (27 f32), `hr_min`, `hr_step`, `h_origin`, the skirt depth and the flags word.
pub const SLOT_HEADER_BYTES: u64 = 9 * 12 + 4 * 4 + 4;

/// Offset of the climate block in the slot (GEN-025).
pub const SLOT_CLIMATE_OFFSET: u64 = SLOT_HEADER_BYTES;

/// Bytes of the climate block: 81 words.
pub const SLOT_CLIMATE_BYTES: u64 = CLIMATE_BYTES as u64;

/// Offset of the pad word (GEN-025).
pub const SLOT_PAD_OFFSET: u64 = SLOT_CLIMATE_OFFSET + SLOT_CLIMATE_BYTES;

/// Offset of the texels in the slot: 456 bytes of header, climate block and pad (GEN-025), a multiple of 8.
pub const SLOT_TEXEL_OFFSET: u64 = SLOT_PAD_OFFSET + 4;

/// Scale of the 15-bit normal components: `2^14 - 1`.
pub const NORMAL_SCALE: f32 = 16_383.0;

/// Quantisation levels of the height: the field is 32 bits wide, the count of levels is 2^24 - 1 so that `q` converts to f32 exactly
/// (an f32 holds integers up to 2^24), identically on the CPU and in the shader (GEN-019).
pub const HEIGHT_STEPS: f64 = 16_777_215.0;

/// Relief (metres) up to which one tile's height step is at most [`MAX_FINEST_STEP_M`]: 1 mm x [`HEIGHT_STEPS`] = 16,777 m.
pub const WIDE_RELIEF_1MM_M: f64 = MAX_FINEST_STEP_M * HEIGHT_STEPS;

/// CON-13: tile-vertex quantisation at the finest level at most this many metres.
pub const MAX_FINEST_STEP_M: f64 = 1.0e-3;

/// Texels of a slot at `cells`: `(cells + 1)^2`.
pub const fn texel_count(cells: u32) -> usize {
    ((cells + 1) * (cells + 1)) as usize
}

/// Bytes of the texels of a slot at `cells` (texels only).
pub const fn slot_texel_bytes(cells: u32) -> u64 {
    texel_count(cells) as u64 * TEXEL_BYTES
}

/// The stride of a slot at `cells`: `456 + 8 (cells + 1)^2` (GEN-025: 2,768, 9,168 and 34,256 B at cells 16, 32 and 64).
pub const fn slot_bytes(cells: u32) -> u64 {
    SLOT_TEXEL_OFFSET + slot_texel_bytes(cells)
}

/// One compact tile: control points, climate, quantised heights and normals (one slot, GEN-025).
#[derive(Clone, Debug, PartialEq)]
pub struct CompactTile {
    pub id: TileId,
    pub cells: u32,
    /// The mesh key of the tile ([`mesh_key_field`]): a tile has one key whichever format it is drawn in, so a plan's key
    /// matches the resident tile.
    pub key: u64,
    /// Tile centre at its terrain height (identical to `TileMesh::origin`).
    pub origin: PlanetFixed,
    /// Control points `(a, b)` at `ctrl[3 * b + a]`: lattice `(a * cells / 2, b * cells / 2)`, tile-local, on the shell through the
    /// origin.
    pub ctrl: [[f32; 3]; 9],
    /// Height of the origin above the sphere (the field at the tile centre).
    pub h_origin: f32,
    /// Height (relative to `h_origin`) of quantisation level 0.
    pub hr_min: f32,
    /// Height step per quantisation level.
    pub hr_step: f32,
    pub skirt_depth: f32,
    /// True when the base surface's `du x dv` points into the planet.
    pub flip: bool,
    /// Relief of the tile in metres (maximum minus minimum height, f64): what the CON-13 guard looks at.
    pub relief_m: f64,
    /// The climate block (GEN-021), 81 words: temperature `f16` low, precipitation `f16` high.
    pub climate_words: [u32; CLIMATE_TEXELS],
    /// `(cells + 1)^2` texels `[height step count, two 15-bit normal components]`, row `j` major.
    pub texels: Vec<[u32; 2]>,
    /// The chunk change of every texel in metres, row `j` major like the texels (STRM-047, REND-039): the texel heights already include it, so a
    /// vertex fades it with `h_q - (1 - w) change`. `None` for a tile without chunk detail (and for every tile of a world without chunks); it is
    /// not part of the slot, so the slot bytes, the pack payloads and the hashes of GEN-025 are what they were.
    pub change: Option<Vec<f32>>,
}

impl CompactTile {
    /// Bytes of this tile's texels.
    pub fn texel_bytes(&self) -> u64 {
        self.texels.len() as u64 * TEXEL_BYTES
    }

    /// Bytes this tile occupies in its slot (the stride of GEN-025).
    pub fn resident_bytes(&self) -> u64 {
        SLOT_TEXEL_OFFSET + self.texel_bytes()
    }

    /// The texels as the slot holds them: two little-endian words per texel, row `j` major.
    pub fn texel_data(&self) -> Vec<u8> {
        self.texels.iter().flat_map(|t| t.iter().flat_map(|c| c.to_le_bytes())).collect()
    }

    /// The header as the slot holds it: 9 control points as f32 triples, `hr_min`, `hr_step`, `h_origin`, `skirt_depth`, then the
    /// flags word (bit 0 the orientation flip, every other bit zero); 128 bytes.
    pub fn header_bytes(&self) -> [u8; SLOT_HEADER_BYTES as usize] {
        let mut out = [0u8; SLOT_HEADER_BYTES as usize];
        let mut words = self.ctrl.iter().flatten().copied().collect::<Vec<f32>>();
        words.extend([self.hr_min, self.hr_step, self.h_origin, self.skirt_depth]);
        for (k, w) in words.iter().enumerate() {
            out[k * 4..k * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        out[124..128].copy_from_slice(&u32::from(self.flip).to_le_bytes());
        out
    }

    /// The whole slot, byte for byte as the pool's block holds it (GEN-025): header, climate block, pad, texels.
    pub fn slot_data(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_slot_into(&mut out);
        out
    }

    /// [`slot_data`](CompactTile::slot_data) appended to `out` (word by word: no byte-wise iterator, REND-018).
    pub fn write_slot_into(&self, out: &mut Vec<u8>) {
        let at = out.len();
        out.reserve(self.resident_bytes() as usize);
        out.extend_from_slice(&self.header_bytes());
        for w in &self.climate_words {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 4]);
        for t in &self.texels {
            out.extend_from_slice(&t[0].to_le_bytes());
            out.extend_from_slice(&t[1].to_le_bytes());
        }
        debug_assert_eq!((out.len() - at) as u64, self.resident_bytes());
    }

    /// Rebuild a tile from the bytes of its slot (the inverse of [`slot_data`](CompactTile::slot_data)); the fields the slot does
    /// not hold (key, origin, relief) come from the caller. Names what is wrong with the bytes: a wrong length, flag bits, a pad word
    /// or texel bits that must be zero.
    pub fn from_slot_data(
        id: TileId,
        cells: u32,
        key: u64,
        origin: PlanetFixed,
        relief_m: f64,
        bytes: &[u8],
    ) -> Result<CompactTile, String> {
        if bytes.len() as u64 != slot_bytes(cells) {
            return Err(format!("a slot at cells {cells} is {} bytes, got {}", slot_bytes(cells), bytes.len()));
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
        let f = |k: usize| f32::from_bits(word(k * 4));
        let mut ctrl = [[0f32; 3]; 9];
        for (k, c) in ctrl.iter_mut().enumerate() {
            *c = [f(3 * k), f(3 * k + 1), f(3 * k + 2)];
        }
        let flags = word(124);
        if flags > 1 {
            return Err(format!("the slot's flags word {flags:#x} has reserved bits set"));
        }
        if word(SLOT_PAD_OFFSET as usize) != 0 {
            return Err("the slot's pad word is not zero".into());
        }
        let mut climate_words = [0u32; CLIMATE_TEXELS];
        for (k, w) in climate_words.iter_mut().enumerate() {
            *w = word(SLOT_CLIMATE_OFFSET as usize + 4 * k);
        }
        let texels: Vec<[u32; 2]> = bytes[SLOT_TEXEL_OFFSET as usize..]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| [u32::from_le_bytes(c[..4].try_into().expect("4 bytes")), u32::from_le_bytes(c[4..].try_into().expect("4 bytes"))])
            .collect();
        if texels.iter().any(|t| t[1] >> 30 != 0) {
            return Err("a texel has its two reserved normal bits set".into());
        }
        Ok(CompactTile {
            id,
            cells,
            key,
            origin,
            ctrl,
            hr_min: f(27),
            hr_step: f(28),
            h_origin: f(29),
            skirt_depth: f(30),
            flip: flags == 1,
            relief_m,
            climate_words,
            texels,
            change: None,
        })
    }

    /// The tile's climate field (ADR 0039 point 2), read from the climate block of the slot.
    pub fn climate(&self) -> ClimateTexture {
        ClimateTexture::from_words(&self.climate_words)
    }

    /// The 32-bit height step count of texel `k`.
    pub fn height_q(&self, k: usize) -> u32 {
        self.texels[k][0]
    }

    /// The two signed 15-bit normal components of texel `k`.
    pub fn normal_components(&self, k: usize) -> [i32; 2] {
        unpack_normal(self.texels[k][1])
    }

    /// Height of a texel relative to the origin height, as the shader computes it.
    pub fn rel_height(&self, i: usize, j: usize) -> f32 {
        self.hr_min + self.texels[j * (self.cells as usize + 1) + i][0] as f32 * self.hr_step
    }

    /// The largest error the 32-bit height quantisation can cause: half a step.
    pub fn height_error_bound_m(&self) -> f64 {
        0.5 * f64::from(self.hr_step)
    }

    /// The lowest height (m, f32 as the shader forms it) over the tile's own `(cells + 1)^2` lattice vertices: the exact minimum of the
    /// drawn terrain, which is linear between vertices and morphs only to averages of own vertices (ATMO-020). The step counts are
    /// monotone in the height, so the lowest texel gives it.
    pub fn min_height_m(&self) -> f32 {
        let q = self.texels.iter().map(|t| t[0]).min().unwrap_or(0);
        self.h_origin + (self.hr_min + q as f32 * self.hr_step)
    }

    /// The tile's id (the field `id`, as a method for the code that took it from the former `RenderTile`).
    pub fn id(&self) -> TileId {
        self.id
    }

    /// The tile's mesh key (the field `key`).
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Cells per tile edge (the field `cells`).
    pub fn cells(&self) -> u32 {
        self.cells
    }

    /// Tile centre at its terrain height, planet-fixed f64 (the field `origin`; the origin of the plan's camera-relative offsets).
    pub fn origin(&self) -> PlanetFixed {
        self.origin
    }

    /// Whether the CON-13 step bound (1 mm) holds for this tile.
    pub fn step_within_con13(&self) -> bool {
        f64::from(self.hr_step) <= MAX_FINEST_STEP_M * (1.0 + 1.0e-6)
    }

    /// Hash of the exact slot and origin bits, for known-answer tests (CON-14).
    pub fn content_hash(&self) -> u64 {
        let mut v = vec![
            self.id.raw(),
            u64::from(self.cells),
            self.key,
            self.origin.0.x.to_bits(),
            self.origin.0.y.to_bits(),
            self.origin.0.z.to_bits(),
        ];
        v.extend(self.slot_data().chunks(4).map(|c| u64::from(u32::from_le_bytes(c.try_into().expect("a slot is a multiple of 4 bytes")))));
        // A tile with a change plane hashes it too (after a marker); a tile without one hashes exactly as it always did.
        if let Some(change) = &self.change {
            v.push(0x0063_6870);
            v.extend(change.iter().map(|c| u64::from(c.to_bits())));
        }
        hash_u64s(0x0063_706b, &v)
    }
}

// ---- encoding ----

/// Octahedral encoding of a unit vector into two signed 15-bit components in `-16383..=16383` (GEN-019).
pub fn oct_encode(n: Vec3) -> [i32; 2] {
    let l1 = n.x.abs() + n.y.abs() + n.z.abs();
    let (mut x, mut y) = (n.x / l1, n.y / l1);
    if n.z < 0.0 {
        let sx = if x >= 0.0 { 1.0 } else { -1.0 };
        let sy = if y >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sx, (1.0 - x.abs()) * sy);
    }
    let q = |v: f64| (v.clamp(-1.0, 1.0) * f64::from(NORMAL_SCALE)).round() as i32;
    [q(x), q(y)]
}

/// Decode as the shader does (`extractBits` to a signed value, scale, then the octahedral fold, f32).
pub fn oct_decode(e: [i32; 2]) -> [f32; 3] {
    let s = |b: i32| (b as f32 / NORMAL_SCALE).max(-1.0);
    let (x, y) = (s(e[0]), s(e[1]));
    let mut n = [x, y, 1.0 - x.abs() - y.abs()];
    if n[2] < 0.0 {
        let (sx, sy) = (if n[0] >= 0.0 { 1.0 } else { -1.0 }, if n[1] >= 0.0 { 1.0 } else { -1.0 });
        n = [(1.0 - n[1].abs()) * sx, (1.0 - n[0].abs()) * sy, n[2]];
    }
    norm(n)
}

/// The normal word of a texel: component `x` in bits 0-14, `y` in bits 15-29 (two's complement), bits 30-31 zero.
pub fn pack_normal(e: [i32; 2]) -> u32 {
    (e[0] as u32 & 0x7fff) | (e[1] as u32 & 0x7fff) << 15
}

/// Inverse of [`pack_normal`]: sign-extend the two 15-bit fields (what `extractBits` does in the shader).
pub fn unpack_normal(w: u32) -> [i32; 2] {
    let s = |v: u32| ((v << 17) as i32) >> 17;
    [s(w & 0x7fff), s((w >> 15) & 0x7fff)]
}

/// The largest f32 not above `x`.
fn f32_down(x: f64) -> f32 {
    let r = x as f32;
    if f64::from(r) > x {
        f32::from_bits(if r > 0.0 { r.to_bits() - 1 } else { r.to_bits() + 1 })
    } else {
        r
    }
}

/// The smallest f32 not below `x`.
fn f32_up(x: f64) -> f32 {
    let r = x as f32;
    if f64::from(r) < x {
        f32::from_bits(if r >= 0.0 { r.to_bits() + 1 } else { r.to_bits() - 1 })
    } else {
        r
    }
}

/// Build the compact tile from a height field (all geometry in f64, rounded once).
pub fn compact_tile_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> CompactTile {
    assert!(
        cells >= CLIMATE_MIN_COMPACT_CELLS && cells.is_power_of_two(),
        "a compact tile needs a power-of-two number of cells of at least {CLIMATE_MIN_COMPACT_CELLS}"
    );
    let w = (cells + 1) as usize;
    let centre = id.center_direction(map);
    // One batch call for every height the tile needs (GEN-020): the origin, the lattice points and their ring (ADR 0073).
    let count = w * w;
    let mut dirs = Vec::with_capacity(1 + crate::normals::lattice_direction_count(cells));
    dirs.push(centre);
    dirs.extend(crate::normals::lattice_directions(map, id, cells));
    let (all, climate) = field.heights_and_climate(map, id, &dirs, &|| false);
    // The chunk change at the lattice points (the origin and the ring excluded), when the field has chunk detail here (STRM-047): stored only
    // when some texel moved, so a tile away from every chunk is the tile it was.
    let change = field
        .level_change(&dirs[1..1 + count], id.level())
        .filter(|c| c.iter().any(|&v| v != 0.0))
        .map(|c| c.into_iter().map(|v| v as f32).collect::<Vec<f32>>());
    let h_origin = all[0];
    let origin = PlanetFixed(centre * (radius_m + h_origin));
    // The shell through the origin: control points are small tile-local numbers (CON-11), not heights from the sea level.
    let shell = radius_m + h_origin;
    let half = cells / 2;
    let mut ctrl = [[0f32; 3]; 9];
    for b in 0..3 {
        for a in 0..3 {
            let p = id.sample_direction(map, cells, a * half, b * half) * shell - origin.0;
            ctrl[3 * b as usize + a as usize] = [p.x as f32, p.y as f32, p.z as f32];
        }
    }
    let mut hr = vec![0f64; count];
    let lattice_normals = crate::normals::lattice_normals(id, cells, radius_m, &dirs[1..], &all[1..]);
    let mut normals = vec![0u32; count];
    for k in 0..count {
        hr[k] = all[1 + k] - h_origin;
        normals[k] = pack_normal(oct_encode(lattice_normals[k]));
    }
    let lo = hr.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = hr.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // Rounded so that level 0 is not above the lowest height and the top level not below the highest: no clamping error.
    let hr_min = f32_down(lo);
    let hr_step = if hi > lo { f32_up((hi - f64::from(hr_min)) / HEIGHT_STEPS) } else { 1.0e-30 };
    let texels: Vec<[u32; 2]> = hr
        .iter()
        .zip(&normals)
        .map(|(h, n)| {
            let q = if hi > lo { ((h - f64::from(hr_min)) / f64::from(hr_step)).round().clamp(0.0, HEIGHT_STEPS) as u32 } else { 0 };
            [q, *n]
        })
        .collect();
    let mut tile = CompactTile {
        id,
        cells,
        key: mesh_key_field(field, map, id, cells, radius_m),
        origin,
        ctrl,
        h_origin: h_origin as f32,
        hr_min,
        hr_step,
        skirt_depth: skirt_depth(id.level(), radius_m) as f32,
        flip: false,
        relief_m: hi - lo,
        climate_words: climate.to_words(),
        texels,
        change,
    };
    let (du, dv) = tile.base_derivs(0.5, 0.5);
    let c = [centre.x as f32, centre.y as f32, centre.z as f32];
    let d = cross(du, dv);
    tile.flip = d[0] * c[0] + d[1] * c[1] + d[2] * c[2] < 0.0;
    tile
}

// ---- CPU mirror of the vertex shader (f32; same operations, but the shader may fuse multiply-adds, so it is not bit-exact with the GPU) ----

type V = [f32; 3];
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V, k: f32) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V) -> V {
    scale(a, 1.0 / (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt())
}

fn basis(t: f32) -> [f32; 3] {
    [(1.0 - t) * (1.0 - 2.0 * t), 4.0 * t * (1.0 - t), t * (2.0 * t - 1.0)]
}

fn dbasis(t: f32) -> [f32; 3] {
    [4.0 * t - 3.0, 4.0 - 8.0 * t, 4.0 * t - 1.0]
}

#[allow(clippy::needless_range_loop)]
impl CompactTile {
    fn base_point(&self, u: f32, v: f32) -> V {
        let (bu, bv) = (basis(u), basis(v));
        let mut p = [0f32; 3];
        for b in 0..3 {
            for a in 0..3 {
                p = add(p, scale(self.ctrl[3 * b + a], bu[a] * bv[b]));
            }
        }
        p
    }

    fn base_derivs(&self, u: f32, v: f32) -> (V, V) {
        let (bu, bv, du, dv) = (basis(u), basis(v), dbasis(u), dbasis(v));
        let (mut pu, mut pv) = ([0f32; 3], [0f32; 3]);
        for b in 0..3 {
            for a in 0..3 {
                pu = add(pu, scale(self.ctrl[3 * b + a], du[a] * bv[b]));
                pv = add(pv, scale(self.ctrl[3 * b + a], bu[a] * dv[b]));
            }
        }
        (pu, pv)
    }

    fn base_normal(&self, u: f32, v: f32) -> V {
        let (pu, pv) = self.base_derivs(u, v);
        let n = norm(cross(pu, pv));
        if self.flip {
            scale(n, -1.0)
        } else {
            n
        }
    }

    /// Position (tile-local) and tile-relative height of lattice vertex `(i, j)` with `fade` of the chunk change taken off (REND-039): the shader's
    /// `h_q - fade x change`, `fade` being `1 - w` for the tile's weight `w`. Without a plane, or at fade 0, it is the texel's height bit for bit.
    fn place(&self, i: i64, j: i64, fade: f32) -> (V, f32) {
        let c = self.cells as f32;
        let (u, v) = (i as f32 / c, j as f32 / c);
        let mut hr = self.rel_height(i as usize, j as usize);
        if let Some(change) = &self.change {
            hr -= fade * change[j as usize * (self.cells as usize + 1) + i as usize];
        }
        (add(self.base_point(u, v), scale(self.base_normal(u, v), hr)), hr)
    }

    /// What the vertex shader computes for vertex `vi` of the shared grid (grid vertices, then the four skirt runs): position,
    /// morph target, normal and heights, in the vertex format's terms.
    pub fn shade_vertex(&self, vi: usize) -> MeshVertex {
        self.shade_vertex_faded(vi, 0.0)
    }

    /// [`shade_vertex`](CompactTile::shade_vertex) with `fade` (`1 - w`) of the chunk change taken off every height the vertex reads, its morph
    /// target included (REND-039). The normals are the final surface's (the first version, `design.md` decision 4).
    pub fn shade_vertex_faded(&self, vi: usize, fade: f32) -> MeshVertex {
        let cells = self.cells as i64;
        let w = cells + 1;
        let grid = (w * w) as usize;
        let (i, j, skirt) = if vi < grid {
            (vi as i64 % w, vi as i64 / w, false)
        } else {
            let r = (vi - grid) as i64;
            let (k, t) = (r / w, r % w);
            match k {
                0 => (t, 0, true),
                1 => (t, cells, true),
                2 => (0, t, true),
                _ => (cells, t, true),
            }
        };
        let c = self.cells as f32;
        let n_own = self.base_normal(i as f32 / c, j as f32 / c);
        let (own_p, own_h) = self.place(i, j, fade);
        let avg = |a: (i64, i64), b: (i64, i64)| {
            let (pa, ha) = self.place(a.0, a.1, fade);
            let (pb, hb) = self.place(b.0, b.1, fade);
            (scale(add(pa, pb), 0.5), (ha + hb) * 0.5)
        };
        let (cp, ch) = match (i & 1, j & 1) {
            (0, 0) => (own_p, own_h),
            (1, 0) => avg((i - 1, j), (i + 1, j)),
            (0, 1) => avg((i, j - 1), (i, j + 1)),
            _ => avg((i - 1, j - 1), (i + 1, j + 1)),
        };
        let (mut pos, mut cpos) = (own_p, cp);
        if skirt {
            let d = scale(n_own, self.skirt_depth);
            pos = sub(pos, d);
            cpos = sub(cpos, d);
        }
        let t = self.texels[j as usize * w as usize + i as usize];
        let normal = oct_decode(unpack_normal(t[1]));
        MeshVertex { pos, coarse: cpos, normal, height: self.h_origin + own_h, coarse_height: self.h_origin + ch }
    }

    /// Every vertex as the shader computes it, in the vertex format's order.
    pub fn shade_all(&self) -> Vec<MeshVertex> {
        self.shade_all_faded(0.0)
    }

    /// [`shade_all`](CompactTile::shade_all) with `fade` of the chunk change taken off (REND-039).
    pub fn shade_all_faded(&self, fade: f32) -> Vec<MeshVertex> {
        (0..vertex_count(self.cells)).map(|vi| self.shade_vertex_faded(vi, fade)).collect()
    }
}
