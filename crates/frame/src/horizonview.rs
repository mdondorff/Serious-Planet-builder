//! What the frame decides about the horizon shadows (M8-5, REND-041): which chunk's horizon map sits in which layer of the renderer's texture
//! array, the map packed as the GPU reads it, the rectangle of a tile inside its chunk, and the sun's azimuth in a chunk's frame. GPU-free: the
//! renderer takes the slot, the texels and the numbers, this decides them (CON-03).
//!
//! A chunk's map is `n x n` cells of `azimuths` bytes (GEN-044). The array has `HORIZON_TEXELS x HORIZON_TEXELS` texels per layer and four
//! layers per slot (RGBA8: four azimuths a texel), so a slot holds up to 16 azimuths; the map is box-filtered down to that size. A tile finds
//! its texels from its rectangle inside the chunk, and a texel finds its azimuth neighbours by the channel and the layer.

use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{Face, TileId, Vec3};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::ChunkKey;
use planet_generators::horizon_lookup::{azimuth_in_frame, face_frame};
use std::collections::HashMap;

/// Texels along a side of a slot's layer.
pub const HORIZON_TEXELS: usize = 256;
/// Azimuths a layer holds (the four channels of RGBA8).
pub const AZIMUTHS_PER_LAYER: usize = 4;
/// The most azimuths a slot holds, hence the layers per slot.
pub const MAX_SLOT_AZIMUTHS: usize = 16;
pub const LAYERS_PER_SLOT: usize = MAX_SLOT_AZIMUTHS / AZIMUTHS_PER_LAYER;

/// Bytes of one slot's texels: its layers, each `HORIZON_TEXELS^2` texels of four channels.
pub const SLOT_BYTES: usize = LAYERS_PER_SLOT * HORIZON_TEXELS * HORIZON_TEXELS * 4;

/// The map of a chunk packed for the GPU: `LAYERS_PER_SLOT` layers one after another, each `HORIZON_TEXELS` rows of `HORIZON_TEXELS` texels of
/// four bytes (the azimuths `4 L .. 4 L + 3` for layer `L`; a missing azimuth is 0). Each texel is the mean of the cells of the map under it, at
/// least one cell. `None` for a map that is not `n x n` cells of 1 to `MAX_SLOT_AZIMUTHS` azimuths.
pub fn pack_map(bytes: &[u8], n: usize, azimuths: usize) -> Option<Vec<u8>> {
    if n == 0 || azimuths == 0 || azimuths > MAX_SLOT_AZIMUTHS || bytes.len() != n * n * azimuths {
        return None;
    }
    let h = HORIZON_TEXELS;
    let mut out = vec![0u8; SLOT_BYTES];
    // The cells under texel `t` along an axis: [t n / h, max(t n / h + 1, (t + 1) n / h)), clamped to the map.
    let span = |t: usize| -> (usize, usize) {
        let lo = (t * n / h).min(n - 1);
        let hi = ((t + 1) * n / h).max(lo + 1).min(n);
        (lo, hi)
    };
    for ty in 0..h {
        let (y0, y1) = span(ty);
        for tx in 0..h {
            let (x0, x1) = span(tx);
            let count = ((y1 - y0) * (x1 - x0)) as u32;
            for k in 0..azimuths {
                let mut sum = 0u32;
                for y in y0..y1 {
                    for x in x0..x1 {
                        sum += u32::from(bytes[(y * n + x) * azimuths + k]);
                    }
                }
                let (layer, channel) = (k / AZIMUTHS_PER_LAYER, k % AZIMUTHS_PER_LAYER);
                out[((layer * h + ty) * h + tx) * 4 + channel] = ((sum + count / 2) / count) as u8;
            }
        }
    }
    Some(out)
}

/// The rectangle of the tile `tile` inside the chunk it lies in, in the chunk's own `[0, 1]^2` (origin and size), and the chunk's key: `None`
/// for a tile coarser than the chunk grid. The tile's `x` runs along the chunk's `i` and its `y` along `j`.
pub fn tile_in_chunk(tile: TileId) -> Option<(ChunkKey, [f32; 4])> {
    let level = tile.level();
    if level < CHUNK_GRID_LEVEL {
        return None;
    }
    let d = u32::from(level - CHUNK_GRID_LEVEL);
    let (cx, cy) = (tile.x() >> d, tile.y() >> d);
    let side = 1u64 << d;
    let (lx, ly) = (u64::from(tile.x()) - (u64::from(cx) << d), u64::from(tile.y()) - (u64::from(cy) << d));
    let inv = 1.0 / side as f64;
    Some(((tile.face().0, cx, cy), [(lx as f64 * inv) as f32, (ly as f64 * inv) as f32, inv as f32, inv as f32]))
}

/// The azimuth of `sun` (a unit direction) in the raster frame of chunk `key`, taken at the chunk's centre: radians from the chunk's `+j`
/// clockwise through `+i` (GEN-044's convention), via [`face_frame`] and [`azimuth_in_frame`]. One number a chunk: it varies by well under the
/// 22.5 degrees between stored azimuths across the 5 km of a chunk.
pub fn chunk_sun_azimuth(map: &dyn FaceMapping, key: ChunkKey, sun: Vec3) -> f64 {
    let tile = TileId::new(Face(key.0), CHUNK_GRID_LEVEL, key.1, key.2).expect("a chunk of the grid is a tile");
    let (s0, s1, t0, t1) = tile.bounds();
    let (s, t) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
    let (east, north) = face_frame(map, Face(key.0), s, t);
    azimuth_in_frame(sun, east, north)
}

/// The sun's elevation at the chunk's centre, in radians.
pub fn chunk_sun_elevation(map: &dyn FaceMapping, key: ChunkKey, sun: Vec3) -> f64 {
    let tile = TileId::new(Face(key.0), CHUNK_GRID_LEVEL, key.1, key.2).expect("a chunk of the grid is a tile");
    let (s0, s1, t0, t1) = tile.bounds();
    let up = face_to_direction(map, Face(key.0), 0.5 * (s0 + s1), 0.5 * (t0 + t1));
    planet_generators::horizon_lookup::sun_elevation(sun, up)
}

/// What a frame has to upload: the maps that need their texels written into a slot, and the slots given up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotUpdate {
    /// `(chunk, slot)` whose texels the renderer writes now (a new map, or a re-bake of a chunk that keeps its slot).
    pub uploads: Vec<(ChunkKey, u32)>,
    /// The chunks that lost their slot.
    pub freed: Vec<ChunkKey>,
}

/// Which chunk's map sits in which layer-group of the array. A fixed number of slots; a chunk that is in view and has a map takes one, the
/// least recently wanted slot is given up when none is free and a wanted chunk needs one. Deterministic: ties go to the lower key.
#[derive(Debug)]
pub struct HorizonSlots {
    capacity: u32,
    of: HashMap<ChunkKey, u32>,
    last_wanted: HashMap<ChunkKey, u64>,
    /// The version of the map each slot holds (a re-bake changes it): a slot whose map changed is uploaded again.
    version: HashMap<ChunkKey, u64>,
}

impl HorizonSlots {
    pub fn new(capacity: u32) -> HorizonSlots {
        HorizonSlots { capacity, of: HashMap::new(), last_wanted: HashMap::new(), version: HashMap::new() }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// The slot of `key`, if it has one.
    pub fn slot_of(&self, key: ChunkKey) -> Option<u32> {
        self.of.get(&key).copied()
    }

    /// One frame: `wanted` are the chunks in view that have a map, each with the version of its map (a number that differs after a re-bake), in
    /// priority order (nearest first); `frame` is a counter that rises every frame. Chunks that are not wanted keep their slot until it is needed.
    pub fn update(&mut self, wanted: &[(ChunkKey, u64)], frame: u64) -> SlotUpdate {
        let mut out = SlotUpdate::default();
        for (key, _) in wanted {
            self.last_wanted.insert(*key, frame);
        }
        for &(key, version) in wanted {
            match self.of.get(&key).copied() {
                Some(slot) => {
                    if self.version.get(&key) != Some(&version) {
                        self.version.insert(key, version);
                        out.uploads.push((key, slot));
                    }
                }
                None => {
                    let slot = match self.free_slot() {
                        Some(s) => s,
                        None => {
                            // Give up the slot of the chunk wanted longest ago that is not wanted this frame.
                            let victim = self
                                .of
                                .keys()
                                .filter(|k| self.last_wanted.get(*k) != Some(&frame))
                                .min_by_key(|k| (self.last_wanted.get(*k).copied().unwrap_or(0), **k))
                                .copied();
                            let Some(v) = victim else { continue };
                            let slot = self.of.remove(&v).expect("a slot holder");
                            self.last_wanted.remove(&v);
                            self.version.remove(&v);
                            out.freed.push(v);
                            slot
                        }
                    };
                    self.of.insert(key, slot);
                    self.version.insert(key, version);
                    out.uploads.push((key, slot));
                }
            }
        }
        out
    }

    /// A chunk left RAM: its slot is free at once.
    pub fn release(&mut self, key: ChunkKey) -> bool {
        self.last_wanted.remove(&key);
        self.version.remove(&key);
        self.of.remove(&key).is_some()
    }

    fn free_slot(&self) -> Option<u32> {
        let used: std::collections::HashSet<u32> = self.of.values().copied().collect();
        (0..self.capacity).find(|s| !used.contains(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::cube::TangentWarp;

    fn key(x: u32) -> ChunkKey {
        (2, 1000 + x, 1000)
    }

    // spec: REND-041
    #[test]
    fn a_map_is_packed_to_four_layers_of_256_squared_texels_with_four_azimuths_a_texel() {
        let (n, a) = (8usize, 6usize);
        let bytes: Vec<u8> = (0..n * n * a).map(|i| ((i * 37) % 251) as u8).collect();
        let packed = pack_map(&bytes, n, a).unwrap();
        assert_eq!(packed.len(), SLOT_BYTES);
        let h = HORIZON_TEXELS;
        // A map smaller than the layer: every texel is the cell under it (the span is one cell), azimuth k in layer k/4, channel k%4.
        for (cx, cy, k) in [(0usize, 0usize, 0usize), (7, 7, 5), (3, 2, 4), (5, 6, 1)] {
            let (tx, ty) = (cx * h / n + 1, cy * h / n + 1);
            let v = packed[(((k / 4) * h + ty) * h + tx) * 4 + k % 4];
            assert_eq!(v, bytes[(cy * n + cx) * a + k], "cell ({cx}, {cy}) azimuth {k}");
        }
        // The azimuths a map does not have are 0 in the last layers.
        assert!(packed[(2 * h * h) * 4..].iter().all(|&b| b == 0), "azimuths 8 and up are absent");
    }

    // spec: REND-041
    #[test]
    fn a_larger_map_is_the_mean_of_the_cells_under_a_texel_and_a_bad_map_is_refused() {
        let (n, a) = (512usize, 1usize);
        let bytes: Vec<u8> = (0..n * n).map(|i| if (i % n) % 2 == 0 { 10 } else { 30 }).collect();
        let packed = pack_map(&bytes, n, a).unwrap();
        // 2 x 2 cells a texel: the mean of 10, 30, 10, 30.
        assert!(packed[..HORIZON_TEXELS * HORIZON_TEXELS * 4].chunks(4).all(|t| t[0] == 20 && t[1] == 0), "the mean, and no other channel");
        assert_eq!(pack_map(&bytes[1..], n, a), None);
        assert_eq!(pack_map(&bytes, 0, a), None);
        assert_eq!(pack_map(&vec![0u8; 4 * 4 * 17], 4, 17), None, "more azimuths than a slot holds");
        // A map that is not a multiple of the layer: the spans cover every cell once or more and never run past the map.
        let odd = pack_map(&vec![200u8; 614 * 614 * 16], 614, 16).unwrap();
        assert!(odd.iter().all(|&b| b == 200), "a constant map packs to the same constant, 614 cells to 256 texels");
    }

    // spec: REND-041
    #[test]
    fn a_tile_finds_its_rectangle_inside_the_chunk_it_lies_in() {
        let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1001).unwrap();
        assert_eq!(tile_in_chunk(chunk), Some(((2, 1000, 1001), [0.0, 0.0, 1.0, 1.0])));
        let child = chunk.children().unwrap()[3];
        let (k, r) = tile_in_chunk(child).unwrap();
        assert_eq!((k, r), ((2, 1000, 1001), [0.5, 0.5, 0.5, 0.5]));
        // Four levels down: 16 x 16 tiles in a chunk; the tile at (5, 9) of them.
        let deep = TileId::new(Face(2), CHUNK_GRID_LEVEL + 4, 1000 * 16 + 5, 1001 * 16 + 9).unwrap();
        let (k, r) = tile_in_chunk(deep).unwrap();
        assert_eq!(k, (2, 1000, 1001));
        assert_eq!(r, [5.0 / 16.0, 9.0 / 16.0, 1.0 / 16.0, 1.0 / 16.0]);
        // A tile coarser than the chunk grid lies in no chunk; every tile finer than it lies in the chunk of its ancestor.
        assert_eq!(tile_in_chunk(TileId::new(Face(2), CHUNK_GRID_LEVEL - 1, 3, 3).unwrap()), None);
        let (parent_key, _) = tile_in_chunk(deep).unwrap();
        let mut up = deep;
        while up.level() > CHUNK_GRID_LEVEL {
            up = up.parent().unwrap();
        }
        assert_eq!(parent_key, (up.face().0, up.x(), up.y()));
    }

    // spec: REND-041
    #[test]
    fn the_suns_azimuth_in_a_chunks_frame_is_north_toward_plus_j_and_east_toward_plus_i() {
        let key: ChunkKey = (2, 1000, 1000);
        let tile = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
        let (s0, s1, t0, t1) = tile.bounds();
        let (s, t) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
        let up = face_to_direction(&TangentWarp, Face(2), s, t);
        let toward_i = face_to_direction(&TangentWarp, Face(2), s + 1.0e-3, t);
        let toward_j = face_to_direction(&TangentWarp, Face(2), s, t + 1.0e-3);
        let sun = |toward: Vec3| (toward - up * toward.dot(up)).normalized() * libm::cos(0.4) + up * libm::sin(0.4);
        assert!((chunk_sun_azimuth(&TangentWarp, key, sun(toward_i)) - core::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(chunk_sun_azimuth(&TangentWarp, key, sun(toward_j)).abs() < 1e-6);
        assert!((chunk_sun_elevation(&TangentWarp, key, sun(toward_j)) - 0.4).abs() < 1e-9);
    }

    // spec: REND-041
    #[test]
    fn slots_go_to_the_chunks_in_view_are_uploaded_again_after_a_rebake_and_the_stalest_is_given_up() {
        let mut s = HorizonSlots::new(2);
        let up = s.update(&[(key(0), 1), (key(1), 1)], 1);
        assert_eq!((up.uploads.len(), up.freed.len()), (2, 0));
        assert_eq!((s.slot_of(key(0)), s.slot_of(key(1))), (Some(0), Some(1)));
        // The same maps again: nothing to upload.
        assert_eq!(s.update(&[(key(0), 1), (key(1), 1)], 2), SlotUpdate::default());
        // A re-bake of key(1) (another version): the same slot, uploaded again.
        let up = s.update(&[(key(0), 1), (key(1), 2)], 3);
        assert_eq!(up.uploads, vec![(key(1), 1)]);
        // A third chunk with both slots held: the one wanted longest ago (key(0), last wanted at frame 3 against key(1)'s 3...): only key(1) is
        // wanted now with key(2), so key(0) is the victim.
        let up = s.update(&[(key(2), 1), (key(1), 2)], 4);
        assert_eq!(up.freed, vec![key(0)]);
        assert_eq!(up.uploads, vec![(key(2), 0)], "it takes the slot key(0) held");
        assert_eq!((s.slot_of(key(0)), s.slot_of(key(2))), (None, Some(0)));
        // More chunks wanted than slots: the nearest (first) get them, the others wait with no slot and no panic.
        let up = s.update(&[(key(3), 1), (key(4), 1), (key(5), 1)], 5);
        assert_eq!(up.uploads.len(), 2, "two slots, two uploads");
        assert!(s.slot_of(key(5)).is_none());
        // A chunk that left RAM frees its slot at once.
        let held = s.slot_of(key(3)).is_some();
        assert_eq!(s.release(key(3)), held);
        assert!(s.slot_of(key(3)).is_none());
        let again = s.update(&[(key(5), 1)], 6);
        assert_eq!(again.uploads.len(), 1);
    }
}
