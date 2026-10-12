//! The staged generation model (GEN-007, CON-08, ADR 0006, ADR 0021 point 6, ADR 0022).
//!
//! Generation runs in three stages, and a stage reads only the outputs of earlier ones:
//! 1. **global coarse**: once per definition, on the whole sphere at a coarse resolution (continents, climate), baked;
//! 2. **regional**: per tile with a halo, from global-coarse inputs, so tiles stay independent of each other;
//! 3. **local**: per point (detail noise, the erosion filter).
//!
//! A [`FieldGraph`] lists the fields with their stage, their inputs and their versions, and refuses a graph in which a
//! field reads one of its own stage or a later one, naming both stages. The key of a field's output chains the keys of
//! the outputs it reads (a Merkle chain), so bumping one field's version re-keys exactly its consumers and nothing else.
//!
//! The reads are *declared* here, and the graph checks the declaration; nothing yet stops an implementation from reading
//! something it did not declare (ADR 0021 point 7 asks for a contract test before the first real field).

use std::collections::BTreeMap;
use std::fmt;

use planet_core::hash::{hash_u64s, HASH_VERSION};
use planet_core::TileId;
use planet_world_def::BodyId;

use crate::height::fold_str;

/// Version of the stage key layout (which words enter [`FieldGraph::output_key`] and in which order). Separate from
/// the height and mesh key format so changing one never silently re-keys the other. History: 1 = first layout.
pub const STAGE_KEY_VERSION: u32 = 1;

/// The three stages, in the only order data may flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    GlobalCoarse,
    Regional,
    Local,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::GlobalCoarse => "global-coarse",
            Stage::Regional => "regional",
            Stage::Local => "local",
        }
    }

    /// The number the key sees: fixed here so reordering the variants cannot re-key every output.
    pub fn tag(self) -> u64 {
        match self {
            Stage::GlobalCoarse => 0,
            Stage::Regional => 1,
            Stage::Local => 2,
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What an output is for: the whole body (the global-coarse bake) or one tile (regional and local outputs). A tile id
/// of `0` is a real tile, so the kind is part of the key, not implied by a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Tile(TileId),
}

impl Scope {
    fn words(self) -> [u64; 2] {
        match self {
            Scope::Global => [0, 0],
            Scope::Tile(t) => [1, t.raw()],
        }
    }
}

/// The scope a stage's outputs have: the global bake is per body, regional and local outputs are per tile.
pub fn scope_fits(stage: Stage, scope: Scope) -> bool {
    matches!((stage, scope), (Stage::GlobalCoarse, Scope::Global) | (Stage::Regional | Stage::Local, Scope::Tile(_)))
}

/// One field layer: what it is, which stage produces it, what it reads, and the version of its algorithm.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSpec {
    pub id: &'static str,
    pub stage: Stage,
    /// Fields of strictly earlier stages this field reads, looked up by name (their order does not matter).
    pub reads: &'static [&'static str],
    /// Bumped whenever the field's output changes for the same inputs (ADR 0006).
    pub version: u32,
    /// The one implementation that produces this layer in a cache (CON-07).
    pub implementation_id: &'static str,
    /// How far beyond the tile edge a regional field computes its output, as a fraction of the tile edge (an *output*
    /// parameter, ADR 0006: another halo is another output); `0.0` for the other stages. A value of `-0.0` is read as
    /// `0.0`.
    pub halo_fraction: f64,
}

/// What every output key shares besides the field and its scope: the body, the recorded read set of the definition,
/// the face mapping the tiles were made with and the resolution of the output (ADR 0006 output parameters).
#[derive(Clone, Copy, Debug)]
pub struct OutputContext<'a> {
    pub body: BodyId,
    pub surface_id: &'a str,
    pub inputs_hash: u64,
    pub face_mapping_id: &'a str,
    /// Cells (intervals) per tile edge for a tile output, per face edge for the global bake.
    pub resolution: u32,
}

/// Why a field graph was refused. Every variant names the fields and stages involved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    DuplicateField {
        id: &'static str,
    },
    DuplicateRead {
        field: &'static str,
        reads: &'static str,
    },
    UnknownInput {
        field: &'static str,
        reads: &'static str,
    },
    /// A field read the output of its own stage or of a later one (CON-08).
    IllegalRead {
        field: &'static str,
        field_stage: Stage,
        reads: &'static str,
        reads_stage: Stage,
    },
    BadHalo {
        field: &'static str,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::DuplicateField { id } => write!(f, "field '{id}' is declared twice"),
            GraphError::DuplicateRead { field, reads } => write!(f, "field '{field}' lists its input '{reads}' twice"),
            GraphError::UnknownInput { field, reads } => write!(f, "field '{field}' reads '{reads}', which no stage produces"),
            GraphError::IllegalRead { field, field_stage, reads, reads_stage } => write!(
                f,
                "field '{field}' ({field_stage} stage) reads '{reads}' ({reads_stage} stage): a stage may read only the outputs of earlier stages"
            ),
            GraphError::BadHalo { field } => write!(f, "field '{field}': the halo must be a finite fraction in [0, 1] (and only regional fields have one)"),
        }
    }
}

impl std::error::Error for GraphError {}

/// A checked set of fields. Validity is by construction: a graph that exists has no illegal read and no cycle (strictly
/// increasing stages make a cycle impossible).
#[derive(Clone, Debug)]
pub struct FieldGraph {
    fields: BTreeMap<&'static str, FieldSpec>,
}

impl FieldGraph {
    pub fn new(specs: &[FieldSpec]) -> Result<FieldGraph, GraphError> {
        let mut fields = BTreeMap::new();
        for s in specs {
            let mut s = s.clone();
            let halo_ok = s.halo_fraction.is_finite() && (0.0..=1.0).contains(&s.halo_fraction);
            if !halo_ok || (s.stage != Stage::Regional && s.halo_fraction != 0.0) {
                return Err(GraphError::BadHalo { field: s.id });
            }
            // `-0.0` is `0.0` (the same output must not have two keys).
            s.halo_fraction += 0.0;
            for (i, r) in s.reads.iter().enumerate() {
                if s.reads[..i].contains(r) {
                    return Err(GraphError::DuplicateRead { field: s.id, reads: r });
                }
            }
            if fields.insert(s.id, s.clone()).is_some() {
                return Err(GraphError::DuplicateField { id: s.id });
            }
        }
        for s in fields.values() {
            for r in s.reads {
                let Some(input) = fields.get(r) else { return Err(GraphError::UnknownInput { field: s.id, reads: r }) };
                if input.stage >= s.stage {
                    return Err(GraphError::IllegalRead { field: s.id, field_stage: s.stage, reads: r, reads_stage: input.stage });
                }
            }
        }
        Ok(FieldGraph { fields })
    }

    pub fn field(&self, id: &str) -> Option<&FieldSpec> {
        self.fields.get(id)
    }

    /// Field ids in an order that lists every field after the ones it reads (by stage, then by id).
    pub fn order(&self) -> Vec<&'static str> {
        let mut v: Vec<&FieldSpec> = self.fields.values().collect();
        v.sort_by_key(|s| (s.stage, s.id));
        v.into_iter().map(|s| s.id).collect()
    }

    /// Every field that depends, directly or not, on `id`.
    pub fn consumers_of(&self, id: &str) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        let mut changed = true;
        while changed {
            changed = false;
            for s in self.fields.values() {
                let reads_it = s.reads.iter().any(|r| *r == id || out.contains(r));
                if reads_it && !out.contains(&s.id) {
                    out.push(s.id);
                    changed = true;
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The key of the output of field `id` for `scope` (checked against the field's stage: the global bake is keyed with
    /// [`Scope::Global`], regional and local outputs with their tile) in the context `ctx`. It chains the keys of the
    /// field's inputs, which the caller passes by field id (`upstream`).
    ///
    /// The read set of the definition (`ctx.inputs_hash`, WDEF-004) already covers the cells a field queries, including
    /// the margin it asked for (ADR 0021): another margin records other reads. An input missing from `upstream`, an
    /// unknown field and a scope of the wrong kind are bugs and panic, because a key that silently left something out
    /// would serve stale data or bake a global field once per tile.
    pub fn output_key(&self, id: &str, scope: Scope, ctx: &OutputContext, upstream: &BTreeMap<&str, u64>) -> u64 {
        let s = self.fields.get(id).unwrap_or_else(|| panic!("no field '{id}' in the graph"));
        assert!(scope_fits(s.stage, scope), "field '{id}' ({} stage) cannot be keyed with {scope:?}", s.stage);
        let [scope_kind, scope_value] = scope.words();
        let mut v = vec![
            u64::from(STAGE_KEY_VERSION),
            u64::from(HASH_VERSION),
            u64::from(s.version),
            s.stage.tag(),
            hash_u64s(1, &fold_str(s.id)),
            hash_u64s(2, &fold_str(s.implementation_id)),
            s.halo_fraction.to_bits(),
            scope_kind,
            scope_value,
            u64::from(ctx.body.0),
            hash_u64s(3, &fold_str(ctx.surface_id)),
            hash_u64s(4, &fold_str(ctx.face_mapping_id)),
            u64::from(ctx.resolution),
            ctx.inputs_hash,
            s.reads.len() as u64,
        ];
        let mut reads: Vec<&str> = s.reads.to_vec();
        reads.sort_unstable();
        for r in reads {
            let key = upstream.get(r).unwrap_or_else(|| panic!("the key of input '{r}' of field '{id}' was not supplied"));
            v.push(hash_u64s(5, &fold_str(r)));
            v.push(*key);
        }
        hash_u64s(0x0073_7467, &v)
    }

    /// Keys of every field in stage order, each chained to the keys of its inputs: global fields keyed [`Scope::Global`],
    /// the others with `tile`, all in the one context `ctx` (a real pipeline gives each stage the resolution of its own
    /// output; this is for tests and for pipelines whose stages share it).
    pub fn all_keys(&self, tile: TileId, ctx: &OutputContext) -> BTreeMap<&'static str, u64> {
        let mut keys: BTreeMap<&'static str, u64> = BTreeMap::new();
        for id in self.order() {
            let s = &self.fields[id];
            let scope = if s.stage == Stage::GlobalCoarse { Scope::Global } else { Scope::Tile(tile) };
            let up: BTreeMap<&str, u64> = s.reads.iter().map(|r| (*r, keys[r])).collect();
            keys.insert(id, self.output_key(id, scope, ctx, &up));
        }
        keys
    }
}
