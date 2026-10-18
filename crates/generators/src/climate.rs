//! The climate field of a tile (ADR 0039 point 2, GEN-021): temperature and precipitation at 9 x 9 points of the tile, two `f16`
//! channels, 324 bytes per tile. A **field** (ADR 0035 point 1): the cache stores it with the tile and keys it, so it has its own
//! implementation id and version, uses `libm`/integer lattice arithmetic only, and is bit-exact across runs, thread counts and
//! operating systems (CON-14). The GPU only decodes the stored values (an exact `f16` to `f32` conversion) and interpolates them
//! for display (presentation data).
//!
//! Where the values come from: the global bake, read with the integer reader of ADR 0027 ([`read_tile_sample`], a tile-array
//! consumer): texel `(a, b)` of the tile is the bake's value at tile sample `(a, b)` of a tile grid of 8 cells per edge, so
//! the 9 x 9 points sit on the tile square including its border and two neighbouring tiles agree on their shared border texels
//! (the reader is face-edge symmetric; its halo rule is never needed because every position is inside the tile). The reader's
//! [`READER_VERSION`] and [`GLOBAL_VERSION`] enter the field's version word ([`climate_version_word`]) and so every mesh key.
//! A field without a bake (the reference generator, synthetic test fields) takes the analytic global-coarse function of its seed
//! at the same directions ([`analytic_climate`]), a pure function of `(seed, direction)`.
//!
//! The stored value is the `f16` rounding (round to nearest, ties to even) of the reader's `f32`. That rounding is part of the
//! definition: the field *is* the rounded value, so CPU and GPU agree exactly (the GPU decode is exact). Against the bake the
//! rounding costs at most `2^-11` of the value (0.016 degrees below 32 degrees C, 0.03 above; 2 mm per 1,000 mm of rain, 4 mm above
//! 2,048 mm), the declared quantisation of the field (GEN-021).

use crate::global::{gather_cells, read_tile_sample, sample, LatticeSource, ReadError, GLOBAL_VERSION, READER_VERSION};
use planet_core::cube::FaceMapping;
use planet_core::hash::hash_u64s;
use planet_core::TileId;

/// Bump whenever any output of this module changes (ADR 0006): the layout of the texture, the sampling positions, the rounding.
pub const CLIMATE_VERSION: u32 = 1;
/// Names this implementation in cache keys (CON-07).
pub const CLIMATE_IMPLEMENTATION_ID: &str = "cpu-climate-v1";

/// Texels along one edge of the climate texture (cells + 1).
pub const CLIMATE_EDGE: usize = 9;
/// Cells of the tile grid the texture is read on: texel `(a, b)` is tile sample `(a, b)` of an 8-cell grid.
pub const CLIMATE_CELLS: u32 = 8;
/// Texels of the texture.
pub const CLIMATE_TEXELS: usize = CLIMATE_EDGE * CLIMATE_EDGE;
/// Bytes of one tile's climate: two `f16` per texel.
pub const CLIMATE_BYTES: usize = CLIMATE_TEXELS * 4;
/// A compact tile has at least this many cells per edge (GEN-019): the climate no longer rides in the texels, the minimum stays so
/// that the cells 16 configurations and their pool arithmetic are unchanged.
pub const CLIMATE_MIN_COMPACT_CELLS: u32 = 16;

/// `f32` to `f16` bits, round to nearest, ties to even; overflow gives infinity (the climate never reaches it: 65,504 is the
/// largest finite value).
pub fn f16_from_f32(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let mant = b & 0x007f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if mant != 0 { 0x0200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = mant | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rem = m & ((1 << shift) - 1);
        let tie = 1u32 << (shift - 1);
        let up = rem > tie || (rem == tie && half & 1 == 1);
        return sign | (half + u32::from(up)) as u16;
    }
    let r = ((e as u32) << 10) | (mant >> 13);
    let rem = mant & 0x1fff;
    let up = rem > 0x1000 || (rem == 0x1000 && r & 1 == 1);
    sign | (r + u32::from(up)) as u16
}

/// `f16` bits to `f32`, exact.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let exp = u32::from((h >> 10) & 0x1f);
    let mant = u32::from(h & 0x03ff);
    let bits = match (exp, mant) {
        (0, 0) => sign,
        (0, m) => {
            // Subnormal: m * 2^-24 with the highest set bit p, normalised: 1.xxx * 2^(p - 24).
            let p = 31 - m.leading_zeros();
            sign | ((p + 103) << 23) | (((m << (10 - p)) & 0x03ff) << 13)
        }
        (0x1f, 0) => sign | 0x7f80_0000,
        (0x1f, m) => sign | 0x7f80_0000 | (m << 13),
        (e, m) => sign | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// The climate of one tile: temperature (degrees C) and precipitation (mm per year) as `f16` bits at 9 x 9 points, row `b`
/// major (`index = 9 b + a`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClimateTexture {
    pub temperature: [u16; CLIMATE_TEXELS],
    pub precipitation: [u16; CLIMATE_TEXELS],
}

impl Default for ClimateTexture {
    fn default() -> Self {
        Self { temperature: [0; CLIMATE_TEXELS], precipitation: [0; CLIMATE_TEXELS] }
    }
}

impl ClimateTexture {
    /// The texture of 32-bit values, rounded once to `f16`.
    pub fn from_f32(temperature_c: &[f32; CLIMATE_TEXELS], precipitation_mm: &[f32; CLIMATE_TEXELS]) -> Self {
        Self { temperature: temperature_c.map(f16_from_f32), precipitation: precipitation_mm.map(f16_from_f32) }
    }

    /// Temperature in degrees C and precipitation in mm at texel `(a, b)`, decoded exactly.
    pub fn texel(&self, a: usize, b: usize) -> (f32, f32) {
        let k = b * CLIMATE_EDGE + a;
        (f16_to_f32(self.temperature[k]), f16_to_f32(self.precipitation[k]))
    }

    /// The bytes the pack stores: per texel the temperature then the precipitation, each `f16` little endian.
    pub fn to_bytes(&self) -> [u8; CLIMATE_BYTES] {
        let mut out = [0u8; CLIMATE_BYTES];
        for k in 0..CLIMATE_TEXELS {
            out[4 * k..4 * k + 2].copy_from_slice(&self.temperature[k].to_le_bytes());
            out[4 * k + 2..4 * k + 4].copy_from_slice(&self.precipitation[k].to_le_bytes());
        }
        out
    }

    /// Inverse of [`to_bytes`](ClimateTexture::to_bytes).
    pub fn from_bytes(bytes: &[u8; CLIMATE_BYTES]) -> Self {
        let mut t = Self::default();
        for k in 0..CLIMATE_TEXELS {
            t.temperature[k] = u16::from_le_bytes([bytes[4 * k], bytes[4 * k + 1]]);
            t.precipitation[k] = u16::from_le_bytes([bytes[4 * k + 2], bytes[4 * k + 3]]);
        }
        t
    }

    /// The climate block of a compact slot (GEN-025): per texel one word, the temperature `f16` in the low half and the
    /// precipitation `f16` in the high half.
    pub fn to_words(&self) -> [u32; CLIMATE_TEXELS] {
        std::array::from_fn(|k| u32::from(self.temperature[k]) | u32::from(self.precipitation[k]) << 16)
    }

    /// Inverse of [`to_words`](ClimateTexture::to_words).
    pub fn from_words(words: &[u32; CLIMATE_TEXELS]) -> Self {
        Self {
            temperature: std::array::from_fn(|k| (words[k] & 0xffff) as u16),
            precipitation: std::array::from_fn(|k| (words[k] >> 16) as u16),
        }
    }

    /// Bilinear interpolation at tile coordinates `(u, v)` in `[0, 1]` (clamped), in `f32` with `mix(a, b, t) = a (1 - t) + b t`:
    /// what the fragment stage computes (the CPU mirror of the GPU lookup; presentation data).
    pub fn sample(&self, u: f32, v: f32) -> (f32, f32) {
        let e = (CLIMATE_EDGE - 1) as f32;
        let (x, y) = (u.clamp(0.0, 1.0) * e, v.clamp(0.0, 1.0) * e);
        let (a, b) = ((x as usize).min(CLIMATE_EDGE - 2), (y as usize).min(CLIMATE_EDGE - 2));
        let (wx, wy) = (x - a as f32, y - b as f32);
        let mix = |p: f32, q: f32, t: f32| p * (1.0 - t) + q * t;
        let at = |a: usize, b: usize| self.texel(a, b);
        let (t00, t10, t01, t11) = (at(a, b), at(a + 1, b), at(a, b + 1), at(a + 1, b + 1));
        (mix(mix(t00.0, t10.0, wx), mix(t01.0, t11.0, wx), wy), mix(mix(t00.1, t10.1, wx), mix(t01.1, t11.1, wx), wy))
    }

    /// The value the vertex format carries for grid vertex `(i, j)` of a tile of `cells` cells (skirt vertices take their border
    /// vertex's): [`sample`](ClimateTexture::sample) at `(i / cells, j / cells)`, rounded once to `f16`. Presentation data derived
    /// at upload time; the stored field is the 9 x 9 texture.
    pub fn vertex_value(&self, i: u32, j: u32, cells: u32) -> [u16; 2] {
        let c = cells as f32;
        let (t, p) = self.sample(i as f32 / c, j as f32 / c);
        [f16_from_f32(t), f16_from_f32(p)]
    }

    /// Hash of the exact bits, for known-answer tests (CON-14).
    pub fn content_hash(&self) -> u64 {
        let v: Vec<u64> = self.temperature.iter().chain(&self.precipitation).map(|&h| u64::from(h)).collect();
        hash_u64s(0x636c_696d, &v)
    }
}

/// The version word folded into the keys of a field that reads the bake: the climate version, the reader version and the bake
/// format version (ADR 0039 point 2: `READER_VERSION` folds into this field's version).
pub fn climate_version_word() -> u64 {
    let mut v = vec![u64::from(CLIMATE_VERSION), u64::from(READER_VERSION), u64::from(GLOBAL_VERSION)];
    v.extend(crate::height::fold_str(CLIMATE_IMPLEMENTATION_ID));
    hash_u64s(0x636c_7672, &v)
}

/// The version word of the analytic climate (fields without a bake): the climate version and the analytic function's version.
pub fn analytic_version_word() -> u64 {
    let mut v = vec![u64::from(CLIMATE_VERSION), u64::from(GLOBAL_VERSION)];
    v.extend(crate::height::fold_str(CLIMATE_IMPLEMENTATION_ID));
    hash_u64s(0x636c_7673, &v)
}

/// The climate of tile `id` read from the bake `src` (ADR 0027 integer reader). An unreadable block is an error naming it, never a
/// default.
pub fn climate_tile(src: &dyn LatticeSource, id: TileId) -> Result<ClimateTexture, ReadError> {
    // The 81 reads are gathered by block like the heights of a tile (GEN-020): a block is decoded once however small the cache.
    match gather_cells(src, &|g| {
        let _ = climate_tile_direct(g, id);
    }) {
        Some(table) => climate_tile_direct(&table, id),
        None => climate_tile_direct(src, id),
    }
}

/// [`climate_tile`] reading `src` cell by cell as the reader asks: what a caller that has already gathered the cells (a tile
/// builder that gathers its heights and its climate together, `HeightField::heights_and_climate`) runs against its table.
pub fn climate_tile_direct(src: &dyn LatticeSource, id: TileId) -> Result<ClimateTexture, ReadError> {
    let (mut t, mut p) = ([0f32; CLIMATE_TEXELS], [0f32; CLIMATE_TEXELS]);
    for b in 0..CLIMATE_EDGE {
        for a in 0..CLIMATE_EDGE {
            let s = read_tile_sample(src, id, CLIMATE_CELLS, a as i64, b as i64)?;
            t[b * CLIMATE_EDGE + a] = s.temperature_c;
            p[b * CLIMATE_EDGE + a] = s.precipitation_mm;
        }
    }
    Ok(ClimateTexture::from_f32(&t, &p))
}

/// The climate of tile `id` from the analytic global-coarse function of `seed` at the texel directions: for fields that have no
/// bake. A pure function of `(seed, mapping, tile)`.
pub fn analytic_climate(seed: u64, map: &dyn FaceMapping, id: TileId) -> ClimateTexture {
    let (mut t, mut p) = ([0f32; CLIMATE_TEXELS], [0f32; CLIMATE_TEXELS]);
    for b in 0..CLIMATE_EDGE {
        for a in 0..CLIMATE_EDGE {
            let s = sample(seed, id.sample_direction(map, CLIMATE_CELLS, a as u32, b as u32));
            t[b * CLIMATE_EDGE + a] = s.temperature_c;
            p[b * CLIMATE_EDGE + a] = s.precipitation_mm;
        }
    }
    ClimateTexture::from_f32(&t, &p)
}
