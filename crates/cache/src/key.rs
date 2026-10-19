//! Persistent cache keys (ADR 0018, STRM-003): 128 bits of BLAKE3 over a canonical byte encoding.
//!
//! The in-memory keys of `generators` are 64-bit SplitMix chains: fine for maps, not for anything stored or shared. A
//! key here is the first 16 bytes of BLAKE3 in `derive_key` mode over an encoding in which every variable part is
//! length-prefixed and every list is preceded by its count, so no two different inputs share an encoding. Every
//! hash that feeds a key (node, body parameters, query result) has its own `derive_key` context, so a hash of one use
//! can never equal a hash of another.

use std::collections::BTreeMap;
use std::fmt;

use planet_core::{FaceMapping, TileId};
use planet_generators::height::HeightField;
use planet_generators::mesh::{MESH_VERSION, SKIRT_FRACTION};
use planet_world_def::cells::ItemEntry;
use planet_world_def::footprint::FootprintItem;
use planet_world_def::{BodyId, NodeValue, ReadRecord};

/// Version of the encoding. Changing the encoding bumps this and invalidates every stored entry; it never aliases one.
/// History: 1 = first layout; 2 = a presence byte before the source key (a missing source and an all-zero key differ).
pub const KEY_FORMAT_TAG: u8 = 2;

const KEY_CONTEXT: &str = "planet-builder cache key v1";
const NODE_CONTEXT: &str = "planet-builder node hash v1";
const BODY_CONTEXT: &str = "planet-builder body hash v1";
const QUERY_CONTEXT: &str = "planet-builder query result v1";
const FOOTPRINT_CONTEXT: &str = "planet-builder footprint result v1";

/// The tag of a key that carries a footprint (kind 3) or a region summary (kind 4) entry (ADR 0066 point 2). Keys without
/// such an entry keep [`KEY_FORMAT_TAG`] and stay byte for byte what they were, so no stored entry is invalidated; the tag
/// byte differs, so the two layouts never alias.
pub const KEY_FORMAT_TAG_FOOTPRINT: u8 = 3;

/// A 128-bit content key, written as 32 lowercase hex characters in file names and logs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey(pub [u8; 16]);

impl CacheKey {
    pub const NONE: CacheKey = CacheKey([0; 16]);

    pub fn to_hex(self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Parse 32 lowercase hex characters.
    pub fn from_hex(s: &str) -> Option<CacheKey> {
        if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        let mut out = [0u8; 16];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(CacheKey(out))
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CacheKey({})", self.to_hex())
    }
}

/// What a key is for. The number is part of the encoding and never changes meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Height = 1,
    Mesh = 2,
    /// A block of the global-coarse bake (ADR 0026 point 6).
    GlobalBlock = 3,
    /// A block of the ocean-connectivity channel of the global bake (ADR 0041 point 2): one bit per vertex.
    GlobalOcean = 4,
    /// The coarse level of the water field: the summary of a global bake (ADR 0058), one record per bake.
    GlobalWaterSummary = 5,
}

/// One entry of a read set, in its 128-bit form (ADR 0018).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadEntry {
    /// `0x01 | path | node content hash`.
    Node { path: String, hash: [u8; 16] },
    /// `0x02 | query kind | canonical parameters | result hash`; an empty result still has an entry.
    Query { kind: u8, params: Vec<u8>, result: [u8; 16] },
}

/// Query kinds. A cell query is parameterised by body and cell; a whole-body read by the body alone.
pub const QUERY_CELL: u8 = 1;
pub const QUERY_BODY: u8 = 2;
/// The items of one layer whose reach touches a tile (WDEF-007): body, layer, tile and halo.
pub const QUERY_FOOTPRINT: u8 = 3;
/// One node of the region summary tree a coarse tile reads (WDEF-008, ADR 0066 point 3): a key carrying one has
/// [`KEY_FORMAT_TAG_FOOTPRINT`] too.
pub const QUERY_SUMMARY: u8 = 4;

/// Everything a key covers (ADR 0018).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInputs {
    pub kind: Kind,
    pub hash_version: u32,
    pub generator_version: u32,
    pub implementation_id: String,
    pub face_mapping_id: String,
    pub body: BodyId,
    pub body_params: [u8; 16],
    pub tile: TileId,
    /// The key of the artefact this one is derived from (a mesh from its heights), if any.
    pub source: Option<CacheKey>,
    /// Output parameters by tag; the same set in another order gives the same key.
    pub output_params: BTreeMap<u8, Vec<u8>>,
    pub reads: Vec<ReadEntry>,
}

/// A length that must fit the encoding's `u32` prefix; a longer part would alias another input, so it fails loudly.
pub(crate) fn len32(len: usize) -> [u8; 4] {
    u32::try_from(len).expect("a key part longer than u32::MAX bytes").to_le_bytes()
}

pub(crate) fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&len32(s.len()));
    out.extend_from_slice(s.as_bytes());
}

pub(crate) fn derive(context: &str, material: &[u8]) -> [u8; 16] {
    let full = blake3::derive_key(context, material);
    full[..16].try_into().expect("16 of 32 bytes")
}

/// 128-bit content hash of a node: its path and value with the exact bits (so `0.0` and `-0.0` differ).
pub fn node_hash128(path: &str, value: NodeValue) -> [u8; 16] {
    let mut m = Vec::new();
    put_str(&mut m, path);
    match value {
        NodeValue::U64(v) => {
            m.push(1);
            m.extend_from_slice(&v.to_le_bytes());
        }
        NodeValue::F64(v) => {
            m.push(2);
            m.extend_from_slice(&v.to_bits().to_le_bytes());
        }
    }
    derive(NODE_CONTEXT, &m)
}

/// 128-bit hash of the parameters of a body that every key covers: id, seed, radius bits and reference-surface id.
pub fn body_params_hash128(body: BodyId, seed: u64, radius_m: f64, surface_id: &str) -> [u8; 16] {
    let mut m = Vec::new();
    m.extend_from_slice(&body.0.to_le_bytes());
    m.extend_from_slice(&seed.to_le_bytes());
    m.extend_from_slice(&radius_m.to_bits().to_le_bytes());
    put_str(&mut m, surface_id);
    derive(BODY_CONTEXT, &m)
}

/// 128-bit hash of a query result: the matching items in id order, each as id, content hash and direction bits. An empty
/// result is the hash of the empty input, so a query that found nothing is still a read whose result can change.
pub fn query_result_hash128(items: &[ItemEntry]) -> [u8; 16] {
    let mut sorted: Vec<&ItemEntry> = items.iter().collect();
    sorted.sort_unstable();
    let mut m = Vec::with_capacity(sorted.len() * 48);
    // Rows of 40 bytes: the low 8 bytes of the id and of the content hash (the whole of them for values below `2^64`, so those keys are
    // what they were when both were 64 bits) and the direction bits; the high bytes of wider values follow in a tail.
    for (id, content, dir) in &sorted {
        m.extend_from_slice(&planet_world_def::item_id::lo(*id).to_le_bytes());
        m.extend_from_slice(&planet_world_def::item_id::lo(*content).to_le_bytes());
        for d in dir {
            m.extend_from_slice(&d.to_le_bytes());
        }
    }
    m.extend(planet_world_def::item_id::wide_tail_bytes(sorted.iter().map(|(id, content, _)| (*id, *content))));
    derive(QUERY_CONTEXT, &m)
}

/// 128-bit hash of one footprint item (`planet-builder item hash v1`): the layer, the priority, the content hash, the exact
/// bits of the anchor, the reach and the height bound. The anchor is inside it, so the result needs no separate direction.
pub fn footprint_item_hash128(item: &FootprintItem) -> [u8; 16] {
    planet_world_def::footprint::item_hash128(item)
}

/// The 16-byte id of an item, big-endian, so the byte order is the numeric order (an id below `2^64` is zero-extended).
pub fn footprint_id_bytes(id: planet_world_def::ItemId) -> [u8; 16] {
    planet_world_def::footprint::id_bytes(id)
}

/// 128-bit hash of a footprint result: the items in application order (priority, then id big-endian), each as its 16-byte id
/// and its 128-bit item hash. An empty result is the hash of the empty input.
pub fn footprint_result_hash128(items: &[FootprintItem]) -> [u8; 16] {
    let mut sorted: Vec<&FootprintItem> = items.iter().collect();
    sorted.sort_by_key(|i| (i.priority, footprint_id_bytes(i.id)));
    let mut m = Vec::with_capacity(sorted.len() * 32);
    for i in sorted {
        m.extend_from_slice(&footprint_id_bytes(i.id));
        m.extend_from_slice(&footprint_item_hash128(i));
    }
    derive(FOOTPRINT_CONTEXT, &m)
}

/// The canonical parameters of a footprint query: `body u32 | layer (u32 length + UTF-8) | tile raw id u64 | halo bits`.
pub fn footprint_params(body: BodyId, layer: &str, tile: TileId, halo_bits: u64) -> Vec<u8> {
    let mut p = Vec::with_capacity(24 + layer.len());
    p.extend_from_slice(&body.0.to_le_bytes());
    put_str(&mut p, layer);
    p.extend_from_slice(&tile.raw().to_le_bytes());
    p.extend_from_slice(&halo_bits.to_le_bytes());
    p
}

/// The canonical parameters of a region summary entry: `body u32 | layer (u32 length + UTF-8) | level u8 | node raw id u64`.
pub fn summary_params(body: BodyId, layer: &str, node: TileId) -> Vec<u8> {
    let mut p = Vec::with_capacity(17 + layer.len());
    p.extend_from_slice(&body.0.to_le_bytes());
    put_str(&mut p, layer);
    p.push(node.level());
    p.extend_from_slice(&node.raw().to_le_bytes());
    p
}

/// The 128-bit entry for one recorded read.
pub fn read_entry(record: &ReadRecord) -> ReadEntry {
    match record {
        ReadRecord::Node { path, value } => ReadEntry::Node { path: path.clone(), hash: node_hash128(path, *value) },
        ReadRecord::Cell { body, cell, items } => {
            let mut params = Vec::with_capacity(12);
            params.extend_from_slice(&body.0.to_le_bytes());
            params.extend_from_slice(&cell.raw().to_le_bytes());
            ReadEntry::Query { kind: QUERY_CELL, params, result: query_result_hash128(items) }
        }
        ReadRecord::Body { body, items } => {
            ReadEntry::Query { kind: QUERY_BODY, params: body.0.to_le_bytes().to_vec(), result: query_result_hash128(items) }
        }
        ReadRecord::SummaryNode { body, layer, node, hash } => {
            ReadEntry::Query { kind: QUERY_SUMMARY, params: summary_params(*body, layer, *node), result: *hash }
        }
        ReadRecord::Footprint { body, layer, tile, halo_bits, items } => ReadEntry::Query {
            kind: QUERY_FOOTPRINT,
            params: footprint_params(*body, layer, *tile, *halo_bits),
            result: footprint_result_hash128(items),
        },
    }
}

/// The canonical bytes of a key's inputs (what the key is a hash of).
pub fn encode(inputs: &KeyInputs) -> Vec<u8> {
    let footprint = inputs.reads.iter().any(|r| matches!(r, ReadEntry::Query { kind: QUERY_FOOTPRINT | QUERY_SUMMARY, .. }));
    let mut out = vec![if footprint { KEY_FORMAT_TAG_FOOTPRINT } else { KEY_FORMAT_TAG }, inputs.kind as u8];
    out.extend_from_slice(&inputs.hash_version.to_le_bytes());
    out.extend_from_slice(&inputs.generator_version.to_le_bytes());
    put_str(&mut out, &inputs.implementation_id);
    put_str(&mut out, &inputs.face_mapping_id);
    out.extend_from_slice(&inputs.body.0.to_le_bytes());
    out.extend_from_slice(&inputs.body_params);
    out.extend_from_slice(&inputs.tile.raw().to_le_bytes());
    match inputs.source {
        Some(k) => {
            out.push(1);
            out.extend_from_slice(&k.0);
        }
        None => {
            out.push(0);
            out.extend_from_slice(&[0; 16]);
        }
    }
    out.extend_from_slice(&len32(inputs.output_params.len()));
    for (tag, bytes) in &inputs.output_params {
        out.push(*tag);
        out.extend_from_slice(&len32(bytes.len()));
        out.extend_from_slice(bytes);
    }
    // Nodes first, sorted by path bytes; then queries, sorted by (kind, parameter bytes); duplicates collapse.
    let mut nodes: Vec<(&str, &[u8; 16])> = Vec::new();
    let mut queries: Vec<(u8, &[u8], &[u8; 16])> = Vec::new();
    for r in &inputs.reads {
        match r {
            ReadEntry::Node { path, hash } => nodes.push((path, hash)),
            ReadEntry::Query { kind, params, result } => queries.push((*kind, params, result)),
        }
    }
    nodes.sort_unstable();
    nodes.dedup();
    queries.sort_unstable();
    queries.dedup();
    debug_assert!(nodes.windows(2).all(|w| w[0].0 != w[1].0), "one node path read with two different hashes");
    out.extend_from_slice(&len32(nodes.len() + queries.len()));
    for (path, hash) in nodes {
        out.push(0x01);
        put_str(&mut out, path);
        out.extend_from_slice(hash);
    }
    for (kind, params, result) in queries {
        out.push(0x02);
        out.push(kind);
        out.extend_from_slice(&len32(params.len()));
        out.extend_from_slice(params);
        out.extend_from_slice(result);
    }
    out
}

/// The key of an artefact: the first 16 bytes of BLAKE3 in `derive_key` mode over [`encode`].
pub fn cache_key(inputs: &KeyInputs) -> CacheKey {
    CacheKey(derive(KEY_CONTEXT, &encode(inputs)))
}

/// Output parameter tags.
pub const PARAM_RESOLUTION: u8 = 1;
pub const PARAM_SKIRT_FRACTION: u8 = 2;
pub const PARAM_MESH_VERSION: u8 = 3;
/// Cells per face edge of a stored global bake (a block's own `PARAM_RESOLUTION` is its cells per block edge, ADR 0026 point 6).
pub const PARAM_GRID_RES: u8 = 4;
/// The climate version word of a mesh key ([`HeightField::climate_version_word`]); tag 5 is the form of a pre-filled tile.
pub const PARAM_CLIMATE_VERSION: u8 = 6;
/// The area threshold of an ocean block (the bits of the `f64` fraction of the body's surface, ADR 0041 point 2).
pub const PARAM_OCEAN_AREA_FRACTION: u8 = 7;
/// The layer name of an authored stage (UTF-8 bytes) and its index in the stack of ADR 0007 (`u32` little endian), STRM-027.
pub const PARAM_STAGE_LAYER: u8 = 8;
pub const PARAM_STAGE_INDEX: u8 = 9;

fn field_inputs(kind: Kind, field: &dyn HeightField, map: &dyn FaceMapping, tile: TileId, radius_m: f64) -> KeyInputs {
    assert!(
        radius_m.is_finite() && radius_m > 0.0,
        "the body radius of a key must be the finite positive radius of the definition, got {radius_m}"
    );
    let reads: Vec<ReadEntry> = field.read_records_for(tile, map).iter().map(read_entry).collect();
    assert!(
        !field.reads_cells() || reads.iter().any(|r| matches!(r, ReadEntry::Query { .. })),
        "a field that reads cells must report its cell queries in read_records_for"
    );
    let mut output_params = BTreeMap::new();
    if let Some((layer, index)) = field.stage_params() {
        output_params.insert(PARAM_STAGE_LAYER, layer.as_bytes().to_vec());
        output_params.insert(PARAM_STAGE_INDEX, index.to_le_bytes().to_vec());
    }
    KeyInputs {
        kind,
        hash_version: planet_core::hash::HASH_VERSION,
        generator_version: field.version(),
        implementation_id: field.implementation_id().to_string(),
        face_mapping_id: map.id().to_string(),
        body: field.body(),
        body_params: body_params_hash128(field.body(), field.seed(), radius_m, field.surface_id()),
        tile,
        // A field built over a baked input names that bake's stage key as its source (ADR 0028 point 5); a mesh key replaces it with
        // its height key, which already carries it.
        source: field.source_key().map(CacheKey),
        output_params,
        reads,
    }
}

/// The persistent key of the height tile of `field` at `tile` with `res` cells (intervals) per edge, so `(res + 1)^2`
/// samples: the unit of `TileData::res` and of the mesh's `cells`, which is why a mesh key chains the height key of the
/// same number. `radius_m` is the body's
/// radius: heights of the reference generator do not depend on it, but the body parameter hash covers it (ADR 0018).
pub fn height_key(field: &dyn HeightField, map: &dyn FaceMapping, tile: TileId, res: u32, radius_m: f64) -> CacheKey {
    // A stage that is the identity on its source for this tile keys as its source (STRM-029).
    let field = planet_generators::height::key_source(field, tile, map);
    let mut inputs = field_inputs(Kind::Height, field, map, tile, radius_m);
    inputs.output_params.insert(PARAM_RESOLUTION, res.to_le_bytes().to_vec());
    // An authored stage chains the key of the same tile of the stage below, at the same resolution (STRM-027); the base's own key
    // carries a bake's stage key, so a stage has no field-wide source of its own.
    if let Some(upstream) = field.upstream() {
        inputs.source = Some(height_key(upstream, map, tile, res, radius_m));
    }
    cache_key(&inputs)
}

/// The inputs of the persistent key of the mesh of that tile: chained to the height key of the same tile at the same number
/// of cells (`source`), with the grid size, the skirt fraction and the mesh version as output parameters.
pub fn mesh_key_inputs(field: &dyn HeightField, map: &dyn FaceMapping, tile: TileId, cells: u32, radius_m: f64) -> KeyInputs {
    let field = planet_generators::height::key_source(field, tile, map);
    let mut inputs = field_inputs(Kind::Mesh, field, map, tile, radius_m);
    inputs.source = Some(height_key(field, map, tile, cells, radius_m));
    inputs.output_params.insert(PARAM_RESOLUTION, cells.to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_SKIRT_FRACTION, SKIRT_FRACTION.to_bits().to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_MESH_VERSION, MESH_VERSION.to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_CLIMATE_VERSION, field.climate_version_word().to_le_bytes().to_vec());
    inputs
}

/// The persistent key of the mesh of that tile (see [`mesh_key_inputs`]).
pub fn mesh_key(field: &dyn HeightField, map: &dyn FaceMapping, tile: TileId, cells: u32, radius_m: f64) -> CacheKey {
    cache_key(&mesh_key_inputs(field, map, tile, cells, radius_m))
}
