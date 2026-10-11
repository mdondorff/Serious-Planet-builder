//! Generating tiles into a store and verifying a store against the definition (STRM-004, ADR 0006, ADR 0019).
//!
//! `cargo xtask cache-verify` regenerates a sample of stored tiles from scratch and compares them with what the store
//! holds. A stored tile whose key matches the key the current definition gives for it must equal the regenerated tile
//! bit for bit; if it does not, an input of the tile is missing from its key, and the report names the tile, the pack
//! offset and the first differing sample. A stored tile whose key is no longer the definition's key is *stale* (the
//! definition or a version changed since): that is not an error, only reclaimable space.

use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::TileId;
use planet_generators::height::{generate_tile_field, HeightField, HeightGenerator, ReferenceGenerator};
use planet_world_def::{BodyId, GenContext, MissingNode, ReadRecord, WorldDefinition};

use crate::codec::{decode_height_tile, encode_height_tile, payload_res};
use crate::key::{height_key, CacheKey, Kind};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore};

/// The kind byte of a height tile record.
pub const KIND_HEIGHT: u8 = Kind::Height as u8;

/// A definition as the tile store sees it: the single planet's height field, its radius and what identifies its store.
pub struct World {
    field: Box<dyn HeightField>,
    radius_m: f64,
    node_set_id: [u8; 16],
}

impl World {
    /// Build the world of body 0 of a definition: its height field comes from the reference generator through a context,
    /// so its keys follow the recorded read set.
    pub fn from_definition(def: &WorldDefinition) -> Result<World, MissingNode> {
        let ctx = GenContext::new(def);
        let field = ReferenceGenerator.field(&ctx, BodyId::PLANET)?;
        let radius_m = ctx.body_radius_m(BodyId::PLANET)?;
        let mut paths: Vec<String> = field.read_records().iter().map(ReadRecord::path).collect();
        paths.push(planet_world_def::body_radius_path(BodyId::PLANET));
        paths.sort();
        paths.dedup();
        let mut material = Vec::new();
        for p in &paths {
            material.extend_from_slice(&(p.len() as u32).to_le_bytes());
            material.extend_from_slice(p.as_bytes());
        }
        let full = blake3::derive_key("planet-builder node set v1", &material);
        Ok(World { field, radius_m, node_set_id: full[..16].try_into().expect("16 bytes") })
    }

    /// What identifies the store of this world: the hash version and the body.
    pub fn params(&self) -> StoreParams {
        StoreParams { hash_version: HASH_VERSION, body: 0 }
    }

    pub fn radius_m(&self) -> f64 {
        self.radius_m
    }

    /// The persistent key of the height tile `tile` at `res` samples per edge.
    pub fn tile_key(&self, tile: TileId, res: u32) -> CacheKey {
        height_key(self.field.as_ref(), &TangentWarp, tile, res, self.radius_m)
    }

    /// Generate the height tile and wrap it in a record ready for a pack.
    pub fn tile_record(&self, tile: TileId, res: u32) -> Record {
        let data = generate_tile_field(self.field.as_ref(), &TangentWarp, tile, res);
        Record {
            key: self.tile_key(tile, res),
            kind: KIND_HEIGHT,
            tile,
            generator_version: self.field.version(),
            hash_version: HASH_VERSION,
            node_set_id: self.node_set_id,
            payload: encode_height_tile(&data),
        }
    }
}

/// Every tile of one face at one level, in row order.
pub fn region_tiles(face: planet_core::Face, level: u8) -> Vec<TileId> {
    let n = 1u32 << level;
    (0..n).flat_map(|y| (0..n).map(move |x| TileId::new(face, level, x, y).expect("a tile of its own grid"))).collect()
}

/// What a build did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildReport {
    pub written: usize,
    pub reused: usize,
}

/// Generate `tiles` at `res` into the store in `dir`. A tile the store already holds under the current key is reused, not
/// generated again, so a second run writes nothing.
pub fn build_tiles(
    dir: &std::path::Path,
    world: &World,
    tiles: &[TileId],
    res: u32,
    max_pack_bytes: u64,
) -> Result<BuildReport, PackError> {
    let existing = TileStore::open(dir, world.params())?;
    let mut writer = PackWriter::open(dir, world.params(), max_pack_bytes)?;
    let mut report = BuildReport { written: 0, reused: 0 };
    for tile in tiles {
        let key = world.tile_key(*tile, res);
        if matches!(existing.get(KIND_HEIGHT, *tile, key), Ok(Some(_))) {
            report.reused += 1;
            continue;
        }
        writer.append(&world.tile_record(*tile, res))?;
        report.written += 1;
    }
    writer.finish()?;
    Ok(report)
}

/// One stored tile that differs from its regeneration.
#[derive(Clone, Debug, PartialEq)]
pub struct Mismatch {
    pub tile: TileId,
    pub key: CacheKey,
    pub pack: String,
    pub offset: u64,
    pub detail: String,
}

/// The outcome of verifying a sample of a store.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VerifyReport {
    /// Entries looked at.
    pub checked: usize,
    /// Regenerated bit for bit equal to what is stored.
    pub matching: usize,
    /// Stored under a key the current definition no longer gives: not producible from it any more, space to reclaim.
    pub stale: usize,
    /// Entries of a kind this verifier does not regenerate.
    pub unsupported: usize,
    /// Stored under the current key but different from the regeneration: an input is missing from the key.
    pub mismatches: Vec<Mismatch>,
    /// Entries that could not be read or decoded (CRC failure, damaged payload).
    pub unreadable: Vec<String>,
}

impl VerifyReport {
    /// Whether the store passes: nothing differs and nothing is unreadable. Stale entries are not failures.
    pub fn passed(&self) -> bool {
        self.mismatches.is_empty() && self.unreadable.is_empty()
    }
}

/// Verify `sample` evenly spaced entries of the store (all of them when `sample` is 0).
pub fn verify_store(store: &TileStore, world: &World, sample: usize) -> VerifyReport {
    let mut report = VerifyReport::default();
    for item in store.sample_records(sample) {
        report.checked += 1;
        let stored = match item {
            Ok(s) => s,
            Err(e) => {
                report.unreadable.push(e.to_string());
                continue;
            }
        };
        let r = &stored.record;
        if r.kind != KIND_HEIGHT {
            report.unsupported += 1;
            continue;
        }
        let res = match payload_res(&r.payload) {
            Ok(res) => res,
            Err(e) => {
                report.unreadable.push(format!(
                    "{}: tile {:#018x} key {} at offset {}: {e}",
                    stored.pack,
                    r.tile.raw(),
                    r.key,
                    stored.offset
                ));
                continue;
            }
        };
        if world.tile_key(r.tile, res) != r.key {
            report.stale += 1;
            continue;
        }
        let kept = match decode_height_tile(r.tile, &r.payload) {
            Ok(t) => t,
            Err(e) => {
                report.unreadable.push(format!(
                    "{}: tile {:#018x} key {} at offset {}: {e}",
                    stored.pack,
                    r.tile.raw(),
                    r.key,
                    stored.offset
                ));
                continue;
            }
        };
        let fresh = generate_tile_field(world.field.as_ref(), &TangentWarp, r.tile, res);
        let differing: Vec<usize> =
            (0..fresh.heights.len()).filter(|i| fresh.heights[*i].to_bits() != kept.heights[*i].to_bits()).collect();
        match differing.first() {
            None => report.matching += 1,
            Some(&i) => {
                let (k, l) = (i as u32 % (res + 1), i as u32 / (res + 1));
                report.mismatches.push(Mismatch {
                    tile: r.tile,
                    key: r.key,
                    pack: stored.pack.clone(),
                    offset: stored.offset,
                    detail: format!(
                        "sample ({k}, {l}): stored {} (bits {:#010x}), regenerated {} (bits {:#010x}); {} of {} samples differ: an input of this tile is missing from its key",
                        kept.heights[i],
                        kept.heights[i].to_bits(),
                        fresh.heights[i],
                        fresh.heights[i].to_bits(),
                        differing.len(),
                        fresh.heights.len()
                    ),
                });
            }
        }
    }
    report
}
