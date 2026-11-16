//! The canonical river graph (GEN-189; the G0 contracts of the river-graph plan, sections 1 to 8): its entities, identities, canonical serialisation, diff, rasterisation and validators, and the
//! terrain-change budget's validator. Data and checks only: no algorithm here builds a graph from terrain (KS, KG, KV and KC wait for their milestones).
//!
//! - **Entities.** Nodes (kinds `source`, `confluence`, `trib_junction` and `exception` markers, `tile_crossing`, `outlet_sea`, `lake_inlet`, `lake_spill`), reaches (directed downstream, an 8-connected chain
//!   of global cell ids from `up`'s anchor to `down`'s, per-vertex attributes) and basins, at two levels (H1 and L), owned by a domain (`landmass` or a canonical tile).
//! - **Identities.** `node = hash(body, kind, level, anchor)`, `reach = hash(body, level, up, down)`, `basin = hash(body, level, floor anchor)` ([`id_hash`], GEN-145's mixer). Ownership never enters an id.
//! - **Canonical form.** Nodes, reaches and basins sorted by id, fixed field order, f64 as IEEE-754 bit patterns in hex: the same graph gives the same bytes, and [`Graph::parse`] reads them back.
//! - **Diff.** Ids added or removed (topology) apart from same ids with different fields (attributes), each named by (id, field).
//! - **Validators.** Each check of the contract's section 8 returns [`Violation`]s; none repairs anything.
//!
//! Sums and orders are fixed (CON-14); nothing here reads a thread count.

use super::hydro::id_hash;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

const NODE_TAG: u64 = 0x6E0D_E000_0000_0000;
const REACH_TAG: u64 = 0x7EAC_4000_0000_0000;
const BASIN_TAG: u64 = 0xBA51_7000_0000_0000;

/// The relative tolerance of the conservation checks (the contract's 1e-9 relative).
pub const CONSERVATION_REL: f64 = 1e-9;
/// The absolute slack of the budget checks on f64 (metres): rounding only.
pub const BUDGET_EPS_M: f64 = 1e-9;

/// A level of the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// 814 m global cells.
    H1,
    /// 101.8 m global cells.
    L,
}

impl Level {
    fn code(self) -> u64 {
        match self {
            Level::H1 => 1,
            Level::L => 2,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Level::H1 => "h1",
            Level::L => "l",
        }
    }
    fn parse(s: &str) -> Result<Level, String> {
        match s {
            "h1" => Ok(Level::H1),
            "l" => Ok(Level::L),
            _ => Err(format!("level '{s}'")),
        }
    }
}

/// The owner of an item: the landmass (H1, every trunk reach at both levels, H1 basins and their L images) or one canonical tile of K7's grid (GEN-182).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    Landmass,
    Tile { face: u8, ti: u32, tj: u32 },
}

impl Domain {
    fn text(self) -> String {
        match self {
            Domain::Landmass => "landmass".into(),
            Domain::Tile { face, ti, tj } => format!("tile:{face}:{ti}:{tj}"),
        }
    }
    fn parse(s: &str) -> Result<Domain, String> {
        if s == "landmass" {
            return Ok(Domain::Landmass);
        }
        let p: Vec<&str> = s.split(':').collect();
        if p.len() == 4 && p[0] == "tile" {
            let n = |x: &str| x.parse::<u32>().map_err(|_| format!("domain '{s}'"));
            return Ok(Domain::Tile { face: n(p[1])? as u8, ti: n(p[2])?, tj: n(p[3])? });
        }
        Err(format!("domain '{s}'"))
    }
}

/// The kind of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    Source,
    Confluence,
    /// A marker: where tributaries end on a trunk vertex.
    TribJunction,
    /// A marker: an exception on a reach (never splits it).
    Exception,
    TileCrossing,
    OutletSea,
    LakeInlet,
    LakeSpill,
}

const KINDS: [(NodeKind, &str); 8] = [
    (NodeKind::Source, "source"),
    (NodeKind::Confluence, "confluence"),
    (NodeKind::TribJunction, "trib_junction"),
    (NodeKind::Exception, "exception"),
    (NodeKind::TileCrossing, "tile_crossing"),
    (NodeKind::OutletSea, "outlet_sea"),
    (NodeKind::LakeInlet, "lake_inlet"),
    (NodeKind::LakeSpill, "lake_spill"),
];

impl NodeKind {
    fn code(self) -> u64 {
        KINDS.iter().position(|k| k.0 == self).expect("a kind") as u64 + 1
    }
    fn name(self) -> &'static str {
        KINDS.iter().find(|k| k.0 == self).expect("a kind").1
    }
    fn parse(s: &str) -> Result<NodeKind, String> {
        KINDS.iter().find(|k| k.1 == s).map(|k| k.0).ok_or_else(|| format!("node kind '{s}'"))
    }
    /// A chain ends here: no reach leaves it.
    pub fn is_outlet(self) -> bool {
        matches!(self, NodeKind::OutletSea | NodeKind::LakeInlet)
    }
    /// A marker on a reach: no reach starts here.
    pub fn is_marker(self) -> bool {
        matches!(self, NodeKind::TribJunction | NodeKind::Exception)
    }
}

/// Where a marker sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    pub on_reach: u64,
    pub at_vertex: u32,
}

/// A node.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: u64,
    pub kind: NodeKind,
    pub level: Level,
    /// The global cell id at the node's level.
    pub anchor: u64,
    /// The H1 node of an L trunk image.
    pub parent: Option<u64>,
    pub owner: Domain,
    pub marker: Option<Marker>,
}

/// Why a reach is exceptional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    RiseOverBudget,
    UnresolvedEndpoint,
    BudgetResidual,
}

impl Reason {
    fn name(self) -> &'static str {
        match self {
            Reason::RiseOverBudget => "rise_over_budget",
            Reason::UnresolvedEndpoint => "unresolved_endpoint",
            Reason::BudgetResidual => "budget_residual",
        }
    }
    fn parse(s: &str) -> Result<Reason, String> {
        [Reason::RiseOverBudget, Reason::UnresolvedEndpoint, Reason::BudgetResidual]
            .into_iter()
            .find(|r| r.name() == s)
            .ok_or_else(|| format!("reason '{s}'"))
    }
}

/// A reach's status.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Ok,
    Exception { reason: Reason, value: f64 },
}

/// The per-vertex attributes of a reach (contract section 3); every array has one entry per vertex.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Attrs {
    /// Geometric contributing area (m2).
    pub a_geo: Vec<f64>,
    /// Area entering at a window-entry vertex from outside the tile window (m2; zero elsewhere).
    pub a_ext: Vec<f64>,
    /// Runoff-weighted area (m2), with and without `a_ext`.
    pub a_run: Vec<f64>,
    pub a_run_no_ext: Vec<f64>,
    /// Potential and routed discharge (m3/s).
    pub q: Vec<f64>,
    pub q_w: Vec<f64>,
    pub strahler: Vec<u8>,
    /// Bed level in codes of 0.5 m (GEN-170).
    pub bed: Vec<i32>,
}

/// The fields of [`Attrs`] in canonical order, with their accessors.
/// An accessor of one f64 attribute array.
type F64Field = fn(&Attrs) -> &Vec<f64>;

const F64_FIELDS: [(&str, F64Field); 6] = [
    ("a_geo", |a| &a.a_geo),
    ("a_ext", |a| &a.a_ext),
    ("a_run", |a| &a.a_run),
    ("a_run_no_ext", |a| &a.a_run_no_ext),
    ("q", |a| &a.q),
    ("q_w", |a| &a.q_w),
];

/// A reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Reach {
    pub id: u64,
    pub level: Level,
    pub up: u64,
    pub down: u64,
    /// Global cell ids from `up`'s anchor to `down`'s, both included.
    pub vertices: Vec<u64>,
    /// The H1 reach of an L trunk image.
    pub parent: Option<u64>,
    pub owner: Domain,
    pub attrs: Attrs,
    pub status: Status,
}

/// A basin's outlet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outlet {
    Active,
    Endorheic,
}

/// How a basin is represented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Represented {
    ProtectedSurface,
    None,
}

/// A basin (topology, no water balance; contract section 4).
#[derive(Clone, Debug, PartialEq)]
pub struct Basin {
    pub id: u64,
    pub level: Level,
    /// The larger basin it merges into at its spill, or for an L image the H1 basin.
    pub parent: Option<u64>,
    pub floor_cell: u64,
    pub floor_m: f64,
    pub spill_cell: u64,
    pub spill_m: f64,
    /// The node that receives the spill: a `trib_junction` on a reach or another basin's `lake_inlet`.
    pub spill_node: u64,
    pub outlet: Outlet,
    pub represented: Represented,
    pub owner: Domain,
}

/// A canonical river graph, or a slice of one.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Graph {
    pub body: u64,
    pub nodes: Vec<Node>,
    pub reaches: Vec<Reach>,
    pub basins: Vec<Basin>,
}

/// The id of a node.
pub fn node_id(body: u64, kind: NodeKind, level: Level, anchor: u64) -> u64 {
    id_hash(body ^ NODE_TAG, (kind.code() << 8) | level.code(), anchor)
}

/// The id of a reach (GEN-145's rule on node ids).
pub fn reach_id(body: u64, level: Level, up: u64, down: u64) -> u64 {
    id_hash(body ^ REACH_TAG ^ level.code(), up, down)
}

/// The id of a basin.
pub fn basin_id(body: u64, level: Level, floor: u64) -> u64 {
    id_hash(body ^ BASIN_TAG ^ level.code(), floor, 0)
}

fn opt(v: Option<u64>) -> String {
    v.map_or("-".into(), |x| format!("{x:016x}"))
}

fn hex_f64(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn list_f64(v: &[f64]) -> String {
    v.iter().map(|x| hex_f64(*x)).collect::<Vec<_>>().join(",")
}

impl Graph {
    /// Sorts nodes, reaches and basins by id (the canonical order).
    pub fn sort(&mut self) {
        self.nodes.sort_by_key(|n| n.id);
        self.reaches.sort_by_key(|r| r.id);
        self.basins.sort_by_key(|b| b.id);
    }

    /// The canonical serialisation: one line per item in id order, fixed field order, f64 as bit patterns.
    pub fn canonical(&self) -> String {
        let mut g = self.clone();
        g.sort();
        let mut s = format!("river-graph 1 body={:016x}\n", g.body);
        for n in &g.nodes {
            let m = n.marker.map_or("-".into(), |m| format!("{:016x}@{}", m.on_reach, m.at_vertex));
            let _ = writeln!(
                s,
                "node {:016x} kind={} level={} anchor={} parent={} owner={} marker={}",
                n.id,
                n.kind.name(),
                n.level.name(),
                n.anchor,
                opt(n.parent),
                n.owner.text(),
                m
            );
        }
        for r in &g.reaches {
            let status = match r.status {
                Status::Ok => "ok".to_string(),
                Status::Exception { reason, value } => format!("{}:{}", reason.name(), hex_f64(value)),
            };
            let _ = write!(
                s,
                "reach {:016x} level={} up={:016x} down={:016x} parent={} owner={} status={} vertices={}",
                r.id,
                r.level.name(),
                r.up,
                r.down,
                opt(r.parent),
                r.owner.text(),
                status,
                r.vertices.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
            );
            for (name, get) in F64_FIELDS {
                let _ = write!(s, " {name}={}", list_f64(get(&r.attrs)));
            }
            let _ = write!(s, " strahler={}", r.attrs.strahler.iter().map(u8::to_string).collect::<Vec<_>>().join(","));
            let _ = writeln!(s, " bed={}", r.attrs.bed.iter().map(i32::to_string).collect::<Vec<_>>().join(","));
        }
        for b in &g.basins {
            let _ = writeln!(
                s,
                "basin {:016x} level={} parent={} floor={}:{} spill={}:{} spill_node={:016x} outlet={} represented={} owner={}",
                b.id,
                b.level.name(),
                opt(b.parent),
                b.floor_cell,
                hex_f64(b.floor_m),
                b.spill_cell,
                hex_f64(b.spill_m),
                b.spill_node,
                if b.outlet == Outlet::Active { "active" } else { "endorheic" },
                if b.represented == Represented::ProtectedSurface { "protected_surface" } else { "none" },
                b.owner.text()
            );
        }
        s
    }

    /// Reads a canonical serialisation back.
    pub fn parse(text: &str) -> Result<Graph, String> {
        let mut lines = text.lines();
        let head = lines.next().ok_or("empty")?;
        let body = head.strip_prefix("river-graph 1 body=").ok_or_else(|| format!("header '{head}'"))?;
        let hex = |s: &str| u64::from_str_radix(s, 16).map_err(|_| format!("hex '{s}'"));
        let f = |s: &str| hex(s).map(f64::from_bits);
        let optid = |s: &str| if s == "-" { Ok(None) } else { hex(s).map(Some) };
        let mut g = Graph { body: hex(body)?, ..Graph::default() };
        for line in lines {
            let mut words = line.split(' ');
            let tag = words.next().unwrap_or("");
            let id = hex(words.next().ok_or_else(|| format!("no id: '{line}'"))?)?;
            let kv: BTreeMap<&str, &str> = words.filter_map(|w| w.split_once('=')).collect();
            let get = |k: &str| kv.get(k).copied().ok_or_else(|| format!("no '{k}' in '{line}'"));
            let list = |k: &str| -> Result<Vec<&str>, String> { Ok(get(k)?.split(',').filter(|x| !x.is_empty()).collect()) };
            match tag {
                "node" => {
                    let marker = match get("marker")? {
                        "-" => None,
                        m => {
                            let (r, v) = m.split_once('@').ok_or_else(|| format!("marker '{m}'"))?;
                            Some(Marker { on_reach: hex(r)?, at_vertex: v.parse().map_err(|_| format!("marker '{m}'"))? })
                        }
                    };
                    g.nodes.push(Node {
                        id,
                        kind: NodeKind::parse(get("kind")?)?,
                        level: Level::parse(get("level")?)?,
                        anchor: get("anchor")?.parse().map_err(|_| format!("anchor in '{line}'"))?,
                        parent: optid(get("parent")?)?,
                        owner: Domain::parse(get("owner")?)?,
                        marker,
                    });
                }
                "reach" => {
                    let status = match get("status")? {
                        "ok" => Status::Ok,
                        s => {
                            let (r, v) = s.split_once(':').ok_or_else(|| format!("status '{s}'"))?;
                            Status::Exception { reason: Reason::parse(r)?, value: f(v)? }
                        }
                    };
                    let fl = |k: &str| -> Result<Vec<f64>, String> { list(k)?.into_iter().map(f).collect() };
                    let attrs = Attrs {
                        a_geo: fl("a_geo")?,
                        a_ext: fl("a_ext")?,
                        a_run: fl("a_run")?,
                        a_run_no_ext: fl("a_run_no_ext")?,
                        q: fl("q")?,
                        q_w: fl("q_w")?,
                        strahler: list("strahler")?
                            .into_iter()
                            .map(|x| x.parse().map_err(|_| format!("strahler '{x}'")))
                            .collect::<Result<_, _>>()?,
                        bed: list("bed")?.into_iter().map(|x| x.parse().map_err(|_| format!("bed '{x}'"))).collect::<Result<_, _>>()?,
                    };
                    g.reaches.push(Reach {
                        id,
                        level: Level::parse(get("level")?)?,
                        up: hex(get("up")?)?,
                        down: hex(get("down")?)?,
                        vertices: list("vertices")?
                            .into_iter()
                            .map(|x| x.parse().map_err(|_| format!("vertex '{x}'")))
                            .collect::<Result<_, _>>()?,
                        parent: optid(get("parent")?)?,
                        owner: Domain::parse(get("owner")?)?,
                        attrs,
                        status,
                    });
                }
                "basin" => {
                    let pair = |k: &str| -> Result<(u64, f64), String> {
                        let (c, h) = get(k)?.split_once(':').ok_or_else(|| format!("'{k}' in '{line}'"))?;
                        Ok((c.parse().map_err(|_| format!("cell in '{line}'"))?, f(h)?))
                    };
                    let (floor_cell, floor_m) = pair("floor")?;
                    let (spill_cell, spill_m) = pair("spill")?;
                    g.basins.push(Basin {
                        id,
                        level: Level::parse(get("level")?)?,
                        parent: optid(get("parent")?)?,
                        floor_cell,
                        floor_m,
                        spill_cell,
                        spill_m,
                        spill_node: hex(get("spill_node")?)?,
                        outlet: if get("outlet")? == "active" { Outlet::Active } else { Outlet::Endorheic },
                        represented: if get("represented")? == "protected_surface" {
                            Represented::ProtectedSurface
                        } else {
                            Represented::None
                        },
                        owner: Domain::parse(get("owner")?)?,
                    });
                }
                _ => return Err(format!("line '{line}'")),
            }
        }
        Ok(g)
    }
}

/// A difference between two graphs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    /// Topology: an id in the new graph only, or in the old only (`what` names the entity).
    Added {
        what: &'static str,
        id: u64,
    },
    Removed {
        what: &'static str,
        id: u64,
    },
    /// Attribute: the same id, a different field.
    Field {
        what: &'static str,
        id: u64,
        field: String,
    },
}

impl Change {
    pub fn is_topology(&self) -> bool {
        !matches!(self, Change::Field { .. })
    }
}

/// The fields of one canonical line, by name (the id and the tag excluded).
fn fields(line: &str) -> BTreeMap<String, String> {
    line.split(' ').skip(2).filter_map(|w| w.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// The diff of two graphs: topology changes (ids added or removed) and attribute changes (same id, the fields that differ), in a fixed order.
pub fn diff(old: &Graph, new: &Graph) -> Vec<Change> {
    let index = |g: &Graph| -> BTreeMap<(String, u64), String> {
        g.canonical()
            .lines()
            .skip(1)
            .map(|l| {
                let mut w = l.split(' ');
                let tag = w.next().unwrap_or("").to_string();
                let id = u64::from_str_radix(w.next().unwrap_or("0"), 16).unwrap_or(0);
                ((tag, id), l.to_string())
            })
            .collect()
    };
    let what = |t: &str| -> &'static str {
        match t {
            "node" => "node",
            "reach" => "reach",
            _ => "basin",
        }
    };
    let (a, b) = (index(old), index(new));
    let mut out = Vec::new();
    for ((tag, id), line) in &a {
        match b.get(&(tag.clone(), *id)) {
            None => out.push(Change::Removed { what: what(tag), id: *id }),
            Some(other) if other != line => {
                let (fa, fb) = (fields(line), fields(other));
                for (k, v) in &fa {
                    if fb.get(k) != Some(v) {
                        out.push(Change::Field { what: what(tag), id: *id, field: k.clone() });
                    }
                }
            }
            _ => {}
        }
    }
    for (tag, id) in b.keys() {
        if !a.contains_key(&(tag.clone(), *id)) {
            out.push(Change::Added { what: what(tag), id: *id });
        }
    }
    out.sort();
    out
}

/// A violation of the contract's section 8.
#[derive(Clone, Debug, PartialEq)]
pub enum Violation {
    Cycle { reach: u64 },
    DuplicateId { id: u64 },
    TwoNodesOneAnchor { level: Level, anchor: u64 },
    UnresolvedEndpoint { reach: u64, node: u64 },
    ChainWithoutOutlet { node: u64 },
    ConfluenceOneInflow { node: u64 },
    OutgoingCount { node: u64, outgoing: usize },
    InvalidEdge { reach: u64, why: &'static str },
    BedRise { reach: u64, vertex: usize },
    ConservationBreak { at: u64, field: &'static str },
    CellOwnedTwice { cell: u64, reaches: (u64, u64) },
    BudgetOverrun { cell: usize, stage: &'static str, used_m: f64, budget_m: f64 },
    ClampWithoutResidual { cell: usize, stage: &'static str },
    ResidualFeatureNotFlagged { feature: u64 },
}

/// The 8-neighbour relation of the global cells at each level: `adjacent(level, a, b)`.
pub type Adjacency<'a> = &'a dyn Fn(Level, u64, u64) -> bool;

/// Whether `v` is non-decreasing within the relative tolerance.
fn falls(a: f64, b: f64) -> bool {
    b < a - CONSERVATION_REL * a.abs().max(b.abs())
}

/// Validates a graph (contract section 8). An open slice (`open`) reports endpoints owned elsewhere as resolved (a store fact, `missing owner`), not as violations.
pub fn validate(g: &Graph, adjacent: Adjacency, open: bool) -> Vec<Violation> {
    let mut v = Vec::new();
    // duplicate ids
    let mut seen = BTreeSet::new();
    for id in g.nodes.iter().map(|n| n.id).chain(g.reaches.iter().map(|r| r.id)).chain(g.basins.iter().map(|b| b.id)) {
        if !seen.insert(id) {
            v.push(Violation::DuplicateId { id });
        }
    }
    // two nodes on one (level, anchor): among non-marker nodes, and among markers; a marker shares its anchor only with the up node of the reach it sits on, at vertex 0 (a tributary
    // continuing above a trunk's head ends at a trib_junction on the trunk's source; reading of DEC crusader-6, default A)
    let up_of: HashMap<u64, u64> = g.reaches.iter().map(|r| (r.id, r.up)).collect();
    let mut plain: BTreeMap<(Level, u64), Vec<u64>> = BTreeMap::new();
    let mut markers: BTreeMap<(Level, u64), Vec<&Node>> = BTreeMap::new();
    for n in &g.nodes {
        if n.kind.is_marker() {
            markers.entry((n.level, n.anchor)).or_default().push(n);
        } else {
            plain.entry((n.level, n.anchor)).or_default().push(n.id);
        }
    }
    let mut twice = BTreeSet::new();
    for (key, ids) in &plain {
        if ids.len() > 1 {
            twice.insert(*key);
        }
    }
    for (key, ms) in &markers {
        let shared_ok = |m: &Node| {
            m.marker.is_some_and(|mk| {
                mk.at_vertex == 0 && plain.get(key).is_some_and(|ids| ids.len() == 1 && up_of.get(&mk.on_reach) == Some(&ids[0]))
            })
        };
        if ms.len() > 1 || (plain.contains_key(key) && !ms.iter().all(|m| shared_ok(m))) {
            twice.insert(*key);
        }
    }
    for (level, anchor) in twice {
        v.push(Violation::TwoNodesOneAnchor { level, anchor });
    }
    let node: HashMap<u64, &Node> = g.nodes.iter().map(|n| (n.id, n)).collect();
    let mut outgoing: BTreeMap<u64, Vec<&Reach>> = BTreeMap::new();
    let mut incoming: BTreeMap<u64, Vec<&Reach>> = BTreeMap::new();
    for r in &g.reaches {
        outgoing.entry(r.up).or_default().push(r);
        incoming.entry(r.down).or_default().push(r);
        // endpoints and edges
        let (up, down) = (node.get(&r.up), node.get(&r.down));
        for (id, n) in [(r.up, up), (r.down, down)] {
            if n.is_none() && !open {
                v.push(Violation::UnresolvedEndpoint { reach: r.id, node: id });
            }
        }
        if let Some(u) = up {
            if u.kind.is_outlet() {
                v.push(Violation::InvalidEdge { reach: r.id, why: "up is an outlet" });
            }
            if u.kind.is_marker() {
                v.push(Violation::InvalidEdge { reach: r.id, why: "up is a marker" });
            }
            if u.level != r.level {
                v.push(Violation::InvalidEdge { reach: r.id, why: "up at another level" });
            }
            if r.vertices.first() != Some(&u.anchor) {
                v.push(Violation::InvalidEdge { reach: r.id, why: "chain does not start at up's anchor" });
            }
        }
        if let Some(d) = down {
            if d.kind == NodeKind::Source {
                v.push(Violation::InvalidEdge { reach: r.id, why: "down is a source" });
            }
            if d.level != r.level {
                v.push(Violation::InvalidEdge { reach: r.id, why: "down at another level" });
            }
            if r.vertices.last() != Some(&d.anchor) {
                v.push(Violation::InvalidEdge { reach: r.id, why: "chain does not end at down's anchor" });
            }
            if d.kind == NodeKind::TribJunction && d.marker.is_none() {
                v.push(Violation::InvalidEdge { reach: r.id, why: "trib_junction without its reach" });
            }
        }
        if r.vertices.len() < 2 || r.vertices.windows(2).any(|w| w[0] == w[1] || !adjacent(r.level, w[0], w[1])) {
            v.push(Violation::InvalidEdge { reach: r.id, why: "vertices not an 8-connected chain" });
        }
        let n = r.vertices.len();
        let a = &r.attrs;
        if F64_FIELDS.iter().any(|(_, get)| get(a).len() != n) || a.strahler.len() != n || a.bed.len() != n {
            v.push(Violation::InvalidEdge { reach: r.id, why: "attributes not one per vertex" });
            continue;
        }
        // bed: non-increasing on codes, except at an exception marker's vertex on this reach
        let exceptions: BTreeSet<u32> = g
            .nodes
            .iter()
            .filter(|m| m.kind == NodeKind::Exception)
            .filter_map(|m| m.marker)
            .filter(|m| m.on_reach == r.id)
            .map(|m| m.at_vertex)
            .collect();
        for k in 1..n {
            if a.bed[k] > a.bed[k - 1] && !exceptions.contains(&(k as u32)) {
                v.push(Violation::BedRise { reach: r.id, vertex: k });
            }
        }
        // conservation along the reach: the areas and discharges do not fall downstream (q_w may fall only at a basin, which is a node, never inside a reach)
        for (name, get) in [("a_geo", F64_FIELDS[0].1), ("a_run", F64_FIELDS[2].1), ("q", F64_FIELDS[4].1), ("q_w", F64_FIELDS[5].1)] {
            if get(a).windows(2).any(|w| falls(w[0], w[1])) {
                v.push(Violation::ConservationBreak { at: r.id, field: name });
            }
        }
    }
    // node degrees and confluences
    for n in &g.nodes {
        let out = outgoing.get(&n.id).map_or(0, Vec::len);
        let inc = incoming.get(&n.id).map_or(&[][..], |x| &x[..]);
        let wants = if n.kind.is_outlet() || n.kind.is_marker() { 0 } else { 1 };
        if out != wants && !(open && out == 0) {
            v.push(Violation::OutgoingCount { node: n.id, outgoing: out });
        }
        if n.kind == NodeKind::Confluence && inc.len() < 2 {
            v.push(Violation::ConfluenceOneInflow { node: n.id });
        }
        if n.kind == NodeKind::Confluence && inc.len() >= 2 && out == 1 {
            let o = outgoing[&n.id][0];
            for (name, get) in [("a_geo", F64_FIELDS[0].1), ("q", F64_FIELDS[4].1), ("q_w", F64_FIELDS[5].1)] {
                // one value at the node, shared by every reach there, and at least the inflows one vertex upstream
                let at = get(&o.attrs)[0];
                let shared = inc.iter().all(|r| get(&r.attrs).last().is_some_and(|x| *x == at));
                let donors: f64 = inc.iter().map(|r| get(&r.attrs).iter().rev().nth(1).copied().unwrap_or(0.0)).sum();
                if !shared || falls(donors, at) {
                    v.push(Violation::ConservationBreak { at: n.id, field: name });
                }
            }
            // a node's bed is one value shared by its reaches
            if inc.iter().any(|r| r.attrs.bed.last() != o.attrs.bed.first()) {
                v.push(Violation::BedRise { reach: o.id, vertex: 0 });
            }
        }
    }
    // cycles and chains without an outlet: follow each node's single outgoing reach
    for n in &g.nodes {
        let mut at = n.id;
        let mut steps = 0usize;
        while let Some(k) = node.get(&at) {
            if k.kind.is_outlet() {
                break;
            }
            let Some(out) = outgoing.get(&at).and_then(|o| o.first()) else {
                if !open && !k.kind.is_marker() {
                    v.push(Violation::ChainWithoutOutlet { node: n.id });
                }
                break;
            };
            at = out.down;
            steps += 1;
            if steps > g.reaches.len() {
                v.push(Violation::Cycle { reach: out.id });
                break;
            }
        }
    }
    v.dedup();
    v
}

/// The rasterisation of the L reaches (contract section 5): each L cell belongs to one reach, the larger `q_w` there, ties by the lower reach id; a junction cell belongs to the downstream reach
/// (the reach whose first vertex it is). Returns `cell -> (reach id, vertex index)` in cell order.
pub fn rasterise(g: &Graph) -> BTreeMap<u64, (u64, usize)> {
    let mut best: BTreeMap<u64, (u64, usize, f64, bool)> = BTreeMap::new();
    for r in g.reaches.iter().filter(|r| r.level == Level::L) {
        for (k, &c) in r.vertices.iter().enumerate() {
            let qw = r.attrs.q_w.get(k).copied().unwrap_or(0.0);
            let first = k == 0;
            let better = |o: &(u64, usize, f64, bool)| {
                if first != o.3 {
                    return first;
                }
                qw > o.2 || (qw == o.2 && r.id < o.0)
            };
            if best.get(&c).is_none_or(better) {
                best.insert(c, (r.id, k, qw, first));
            }
        }
    }
    best.into_iter().map(|(c, (id, k, _, _))| (c, (id, k))).collect()
}

/// The validator of a raster's ownership: every claim `(cell, reach)` must be the only claim on its cell.
pub fn validate_raster(claims: &[(u64, u64)]) -> Vec<Violation> {
    let mut owner: BTreeMap<u64, u64> = BTreeMap::new();
    let mut v = Vec::new();
    for &(cell, reach) in claims {
        match owner.get(&cell) {
            Some(&other) if other != reach => v.push(Violation::CellOwnedTwice { cell, reaches: (other.min(reach), other.max(reach)) }),
            _ => {
                owner.insert(cell, reach);
            }
        }
    }
    v
}

/// One stage of the terrain-change budget (contract section 6): its output surface, and what it asked for where it was clamped.
pub struct Stage<'a> {
    pub name: &'static str,
    pub out: &'a [f64],
    /// The lowering the stage wanted per cell (`None`: never clamped).
    pub wanted_lowering: Option<&'a [f64]>,
    /// Whether fills count against the budget (KV and KC; KS only lowers).
    pub fills: bool,
}

/// A clamp record: the cell, the stage, what it needed, what was available, and the feature that is flagged `exception(budget_residual)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Residual {
    pub cell: usize,
    pub stage: &'static str,
    pub needed_m: f64,
    pub available_m: f64,
    pub feature: u64,
}

/// The budget validator on f64 (contract section 6): per cell, the lowering of the stages in order (`max(0, in - out)`) summed against `budget`, each stage's fill against `budget`; a cell over
/// either without a residual of that (cell, stage) is an overrun; a stage that lowered less than it wanted needs a residual; every residual's feature must be a reach flagged `budget_residual`.
pub fn validate_budget(s0: &[f64], budget: &[f64], stages: &[Stage], residuals: &[Residual], g: &Graph) -> Vec<Violation> {
    let mut v = Vec::new();
    let has = |cell: usize, stage: &str| residuals.iter().any(|r| r.cell == cell && r.stage == stage);
    let mut used = vec![0.0f64; s0.len()];
    let mut input: Vec<f64> = s0.to_vec();
    for st in stages {
        assert_eq!(st.out.len(), s0.len(), "one value per cell");
        for c in 0..s0.len() {
            let lowered = (input[c] - st.out[c]).max(0.0);
            let filled = (st.out[c] - input[c]).max(0.0);
            used[c] += lowered;
            // reported at the stage that lowers past the budget
            if lowered > 0.0 && used[c] > budget[c] + BUDGET_EPS_M && !has(c, st.name) {
                v.push(Violation::BudgetOverrun { cell: c, stage: st.name, used_m: used[c], budget_m: budget[c] });
            }
            if st.fills && filled > budget[c] + BUDGET_EPS_M && !has(c, st.name) {
                v.push(Violation::BudgetOverrun { cell: c, stage: st.name, used_m: filled, budget_m: budget[c] });
            }
            if let Some(w) = st.wanted_lowering {
                if w[c] > lowered + BUDGET_EPS_M && !has(c, st.name) {
                    v.push(Violation::ClampWithoutResidual { cell: c, stage: st.name });
                }
            }
        }
        input.copy_from_slice(st.out);
    }
    for r in residuals {
        let flagged =
            g.reaches.iter().any(|x| x.id == r.feature && matches!(x.status, Status::Exception { reason: Reason::BudgetResidual, .. }));
        if !flagged {
            v.push(Violation::ResidualFeatureNotFlagged { feature: r.feature });
        }
    }
    v
}

/// The slice of a graph a set of domains holds: the items those domains own. Endpoints owned elsewhere stay as ids (validate the slice with `open`).
pub fn slice(g: &Graph, holds: &dyn Fn(Domain) -> bool) -> Graph {
    let mut s = Graph {
        body: g.body,
        nodes: g.nodes.iter().filter(|n| holds(n.owner)).cloned().collect(),
        reaches: g.reaches.iter().filter(|r| holds(r.owner)).cloned().collect(),
        basins: g.basins.iter().filter(|b| holds(b.owner)).cloned().collect(),
    };
    s.sort();
    s
}
