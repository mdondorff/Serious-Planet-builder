//! `world-def`: the WorldDefinition and the read-recording generation context (CON-05, CON-06, ADR 0001, 0006, 0007).
//!
//! M3 starts small: the definition holds a world seed and the parameters of its bodies as **nodes**. A node is a named
//! value with a canonical encoding and a content hash that depends on neither insertion order nor platform (WDEF-003).
//! Generators read the definition only through a [`GenContext`], which records every read; the hash of the recorded
//! read set enters cache keys (WDEF-004). Authored items, TOML files and the cell index follow in later M3 changes.

pub mod toml_io;

use planet_core::hash::{hash_u64s, HASH_VERSION};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Identifies a body (planet or moon). `BodyId(0)` is the single-planet case (ADR 0001).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(pub u32);

impl BodyId {
    pub const PLANET: BodyId = BodyId(0);
}

/// One value of a definition: what a generator can read and what a read set records.
#[derive(Clone, Copy, Debug)]
pub enum NodeValue {
    U64(u64),
    F64(f64),
}

/// Equality is bit equality, the same relation the hash uses: `0.0` and `-0.0` differ, a NaN equals itself, so that
/// equal definitions and equal hashes always agree.
impl PartialEq for NodeValue {
    fn eq(&self, other: &NodeValue) -> bool {
        self.words() == other.words()
    }
}

impl NodeValue {
    /// Canonical encoding: a type tag and the exact bits (so `0.0` and `-0.0` differ, as their effects can).
    fn words(self) -> [u64; 2] {
        match self {
            NodeValue::U64(v) => [1, v],
            NodeValue::F64(v) => [2, v.to_bits()],
        }
    }
}

/// Path of a node, e.g. `world.seed` or `body.0.radius_m`.
pub type NodePath = String;

fn path_words(path: &str) -> Vec<u64> {
    path.bytes().map(u64::from).collect()
}

/// Content hash of one node: its path and value in canonical form, versioned with [`HASH_VERSION`].
pub fn node_hash(path: &str, value: NodeValue) -> u64 {
    let mut v = vec![u64::from(HASH_VERSION), hash_u64s(0x6e6f_6465, &path_words(path))];
    v.extend(value.words());
    hash_u64s(0x006e_6f64, &v)
}

/// Hash of a set of `(path, node hash)` entries, independent of the order they are given in.
pub fn set_hash(entries: &BTreeMap<NodePath, u64>) -> u64 {
    let mut v = vec![u64::from(HASH_VERSION), entries.len() as u64];
    for (path, h) in entries {
        v.push(hash_u64s(0x7061_7468, &path_words(path)));
        v.push(*h);
    }
    hash_u64s(0x0073_6574, &v)
}

/// The world: a seed and the parameters of its bodies, as a sorted set of nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldDefinition {
    nodes: BTreeMap<NodePath, NodeValue>,
}

impl WorldDefinition {
    pub const SEED_PATH: &'static str = "world.seed";

    /// No nodes at all (every read fails).
    pub fn empty() -> WorldDefinition {
        WorldDefinition { nodes: BTreeMap::new() }
    }

    pub fn new(world_seed: u64) -> WorldDefinition {
        let mut d = WorldDefinition { nodes: BTreeMap::new() };
        d.set(Self::SEED_PATH, NodeValue::U64(world_seed));
        d
    }

    /// One body, `BodyId(0)`, of the given radius.
    pub fn single_planet(radius_m: f64, world_seed: u64) -> WorldDefinition {
        let mut d = WorldDefinition::new(world_seed);
        d.set_body_radius(BodyId::PLANET, radius_m);
        d
    }

    /// Set a node. Paths under `derived.` are reserved for values computed from nodes (read sets record them) and
    /// panic here: a stored node must never share a path with a derived one.
    pub fn set(&mut self, path: &str, value: NodeValue) {
        assert!(!path.starts_with("derived."), "the path '{path}' is in the reserved derived namespace");
        self.nodes.insert(path.to_string(), value);
    }

    pub fn set_body_radius(&mut self, body: BodyId, radius_m: f64) {
        self.set(&body_radius_path(body), NodeValue::F64(radius_m));
    }

    /// Every node in path order.
    pub fn nodes(&self) -> impl Iterator<Item = (&str, NodeValue)> {
        self.nodes.iter().map(|(p, v)| (p.as_str(), *v))
    }

    pub fn get(&self, path: &str) -> Option<NodeValue> {
        self.nodes.get(path).copied()
    }

    /// Hash of every node (WDEF-003): equal for definitions with the same nodes however they were built.
    pub fn hash(&self) -> u64 {
        set_hash(&self.nodes.iter().map(|(p, v)| (p.clone(), node_hash(p, *v))).collect())
    }

    /// Hash of the procedural base: the definition without authored layers (none exist yet, so all nodes).
    pub fn base_hash(&self) -> u64 {
        self.hash()
    }
}

pub fn body_radius_path(body: BodyId) -> NodePath {
    format!("body.{}.radius_m", body.0)
}

/// Path under which the seed derived for a body is recorded in a read set. It lives in its own `derived.` namespace,
/// which definitions cannot set (see [`WorldDefinition::set`]), so it can never collide with a stored node.
pub fn body_seed_path(body: BodyId) -> NodePath {
    format!("derived.body.{}.seed", body.0)
}

/// A read that failed: the node is missing from the definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingNode(pub NodePath);

impl core::fmt::Display for MissingNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "the definition has no node '{}'", self.0)
    }
}

/// Id of the reference surface the geo conversions use (CON-15); the sphere is the only one so far.
pub const SPHERE_SURFACE_ID: &str = "sphere";

/// Hash of the parameters of a body that every key covers (ADR 0001, 0007): its id, its seed, its radius and the id of
/// its reference surface. Two bodies never share it, whatever their tile ids, and a change of any parameter changes it.
pub fn body_params_hash(body: BodyId, seed: u64, radius_m: f64, surface_id: &str) -> u64 {
    hash_u64s(0x626f_6479_7061, &[u64::from(body.0), seed, radius_m.to_bits(), hash_u64s(3, &path_words(surface_id))])
}

/// Seed of a body derived from the world seed (ADR 0001): the single planet uses the world seed itself, so existing
/// single-planet content keeps its identity; other bodies mix their id in.
pub fn derive_body_seed(world_seed: u64, body: BodyId) -> u64 {
    if body == BodyId::PLANET {
        world_seed
    } else {
        hash_u64s(0x626f_6479, &[world_seed, u64::from(body.0)])
    }
}

/// The only way generators read the definition. Every read is recorded with the hash of the node read; the recorded
/// set is what keys are built from, so an unread node cannot affect a key and a read node always does.
pub struct GenContext<'a> {
    def: &'a WorldDefinition,
    reads: RefCell<BTreeMap<NodePath, u64>>,
}

impl<'a> GenContext<'a> {
    pub fn new(def: &'a WorldDefinition) -> GenContext<'a> {
        GenContext { def, reads: RefCell::new(BTreeMap::new()) }
    }

    fn read(&self, path: &str) -> Result<NodeValue, MissingNode> {
        let v = self.def.get(path).ok_or_else(|| MissingNode(path.to_string()))?;
        self.reads.borrow_mut().insert(path.to_string(), node_hash(path, v));
        Ok(v)
    }

    pub fn world_seed(&self) -> Result<u64, MissingNode> {
        match self.read(WorldDefinition::SEED_PATH)? {
            NodeValue::U64(s) => Ok(s),
            NodeValue::F64(_) => Err(MissingNode(format!("{} (not an integer)", WorldDefinition::SEED_PATH))),
        }
    }

    /// Seed of a body: reads the world seed and records both it and the derived body seed, so the body's id is part of
    /// the read set and two bodies never share a key (ADR 0001).
    pub fn body_seed(&self, body: BodyId) -> Result<u64, MissingNode> {
        let seed = derive_body_seed(self.world_seed()?, body);
        let path = body_seed_path(body);
        let h = node_hash(&path, NodeValue::U64(seed));
        self.reads.borrow_mut().insert(path, h);
        Ok(seed)
    }

    pub fn body_radius_m(&self, body: BodyId) -> Result<f64, MissingNode> {
        match self.read(&body_radius_path(body))? {
            NodeValue::F64(r) => Ok(r),
            NodeValue::U64(_) => Err(MissingNode(format!("{} (not a number)", body_radius_path(body)))),
        }
    }

    /// Hash of everything read so far (WDEF-004): the definition part of a cache key.
    pub fn read_set_hash(&self) -> u64 {
        set_hash(&self.reads.borrow())
    }

    /// The recorded reads, for diagnostics and tests: path and node hash, sorted by path.
    pub fn reads(&self) -> Vec<(NodePath, u64)> {
        self.reads.borrow().iter().map(|(p, h)| (p.clone(), *h)).collect()
    }
}

/// The read-set hash of a generator that reads only the seed of the single planet. A field built directly from a seed
/// (tests, the headless tools) must key exactly as one built through a [`GenContext`] over a definition with that
/// world seed.
pub fn single_planet_seed_read_hash(seed: u64) -> u64 {
    let def = WorldDefinition::new(seed);
    let ctx = GenContext::new(&def);
    ctx.body_seed(BodyId::PLANET).expect("the seed node exists");
    ctx.read_set_hash()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: WDEF-003
    #[test]
    fn node_hashes_do_not_depend_on_insertion_order() {
        let mut a = WorldDefinition::new(7);
        a.set_body_radius(BodyId(0), 6_371_000.0);
        a.set_body_radius(BodyId(1), 1_737_400.0);
        let mut b = WorldDefinition::new(0);
        b.set_body_radius(BodyId(1), 1_737_400.0);
        b.set_body_radius(BodyId(0), 6_371_000.0);
        b.set(WorldDefinition::SEED_PATH, NodeValue::U64(7));
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a, b);
    }

    // spec: WDEF-003
    #[test]
    fn every_part_of_a_node_changes_its_hash() {
        let base = node_hash("body.0.radius_m", NodeValue::F64(1.0));
        assert_ne!(base, node_hash("body.1.radius_m", NodeValue::F64(1.0)));
        assert_ne!(base, node_hash("body.0.radius_m", NodeValue::F64(2.0)));
        assert_ne!(base, node_hash("body.0.radius_m", NodeValue::U64(1.0f64.to_bits())));
        assert_ne!(node_hash("x", NodeValue::F64(0.0)), node_hash("x", NodeValue::F64(-0.0)));
        // Known answer, tied to the hash version it was recorded with: identical on every platform.
        assert_eq!(HASH_VERSION, 1, "new known answers belong with a new hash version");
        assert_eq!(WorldDefinition::single_planet(6_371_000.0, 1).hash(), 6_345_583_285_605_900_286);
    }

    // spec: WDEF-004
    #[test]
    fn an_unread_node_cannot_change_the_read_set_and_a_read_node_always_does() {
        let mut def = WorldDefinition::single_planet(6_371_000.0, 5);
        let read = |d: &WorldDefinition| {
            let ctx = GenContext::new(d);
            ctx.body_seed(BodyId::PLANET).unwrap();
            ctx.read_set_hash()
        };
        let before = read(&def);
        def.set_body_radius(BodyId::PLANET, 1.0); // not read by the seed-only generator
        def.set("body.9.radius_m", NodeValue::F64(3.0));
        assert_eq!(read(&def), before);
        def.set(WorldDefinition::SEED_PATH, NodeValue::U64(6));
        assert_ne!(read(&def), before);
    }

    // spec: WDEF-004
    #[test]
    fn reads_are_recorded_by_path_and_missing_nodes_are_errors() {
        let def = WorldDefinition::single_planet(10.0, 3);
        let ctx = GenContext::new(&def);
        assert_eq!(ctx.read_set_hash(), set_hash(&BTreeMap::new()));
        assert_eq!(ctx.body_radius_m(BodyId::PLANET), Ok(10.0));
        assert_eq!(ctx.body_radius_m(BodyId(4)), Err(MissingNode("body.4.radius_m".into())));
        let reads = ctx.reads();
        assert_eq!(reads.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["body.0.radius_m"]);
        ctx.body_seed(BodyId(2)).unwrap();
        assert_eq!(
            ctx.reads().iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
            ["body.0.radius_m", "derived.body.2.seed", "world.seed"]
        );
    }

    // spec: WDEF-004
    #[test]
    fn a_direct_seed_keys_like_a_context_over_a_definition() {
        let def = WorldDefinition::single_planet(6_371_000.0, 42);
        let ctx = GenContext::new(&def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        assert_eq!(ctx.read_set_hash(), single_planet_seed_read_hash(42));
        assert_ne!(single_planet_seed_read_hash(42), single_planet_seed_read_hash(43));
    }

    // spec: WDEF-003
    #[test]
    fn equality_and_hash_agree_for_signed_zero_and_nan() {
        let (a, b) = (WorldDefinition::single_planet(0.0, 1), WorldDefinition::single_planet(-0.0, 1));
        assert_eq!(a == b, a.hash() == b.hash());
        let (n, m) = (WorldDefinition::single_planet(f64::NAN, 1), WorldDefinition::single_planet(f64::NAN, 1));
        assert!(n == m && n.hash() == m.hash());
    }

    // spec: WDEF-004
    #[test]
    #[should_panic(expected = "reserved derived namespace")]
    fn stored_nodes_cannot_use_the_derived_namespace() {
        WorldDefinition::new(1).set("derived.body.0.seed", NodeValue::U64(99));
    }

    // spec: WDEF-004
    #[test]
    fn body_seeds_derive_from_the_world_seed() {
        assert_eq!(derive_body_seed(9, BodyId::PLANET), 9);
        assert_ne!(derive_body_seed(9, BodyId(1)), derive_body_seed(9, BodyId(2)));
        assert_ne!(derive_body_seed(9, BodyId(1)), derive_body_seed(10, BodyId(1)));
    }
}
