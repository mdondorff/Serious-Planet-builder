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
/// least one cell. `None` for a map that is not `n x n` cells of exactly `MAX_SLOT_AZIMUTHS` (16) azimuths: the shader places azimuth `k` at `k / 16` of a turn.
pub fn pack_map(bytes: &[u8], n: usize, azimuths: usize) -> Option<Vec<u8>> {
    if n == 0 || azimuths != MAX_SLOT_AZIMUTHS || bytes.len() != n * n * azimuths {
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

/// The coarse horizon map (GEN-045: faces in order, rows `j` outer, `i`, then azimuth; `m x m` cells a face) packed as the GPU reads it: layer
/// `4 f + L` of the array holds face `f`, azimuths `4 L .. 4 L + 3` in the four channels of the texel `(i, j)`; a missing azimuth is 0. `None`
/// for a map that is not `6 m^2` cells (`m >= 2`: a 1 x 1 texture is the shader's "no coarse map") of exactly [`MAX_SLOT_AZIMUTHS`] (16) azimuths.
pub fn pack_coarse(bytes: &[u8], m: usize, azimuths: usize) -> Option<Vec<u8>> {
    if m < 2 || azimuths != MAX_SLOT_AZIMUTHS || bytes.len() != 6 * m * m * azimuths {
        return None;
    }
    let mut out = vec![0u8; 6 * LAYERS_PER_SLOT * m * m * 4];
    for face in 0..6 {
        for cell in 0..m * m {
            for k in 0..azimuths {
                let (layer, channel) = (face * LAYERS_PER_SLOT + k / AZIMUTHS_PER_LAYER, k % AZIMUTHS_PER_LAYER);
                out[(layer * m * m + cell) * 4 + channel] = bytes[(face * m * m + cell) * azimuths + k];
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

/// Floats a node takes in the per-node table: two `vec4` (binding 4 of the terrain pass).
pub const NODE_TABLE_FLOATS: usize = 8;

/// What the renderer reads for one plan node: the slot of its chunk's horizon map (`-1`: none, the term is the identity), the sun's azimuth
/// (radians from the chunk's `+j` through `+i`) and elevation at the chunk centre, the chunk fade's weight (REND-039; `1.0` until that lands,
/// the identity) and the tile's rectangle inside the chunk, `[u0, v0, du, dv]`. A node without a slot is zero beyond the slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizonNode {
    pub slot: i32,
    pub sun_azimuth: f32,
    pub sun_elevation: f32,
    pub fade: f32,
    pub rect: [f32; 4],
}

impl HorizonNode {
    const NONE: HorizonNode = HorizonNode { slot: -1, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 0.0, rect: [0.0; 4] };
}

/// The frame's horizon data for the renderer: one [`HorizonNode`] per plan node, in the plan's order, the slots' capacity and what to upload.
#[derive(Clone, Debug, PartialEq)]
pub struct HorizonFrame {
    pub capacity: u32,
    pub nodes: Vec<HorizonNode>,
    pub update: SlotUpdate,
}

impl HorizonFrame {
    /// `slots` is the state after this frame's [`HorizonSlots::update`], whose result is `update`; `tiles` are the plan's nodes in order; `sun`
    /// is the unit direction to the sun in the planet-fixed frame. The sun's numbers are decided once per chunk.
    pub fn build(slots: &HorizonSlots, update: SlotUpdate, tiles: &[TileId], sun: Vec3, map: &dyn FaceMapping) -> HorizonFrame {
        let mut per_chunk: HashMap<ChunkKey, (f32, f32)> = HashMap::new();
        let nodes = tiles
            .iter()
            .map(|&tile| {
                let Some((key, rect)) = tile_in_chunk(tile) else { return HorizonNode::NONE };
                let Some(slot) = slots.slot_of(key) else { return HorizonNode::NONE };
                let (sun_azimuth, sun_elevation) = *per_chunk
                    .entry(key)
                    .or_insert_with(|| (chunk_sun_azimuth(map, key, sun) as f32, chunk_sun_elevation(map, key, sun) as f32));
                HorizonNode { slot: slot as i32, sun_azimuth, sun_elevation, fade: 1.0, rect }
            })
            .collect();
        HorizonFrame { capacity: slots.capacity(), nodes, update }
    }

    /// The table as the shader reads it: per node `[slot, azimuth, elevation, fade]` then `[u0, v0, du, dv]`.
    pub fn table(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.nodes.len() * NODE_TABLE_FLOATS);
        for n in &self.nodes {
            out.extend_from_slice(&[n.slot as f32, n.sun_azimuth, n.sun_elevation, n.fade]);
            out.extend_from_slice(&n.rect);
        }
        out
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
    fn the_coarse_map_is_packed_to_four_layers_a_face_with_four_azimuths_a_texel() {
        let (m, a) = (3usize, 16usize);
        let bytes: Vec<u8> = (0..6 * m * m * a).map(|i| (i % 251) as u8).collect();
        let packed = pack_coarse(&bytes, m, a).unwrap();
        assert_eq!(packed.len(), 6 * LAYERS_PER_SLOT * m * m * 4);
        for (face, cell, k) in [(0usize, 0usize, 0usize), (2, 4, 15), (5, 8, 3), (3, 1, 4)] {
            let (layer, channel) = (face * LAYERS_PER_SLOT + k / 4, k % 4);
            assert_eq!(
                packed[(layer * m * m + cell) * 4 + channel],
                bytes[(face * m * m + cell) * a + k],
                "face {face} cell {cell} azimuth {k}"
            );
        }
        // The shader places azimuth k at k / 16 of a turn: any other count, a missing byte and a 1 x 1 map (the shader's "none") are refused.
        assert!(pack_coarse(&bytes[1..], m, a).is_none() && pack_coarse(&bytes, m, 17).is_none());
        assert!(pack_coarse(&vec![0u8; 6 * m * m * 4], m, 4).is_none() && pack_coarse(&[0u8; 6 * 16], 1, 16).is_none());
    }

    // spec: REND-041
    #[test]
    fn a_map_is_packed_to_four_layers_of_256_squared_texels_with_four_azimuths_a_texel() {
        let (n, a) = (8usize, 16usize);
        let bytes: Vec<u8> = (0..n * n * a).map(|i| ((i * 37) % 251) as u8).collect();
        let packed = pack_map(&bytes, n, a).unwrap();
        assert_eq!(packed.len(), SLOT_BYTES);
        let h = HORIZON_TEXELS;
        // A map smaller than the layer: every texel is the cell under it (the span is one cell), azimuth k in layer k/4, channel k%4.
        for (cx, cy, k) in [(0usize, 0usize, 0usize), (7, 7, 15), (3, 2, 4), (5, 6, 1)] {
            let (tx, ty) = (cx * h / n + 1, cy * h / n + 1);
            let v = packed[(((k / 4) * h + ty) * h + tx) * 4 + k % 4];
            assert_eq!(v, bytes[(cy * n + cx) * a + k], "cell ({cx}, {cy}) azimuth {k}");
        }
    }

    // spec: REND-041
    #[test]
    fn a_larger_map_is_the_mean_of_the_cells_under_a_texel_and_a_bad_map_is_refused() {
        let (n, a) = (512usize, 16usize);
        let bytes: Vec<u8> = (0..n * n * a).map(|i| if ((i / a) % n) % 2 == 0 { 10 } else { 30 }).collect();
        let packed = pack_map(&bytes, n, a).unwrap();
        // 2 x 2 cells a texel: the mean of 10, 30, 10, 30.
        assert!(packed[..HORIZON_TEXELS * HORIZON_TEXELS * 4].chunks(4).all(|t| t == [20; 4]), "the mean in every channel");
        assert_eq!(pack_map(&bytes[1..], n, a), None);
        assert_eq!(pack_map(&bytes, 0, a), None);
        assert_eq!(pack_map(&vec![0u8; 4 * 4 * 17], 4, 17), None, "more azimuths than a slot holds");
        assert_eq!(pack_map(&[0u8; 4 * 4 * 8], 4, 8), None, "the shader reads 16 azimuths");
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

    fn sun_at(key: ChunkKey, elevation: f64) -> Vec3 {
        let tile = TileId::new(Face(key.0), CHUNK_GRID_LEVEL, key.1, key.2).unwrap();
        let (s0, s1, t0, t1) = tile.bounds();
        let (s, t) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
        let up = face_to_direction(&TangentWarp, Face(key.0), s, t);
        let toward_i = face_to_direction(&TangentWarp, Face(key.0), s + 1.0e-3, t);
        (toward_i - up * toward_i.dot(up)).normalized() * libm::cos(elevation) + up * libm::sin(elevation)
    }

    // spec: REND-041
    #[test]
    fn a_node_of_a_chunk_with_a_slot_carries_its_slot_the_suns_numbers_and_its_rectangle() {
        let k: ChunkKey = (2, 1000, 1001);
        let mut slots = HorizonSlots::new(4);
        let up = slots.update(&[((2, 7, 7), 1), (k, 1)], 1);
        let sun = sun_at(k, 0.3);
        let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1001).unwrap();
        let deep = TileId::new(Face(2), CHUNK_GRID_LEVEL + 4, 1000 * 16 + 5, 1001 * 16 + 9).unwrap();
        let f = HorizonFrame::build(&slots, up.clone(), &[deep, chunk], sun, &TangentWarp);
        assert_eq!(f.nodes.len(), 2);
        assert_eq!(f.capacity, 4);
        assert_eq!(f.update, up, "the frame carries the uploads the slots decided");
        let slot = slots.slot_of(k).unwrap() as i32;
        let (az, el) = (chunk_sun_azimuth(&TangentWarp, k, sun) as f32, chunk_sun_elevation(&TangentWarp, k, sun) as f32);
        assert_eq!(
            f.nodes[0],
            HorizonNode { slot, sun_azimuth: az, sun_elevation: el, fade: 1.0, rect: [5.0 / 16.0, 9.0 / 16.0, 1.0 / 16.0, 1.0 / 16.0] }
        );
        assert_eq!(f.nodes[1], HorizonNode { slot, sun_azimuth: az, sun_elevation: el, fade: 1.0, rect: [0.0, 0.0, 1.0, 1.0] });
        assert!((el - 0.3).abs() < 1e-6);
        assert!((az - core::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    // spec: REND-041
    #[test]
    fn a_node_outside_a_chunk_or_whose_chunk_has_no_slot_has_slot_minus_one() {
        let k: ChunkKey = (2, 1000, 1001);
        let mut slots = HorizonSlots::new(1);
        slots.update(&[(k, 1)], 1);
        let coarse = TileId::new(Face(2), CHUNK_GRID_LEVEL - 1, 3, 3).unwrap();
        let other = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1002, 1001).unwrap();
        let f = HorizonFrame::build(&slots, SlotUpdate::default(), &[coarse, other], sun_at(k, 0.3), &TangentWarp);
        for n in &f.nodes {
            assert_eq!(n.slot, -1);
            assert_eq!((n.sun_azimuth, n.sun_elevation, n.rect), (0.0, 0.0, [0.0; 4]), "an empty node is zero beyond the slot");
        }
        // A frame with no slots at all: every node reads the coarse horizon alone.
        let none = HorizonFrame::build(&HorizonSlots::new(0), SlotUpdate::default(), &[other], sun_at(k, 0.3), &TangentWarp);
        assert_eq!(none.nodes[0].slot, -1);
        assert!(none.update.uploads.is_empty());
    }

    // spec: REND-041
    #[test]
    fn the_table_is_two_vec4_a_node_in_the_order_of_the_plan_and_the_same_frame_builds_the_same_table() {
        let k: ChunkKey = (2, 1000, 1001);
        let mut slots = HorizonSlots::new(2);
        slots.update(&[(k, 1)], 1);
        let sun = sun_at(k, 0.2);
        let child = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1001).unwrap().children().unwrap()[3];
        let coarse = TileId::new(Face(2), CHUNK_GRID_LEVEL - 2, 3, 3).unwrap();
        let tiles = [coarse, child];
        let f = HorizonFrame::build(&slots, SlotUpdate::default(), &tiles, sun, &TangentWarp);
        let t = f.table();
        assert_eq!(t.len(), tiles.len() * NODE_TABLE_FLOATS);
        assert_eq!(NODE_TABLE_FLOATS, 8);
        assert_eq!(t[0], -1.0, "node 0 is outside a chunk");
        let n = &f.nodes[1];
        assert_eq!(&t[8..16], &[n.slot as f32, n.sun_azimuth, n.sun_elevation, n.fade, 0.5, 0.5, 0.5, 0.5]);
        assert_eq!(t, HorizonFrame::build(&slots, SlotUpdate::default(), &tiles, sun, &TangentWarp).table());
        assert!(HorizonFrame::build(&slots, SlotUpdate::default(), &[], sun, &TangentWarp).table().is_empty());
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
