//! `world-def`: the WorldDefinition and the read-recording generation context (CON-05, CON-06, ADR 0001, 0006, 0007).
//!
//! M3 starts small: the definition holds a world seed and the parameters of its bodies as **nodes**. A node is a named
//! value with a canonical encoding and a content hash that depends on neither insertion order nor platform (WDEF-003).
//! Generators read the definition only through a [`GenContext`], which records every read; the hash of the recorded
//! read set enters cache keys (WDEF-004). Authored items are filed in spatial cells (`cells`, WDEF-005); definitions
//! are read and written as TOML (`toml_io`, WDEF-002).

pub mod cells;
pub mod footprint;
pub mod toml_io;
pub mod water;

pub use water::{
    body_sea_level_path, body_sea_liquid_path, body_sea_min_area_path, liquid_path, LiquidKind, LiquidKindId, LiquidTable, SeaParams,
    DEFAULT_SEA_LEVEL_M, EARTH_OCEAN_MIN_AREA_FRACTION, LIQUID_FIELDS,
};

use planet_core::hash::{hash_u64s, HASH_VERSION};
use planet_core::{TileId, Vec3};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

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

/// An authored item as the index keeps it: the opaque content hash (M6 defines payloads) and the bits of its unit
/// direction, which the cell of the item is derived from whenever the body's radius changes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ItemRec {
    content: u64,
    dir: [u64; 3],
    /// Bits of `reach_m` and `max_delta_m` (`-0.0` stored as `0.0`).
    reach_bits: u64,
    max_delta_bits: u64,
    layer: String,
    priority: i32,
}

/// The layer and the numbers of an item put with [`WorldDefinition::put_item`].
pub const DEFAULT_ITEM_LAYER: &str = "items";

/// An authored item as it is put into the definition (WDEF-007).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemSpec {
    pub id: u64,
    /// The layer the item belongs to (not empty).
    pub layer: String,
    /// Application order within the layer: lower first, ties by id.
    pub priority: i32,
    /// Where the item is anchored; any non-zero finite vector, normalised on entry.
    pub dir: Vec3,
    /// Stands for everything about the item except its place, reach and bound.
    pub content_hash: u64,
    /// Arc distance in metres beyond which the item changes no output; finite, at least 0 and at most [`cells::CELL_EDGE_M`].
    pub reach_m: f64,
    /// Bound in metres on the height change anywhere in the reach; finite and at least 0.
    pub max_delta_m: f64,
}

/// Why an item could not be put into the index.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemError {
    /// The body has no usable radius node (missing, not a number, not finite and positive).
    Radius(MissingNode),
    /// The direction is zero or not finite.
    Direction,
    /// The reach is not finite, below 0 or over the limit of one cell edge (WDEF-007).
    Reach { id: u64, reach_m: f64, limit_m: f64 },
    /// The height bound is not finite or below 0.
    MaxDelta { id: u64, max_delta_m: f64 },
    /// The layer name is empty.
    Layer { id: u64 },
}

impl core::fmt::Display for ItemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ItemError::Radius(m) => write!(f, "{m}: an item needs the radius of its body to find its cell"),
            ItemError::Direction => write!(f, "an item's direction must be a finite non-zero vector"),
            ItemError::Reach { id, reach_m, limit_m } => {
                write!(f, "item {id}: reach {reach_m} m must be finite, at least 0 and at most {limit_m} m (one cell edge)")
            }
            ItemError::MaxDelta { id, max_delta_m } => write!(f, "item {id}: max_delta {max_delta_m} m must be finite and at least 0"),
            ItemError::Layer { id } => write!(f, "item {id}: the layer name must not be empty"),
        }
    }
}

/// `-0.0` read as `0.0`, as bits (WDEF-003).
fn canonical_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0f64.to_bits()
    } else {
        v.to_bits()
    }
}

/// The world: a seed and the parameters of its bodies, as a sorted set of nodes, and the authored items filed in
/// spatial cells.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldDefinition {
    nodes: BTreeMap<NodePath, NodeValue>,
    /// Authored items by `(body, id)` (WDEF-005): the truth the index is derived from.
    items: BTreeMap<(BodyId, u64), ItemRec>,
    /// The index: item ids by `(body, cell)`, at the cell level of the body's current radius. Rebuilt for a body whenever
    /// its radius node changes, so a radius edit across a level boundary never strands items in old cells.
    cells: BTreeMap<(BodyId, u64), BTreeSet<u64>>,
    /// The implementation id of the height field the world names (GEN-018), if it names one. Not a node (nodes are
    /// numbers); it enters the base hash only when set, so a definition that names none hashes as it always did.
    height_field: Option<String>,
}

/// Whether `id` can name an implementation: letters, digits, `_` and `-`, not empty (the TOML key alphabet).
pub fn valid_implementation_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The node path the height-field id is hashed and written under.
pub const HEIGHT_FIELD_PATH: &str = "world.height_field";

/// Whether `path` is the height-field id's or lies below it: those paths are not available to nodes.
pub fn is_height_field_path(path: &str) -> bool {
    path == HEIGHT_FIELD_PATH || path.strip_prefix(HEIGHT_FIELD_PATH).is_some_and(|rest| rest.starts_with('.'))
}

/// The body whose radius node `path` is (`body.<n>.radius_m`), if it is one.
fn radius_path_body(path: &str) -> Option<BodyId> {
    let n = path.strip_prefix("body.")?.strip_suffix(".radius_m")?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then(|| n.parse().ok().map(BodyId)).flatten()
}

impl WorldDefinition {
    pub const SEED_PATH: &'static str = "world.seed";

    /// No nodes at all (every read fails).
    pub fn empty() -> WorldDefinition {
        WorldDefinition { nodes: BTreeMap::new(), items: BTreeMap::new(), cells: BTreeMap::new(), height_field: None }
    }

    pub fn new(world_seed: u64) -> WorldDefinition {
        let mut d = WorldDefinition::empty();
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
    /// panic here: a stored node must never share a path with a derived one. Setting a body's radius re-files that
    /// body's items under the cell level of the new radius.
    pub fn set(&mut self, path: &str, value: NodeValue) {
        assert!(!path.starts_with("derived."), "the path '{path}' is in the reserved derived namespace");
        assert!(!is_height_field_path(path), "the path '{path}' is reserved for the height field id (set_height_field)");
        self.nodes.insert(path.to_string(), value);
        if let Some(body) = radius_path_body(path) {
            self.reindex(body);
        }
    }

    /// Name the height field of the world by implementation id (GEN-018). The id is only checked for being a name
    /// ([`valid_implementation_id`]); whether a build knows it is decided where the implementations are (generators).
    pub fn set_height_field(&mut self, id: &str) {
        assert!(valid_implementation_id(id), "a height field id is letters, digits, `_` and `-`, got '{id}'");
        self.height_field = Some(id.to_string());
    }

    /// The height field the world names, if any.
    pub fn height_field(&self) -> Option<&str> {
        self.height_field.as_deref()
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

    /// Whether any authored item exists (the TOML format does not carry items yet).
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Hash of the whole definition (WDEF-003): the nodes and, if there are authored items, the items. Equal for
    /// definitions with the same content however they were built; a definition without items hashes exactly as its
    /// nodes do.
    pub fn hash(&self) -> u64 {
        let base = self.base_hash();
        if self.items.is_empty() {
            return base;
        }
        let mut v = vec![u64::from(HASH_VERSION), base, self.items.len() as u64];
        for ((body, id), rec) in &self.items {
            v.extend([u64::from(body.0), *id, rec.content, rec.dir[0], rec.dir[1], rec.dir[2]]);
            v.extend([rec.reach_bits, rec.max_delta_bits, u64::from(rec.priority as u32), hash_u64s(3, &path_words(&rec.layer))]);
        }
        hash_u64s(0x0069_7465, &v)
    }

    /// Hash of the procedural base: the nodes without the authored layers (ADR 0007). Authored items record the base
    /// hash they were made against, so adding an item never makes other items look stale.
    pub fn base_hash(&self) -> u64 {
        let mut entries: BTreeMap<NodePath, u64> = self.nodes.iter().map(|(p, v)| (p.clone(), node_hash(p, *v))).collect();
        if let Some(id) = &self.height_field {
            // The path cannot be a node, so this entry never collides with one.
            entries
                .insert(HEIGHT_FIELD_PATH.to_string(), hash_u64s(0x0068_6669, &[u64::from(HASH_VERSION), hash_u64s(3, &path_words(id))]));
        }
        set_hash(&entries)
    }

    /// The radius of a body if it is a finite positive number.
    fn usable_radius(&self, body: BodyId) -> Result<f64, MissingNode> {
        let path = body_radius_path(body);
        match self.get(&path) {
            Some(NodeValue::F64(r)) if r.is_finite() && r > 0.0 => Ok(r),
            Some(NodeValue::F64(_)) => Err(MissingNode(format!("{path} (not finite and positive)"))),
            Some(NodeValue::U64(_)) => Err(MissingNode(format!("{path} (not a number)"))),
            None => Err(MissingNode(path)),
        }
    }

    /// Add or move the authored item `id` of `body` to where the unit direction `dir` points, with the given content
    /// hash (which stands for everything about the item except its place, which the index keeps). The cell level comes
    /// from the body's radius node. Returns the cell.
    pub fn put_item(&mut self, body: BodyId, id: u64, dir: Vec3, content_hash: u64) -> Result<TileId, ItemError> {
        let spec = ItemSpec { id, layer: DEFAULT_ITEM_LAYER.to_string(), priority: 0, dir, content_hash, reach_m: 0.0, max_delta_m: 0.0 };
        self.put_item_spec(body, spec)
    }

    /// [`put_item`](WorldDefinition::put_item) with a layer, a priority, a reach and a height bound (WDEF-007). A refused
    /// item leaves the definition unchanged (an item of the same id stays where it was).
    pub fn put_item_spec(&mut self, body: BodyId, spec: ItemSpec) -> Result<TileId, ItemError> {
        let radius = self.usable_radius(body).map_err(ItemError::Radius)?;
        let len = spec.dir.length();
        if !len.is_finite() || len < 1e-12 {
            return Err(ItemError::Direction);
        }
        if !(spec.reach_m.is_finite() && (0.0..=cells::CELL_EDGE_M).contains(&spec.reach_m)) {
            return Err(ItemError::Reach { id: spec.id, reach_m: spec.reach_m, limit_m: cells::CELL_EDGE_M });
        }
        if !(spec.max_delta_m.is_finite() && spec.max_delta_m >= 0.0) {
            return Err(ItemError::MaxDelta { id: spec.id, max_delta_m: spec.max_delta_m });
        }
        if spec.layer.is_empty() {
            return Err(ItemError::Layer { id: spec.id });
        }
        let (id, unit) = (spec.id, spec.dir.normalized());
        self.remove_item(body, id);
        let rec = ItemRec {
            content: spec.content_hash,
            dir: [canonical_bits(unit.x), canonical_bits(unit.y), canonical_bits(unit.z)],
            reach_bits: canonical_bits(spec.reach_m),
            max_delta_bits: canonical_bits(spec.max_delta_m),
            layer: spec.layer,
            priority: spec.priority,
        };
        let cell = cells::cell_of(radius, unit);
        self.items.insert((body, id), rec);
        self.cells.entry((body, cell.raw())).or_default().insert(id);
        Ok(cell)
    }

    /// Remove an item; `false` if there was none.
    pub fn remove_item(&mut self, body: BodyId, id: u64) -> bool {
        let Some(rec) = self.items.remove(&(body, id)) else { return false };
        // The item sits in the cell of its direction at the level of the current radius (the index is rebuilt on every
        // radius change, so this is where it was filed).
        if let Ok(radius) = self.usable_radius(body) {
            let cell = cells::cell_of(radius, rec.unit());
            if let Some(ids) = self.cells.get_mut(&(body, cell.raw())) {
                ids.remove(&id);
                if ids.is_empty() {
                    self.cells.remove(&(body, cell.raw()));
                }
            }
        }
        true
    }

    /// Rebuild the cell index of one body from its items and its radius.
    fn reindex(&mut self, body: BodyId) {
        self.cells.retain(|(b, _), _| *b != body);
        let Ok(radius) = self.usable_radius(body) else { return };
        let ids: Vec<(u64, Vec3)> = self.items.range((body, 0)..=(body, u64::MAX)).map(|((_, id), rec)| (*id, rec.unit())).collect();
        for (id, unit) in ids {
            let cell = cells::cell_of(radius, unit);
            self.cells.entry((body, cell.raw())).or_default().insert(id);
        }
    }

    /// The items of one cell, `(id, content hash)` in id order.
    pub fn items_in_cell(&self, body: BodyId, cell: TileId) -> Vec<(u64, u64)> {
        self.cell_entries(body, cell).into_iter().map(|(id, content, _)| (id, content)).collect()
    }

    fn cell_entries(&self, body: BodyId, cell: TileId) -> Vec<cells::ItemEntry> {
        let Some(ids) = self.cells.get(&(body, cell.raw())) else { return Vec::new() };
        ids.iter().map(|id| self.entry(body, *id)).collect()
    }

    fn entry(&self, body: BodyId, id: u64) -> cells::ItemEntry {
        let rec = &self.items[&(body, id)];
        (id, rec.content, rec.dir)
    }

    /// The items of `layer` whose reach touches the footprint of `tile` with a halo of `halo` tile edges (WDEF-007), in
    /// application order (priority, then id). The tile must come from the tangent-warp mapping. Candidates are the items
    /// of the cell of the tile plus [`footprint::candidate_rings`] rings, or every item of the body for a tile coarser
    /// than the cell level; the touch test decides, and a test checks that both ways give the same set.
    pub fn footprint_items(
        &self,
        body: BodyId,
        layer: &str,
        tile: TileId,
        halo: f64,
    ) -> Result<Vec<footprint::FootprintItem>, MissingNode> {
        if !(halo.is_finite() && (0.0..=1.0).contains(&halo)) {
            return Err(MissingNode(format!("a footprint halo in [0, 1] (got {halo})")));
        }
        let radius = self.usable_radius(body)?;
        let cap = footprint::tile_cap(&planet_core::TangentWarp, tile, halo);
        let level = cells::cell_level(radius);
        // A tile at or below the cell level lies in one cell, so its cap (a few tile edges wide) is within the rings; a coarser
        // tile covers many cells and its cap and halo are many cells wide, so it scans the body (step 1b replaces the scan).
        let rings = footprint::candidate_rings(cap, radius, level);
        let candidates: Vec<u64> = match rings {
            Some(rings) if tile.level() >= level => {
                let covering = cells::cells_covering(tile, level, footprint::FOOTPRINT_MAX_CELLS).expect("one cell");
                cells::with_margin(&covering, rings)
                    .iter()
                    .filter_map(|c| self.cells.get(&(body, c.raw())))
                    .flat_map(|ids| ids.iter().copied())
                    .collect()
            }
            _ => self.body_item_ids(body),
        };
        let mut out: Vec<footprint::FootprintItem> = candidates
            .into_iter()
            .filter_map(|id| {
                let rec = &self.items[&(body, id)];
                (rec.layer == layer && footprint::touches(cap, rec.unit(), f64::from_bits(rec.reach_bits), radius)).then(|| {
                    footprint::FootprintItem {
                        id,
                        priority: rec.priority,
                        content: rec.content,
                        dir: rec.dir,
                        reach_bits: rec.reach_bits,
                        max_delta_bits: rec.max_delta_bits,
                        layer: rec.layer.clone(),
                    }
                })
            })
            .collect();
        out.sort_by_key(|i| (i.priority, i.id));
        out.dedup_by_key(|i| i.id);
        Ok(out)
    }

    /// The anchor (unit direction) and reach in metres of an item, if it exists.
    pub fn item_anchor(&self, body: BodyId, id: u64) -> Option<(Vec3, f64)> {
        self.items.get(&(body, id)).map(|rec| (rec.unit(), f64::from_bits(rec.reach_bits)))
    }

    /// The height bound in metres of an item, if it exists.
    pub fn item_max_delta(&self, body: BodyId, id: u64) -> Option<f64> {
        self.items.get(&(body, id)).map(|rec| f64::from_bits(rec.max_delta_bits))
    }

    /// The ids of the items of a body, in id order.
    pub fn item_ids(&self, body: BodyId) -> Vec<u64> {
        self.body_item_ids(body)
    }

    fn body_item_ids(&self, body: BodyId) -> Vec<u64> {
        self.items.range((body, 0)..=(body, u64::MAX)).map(|((_, id), _)| *id).collect()
    }

    fn all_entries(&self, body: BodyId) -> Vec<cells::ItemEntry> {
        self.items.range((body, 0)..=(body, u64::MAX)).map(|((_, id), _)| self.entry(body, *id)).collect()
    }
}

impl ItemRec {
    fn unit(&self) -> Vec3 {
        Vec3::new(f64::from_bits(self.dir[0]), f64::from_bits(self.dir[1]), f64::from_bits(self.dir[2]))
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

/// Path under which a spatial query of one cell is recorded in a read set, in the `derived.` namespace.
pub fn cell_query_path(body: BodyId, cell: TileId) -> NodePath {
    format!("derived.cell.{}.{:016x}", body.0, cell.raw())
}

/// Path under which a footprint query is recorded (the layer is length-prefixed so no two queries share a path).
pub fn footprint_query_path(body: BodyId, layer: &str, tile: TileId, halo_bits: u64) -> NodePath {
    format!("derived.footprint.{}.{}:{}.{:016x}.{:016x}", body.0, layer.len(), layer, tile.raw(), halo_bits)
}

/// The 64-bit hash of a footprint result: the items in application order with everything that can change an output.
pub fn footprint_hash(items: &[footprint::FootprintItem]) -> u64 {
    // Application order whatever order a hand-built record was given in, as the 128-bit path does.
    let mut sorted: Vec<&footprint::FootprintItem> = items.iter().collect();
    sorted.sort_by_key(|i| (i.priority, i.id));
    let mut v = vec![u64::from(HASH_VERSION), items.len() as u64];
    for i in sorted {
        v.extend([i.id, u64::from(i.priority as u32), i.content, i.dir[0], i.dir[1], i.dir[2], i.reach_bits, i.max_delta_bits]);
        v.push(hash_u64s(3, &path_words(&i.layer)));
    }
    hash_u64s(0x0066_7074, &v)
}

/// Path under which a read of every item of a body is recorded.
pub fn cell_query_all_path(body: BodyId) -> NodePath {
    format!("derived.cell.{}.all", body.0)
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

/// One recorded read, with what a 128-bit persistent key needs (ADR 0018): the value read, not only its 64-bit hash.
#[derive(Clone, Debug, PartialEq)]
pub enum ReadRecord {
    /// A node (or the derived seed of a body, recorded under its `derived.` path) and the value read.
    Node { path: NodePath, value: NodeValue },
    /// A spatial query of one cell and the items found there (empty included).
    Cell { body: BodyId, cell: TileId, items: Vec<cells::ItemEntry> },
    /// A read of every item of a body (a tile over the cell limit).
    Body { body: BodyId, items: Vec<cells::ItemEntry> },
    /// A footprint query of one layer for one tile (query kind 3, WDEF-007): the items whose reach touches the tile, in
    /// application order. `halo_bits` are the bits of the halo (`-0.0` read as `0.0`).
    Footprint { body: BodyId, layer: String, tile: TileId, halo_bits: u64, items: Vec<footprint::FootprintItem> },
}

impl ReadRecord {
    /// The path this read is recorded under.
    pub fn path(&self) -> NodePath {
        match self {
            ReadRecord::Node { path, .. } => path.clone(),
            ReadRecord::Cell { body, cell, .. } => cell_query_path(*body, *cell),
            ReadRecord::Body { body, .. } => cell_query_all_path(*body),
            ReadRecord::Footprint { body, layer, tile, halo_bits, .. } => footprint_query_path(*body, layer, *tile, *halo_bits),
        }
    }

    /// The 64-bit hash of this read, as the in-memory read-set hash sees it.
    pub fn hash64(&self) -> u64 {
        match self {
            ReadRecord::Node { path, value } => node_hash(path, *value),
            ReadRecord::Cell { items, .. } | ReadRecord::Body { items, .. } => cells::cell_hash(items),
            ReadRecord::Footprint { items, .. } => footprint_hash(items),
        }
    }
}

/// The 64-bit read-set hash of a list of typed reads: equal to [`GenContext::read_set_hash`] of the context they were
/// recorded in. Fields that read cells report their per-tile reads as records and get their 64-bit per-tile hash from
/// here, so the two key paths share one source.
pub fn records_hash64(records: &[ReadRecord]) -> u64 {
    set_hash(&records.iter().map(|r| (r.path(), r.hash64())).collect())
}

/// The only way generators read the definition. Every read is recorded with the hash of the node read; the recorded
/// set is what keys are built from, so an unread node cannot affect a key and a read node always does.
pub struct GenContext<'a> {
    def: &'a WorldDefinition,
    /// Every read by path: its 64-bit hash (what in-memory keys use) and the typed record (what persistent keys use),
    /// inserted together so the two views of the read set can never drift apart.
    reads: RefCell<BTreeMap<NodePath, (u64, ReadRecord)>>,
}

impl<'a> GenContext<'a> {
    pub fn new(def: &'a WorldDefinition) -> GenContext<'a> {
        GenContext { def, reads: RefCell::new(BTreeMap::new()) }
    }

    fn read(&self, path: &str) -> Result<NodeValue, MissingNode> {
        let v = self.def.get(path).ok_or_else(|| MissingNode(path.to_string()))?;
        self.record(path.to_string(), ReadRecord::Node { path: path.to_string(), value: v });
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
        self.record(path.clone(), ReadRecord::Node { path, value: NodeValue::U64(seed) });
        Ok(seed)
    }

    pub fn body_radius_m(&self, body: BodyId) -> Result<f64, MissingNode> {
        match self.read(&body_radius_path(body))? {
            NodeValue::F64(r) => Ok(r),
            NodeValue::U64(_) => Err(MissingNode(format!("{} (not a number)", body_radius_path(body)))),
        }
    }

    /// Record one read under its path: the only place a read enters the context.
    fn record(&self, path: NodePath, record: ReadRecord) {
        let hash = record.hash64();
        self.reads.borrow_mut().insert(path, (hash, record));
    }

    /// The level of the body's cells; reads (and so records) the body's radius.
    pub fn cell_level(&self, body: BodyId) -> Result<u8, MissingNode> {
        Ok(cells::cell_level(self.body_radius_m(body)?))
    }

    /// The items of one cell (WDEF-005). The read records the cell's hash, also when the cell is empty, so adding an
    /// item to a cell that was queried and found empty changes the key. `cell` must be at the body's cell level.
    pub fn query_cell(&self, body: BodyId, cell: TileId) -> Result<Vec<(u64, u64)>, MissingNode> {
        let level = self.cell_level(body)?;
        if cell.level() != level {
            return Err(MissingNode(format!("a cell of body {} at level {level} (got level {})", body.0, cell.level())));
        }
        let path = cell_query_path(body, cell);
        self.record(path, ReadRecord::Cell { body, cell, items: self.def.cell_entries(body, cell) });
        Ok(self.def.items_in_cell(body, cell))
    }

    /// The items a tile depends on: those of every cell its footprint touches plus a ring of `margin_cells` cells around
    /// it (a tile samples its closed border and a halo for normals, and an item reaches past its own cell), each cell read
    /// and recorded. A tile that would touch more than `max_cells` cells reads the whole body instead: the hash of all the
    /// body's items is recorded under `derived.cell.<body>.all` and all of them are returned, so the answer is always
    /// exact and never silently stale. The tile id must come from the tangent-warp mapping ([`cells::CELL_MAPPING_ID`]).
    pub fn query_tile(&self, body: BodyId, tile: TileId, margin_cells: u32, max_cells: usize) -> Result<Vec<(u64, u64)>, MissingNode> {
        let level = self.cell_level(body)?;
        if let Some(covering) = cells::cells_covering(tile, level, max_cells) {
            let wanted = if margin_cells == 0 { covering } else { cells::with_margin(&covering, margin_cells) };
            if wanted.len() <= max_cells {
                let mut out = Vec::new();
                for cell in wanted {
                    out.extend(self.query_cell(body, cell)?);
                }
                return Ok(out);
            }
        }
        self.record(cell_query_all_path(body), ReadRecord::Body { body, items: self.def.all_entries(body) });
        Ok(self.def.items.range((body, 0)..=(body, u64::MAX)).map(|((_, id), rec)| (*id, rec.content)).collect())
    }

    /// The items of `layer` whose reach touches `tile` with a halo of `halo` tile edges (WDEF-007, query kind 3), in
    /// application order. The read is recorded with its result, an empty one included, so adding an item that reaches the
    /// tile changes the key and an item out of reach changes nothing. `halo` must be finite and in `[0, 1]`.
    pub fn query_footprint(
        &self,
        body: BodyId,
        layer: &str,
        tile: TileId,
        halo: f64,
    ) -> Result<Vec<footprint::FootprintItem>, MissingNode> {
        if !(halo.is_finite() && (0.0..=1.0).contains(&halo)) {
            return Err(MissingNode(format!("a footprint halo in [0, 1] (got {halo})")));
        }
        self.body_radius_m(body)?;
        let items = self.def.footprint_items(body, layer, tile, halo)?;
        let halo_bits = if halo == 0.0 { 0.0f64.to_bits() } else { halo.to_bits() };
        let path = footprint_query_path(body, layer, tile, halo_bits);
        self.record(path, ReadRecord::Footprint { body, layer: layer.to_string(), tile, halo_bits, items: items.clone() });
        Ok(items)
    }

    /// Hash of everything read so far (WDEF-004): the definition part of a cache key.
    pub fn read_set_hash(&self) -> u64 {
        set_hash(&self.reads.borrow().iter().map(|(p, (h, _))| (p.clone(), *h)).collect())
    }

    /// The recorded reads with their values, sorted by path: what a 128-bit persistent key is built from (ADR 0018).
    pub fn read_records(&self) -> Vec<ReadRecord> {
        self.reads.borrow().values().map(|(_, r)| r.clone()).collect()
    }

    /// The recorded reads, for diagnostics and tests: path and node hash, sorted by path.
    pub fn reads(&self) -> Vec<(NodePath, u64)> {
        self.reads.borrow().iter().map(|(p, (h, _))| (p.clone(), *h)).collect()
    }
}

/// The reads of a generator that reads only the seed of the single planet, as typed records.
pub fn single_planet_seed_records(seed: u64) -> Vec<ReadRecord> {
    let def = WorldDefinition::new(seed);
    let ctx = GenContext::new(&def);
    ctx.body_seed(BodyId::PLANET).expect("the seed node exists");
    ctx.read_records()
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
