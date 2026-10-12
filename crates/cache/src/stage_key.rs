//! Persistent keys of staged field outputs (STRM-011, ADR 0022, ADR 0023 point 10, ADR 0026 point 6).
//!
//! The 64-bit keys of `generators::stage` chain the outputs of a field graph in memory. An output that is stored gets this
//! 128-bit form: the first 16 bytes of BLAKE3 in `derive_key` mode (its own context, so a stage key can never equal a
//! height or mesh key) over a canonical encoding with its own format tag. A stage output's key covers the field (id,
//! version, implementation id, stage, halo), the scope (the whole body, or one tile), the body (id and the 128-bit hash of
//! its parameters), the face mapping, the output resolution, the read set of the definition as entries, and the 128-bit
//! keys of the outputs of the fields it reads, so a version bump of one field re-keys exactly its consumers.

use std::collections::BTreeMap;

use planet_core::hash::HASH_VERSION;
use planet_generators::stage::{scope_fits, FieldGraph, Scope};
use planet_world_def::BodyId;

use crate::key::{derive, len32, put_str, CacheKey, ReadEntry};

/// Version of the stage key encoding. Changing the encoding bumps this and invalidates every stored stage output; it
/// never aliases one. History: 1 = first layout.
pub const STAGE_KEY_FORMAT_TAG: u8 = 1;

const STAGE_KEY_CONTEXT: &str = "planet-builder stage key v1";

/// What every output key shares besides the field and its scope.
#[derive(Clone, Debug)]
pub struct StageKeyContext<'a> {
    pub body: BodyId,
    /// `body_params_hash128` of the body (id, seed, radius, reference surface).
    pub body_params: [u8; 16],
    pub face_mapping_id: &'a str,
    /// Cells (intervals) per tile edge for a tile output, per face edge for the global bake.
    pub resolution: u32,
    /// The recorded reads of the definition that the output depends on (nodes, cell queries); order and duplicates do not
    /// matter.
    pub reads: &'a [ReadEntry],
}

/// The canonical bytes of the key of field `id` of `graph` for `scope`. The keys of its inputs (`upstream`, by field id)
/// must all be supplied: a missing one is a bug and panics, as does a scope of the wrong kind for the field's stage, an
/// unknown field and an unused extra input would otherwise key silently wrong.
pub fn encode_stage_key(graph: &FieldGraph, id: &str, scope: Scope, ctx: &StageKeyContext, upstream: &BTreeMap<&str, CacheKey>) -> Vec<u8> {
    let s = graph.field(id).unwrap_or_else(|| panic!("no field '{id}' in the graph"));
    assert!(scope_fits(s.stage, scope), "field '{id}' ({} stage) cannot be keyed with {scope:?}", s.stage);
    let mut out = vec![STAGE_KEY_FORMAT_TAG];
    out.extend_from_slice(&HASH_VERSION.to_le_bytes());
    out.extend_from_slice(&s.version.to_le_bytes());
    out.push(s.stage.tag() as u8);
    put_str(&mut out, s.id);
    put_str(&mut out, s.implementation_id);
    // The graph reads `-0.0` as `0.0`; the key sees the same bits for both.
    out.extend_from_slice(&(s.halo_fraction + 0.0).to_bits().to_le_bytes());
    match scope {
        Scope::Global => {
            out.push(0);
            out.extend_from_slice(&[0; 8]);
        }
        Scope::Tile(t) => {
            out.push(1);
            out.extend_from_slice(&t.raw().to_le_bytes());
        }
    }
    out.extend_from_slice(&ctx.body.0.to_le_bytes());
    out.extend_from_slice(&ctx.body_params);
    put_str(&mut out, ctx.face_mapping_id);
    out.extend_from_slice(&ctx.resolution.to_le_bytes());

    // Reads as in the height key: nodes by path, then queries by (kind, parameters); duplicates collapse.
    let mut nodes: Vec<(&str, &[u8; 16])> = Vec::new();
    let mut queries: Vec<(u8, &[u8], &[u8; 16])> = Vec::new();
    for r in ctx.reads {
        match r {
            ReadEntry::Node { path, hash } => nodes.push((path, hash)),
            ReadEntry::Query { kind, params, result } => queries.push((*kind, params, result)),
        }
    }
    nodes.sort_unstable();
    nodes.dedup();
    queries.sort_unstable();
    queries.dedup();
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

    // The inputs, by field id: the id, then the 128-bit key of its output.
    let mut reads: Vec<&str> = s.reads.to_vec();
    reads.sort_unstable();
    out.extend_from_slice(&len32(reads.len()));
    for r in reads {
        let key = upstream.get(r).unwrap_or_else(|| panic!("the key of input '{r}' of field '{id}' was not supplied"));
        put_str(&mut out, r);
        out.extend_from_slice(&key.0);
    }
    assert!(
        upstream.keys().all(|k| s.reads.contains(k)),
        "keys were supplied for fields that '{id}' does not read: {:?}",
        upstream.keys().filter(|k| !s.reads.contains(k)).collect::<Vec<_>>()
    );
    out
}

/// The 128-bit key of field `id` of `graph` for `scope` (see [`encode_stage_key`]).
pub fn stage_key(graph: &FieldGraph, id: &str, scope: Scope, ctx: &StageKeyContext, upstream: &BTreeMap<&str, CacheKey>) -> CacheKey {
    CacheKey(derive(STAGE_KEY_CONTEXT, &encode_stage_key(graph, id, scope, ctx, upstream)))
}
