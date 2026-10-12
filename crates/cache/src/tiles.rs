//! Generating tiles into a store and verifying a store against the definition (STRM-004, ADR 0006, ADR 0019).
//!
//! `cargo xtask cache-verify` regenerates a sample of stored tiles from scratch and compares them with what the store
//! holds. A stored tile whose key matches the key the current definition gives for it must carry the metadata the
//! definition gives (generator version, hash version, node set id) and equal the regenerated tile bit for bit; if it does
//! not, an input of the tile is missing from its key, and the report names the tile, the pack offset and the first
//! differing sample. A stored tile whose key is no longer the definition's key is *stale*: *explained* when its versions or
//! node set id differ from the current ones, *unexplained* when they are all equal (a node value changed, or the key
//! function drifted: the report keeps the two apart). A run that compared nothing, because every entry is stale, is not a
//! pass unless the caller says stale entries are expected.
//!
//! What a same-definition verify can and cannot find: regenerating from the same definition and generator finds
//! nondeterminism, damage and records that disagree with their key's metadata; it finds an input missing from a key only
//! when that input's value differs between building and verifying (learning 2026-10-12).

use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, TileId};
use planet_generators::height::{generate_tile_field, HeightField, HeightGenerator, ReferenceGenerator};
use planet_world_def::{BodyId, GenContext, MissingNode, ReadRecord, WorldDefinition};

use crate::codec::{decode_height_tile, encode_height_tile, payload_res, valid_res, CodecError};
use crate::key::{height_key, CacheKey, Kind};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore};

/// The kind byte of a height tile record.
pub const KIND_HEIGHT: u8 = Kind::Height as u8;

/// Deepest level of a face region the library builds: 4^6 = 4,096 tiles.
pub const MAX_REGION_LEVEL: u8 = 6;

/// Most raw (uncompressed) tile bytes one build may produce: a guard against an accidental `--region 0,6 --res 2048`.
pub const MAX_BUILD_BYTES: u64 = 4 << 30;

/// A definition as the tile store sees it: the single planet's height field, its radius and what identifies its store.
/// Only body 0 is stored and verified; a definition with more bodies is not refused, its other bodies are ignored.
pub struct World {
    field: Box<dyn HeightField>,
    radius_m: f64,
    node_set_id: [u8; 16],
}

impl World {
    /// Build the world of body 0 of a definition with the reference generator.
    pub fn from_definition(def: &WorldDefinition) -> Result<World, MissingNode> {
        World::from_generator(def, &ReferenceGenerator)
    }

    /// Build the world of body 0 with any height generator: its field comes through a context, so its keys follow the
    /// recorded read set. The node set id is the BLAKE3-128 (context `planet-builder node set v1`) of the sorted,
    /// deduplicated, u32-length-prefixed paths of the field's reads and of the body's radius node (proposed ADR 0024).
    pub fn from_generator(def: &WorldDefinition, generator: &dyn HeightGenerator) -> Result<World, MissingNode> {
        let ctx = GenContext::new(def);
        let field = generator.field(&ctx, BodyId::PLANET)?;
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

    /// The persistent key of the height tile `tile` at `res` cells per edge (the unit of the mesh's `cells`).
    pub fn tile_key(&self, tile: TileId, res: u32) -> CacheKey {
        height_key(self.field.as_ref(), &TangentWarp, tile, res, self.radius_m)
    }

    /// Generate the height tile and wrap it in a record ready for a pack. A resolution a payload cannot have is an error.
    pub fn tile_record(&self, tile: TileId, res: u32) -> Result<Record, CodecError> {
        let data = generate_tile_field(self.field.as_ref(), &TangentWarp, tile, res);
        Ok(Record {
            key: self.tile_key(tile, res),
            kind: KIND_HEIGHT,
            tile,
            generator_version: self.field.version(),
            hash_version: HASH_VERSION,
            node_set_id: self.node_set_id,
            payload: encode_height_tile(&data)?,
        })
    }

    fn generator_version(&self) -> u32 {
        self.field.version()
    }
}

/// Every tile of one face at one level, in row order. A face above 5 or a level above [`MAX_REGION_LEVEL`] is refused.
pub fn region_tiles(face: Face, level: u8) -> Result<Vec<TileId>, String> {
    if face.0 > 5 {
        return Err(format!("face {} does not exist (faces are 0..=5)", face.0));
    }
    if level > MAX_REGION_LEVEL {
        return Err(format!("level {level} is too deep for a region (at most {MAX_REGION_LEVEL}: 4^{MAX_REGION_LEVEL} tiles)"));
    }
    let n = 1u32 << level;
    Ok((0..n).flat_map(|y| (0..n).map(move |x| TileId::new(face, level, x, y).expect("a tile of its own grid"))).collect())
}

/// Why a build failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// The resolution is not a power of two up to the codec's limit.
    Res(u32),
    /// The raw tiles would be larger than [`MAX_BUILD_BYTES`].
    TooLarge {
        tiles: usize,
        res: u32,
        bytes: u64,
    },
    Codec(CodecError),
    Pack(PackError),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Res(r) => write!(f, "{r} cells per edge is not a power of two up to {}", crate::codec::MAX_RES),
            BuildError::TooLarge { tiles, res, bytes } => write!(
                f,
                "{tiles} tiles of {res} cells per edge are {bytes} raw bytes, more than the {MAX_BUILD_BYTES} one build may produce: use a lower level or resolution"
            ),
            BuildError::Codec(e) => write!(f, "{e}"),
            BuildError::Pack(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl From<PackError> for BuildError {
    fn from(e: PackError) -> Self {
        BuildError::Pack(e)
    }
}

impl From<CodecError> for BuildError {
    fn from(e: CodecError) -> Self {
        BuildError::Codec(e)
    }
}

/// What a build did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildReport {
    pub written: usize,
    pub reused: usize,
}

/// Generate `tiles` at `res` into the store in `dir`. A tile the store already holds under the current key, and whose
/// payload decodes, is reused, not generated again, so a second run writes nothing.
pub fn build_tiles(
    dir: &std::path::Path,
    world: &World,
    tiles: &[TileId],
    res: u32,
    max_pack_bytes: u64,
) -> Result<BuildReport, BuildError> {
    if !valid_res(res) {
        return Err(BuildError::Res(res));
    }
    let bytes = (tiles.len() as u64).saturating_mul(4 * (u64::from(res) + 1) * (u64::from(res) + 1));
    if bytes > MAX_BUILD_BYTES {
        return Err(BuildError::TooLarge { tiles: tiles.len(), res, bytes });
    }
    let existing = TileStore::open(dir, world.params())?;
    let mut writer = PackWriter::open(dir, world.params(), max_pack_bytes)?;
    let mut report = BuildReport { written: 0, reused: 0 };
    for tile in tiles {
        let key = world.tile_key(*tile, res);
        if let Ok(Some(rec)) = existing.get(KIND_HEIGHT, *tile, key) {
            if decode_height_tile(*tile, &rec.payload).is_ok() {
                report.reused += 1;
                continue;
            }
        }
        writer.append(&world.tile_record(*tile, res)?)?;
        report.written += 1;
    }
    writer.finish()?;
    Ok(report)
}

/// One stored tile that differs from the definition.
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
    /// Metadata as the definition gives it and regenerated bit for bit equal to what is stored.
    pub matching: usize,
    /// Stored under a key the current definition no longer gives, and their versions or node set id differ from the
    /// current ones: explained, space to reclaim.
    pub stale: usize,
    /// Stored under another key but with every version and the node set id equal to the current ones: a node value
    /// changed, or the key function drifted. Not a failure by itself, but never folded into `stale`.
    pub unexplained_stale: usize,
    /// Entries of a kind this verifier does not regenerate.
    pub unsupported: usize,
    /// Stored under the current key but different from the definition (metadata or heights): an input is missing from the
    /// key, or the record was written wrongly.
    pub mismatches: Vec<Mismatch>,
    /// Entries that could not be read or decoded, and records the pack scan found unusable.
    pub unreadable: Vec<String>,
}

impl VerifyReport {
    /// Whether nothing differs and nothing is unreadable.
    pub fn clean(&self) -> bool {
        self.mismatches.is_empty() && self.unreadable.is_empty()
    }

    /// Whether entries were looked at but none could be compared with the definition, because all of them are stale.
    pub fn compared_nothing(&self) -> bool {
        self.stale + self.unexplained_stale > 0 && self.matching + self.mismatches.len() == 0
    }

    /// Whether the store passes: clean, and something was actually compared.
    pub fn passed(&self) -> bool {
        self.clean() && !self.compared_nothing()
    }

    /// Whether the store passes when stale entries are expected (the definition changed on purpose).
    pub fn passed_allowing_stale(&self) -> bool {
        self.clean()
    }
}

/// Verify `sample` evenly spaced entries of the store (all of them when `sample` is 0). Records the scan of the store found
/// unusable are always reported.
pub fn verify_store(store: &TileStore, world: &World, sample: usize) -> VerifyReport {
    let mut report = VerifyReport::default();
    report.unreadable.extend(store.damaged().iter().map(ToString::to_string));
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
        let where_ = format!("{}: tile {:#018x} key {} at offset {}", stored.pack, r.tile.raw(), r.key, stored.offset);
        // Decode first, whatever the key says: damage in a stale entry is still damage.
        let res = match payload_res(&r.payload) {
            Ok(res) => res,
            Err(e) => {
                report.unreadable.push(format!("{where_}: {e}"));
                continue;
            }
        };
        let kept = match decode_height_tile(r.tile, &r.payload) {
            Ok(t) => t,
            Err(e) => {
                report.unreadable.push(format!("{where_}: {e}"));
                continue;
            }
        };
        let metadata_current =
            r.generator_version == world.generator_version() && r.hash_version == HASH_VERSION && r.node_set_id == world.node_set_id;
        if world.tile_key(r.tile, res) != r.key {
            if metadata_current {
                report.unexplained_stale += 1;
            } else {
                report.stale += 1;
            }
            continue;
        }
        let mismatch = |detail: String| Mismatch { tile: r.tile, key: r.key, pack: stored.pack.clone(), offset: stored.offset, detail };
        if !metadata_current {
            report.mismatches.push(mismatch(format!(
                "the key is the definition's but the record disagrees with it: generator version {} (current {}), hash version {} (current {}), node set id {} (current {}): the record was written wrongly",
                r.generator_version,
                world.generator_version(),
                r.hash_version,
                HASH_VERSION,
                CacheKey(r.node_set_id),
                CacheKey(world.node_set_id)
            )));
            continue;
        }
        let fresh = generate_tile_field(world.field.as_ref(), &TangentWarp, r.tile, res);
        let differs = |i: &usize| fresh.heights[*i].to_bits() != kept.heights[*i].to_bits();
        match (0..fresh.heights.len()).position(|i| differs(&i)) {
            None => report.matching += 1,
            Some(i) => {
                let count = (0..fresh.heights.len()).filter(differs).count();
                let (k, l) = (i as u32 % (res + 1), i as u32 / (res + 1));
                report.mismatches.push(mismatch(format!(
                    "sample ({k}, {l}): stored {} (bits {:#010x}), regenerated {} (bits {:#010x}); {count} of {} samples differ: an input of this tile is missing from its key",
                    kept.heights[i],
                    kept.heights[i].to_bits(),
                    fresh.heights[i],
                    fresh.heights[i].to_bits(),
                    fresh.heights.len()
                )));
            }
        }
    }
    report
}
