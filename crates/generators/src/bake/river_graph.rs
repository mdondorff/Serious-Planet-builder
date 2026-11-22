//! The canonical river graph (GEN-189; the G0 contracts of the river-graph plan, sections 1 to 8): its entities, identities, canonical serialisation, diff, rasterisation and validators, and the
//! terrain-change budget's validator. Data and checks only: no algorithm here builds a graph from terrain (KS, KG, KV and KC wait for their milestones).
//!
//! - **Entities.** Nodes (kinds `source`, `confluence`, `trib_junction` and `exception` markers, `tile_crossing`, `outlet_sea`, `lake_inlet`, `lake_spill`), reaches (directed downstream, an 8-connected chain
//!   of global cell ids from `up`'s anchor to `down`'s, per-vertex attributes) and basins, at two levels (H1 and L), owned by a domain (`landmass` or a canonical tile).
//! - **Identities.** `node = hash(body, kind, level, anchor)` (a marker also hashes its reach: [`marker_id`], reading 8), `reach = hash(body, level, up, down)`, `basin = hash(body, level, floor anchor, spill anchor)` ([`id_hash`], GEN-145's mixer). Ownership never enters an id.
//! - **Canonical form.** Nodes, reaches and basins sorted by id, fixed field order, f64 as IEEE-754 bit patterns in hex: the same graph gives the same bytes, and [`Graph::parse`] reads them back.
//! - **Diff.** Ids added or removed (topology) apart from same ids with different fields (attributes), each named by (id, field).
//! - **Validators.** Each check of the contract's section 8 returns [`Violation`]s; none repairs anything.
//!
//! Sums and orders are fixed (CON-14); nothing here reads a thread count.

use super::hydro::id_hash;
use std::collections::{BTreeMap, BTreeSet};
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
            let face = p[1].parse::<u8>().ok().filter(|f| *f < 6).ok_or_else(|| format!("domain '{s}'"))?;
            return Ok(Domain::Tile { face, ti: n(p[2])?, tj: n(p[3])? });
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

/// One exception of a reach: its reason and value (reading 9: a reach carries a list of them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exception {
    pub reason: Reason,
    pub value: f64,
}

impl Reach {
    /// Whether the reach carries an exception for `reason`.
    pub fn has_exception(&self, reason: Reason) -> bool {
        self.exceptions.iter().any(|e| e.reason == reason)
    }
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
    /// The reach's exceptions (reading 9): empty for an ordinary reach; each reported, never repaired.
    pub exceptions: Vec<Exception>,
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

/// The salt of a marker id (reading 15: the spikes lane's G1c value).
pub const MARKER_SALT: u64 = 0x8A2E_0008;

/// The id of a marker node (reading 8): its node hash with the reach it sits on, so markers are unique per (reach, anchor).
pub fn marker_id(body: u64, kind: NodeKind, level: Level, anchor: u64, on_reach: u64) -> u64 {
    id_hash(node_id(body, kind, level, anchor), on_reach, MARKER_SALT)
}

/// The id a node must have: [`marker_id`] for a node with a marker field, else [`node_id`].
pub fn node_id_of(body: u64, n: &Node) -> u64 {
    match n.marker {
        Some(m) => marker_id(body, n.kind, n.level, n.anchor, m.on_reach),
        None => node_id(body, n.kind, n.level, n.anchor),
    }
}

/// The id of a reach (GEN-145's rule on node ids).
pub fn reach_id(body: u64, level: Level, up: u64, down: u64) -> u64 {
    id_hash(body ^ REACH_TAG ^ level.code(), up, down)
}

/// The id of a basin: its floor and spill anchors (the contract's reading 4: a parent shares its floor with its deepest child, so the floor alone collides). `spill` is the spill cell,
/// `None` only for a basin with no spill cell at all; it enters as the cell plus one, so 0 means none.
pub fn basin_id(body: u64, level: Level, floor: u64, spill: Option<u64>) -> u64 {
    id_hash(body ^ BASIN_TAG ^ level.code(), floor, spill.map_or(0, |s| s.checked_add(1).expect("a spill cell id below u64::MAX")))
}

/// The spill anchor of a stored basin: its spill cell, or `None` when it has none (a spill height of +inf; a NaN or -inf height is not finite, which the validator reports).
pub fn spill_anchor(b: &Basin) -> Option<u64> {
    (b.spill_m != f64::INFINITY).then_some(b.spill_cell)
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
        for r in &mut self.reaches {
            r.exceptions.sort_unstable_by_key(|e| (e.reason.name(), (e.value + 0.0).to_bits()));
        }
    }

    /// The canonical serialisation: one line per item in id order, fixed field order, f64 as bit patterns.
    pub fn canonical(&self) -> String {
        let mut g = self.clone();
        g.sort();
        let mut s = format!("river-graph 2 body={:016x}\n", g.body);
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
            // the exceptions in a fixed order: by the reason's name, then by the value's bits
            // (a signed zero written as +0.0)
            let mut ex: Vec<(&str, u64)> = r.exceptions.iter().map(|e| (e.reason.name(), (e.value + 0.0).to_bits())).collect();
            ex.sort_unstable();
            let exceptions =
                if ex.is_empty() { "-".to_string() } else { ex.iter().map(|(n, b)| format!("{n}:{b:016x}")).collect::<Vec<_>>().join(";") };
            let _ = write!(
                s,
                "reach {:016x} level={} up={:016x} down={:016x} parent={} owner={} exceptions={} vertices={}",
                r.id,
                r.level.name(),
                r.up,
                r.down,
                opt(r.parent),
                r.owner.text(),
                exceptions,
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
        let body = head.strip_prefix("river-graph 2 body=").ok_or_else(|| format!("header '{head}'"))?;
        // exactly the 16 lowercase digits the canonical form writes
        let hex = |s: &str| {
            if s.len() != 16 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(format!("hex '{s}'"));
            }
            u64::from_str_radix(s, 16).map_err(|_| format!("hex '{s}'"))
        };
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
                    let exceptions = match get("exceptions")? {
                        "-" => Vec::new(),
                        s => s
                            .split(';')
                            .map(|e| {
                                let (r, v) = e.split_once(':').ok_or_else(|| format!("exception '{e}'"))?;
                                Ok(Exception { reason: Reason::parse(r)?, value: f(v)? })
                            })
                            .collect::<Result<Vec<Exception>, String>>()?,
                    };
                    // the canonical order only: by the reason's name, then the value's bits
                    if exceptions
                        .windows(2)
                        .any(|w| (w[0].reason.name(), w[0].value.to_bits()) > (w[1].reason.name(), w[1].value.to_bits()))
                    {
                        return Err(format!("exceptions out of order '{}'", get("exceptions")?));
                    }
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
                        exceptions,
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
                        outlet: match get("outlet")? {
                            "active" => Outlet::Active,
                            "endorheic" => Outlet::Endorheic,
                            o => return Err(format!("outlet '{o}'")),
                        },
                        represented: match get("represented")? {
                            "protected_surface" => Represented::ProtectedSurface,
                            "none" => Represented::None,
                            o => return Err(format!("represented '{o}'")),
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
    Cycle {
        reach: u64,
    },
    DuplicateId {
        id: u64,
    },
    /// An id that is not the hash of its defining fields (a kind or an endpoint changed under a kept id).
    IdMismatch {
        id: u64,
    },
    TwoNodesOneAnchor {
        level: Level,
        anchor: u64,
    },
    UnresolvedEndpoint {
        reach: u64,
        node: u64,
    },
    ChainWithoutOutlet {
        node: u64,
    },
    ConfluenceOneInflow {
        node: u64,
    },
    /// Two or more reaches enter a node that is not a confluence (or an outlet).
    InflowsToNonConfluence {
        node: u64,
    },
    OutgoingCount {
        node: u64,
        outgoing: usize,
    },
    InvalidEdge {
        reach: u64,
        why: &'static str,
    },
    /// A marker that does not sit on its reach (`on_reach` missing, `at_vertex` out of range, or the vertex not its anchor), or a marker field on a non-marker kind.
    InvalidMarker {
        node: u64,
        why: &'static str,
    },
    /// A tributary reach whose vertices after the first leave one tile core.
    TributaryLeavesTile {
        reach: u64,
    },
    BedRise {
        reach: u64,
        vertex: usize,
    },
    /// The reaches at a node carry different beds there, or a tributary's last bed is not the trunk's at its junction.
    BedNotShared {
        node: u64,
    },
    ConservationBreak {
        at: u64,
        field: &'static str,
    },
    /// A value that is not finite.
    NotFinite {
        at: u64,
        field: &'static str,
    },
    /// A reach lists one reason twice (reading 9: one exception per reason).
    DuplicateException {
        at: u64,
        reason: Reason,
    },
    CellOwnedTwice {
        cell: u64,
        reaches: (u64, u64),
    },
    BudgetOverrun {
        cell: usize,
        stage: &'static str,
        used_m: f64,
        budget_m: f64,
    },
    /// A stage that only lowers (KS) raised a cell.
    RaiseInLoweringStage {
        cell: usize,
        stage: &'static str,
    },
    ClampWithoutResidual {
        cell: usize,
        stage: &'static str,
    },
    /// A stage filled less than it wanted (reading 9) without a residual.
    FillClampWithoutResidual {
        cell: usize,
        stage: &'static str,
    },
    /// A residual whose `needed_m` or `available_m` is not the stage's (the wanted lowering and the budget left, or the fill and the budget), or that records no clamp or fill overrun.
    ResidualMismatch {
        cell: usize,
        stage: &'static str,
        why: &'static str,
    },
    ResidualFeatureNotFlagged {
        feature: u64,
    },
    /// The budget's inputs do not fit (lengths) or are not finite.
    BudgetInput {
        cell: usize,
        stage: &'static str,
    },
}

/// The 8-neighbour relation of the global cells at each level: `adjacent(level, a, b)`.
pub type Adjacency<'a> = &'a dyn Fn(Level, u64, u64) -> bool;

/// What the validator needs besides the graph.
pub struct Checks<'a> {
    pub adjacent: Adjacency<'a>,
    /// The tile core of an L cell (the tile-crossing rule); `None` skips that rule.
    pub tile_of: Option<&'a dyn Fn(u64) -> Domain>,
    /// An open slice: endpoints, markers' reaches and inflows owned elsewhere are not violations (see [`missing_owners`]).
    pub open: bool,
}

/// Reading 41 (lead, 2026-11-22): a bed rise at vertex `k` of `r` into the reach's own down node (its last vertex) is excused when the reach carries `budget_residual`
/// with a value (metres) of at least the rise (the codes' difference times 0.5 m, compared exactly up to 1e-9 m: the residual records the coded rise): the residual records
/// the need not done, and a down node has no place for a marker. `Some(rise in metres)` when excused.
fn down_rise_m(r: &Reach, k: usize) -> Option<f64> {
    let n = r.vertices.len();
    if k + 1 != n || k == 0 {
        return None;
    }
    let rise_m = 0.5 * (i64::from(r.attrs.bed[k]) - i64::from(r.attrs.bed[k - 1])) as f64;
    (rise_m > 0.0 && r.exceptions.iter().any(|e| e.reason == Reason::BudgetResidual && e.value.is_finite() && e.value + 1e-9 >= rise_m))
        .then_some(rise_m)
}

/// The bed rises reading 41 excuses, from [`validate_excused`]: their count and metres, for the gate's own column beside the violations.
pub fn excused_down_rises(g: &Graph, checks: &Checks) -> (usize, f64) {
    let (_, ex) = validate_excused(g, checks);
    (ex.len(), ex.iter().map(|x| x.2).sum())
}

/// Whether `b` lies below `a` beyond the relative tolerance.
fn falls(a: f64, b: f64) -> bool {
    b < a - CONSERVATION_REL * a.abs().max(b.abs())
}

/// The conserved fields, checked along a reach and at a node.
const CONSERVED: [(&str, usize); 4] = [("a_geo", 0), ("a_run", 2), ("q", 4), ("q_w", 5)];

/// Validates a graph (contract section 8, with the readings of its section 12). Works on a sorted copy, so the violations come in a fixed order whatever the input order.
pub fn validate(graph: &Graph, checks: &Checks) -> Vec<Violation> {
    validate_excused(graph, checks).0
}

/// [`validate`], and the bed rises reading 41 excused on the way: `(reach, vertex, metres)` for each rise into a reach's own down node held by its `budget_residual`, on
/// whole reaches only and not where a marker already excuses it (numerics review: the gate counts exactly what the validator accepted).
pub fn validate_excused(graph: &Graph, checks: &Checks) -> (Vec<Violation>, Vec<(u64, usize, f64)>) {
    let mut g = graph.clone();
    g.sort();
    let (adjacent, open) = (checks.adjacent, checks.open);
    let mut v = Vec::new();
    let mut excused: Vec<(u64, usize, f64)> = Vec::new();
    // duplicate ids, and ids that are not the hash of their fields
    let mut seen = BTreeSet::new();
    for id in g.nodes.iter().map(|n| n.id).chain(g.reaches.iter().map(|r| r.id)).chain(g.basins.iter().map(|b| b.id)) {
        if !seen.insert(id) {
            v.push(Violation::DuplicateId { id });
        }
    }
    for n in &g.nodes {
        if n.id != node_id_of(g.body, n) {
            v.push(Violation::IdMismatch { id: n.id });
        }
    }
    for r in &g.reaches {
        if r.id != reach_id(g.body, r.level, r.up, r.down) {
            v.push(Violation::IdMismatch { id: r.id });
        }
    }
    for b in &g.basins {
        if b.id != basin_id(g.body, b.level, b.floor_cell, spill_anchor(b)) {
            v.push(Violation::IdMismatch { id: b.id });
        }
        // a basin's heights: a finite floor; a finite spill, or +inf for a basin with no spill cell
        if !b.floor_m.is_finite() {
            v.push(Violation::NotFinite { at: b.id, field: "floor_m" });
        }
        if !b.spill_m.is_finite() && b.spill_m != f64::INFINITY {
            v.push(Violation::NotFinite { at: b.id, field: "spill_m" });
        }
    }
    let node: BTreeMap<u64, &Node> = g.nodes.iter().map(|n| (n.id, n)).collect();
    let reach: BTreeMap<u64, &Reach> = g.reaches.iter().map(|r| (r.id, r)).collect();
    // markers: a marker kind carries its reach and vertex, on that reach, at its anchor; a non-marker carries none
    for n in &g.nodes {
        match (n.kind.is_marker(), n.marker) {
            (true, None) => v.push(Violation::InvalidMarker { node: n.id, why: "a marker kind without its reach" }),
            (false, Some(_)) => v.push(Violation::InvalidMarker { node: n.id, why: "a marker field on a non-marker" }),
            (true, Some(m)) => match reach.get(&m.on_reach) {
                None if !open => v.push(Violation::InvalidMarker { node: n.id, why: "its reach does not exist" }),
                None => {}
                Some(r) => match r.vertices.get(m.at_vertex as usize) {
                    None => v.push(Violation::InvalidMarker { node: n.id, why: "at_vertex out of range" }),
                    Some(&c) if c != n.anchor || r.level != n.level => {
                        v.push(Violation::InvalidMarker { node: n.id, why: "not at its reach's vertex" })
                    }
                    // a reach ends at a node, so a junction on its last vertex joins nothing (numerics review of reading 8)
                    Some(_) if n.kind == NodeKind::TribJunction && m.at_vertex as usize + 1 == r.vertices.len() => {
                        v.push(Violation::InvalidMarker { node: n.id, why: "a junction at its reach's last vertex" })
                    }
                    _ => {}
                },
            },
            (false, None) => {}
        }
    }
    // uniqueness (section 2.1, readings 1 and 8): one non-marker node per (level, anchor); markers of one kind once per (reach, anchor); a marker shares an anchor with a non-marker only as the `up` node of
    // the reach it sits on, at vertex 0
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
        let kinds: BTreeSet<(NodeKind, Option<u64>)> = ms.iter().map(|m| (m.kind, m.marker.map(|x| x.on_reach))).collect();
        let shared_ok = |m: &Node| {
            m.marker.is_some_and(|mk| {
                mk.at_vertex == 0
                    && plain.get(key).is_some_and(|ids| ids.len() == 1 && reach.get(&mk.on_reach).map(|r| r.up) == Some(ids[0]))
            })
        };
        if kinds.len() < ms.len() || (plain.contains_key(key) && !ms.iter().all(|m| shared_ok(m))) {
            twice.insert(*key);
        }
    }
    // tributaries ending at one (level, anchor) end at one trib_junction, whatever reaches the junctions sit on (section 2.1 with reading 8)
    let ends: BTreeSet<u64> = g.reaches.iter().map(|r| r.down).collect();
    for (key, ms) in &markers {
        if ms.iter().filter(|m| m.kind == NodeKind::TribJunction && ends.contains(&m.id)).count() > 1 {
            twice.insert(*key);
        }
    }
    for (level, anchor) in twice {
        v.push(Violation::TwoNodesOneAnchor { level, anchor });
    }
    // exception markers by reach
    let mut excepted: BTreeMap<u64, BTreeSet<u32>> = BTreeMap::new();
    for m in g.nodes.iter().filter(|m| m.kind == NodeKind::Exception) {
        if let Some(mk) = m.marker {
            excepted.entry(mk.on_reach).or_default().insert(mk.at_vertex);
        }
    }
    let mut outgoing: BTreeMap<u64, Vec<&Reach>> = BTreeMap::new();
    let mut incoming: BTreeMap<u64, Vec<&Reach>> = BTreeMap::new();
    let mut whole: BTreeSet<u64> = BTreeSet::new();
    for r in &g.reaches {
        outgoing.entry(r.up).or_default().push(r);
        incoming.entry(r.down).or_default().push(r);
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
            if d.kind == NodeKind::Exception {
                v.push(Violation::InvalidEdge { reach: r.id, why: "down is an exception marker" });
            }
            if d.level != r.level {
                v.push(Violation::InvalidEdge { reach: r.id, why: "down at another level" });
            }
            if r.vertices.last() != Some(&d.anchor) {
                v.push(Violation::InvalidEdge { reach: r.id, why: "chain does not end at down's anchor" });
            }
        }
        if r.vertices.len() < 2 || r.vertices.windows(2).any(|w| w[0] == w[1] || !adjacent(r.level, w[0], w[1])) {
            v.push(Violation::InvalidEdge { reach: r.id, why: "vertices not an 8-connected chain" });
        }
        let n = r.vertices.len();
        let a = &r.attrs;
        if n == 0 || F64_FIELDS.iter().any(|(_, get)| get(a).len() != n) || a.strahler.len() != n || a.bed.len() != n {
            v.push(Violation::InvalidEdge { reach: r.id, why: "attributes not one per vertex" });
            continue;
        }
        whole.insert(r.id);
        for (name, get) in F64_FIELDS {
            if get(a).iter().any(|x| !x.is_finite()) {
                v.push(Violation::NotFinite { at: r.id, field: name });
            }
        }
        if r.exceptions.iter().any(|e| !e.value.is_finite()) {
            v.push(Violation::NotFinite { at: r.id, field: "exceptions" });
        }
        for (k, e) in r.exceptions.iter().enumerate() {
            if r.exceptions[..k].iter().any(|x| x.reason == e.reason) {
                v.push(Violation::DuplicateException { at: r.id, reason: e.reason });
            }
        }
        // a tributary (an L reach that is no trunk image) lies in one tile core plus its first vertex
        if let (Some(tile_of), Level::L, None) = (checks.tile_of, r.level, r.parent) {
            if r.vertices[1..].iter().any(|&c| tile_of(c) != tile_of(r.vertices[1])) {
                v.push(Violation::TributaryLeavesTile { reach: r.id });
            }
        }
        // bed: non-increasing on codes, except at an exception marker's vertex on this reach, (reading 32, spike only) at the down end of a two-vertex L reach that
        // carries a rise_over_budget exception (it has no vertex for the marker), and (reading 41) a rise into the reach's own down node no larger than its budget_residual
        // value (a down node takes no marker)
        let ex = excepted.get(&r.id);
        let short_rise = n == 2 && r.level == Level::L && r.has_exception(Reason::RiseOverBudget);
        for k in 1..n {
            if a.bed[k] <= a.bed[k - 1] || ex.is_some_and(|e| e.contains(&(k as u32))) || (short_rise && k == 1) {
                continue;
            }
            match down_rise_m(r, k) {
                Some(m) => excused.push((r.id, k, m)),
                None => v.push(Violation::BedRise { reach: r.id, vertex: k }),
            }
        }
        // conservation along the reach
        for (name, f) in CONSERVED {
            if F64_FIELDS[f].1(a).windows(2).any(|w| falls(w[0], w[1])) {
                v.push(Violation::ConservationBreak { at: r.id, field: name });
            }
        }
    }
    // nodes: degrees, the shared value and bed at a node joining reaches, conservation at a confluence (inflows by id, as the canonical order)
    for n in &g.nodes {
        let outs = outgoing.get(&n.id).map_or(&[][..], |x| &x[..]);
        let ins = incoming.get(&n.id).map_or(&[][..], |x| &x[..]);
        let wants = if n.kind.is_outlet() || n.kind.is_marker() { 0 } else { 1 };
        if outs.len() != wants && !(open && outs.is_empty()) {
            v.push(Violation::OutgoingCount { node: n.id, outgoing: outs.len() });
        }
        if n.kind == NodeKind::Confluence && ins.len() < 2 && !open {
            v.push(Violation::ConfluenceOneInflow { node: n.id });
        }
        if n.kind != NodeKind::Confluence && !n.kind.is_outlet() && !n.kind.is_marker() && ins.len() > 1 {
            v.push(Violation::InflowsToNonConfluence { node: n.id });
        }
        let ins: Vec<&Reach> = ins.iter().copied().filter(|r| whole.contains(&r.id)).collect();
        let Some(o) = outs.first().filter(|o| whole.contains(&o.id) && outs.len() == 1) else { continue };
        if ins.is_empty() {
            continue;
        }
        for (name, f) in CONSERVED {
            let get = F64_FIELDS[f].1;
            let at = get(&o.attrs)[0];
            let shared = ins.iter().all(|r| get(&r.attrs).last().is_some_and(|x| *x == at));
            let donors: f64 = ins
                .iter()
                .map(|r| {
                    let x = get(&r.attrs);
                    if x.len() >= 2 {
                        x[x.len() - 2]
                    } else {
                        0.0
                    }
                })
                .sum();
            if !shared || (n.kind == NodeKind::Confluence && falls(donors, at)) {
                v.push(Violation::ConservationBreak { at: n.id, field: name });
            }
        }
        if ins.iter().any(|r| r.attrs.bed.last() != o.attrs.bed.first()) {
            v.push(Violation::BedNotShared { node: n.id });
        }
    }
    // a trib_junction: every tributary ending there carries the trunk's bed at the junction's vertex
    for n in g.nodes.iter().filter(|n| n.kind == NodeKind::TribJunction) {
        let Some(trunk_bed) = n.marker.and_then(|m| {
            reach.get(&m.on_reach).filter(|r| whole.contains(&r.id)).and_then(|r| r.attrs.bed.get(m.at_vertex as usize).copied())
        }) else {
            continue;
        };
        if incoming
            .get(&n.id)
            .is_some_and(|ins| ins.iter().filter(|r| whole.contains(&r.id)).any(|r| r.attrs.bed.last() != Some(&trunk_bed)))
        {
            v.push(Violation::BedNotShared { node: n.id });
        }
    }
    // cycles and chains without an outlet: follow each node downstream, through a marker along the reach it sits on
    for n in &g.nodes {
        let mut at = n.id;
        let mut steps = 0usize;
        while let Some(k) = node.get(&at) {
            if k.kind.is_outlet() {
                break;
            }
            let next = if k.kind.is_marker() {
                k.marker.and_then(|m| reach.get(&m.on_reach)).map(|r| (r.id, r.down))
            } else {
                outgoing.get(&at).and_then(|o| o.first()).map(|r| (r.id, r.down))
            };
            let Some((rid, down)) = next else {
                if !open {
                    v.push(Violation::ChainWithoutOutlet { node: n.id });
                }
                break;
            };
            at = down;
            steps += 1;
            if steps > g.reaches.len() + 1 {
                v.push(Violation::Cycle { reach: rid });
                break;
            }
        }
    }
    v.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    v.dedup();
    (v, excused)
}

/// The endpoints and markers' reaches an open slice names but does not hold: `missing owner` (a store fact, contract section 1), as (the item that names it, the id it names).
pub fn missing_owners(g: &Graph) -> Vec<(u64, u64)> {
    let nodes: BTreeSet<u64> = g.nodes.iter().map(|n| n.id).collect();
    let reaches: BTreeSet<u64> = g.reaches.iter().map(|r| r.id).collect();
    let mut out: Vec<(u64, u64)> = Vec::new();
    for r in &g.reaches {
        for id in [r.up, r.down] {
            if !nodes.contains(&id) {
                out.push((r.id, id));
            }
        }
    }
    for n in &g.nodes {
        if let Some(m) = n.marker {
            if !reaches.contains(&m.on_reach) {
                out.push((n.id, m.on_reach));
            }
        }
    }
    for b in &g.basins {
        if !nodes.contains(&b.spill_node) {
            out.push((b.id, b.spill_node));
        }
    }
    out.sort_unstable();
    out
}

/// The rasterisation of the L reaches (contract section 5): each L cell belongs to one reach. A reach's last vertex is a junction cell: it belongs to the downstream reach, so it yields to any
/// other claim; among the other claims the larger `q_w` wins (`total_cmp`), ties by the lower reach id. Returns `cell -> (reach id, vertex index)` in cell order, the same for any input order.
pub fn rasterise(g: &Graph) -> BTreeMap<u64, (u64, usize)> {
    let mut sorted = g.clone();
    sorted.sort();
    // (reach id, vertex, q_w, arriving: the reach's last vertex)
    let mut best: BTreeMap<u64, (u64, usize, f64, bool)> = BTreeMap::new();
    for r in sorted.reaches.iter().filter(|r| r.level == Level::L) {
        let n = r.vertices.len();
        for (k, &c) in r.vertices.iter().enumerate() {
            let claim = (r.id, k, r.attrs.q_w.get(k).copied().unwrap_or(0.0) + 0.0, k + 1 == n); // + 0.0: -0.0 becomes +0.0, so a signed zero never decides against the id tie
            let better = |o: &(u64, usize, f64, bool)| {
                if claim.3 != o.3 {
                    return !claim.3;
                }
                match claim.2.total_cmp(&o.2) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Less => false,
                    std::cmp::Ordering::Equal => claim.0 < o.0,
                }
            };
            if best.get(&c).is_none_or(better) {
                best.insert(c, claim);
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
    /// The fill the stage wanted per cell (reading 9; `None`: never clamped); a fill held at the budget is a clamp with its residual.
    pub wanted_fill: Option<&'a [f64]>,
    /// Whether the stage may fill (KV and KC); a stage that may not (KS) only lowers.
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

/// The budget validator on f64 (contract section 6). Per cell, in stage order:
/// - the lowering `max(0, in - out)` adds to the cell's use; a use over `budget` is an overrun, reported once, at the stage that first passes the budget (a residual never excuses it: a clamp holds
///   the lowering at the remainder);
/// - a stage that only lowers must not raise; a fill `max(0, out - in)` over `budget` needs a residual of that (cell, stage);
/// - a stage that lowered less than it wanted needs a residual of that (cell, stage);
/// - a residual carries its stage's numbers (`needed_m`, `available_m` within 1e-9 m) and records a clamp or a fill overrun;
/// - every residual's feature is a reach flagged `budget_residual`;
/// - every input is finite and one value per cell.
pub fn validate_budget(s0: &[f64], budget: &[f64], stages: &[Stage], residuals: &[Residual], g: &Graph) -> Vec<Violation> {
    let mut v = Vec::new();
    let n = s0.len();
    if budget.len() != n {
        return vec![Violation::BudgetInput { cell: budget.len().min(n), stage: "budget" }];
    }
    for st in stages {
        if st.out.len() != n || st.wanted_lowering.is_some_and(|w| w.len() != n) || st.wanted_fill.is_some_and(|w| w.len() != n) {
            return vec![Violation::BudgetInput { cell: st.out.len().min(n), stage: st.name }];
        }
    }
    let mut used = vec![0.0f64; n];
    let mut input: Vec<f64> = s0.to_vec();
    for c in 0..n {
        if !s0[c].is_finite() || !budget[c].is_finite() {
            v.push(Violation::BudgetInput { cell: c, stage: "s0" });
        }
    }
    for st in stages {
        for c in 0..n {
            // a wanted value is finite and not negative; a stage that may not fill wants no fill
            let bad_wanted = |w: Option<&[f64]>| w.is_some_and(|w| !w[c].is_finite() || w[c] < 0.0);
            let fill_on_lowering = !st.fills && st.wanted_fill.is_some_and(|w| w[c] > 0.0);
            if !st.out[c].is_finite() || bad_wanted(st.wanted_lowering) || bad_wanted(st.wanted_fill) || fill_on_lowering {
                v.push(Violation::BudgetInput { cell: c, stage: st.name });
                // a finite output still uses the budget: a later overrun stays reported
                if st.out[c].is_finite() {
                    let lowered = (input[c] - st.out[c]).max(0.0);
                    let before = used[c];
                    used[c] += lowered;
                    if lowered > 0.0 && used[c] > budget[c] + BUDGET_EPS_M && before <= budget[c] + BUDGET_EPS_M {
                        v.push(Violation::BudgetOverrun { cell: c, stage: st.name, used_m: used[c], budget_m: budget[c] });
                    }
                }
                // a residual here records nothing that can be checked
                for _ in residuals.iter().filter(|r| r.cell == c && r.stage == st.name) {
                    v.push(Violation::ResidualMismatch { cell: c, stage: st.name, why: "its stage's output is not finite there" });
                }
                continue;
            }
            let lowered = (input[c] - st.out[c]).max(0.0);
            let filled = (st.out[c] - input[c]).max(0.0);
            let before = used[c];
            used[c] += lowered;
            if lowered > 0.0 && used[c] > budget[c] + BUDGET_EPS_M && before <= budget[c] + BUDGET_EPS_M {
                v.push(Violation::BudgetOverrun { cell: c, stage: st.name, used_m: used[c], budget_m: budget[c] });
            }
            if !st.fills && filled > 0.0 {
                v.push(Violation::RaiseInLoweringStage { cell: c, stage: st.name });
            }
            // the events of this cell and stage, and the residuals that record them one to one: a clamp (it lowered less than it wanted) needs a residual with the wanted lowering and the
            // budget left before the stage; a fill over the budget needs another with the fill and the budget; a fill held under what the stage wanted (reading 9) needs one with the
            // wanted fill and the budget
            let clamped = st.wanted_lowering.is_some_and(|w| w[c] > lowered + BUDGET_EPS_M);
            let fill_over = st.fills && filled > budget[c] + BUDGET_EPS_M;
            let fill_clamp = !fill_over && st.fills && st.wanted_fill.is_some_and(|f| f[c] > filled + BUDGET_EPS_M);
            let wf = st.wanted_fill.map_or(0.0, |f| f[c]);
            let near = |a: f64, b: f64| (a - b).abs() <= BUDGET_EPS_M;
            let w = st.wanted_lowering.map_or(0.0, |w| w[c]);
            let is_clamp = |r: &Residual| clamped && near(r.needed_m, w) && near(r.available_m, (budget[c] - before).max(0.0));
            let is_fill = |r: &Residual| {
                (fill_over && near(r.needed_m, filled) && near(r.available_m, budget[c]))
                    || (fill_clamp && near(r.needed_m, wf) && near(r.available_m, budget[c]))
            };
            let here: Vec<&Residual> = residuals.iter().filter(|r| r.cell == c && r.stage == st.name).collect();
            let clamp_at = here.iter().position(|r| is_clamp(r));
            let fill_at = (0..here.len()).find(|&i| is_fill(here[i]) && Some(i) != clamp_at).or_else(|| {
                // a residual matching both events serves the fill only when no other residual records the clamp
                (0..here.len()).find(|&i| is_fill(here[i]))
            });
            let (clamp_at, fill_at) = match (clamp_at, fill_at) {
                (Some(a), Some(b)) if a == b => (here.iter().enumerate().position(|(i, r)| i != b && is_clamp(r)), Some(b)),
                pair => pair,
            };
            if clamped && clamp_at.is_none() {
                v.push(Violation::ClampWithoutResidual { cell: c, stage: st.name });
            }
            if fill_over && fill_at.is_none() {
                v.push(Violation::BudgetOverrun { cell: c, stage: st.name, used_m: filled, budget_m: budget[c] });
            }
            if fill_clamp && fill_at.is_none() {
                v.push(Violation::FillClampWithoutResidual { cell: c, stage: st.name });
            }
            for (i, r) in here.iter().enumerate() {
                if Some(i) == clamp_at || Some(i) == fill_at {
                    continue;
                }
                let why = if !clamped && !fill_over && !fill_clamp {
                    "no clamp or fill overrun at its cell and stage"
                } else if is_clamp(r) || is_fill(r) {
                    "more residuals than clamps and fill overruns"
                } else if clamped {
                    "not the clamp's needed and available"
                } else {
                    "not the fill's needed and available"
                };
                v.push(Violation::ResidualMismatch { cell: c, stage: st.name, why });
            }
        }
        for (i, &o) in input.iter_mut().zip(st.out) {
            if o.is_finite() {
                *i = o;
            }
        }
    }
    for r in residuals {
        // a residual naming a cell outside the surface or a stage that did not run is visited nowhere above
        if r.cell >= n || !stages.iter().any(|st| st.name == r.stage) {
            v.push(Violation::ResidualMismatch { cell: r.cell, stage: r.stage, why: "no such cell or stage" });
        }
        let flagged = g.reaches.iter().any(|x| x.id == r.feature && x.has_exception(Reason::BudgetResidual));
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
