//! The staged generation model (GEN-007, CON-08, ADR 0006, ADR 0021 point 6).
//!
//! Generation runs in three stages, and a stage reads only the outputs of earlier ones:
//! 1. **global coarse**: once per definition, on the whole sphere at a coarse resolution (continents, climate), baked;
//! 2. **regional**: per tile with a halo, from global-coarse inputs, so tiles stay independent of each other;
//! 3. **local**: per point (detail noise, the erosion filter).
//!
//! A [`FieldGraph`] lists the fields with their stage, their inputs and their versions, and refuses a graph in which a
//! field reads one of its own stage or a later one, naming both stages. The key of a field's output chains the keys of
//! the outputs it reads (a Merkle chain), so bumping one field's version re-keys exactly its consumers and nothing else.

use std::collections::BTreeMap;
use std::fmt;

use planet_core::hash::{hash_u64s, HASH_VERSION};

use crate::height::{fold_str, KEY_FORMAT_VERSION};

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
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One field layer: what it is, which stage produces it, what it reads, and the version of its algorithm.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSpec {
    pub id: &'static str,
    pub stage: Stage,
    /// Fields of strictly earlier stages this field reads.
    pub reads: &'static [&'static str],
    /// Bumped whenever the field's output changes for the same inputs (ADR 0006).
    pub version: u32,
    /// The one implementation that produces this layer in a cache (CON-07).
    pub implementation_id: &'static str,
    /// Halo around a tile, as a fraction of the tile edge, that a regional field needs from its inputs; `0.0` for the
    /// others. It is an output parameter of the key (ADR 0006): another halo is another output.
    pub halo_fraction: f64,
}

/// Why a field graph was refused. Every variant names the fields and stages involved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    DuplicateField {
        id: &'static str,
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
            if fields.insert(s.id, s.clone()).is_some() {
                return Err(GraphError::DuplicateField { id: s.id });
            }
            let halo_ok = s.halo_fraction.is_finite() && (0.0..=1.0).contains(&s.halo_fraction);
            if !halo_ok || (s.stage != Stage::Regional && s.halo_fraction != 0.0) {
                return Err(GraphError::BadHalo { field: s.id });
            }
        }
        for s in specs {
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

    /// The key of the output of field `id` for `scope` (a tile id, a region id or `0` for a global field) and the
    /// recorded read set of the definition (`inputs_hash`, WDEF-004). It chains the keys of the field's inputs, which the
    /// caller passes by field id (`upstream`); an input that is missing from `upstream` is a bug and panics, because a
    /// key that silently left an input out would serve stale data.
    pub fn output_key(&self, id: &str, scope: u64, inputs_hash: u64, upstream: &BTreeMap<&str, u64>) -> u64 {
        let s = self.fields.get(id).unwrap_or_else(|| panic!("no field '{id}' in the graph"));
        let mut v = vec![
            u64::from(KEY_FORMAT_VERSION),
            u64::from(HASH_VERSION),
            u64::from(s.version),
            s.stage as u64,
            hash_u64s(1, &fold_str(s.id)),
            hash_u64s(2, &fold_str(s.implementation_id)),
            s.halo_fraction.to_bits(),
            scope,
            inputs_hash,
            s.reads.len() as u64,
        ];
        let mut reads: Vec<&str> = s.reads.to_vec();
        reads.sort_unstable();
        for r in reads {
            let key = upstream.get(r).unwrap_or_else(|| panic!("the key of input '{r}' of field '{id}' was not supplied"));
            v.push(hash_u64s(3, &fold_str(r)));
            v.push(*key);
        }
        hash_u64s(0x0073_7467, &v)
    }

    /// Keys of every field for one `scope` and read set, each chained to the keys of its inputs in stage order. Global
    /// and regional inputs of a local field share the `scope` here, which is how the tests exercise the chain; a real
    /// stage keys its inputs with their own scope (the global bake with `0`, a tile with its tile id).
    pub fn all_keys(&self, scope: u64, inputs_hash: u64) -> BTreeMap<&'static str, u64> {
        let mut keys: BTreeMap<&'static str, u64> = BTreeMap::new();
        for id in self.order() {
            let up: BTreeMap<&str, u64> = self.fields[id].reads.iter().map(|r| (*r, keys[r])).collect();
            let key = self.output_key(id, scope, inputs_hash, &up);
            keys.insert(id, key);
        }
        keys
    }
}
