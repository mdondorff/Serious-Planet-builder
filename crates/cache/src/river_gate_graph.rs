//! The G2 gate columns for river graphs (BUILD-060; the G0 contracts' sections 5, 6 and 9 and the plan addendum's graph reports): what the river gate reads from a canonical graph
//! (GEN-189), from the per-stage budget fields and from a second partition's graph, beside its raster numbers. Pure functions; the river gate's `measure` calls them when the input names a graph,
//! stages or a partition pair, so a B row without them prints none of these columns. Every column carries its gate class ([`GateClass`]).
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use planet_generators::bake::river_graph::{
    diff, rasterise, validate, Change, Checks, Graph, Level, NodeKind, Reason, Represented, Violation,
};
use planet_generators::bake::route::NONE;

use crate::river_suite::{fit, UPHILL_M};

/// The gate class of a column (the plan addendum's section 4): a pass in one class never covers a failure in another. BUILD-060 emits no incremental or product column (the incremental
/// build and the owner's flight are measured elsewhere); the classes are kept so the names match the addendum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateClass {
    Correctness,
    Partition,
    Incremental,
    Preservation,
    Geomorphology,
    Product,
}

impl GateClass {
    pub fn name(self) -> &'static str {
        match self {
            GateClass::Correctness => "correctness",
            GateClass::Partition => "partition",
            GateClass::Incremental => "incremental",
            GateClass::Preservation => "preservation",
            GateClass::Geomorphology => "geomorphology",
            GateClass::Product => "product",
        }
    }
}

/// The window's L cells as global cell ids: cell `(x, y)` of the window is the global L cell `(x0 + x, y0 + y)`, id `(gy << 32) | gx` (`Grid::global_id`'s plane form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub x0: u64,
    pub y0: u64,
    pub w: usize,
    pub h: usize,
}

impl Window {
    /// The window cell of a global L cell id, `None` outside the window.
    pub fn cell(&self, id: u64) -> Option<usize> {
        let (gx, gy) = (id & 0xFFFF_FFFF, id >> 32);
        (gx >= self.x0 && gy >= self.y0 && gx < self.x0 + self.w as u64 && gy < self.y0 + self.h as u64)
            .then(|| (gy - self.y0) as usize * self.w + (gx - self.x0) as usize)
    }
}

/// Two global cell ids of one level are 8-neighbours by their plane coordinates (a window lies on one face; a graph from another face never matches it). The level is not read: both
/// levels use the plane form `(row << 32) | column`.
pub fn adjacent(_: Level, a: u64, b: u64) -> bool {
    let d = |p: u64, q: u64| p.abs_diff(q);
    a != b && d(a & 0xFFFF_FFFF, b & 0xFFFF_FFFF) <= 1 && d(a >> 32, b >> 32) <= 1
}

/// The per-stage diagnostic fields of the terrain-change budget (G0 contracts section 6): u16 `floor(m / 0.1)` per window cell, the lowering and (KV, KC) the fill, in stage order.
#[derive(Clone, Debug, PartialEq)]
pub struct StageField {
    pub name: String,
    pub lower: Vec<u16>,
    pub fill: Option<Vec<u16>>,
}

/// The diagnostic fields' step (metres).
pub const STAGE_STEP_M: f64 = 0.1;

/// What the G2 columns read besides the gate's own input.
pub struct G2Input<'a> {
    pub window: Window,
    pub graph: Option<&'a Graph>,
    pub stages: Option<&'a [StageField]>,
    /// The residual records written with the stages (count), when given.
    pub residual_records: Option<usize>,
    /// The canonical graphs of one region and of two regions (the partition diff).
    pub partition: Option<(&'a Graph, &'a Graph)>,
    /// KC's patches written with the stages (`kc-patches.csv`, lead #172): their count and total volume (m3), when given.
    pub kc_patches: Option<(usize, f64)>,
}

/// The graph report (BUILD-060 item 1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphReport {
    pub nodes: usize,
    pub reaches_h1: usize,
    pub reaches_l: usize,
    pub basins: usize,
    /// The validators' violations (GEN-189; an open slice: endpoints owned elsewhere are not violations), by kind, and the endpoints owned elsewhere.
    pub violations: BTreeMap<&'static str, usize>,
    pub missing_owner: usize,
    /// Sources; those whose chain (followed through outgoing reaches) ends at an outlet of the graph; those that end at a node the slice does not hold.
    pub sources: usize,
    pub sources_to_outlet: usize,
    pub sources_open: usize,
    /// Trunk reaches (H1, or L with a parent) and those whose `down` node is held, neither an outlet nor left by a reach (a break).
    pub trunk_reaches: usize,
    pub trunk_breaks: usize,
    /// Bed profile: reaches whose bed rises above its running minimum somewhere (exception vertices included), the largest such rise (m), and the validator's rises outside exceptions.
    pub bed_rise_reaches: usize,
    pub bed_residual_max_m: f64,
    pub bed_rises: usize,
    /// Exceptional reaches by reason.
    pub exc_rise_over_budget: usize,
    pub exc_unresolved_endpoint: usize,
    pub exc_budget_residual: usize,
    /// L tributaries (L reaches without a parent) and those that end at a held node of a kind a tributary may end at (a `trib_junction`, a confluence, an outlet, a tile crossing).
    pub tributaries: usize,
    pub tributaries_attached: usize,
    /// Hierarchy over the rasterised L graph inside the counted land: channel length (km), the land area (km2), the density, Strahler streams by order and `R_b` (BUILD-059's fit).
    pub channel_km: f64,
    pub land_km2: f64,
    pub density_km_per_km2: f64,
    pub streams: Vec<usize>,
    pub rb_orders: usize,
    pub rb: f64,
    /// Basins: kept (protected), endorheic; a parent the graph does not hold; a spill node the graph does not hold; a same-level parent whose spill lies below the child's or floor above it.
    pub basins_protected: usize,
    pub basins_endorheic: usize,
    pub basin_parent_missing: usize,
    pub basin_spill_missing: usize,
    /// Basins whose floor lies outside the counted cells and whose spill node the graph does not hold (apart from `basin_spill_missing` and its bar).
    pub basin_spill_missing_edge: usize,
    pub basin_order_bad: usize,
}

/// Graph/raster agreement at one channel threshold (contract section 5): routed river cells (land inside, drained area at least the threshold) with a rasterised graph cell within one cell
/// (Chebyshev), and rasterised cells inside with a routed river cell within one cell. Counted apart, outside both shares (lead #166): cells inside a kept basin's interior
/// (`routed_kept`, `graph_kept`), and routed cells upstream of a graph head (`routed_above_head`: a miss whose downstream path first comes within one cell of the graph near a source).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agreement {
    pub threshold_km2: f64,
    pub routed: usize,
    pub routed_agree: usize,
    pub graph: usize,
    pub graph_agree: usize,
    pub routed_kept: usize,
    pub routed_above_head: usize,
    pub graph_kept: usize,
}

/// The river steps inside kept-basin interiors (lead #166): a kept basin (`protected_surface`) whose floor cell lies in a depression of the measured surface holds that depression. Its river steps (BUILD-057's, either end inside the interior) and their uphill ones are counted apart from the uphill bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeptReport {
    pub basins: usize,
    pub cells: usize,
    pub steps: usize,
    pub uphill: usize,
}

/// One stage's budget use (land inside, against `S0`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StageUse {
    pub name: String,
    pub lowered: usize,
    pub lower_p50_m: f64,
    pub lower_p95_m: f64,
    pub lower_max_m: f64,
    pub filled: usize,
    pub fill_max_m: f64,
    /// Cells whose fill exceeds the budget.
    pub fill_over: usize,
    /// Cells the stage lowers by more than [`OVER_CUT_M`] (plan section 4.1: ground lowered over 50 m per stage).
    pub over_cut: usize,
}

/// A stage's lowering counted as deep (metres): the plan's "ground lowered > 50 m" per stage (lead #172).
pub const OVER_CUT_M: f64 = 50.0;

/// The budget report (BUILD-060 item 3).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BudgetReport {
    pub land: usize,
    pub stages: Vec<StageUse>,
    /// Cells whose summed lowering exceeds the budget by more than the fields' rounding (a field's step per stage); cells at 95 % of the budget or more.
    pub overrun: usize,
    pub at_95: usize,
    /// The same with stage `kh`'s lowering added (reading 21: the use against `S0`; equal to the above without a `kh` field).
    pub overrun_s0: usize,
    pub at_95_s0: usize,
    pub residual_records: Option<usize>,
    /// Reaches flagged `budget_residual` in the graph, when a graph is given.
    pub residual_reaches: Option<usize>,
    /// KC's patches: count and total volume (m3), when the stages carry `kc-patches.csv`.
    pub kc_patches: Option<(usize, f64)>,
}

/// The partition diff (BUILD-060 item 4): the changes between the canonical graphs of one and of two regions, the topology changes among them, and the first change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartitionReport {
    pub changes: usize,
    pub topology: usize,
    pub first: String,
}

/// The G2 report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct G2Report {
    pub graph: Option<GraphReport>,
    pub agreement: Vec<Agreement>,
    /// The kept-basin interiors' river steps (a row with a graph).
    pub kept: Option<KeptReport>,
    pub budget: Option<BudgetReport>,
    pub partition: Option<PartitionReport>,
}

fn violation_kind(v: &Violation) -> &'static str {
    match v {
        Violation::Cycle { .. } => "cycle",
        Violation::DuplicateId { .. } => "duplicate_id",
        Violation::IdMismatch { .. } => "id_mismatch",
        Violation::TwoNodesOneAnchor { .. } => "two_nodes_one_anchor",
        Violation::UnresolvedEndpoint { .. } => "unresolved_endpoint",
        Violation::ChainWithoutOutlet { .. } => "chain_without_outlet",
        Violation::ConfluenceOneInflow { .. } => "confluence_one_inflow",
        Violation::InflowsToNonConfluence { .. } => "inflows_to_non_confluence",
        Violation::OutgoingCount { .. } => "outgoing_count",
        Violation::InvalidEdge { .. } => "invalid_edge",
        Violation::InvalidMarker { .. } => "invalid_marker",
        Violation::TributaryLeavesTile { .. } => "tributary_leaves_tile",
        Violation::BedRise { .. } => "bed_rise",
        Violation::BedNotShared { .. } => "bed_not_shared",
        Violation::ConservationBreak { .. } => "conservation_break",
        Violation::NotFinite { .. } => "not_finite",
        Violation::DuplicateException { .. } => "duplicate_exception",
        Violation::CellOwnedTwice { .. } => "cell_owned_twice",
        Violation::BudgetOverrun { .. } => "budget_overrun",
        Violation::RaiseInLoweringStage { .. } => "raise_in_lowering_stage",
        Violation::ClampWithoutResidual { .. } => "clamp_without_residual",
        Violation::FillClampWithoutResidual { .. } => "fill_clamp_without_residual",
        Violation::ResidualMismatch { .. } => "residual_mismatch",
        Violation::ResidualFeatureNotFlagged { .. } => "residual_feature_not_flagged",
        Violation::BudgetInput { .. } => "budget_input",
    }
}

/// The graph report of `g` over the window: `inside` and `land` the gate's counted cells, `dx` the L cell size (m).
pub fn graph_report(g: &Graph, window: &Window, inside: &[bool], land: &[bool], dx: f64) -> GraphReport {
    let mut r = GraphReport { nodes: g.nodes.len(), basins: g.basins.len(), ..GraphReport::default() };
    r.reaches_h1 = g.reaches.iter().filter(|x| x.level == Level::H1).count();
    r.reaches_l = g.reaches.len() - r.reaches_h1;
    let checks = Checks { adjacent: &adjacent, tile_of: None, open: true };
    for v in validate(g, &checks) {
        *r.violations.entry(violation_kind(&v)).or_default() += 1;
    }
    r.bed_rises = r.violations.get("bed_rise").copied().unwrap_or(0);
    r.missing_owner = planet_generators::bake::river_graph::missing_owners(g).len();
    let kind: HashMap<u64, NodeKind> = g.nodes.iter().map(|n| (n.id, n.kind)).collect();
    // the reach leaving each node (the lowest id when a broken graph has two)
    let mut out: BTreeMap<u64, usize> = BTreeMap::new();
    let mut order: Vec<usize> = (0..g.reaches.len()).collect();
    order.sort_by_key(|&i| g.reaches[i].id);
    for &i in &order {
        out.entry(g.reaches[i].up).or_insert(i);
    }
    let marker_reach: HashMap<u64, u64> = g.nodes.iter().filter_map(|n| n.marker.map(|m| (n.id, m.on_reach))).collect();
    let reach_index: HashMap<u64, usize> = g.reaches.iter().enumerate().map(|(i, x)| (x.id, i)).collect();
    let mut sources: Vec<u64> = g.nodes.iter().filter(|n| n.kind == NodeKind::Source).map(|n| n.id).collect();
    sources.sort_unstable();
    r.sources = sources.len();
    for s in sources {
        let (mut at, mut steps) = (s, 0usize);
        loop {
            match kind.get(&at) {
                None => {
                    r.sources_open += 1;
                    break;
                }
                Some(k) if k.is_outlet() => {
                    r.sources_to_outlet += 1;
                    break;
                }
                _ => {}
            }
            // a chain is followed through a marker along the reach it sits on (the contract's degrees rule)
            let next = match marker_reach.get(&at) {
                Some(&rid) => reach_index.get(&rid).copied(),
                None => out.get(&at).copied(),
            };
            match next {
                Some(i) if steps <= g.reaches.len() => {
                    at = g.reaches[i].down;
                    steps += 1;
                }
                _ => break,
            }
        }
    }
    for x in g.reaches.iter().filter(|x| x.level == Level::H1 || x.parent.is_some()) {
        r.trunk_reaches += 1;
        if let Some(k) = kind.get(&x.down) {
            if !k.is_outlet() && !out.contains_key(&x.down) && !k.is_marker() {
                r.trunk_breaks += 1;
            }
        }
    }
    for x in &g.reaches {
        let (mut low, mut worst) = (i32::MAX, 0i32);
        for &b in &x.attrs.bed {
            low = low.min(b);
            worst = worst.max(b - low);
        }
        if worst > 0 {
            r.bed_rise_reaches += 1;
            r.bed_residual_max_m = r.bed_residual_max_m.max(f64::from(worst) * 0.5);
        }
        r.exc_rise_over_budget += usize::from(x.has_exception(Reason::RiseOverBudget));
        r.exc_unresolved_endpoint += usize::from(x.has_exception(Reason::UnresolvedEndpoint));
        r.exc_budget_residual += usize::from(x.has_exception(Reason::BudgetResidual));
        if x.level == Level::L && x.parent.is_none() {
            r.tributaries += 1;
            let ok = kind.get(&x.down).is_some_and(|k| {
                matches!(
                    k,
                    NodeKind::TribJunction | NodeKind::Confluence | NodeKind::OutletSea | NodeKind::LakeInlet | NodeKind::TileCrossing
                )
            });
            r.tributaries_attached += usize::from(ok);
        }
    }
    // hierarchy on the rasterised L graph: each owned cell once; a step counts from the cell that owns it; a stream of order k starts at an owned cell of order k that no donor of order k enters
    let raster = rasterise(g);
    let reach_at: HashMap<u64, usize> = g.reaches.iter().enumerate().map(|(i, x)| (x.id, i)).collect();
    let mut inflow_orders: HashMap<u64, Vec<u8>> = HashMap::new();
    for x in g.reaches.iter().filter(|x| x.level == Level::L) {
        if let Some(&o) = x.attrs.strahler.last() {
            inflow_orders.entry(x.down).or_default().push(o);
        }
    }
    let (mut len_m, mut streams) = (0.0, Vec::<usize>::new());
    for (&cell, &(rid, k)) in &raster {
        let Some(c) = window.cell(cell) else { continue };
        if !inside[c] || !land[c] {
            continue;
        }
        let x = &g.reaches[reach_at[&rid]];
        if let Some(&next) = x.vertices.get(k + 1) {
            let (dx_c, dy_c) = ((next & 0xFFFF_FFFF).abs_diff(cell & 0xFFFF_FFFF), (next >> 32).abs_diff(cell >> 32));
            len_m += libm::hypot(dx_c as f64, dy_c as f64) * dx;
        }
        let Some(&o) = x.attrs.strahler.get(k) else { continue };
        let continues = if k == 0 { inflow_orders.get(&x.up).is_some_and(|v| v.contains(&o)) } else { x.attrs.strahler[k - 1] == o };
        if !continues && o > 0 {
            if streams.len() < o as usize {
                streams.resize(o as usize, 0);
            }
            streams[o as usize - 1] += 1;
        }
    }
    let land_cells = (0..inside.len()).filter(|&c| inside[c] && land[c]).count();
    r.land_km2 = land_cells as f64 * dx * dx / 1.0e6;
    r.channel_km = len_m / 1000.0;
    r.density_km_per_km2 = if r.land_km2 > 0.0 { r.channel_km / r.land_km2 } else { 0.0 };
    let pts: Vec<(f64, f64)> = streams
        .iter()
        .enumerate()
        .take(streams.len().saturating_sub(1))
        .filter(|(_, &s)| s > 0)
        .map(|(k, &s)| ((k + 1) as f64, libm::log10(s as f64)))
        .collect();
    r.rb_orders = pts.len();
    r.rb = if pts.len() >= 2 { libm::pow(10.0, -fit(&pts)) } else { 0.0 };
    r.streams = streams;
    let basins: HashMap<u64, usize> = g.basins.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let nodes: BTreeSet<u64> = g.nodes.iter().map(|n| n.id).collect();
    for b in &g.basins {
        r.basins_protected += usize::from(b.represented == planet_generators::bake::river_graph::Represented::ProtectedSurface);
        r.basins_endorheic += usize::from(b.outlet == planet_generators::bake::river_graph::Outlet::Endorheic);
        // a basin whose floor lies outside the counted cells (the window's margin) is counted apart: its spill path may leave the window (lead, 2026-11-18)
        if !nodes.contains(&b.spill_node) {
            if window.cell(b.floor_cell).is_some_and(|c| inside[c]) {
                r.basin_spill_missing += 1;
            } else {
                r.basin_spill_missing_edge += 1;
            }
        }
        if let Some(p) = b.parent {
            match basins.get(&p) {
                None => r.basin_parent_missing += 1,
                Some(&i) => {
                    let pb = &g.basins[i];
                    if pb.level == b.level && (b.spill_m > pb.spill_m + 1e-9 || b.floor_m < pb.floor_m - 1e-9) {
                        r.basin_order_bad += 1;
                    }
                }
            }
        }
    }
    r
}

/// Dilates a mask by one cell (Chebyshev).
fn dilate(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut out = vec![false; m.len()];
    for c in (0..m.len()).filter(|&c| m[c]) {
        let (x, y) = (c % w, c / w);
        for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                out[yy * w + xx] = true;
            }
        }
    }
    out
}

/// The kept-basin interiors on the window (lead #166; [`KeptReport`]): `label` the gate's depression of each cell on the measured surface (BUILD-057's basin flood, `u32::MAX` dry); an
/// interior is every cell of a depression that holds a kept basin's floor cell. Returns per cell whether it lies in one, and the number of kept basins whose floor lies in a depression.
pub fn kept_interiors(g: &Graph, window: &Window, label: &[u32]) -> (Vec<bool>, usize) {
    let mut held = BTreeSet::new();
    let mut basins = 0usize;
    for b in g.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface) {
        if let Some(f) = window.cell(b.floor_cell).filter(|&f| label[f] != u32::MAX) {
            held.insert(label[f]);
            basins += 1;
        }
    }
    (label.iter().map(|l| *l != u32::MAX && held.contains(l)).collect(), basins)
}

/// The river steps of BUILD-057 (a river cell inside to its river receiver; uphill above [`UPHILL_M`] on `surface`) with either end in `kept`.
pub fn kept_steps(kept: &[bool], basins: usize, receiver: &[u32], river: &[bool], inside: &[bool], surface: &[f64]) -> KeptReport {
    let mut k = KeptReport { basins, cells: (0..kept.len()).filter(|&c| kept[c] && inside[c]).count(), ..KeptReport::default() };
    for c in (0..kept.len()).filter(|&c| river[c] && inside[c]) {
        let r = receiver[c];
        if r == NONE || !river[r as usize] || !(kept[c] || kept[r as usize]) {
            continue;
        }
        k.steps += 1;
        k.uphill += usize::from(surface[r as usize] - surface[c] > UPHILL_M);
    }
    k
}

/// The graph/raster agreement at `threshold_m2` (contract section 5): `area` the gate's drained area (m2), `land` above the sea, `inside` the counted cells; `kept` the kept-basin interiors
/// and `receiver` the gate's routing (lead #166: both counted apart, [`Agreement`]).
#[allow(clippy::too_many_arguments)]
pub fn agreement(
    g: &Graph,
    window: &Window,
    area: &[f64],
    land: &[bool],
    inside: &[bool],
    kept: &[bool],
    receiver: &[u32],
    threshold_m2: f64,
) -> Agreement {
    let n = window.w * window.h;
    let mut graph = vec![false; n];
    for &cell in rasterise(g).keys() {
        if let Some(c) = window.cell(cell) {
            graph[c] = true;
        }
    }
    let mut head = vec![false; n];
    for nd in g.nodes.iter().filter(|nd| nd.kind == NodeKind::Source && nd.level == Level::L) {
        if let Some(c) = window.cell(nd.anchor) {
            head[c] = true;
        }
    }
    let routed: Vec<bool> = (0..n).map(|c| area[c] >= threshold_m2 && land[c]).collect();
    let (gd, rd, hd) = (dilate(&graph, window.w, window.h), dilate(&routed, window.w, window.h), dilate(&head, window.w, window.h));
    // per cell off the graph: 1 when its downstream path first comes within one cell of the graph next to a head, 2 elsewhere or never (0 not yet known)
    let mut first = vec![0u8; n];
    let mut path = Vec::new();
    let mut a = Agreement { threshold_km2: threshold_m2 / 1.0e6, ..Agreement::default() };
    for c in (0..n).filter(|&c| inside[c]) {
        if routed[c] {
            if kept[c] {
                a.routed_kept += 1;
            } else if gd[c] {
                a.routed += 1;
                a.routed_agree += 1;
            } else {
                let mut at = c;
                let label = loop {
                    if first[at] != 0 {
                        break first[at];
                    }
                    if gd[at] {
                        break if hd[at] { 1 } else { 2 };
                    }
                    path.push(at);
                    let r = receiver[at];
                    if r == NONE || path.len() > n {
                        break 2;
                    }
                    at = r as usize;
                };
                for &p in &path {
                    first[p] = label;
                }
                path.clear();
                if label == 1 {
                    a.routed_above_head += 1;
                } else {
                    a.routed += 1;
                }
            }
        }
        if graph[c] {
            if kept[c] {
                a.graph_kept += 1;
            } else {
                a.graph += 1;
                a.graph_agree += usize::from(rd[c]);
            }
        }
    }
    a
}

fn quant(v: &mut [f64], f: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let k = ((v.len() as f64 * f) as usize).min(v.len() - 1);
    *v.select_nth_unstable_by(k, f64::total_cmp).1
}

/// The terrain-change budget field (G0 contracts section 6) on the window's L cells, from `S0`: `block` L cells an H1 cell (the window's origin on an H1 corner), `relief_1km` per H1 cell the range
/// of `S0` over the L cells within 5 cells (Chebyshev) of its centre L cell (index `block / 2`), `relief10` its p50 (the upper median) over the H1 cells within `r_relief_m`, and
/// `budget = min(50, max(10, 0.5 relief10))` m, read at an L cell through its H1 cell.
pub fn budget_field(s0: &[f64], w: usize, h: usize, block: usize, dx: f64, r_relief_m: f64) -> Vec<f64> {
    let (bw, bh) = (w.div_ceil(block), h.div_ceil(block));
    let relief: Vec<f64> = (0..bw * bh)
        .map(|b| {
            let (cx, cy) = (((b % bw) * block + block / 2).min(w - 1), ((b / bw) * block + block / 2).min(h - 1));
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for y in cy.saturating_sub(5)..=(cy + 5).min(h - 1) {
                for x in cx.saturating_sub(5)..=(cx + 5).min(w - 1) {
                    lo = lo.min(s0[y * w + x]);
                    hi = hi.max(s0[y * w + x]);
                }
            }
            hi - lo
        })
        .collect();
    let rad = r_relief_m / (block as f64 * dx);
    let ri = rad.ceil() as i64;
    let offsets: Vec<(i64, i64)> =
        (-ri..=ri).flat_map(|y| (-ri..=ri).map(move |x| (x, y))).filter(|&(x, y)| libm::hypot(x as f64, y as f64) <= rad).collect();
    let budget: Vec<f64> = (0..bw * bh)
        .map(|b| {
            let (bx, by) = ((b % bw) as i64, (b / bw) as i64);
            let mut v: Vec<f64> = offsets
                .iter()
                .filter_map(|&(ox, oy)| {
                    let (x, y) = (bx + ox, by + oy);
                    (x >= 0 && y >= 0 && x < bw as i64 && y < bh as i64).then(|| relief[y as usize * bw + x as usize])
                })
                .collect();
            let k = v.len() / 2;
            (0.5 * *v.select_nth_unstable_by(k, f64::total_cmp).1).clamp(10.0, 50.0)
        })
        .collect();
    (0..w * h).map(|c| budget[(c / w / block) * bw + (c % w) / block]).collect()
}

/// The budget report: per stage the lowering and the fill from the diagnostic fields, over land inside, against `budget`.
pub fn budget_report(
    stages: &[StageField],
    budget: &[f64],
    land: &[bool],
    inside: &[bool],
    residual_records: Option<usize>,
    g: Option<&Graph>,
) -> BudgetReport {
    let n = budget.len();
    let mut r = BudgetReport {
        residual_records,
        residual_reaches: g.map(|g| g.reaches.iter().filter(|x| x.has_exception(Reason::BudgetResidual)).count()),
        ..BudgetReport::default()
    };
    let cells: Vec<usize> = (0..n).filter(|&c| inside[c] && land[c]).collect();
    r.land = cells.len();
    // reading 21: ks, kv and kc are against S_KH; kh (S0 against KH) only enters the use against S0
    let mut used = vec![0.0f64; n];
    let mut used_kh = vec![0.0f64; n];
    for st in stages {
        let mut u = StageUse { name: st.name.clone(), ..StageUse::default() };
        let mut low: Vec<f64> = Vec::new();
        for &c in &cells {
            let l = f64::from(st.lower[c]) * STAGE_STEP_M;
            if l > 0.0 {
                low.push(l);
                if st.name == "kh" {
                    used_kh[c] += l;
                } else {
                    used[c] += l;
                }
            }
            if let Some(f) = &st.fill {
                let fm = f64::from(f[c]) * STAGE_STEP_M;
                if fm > 0.0 {
                    u.filled += 1;
                    u.fill_max_m = u.fill_max_m.max(fm);
                    u.fill_over += usize::from(fm > budget[c]);
                }
            }
        }
        u.lowered = low.len();
        u.over_cut = low.iter().filter(|&&l| l > OVER_CUT_M).count();
        u.lower_max_m = low.iter().copied().fold(0.0, f64::max);
        u.lower_p50_m = quant(&mut low, 0.5);
        u.lower_p95_m = quant(&mut low, 0.95);
        r.stages.push(u);
    }
    // a field floors each stage's use to 0.1 m, so the sum is at most the true use: over the budget it is a true overrun; an overrun under the fields' step is not seen, so these
    // counts are diagnostic (G0 contracts section 6: the fields are never the gate; GEN-189's f64 budget validator is)
    for &c in &cells {
        r.overrun += usize::from(used[c] > budget[c] + 1e-9);
        r.at_95 += usize::from(used[c] >= 0.95 * budget[c]);
        let s0 = used[c] + used_kh[c];
        r.overrun_s0 += usize::from(s0 > budget[c] + 1e-9);
        r.at_95_s0 += usize::from(s0 >= 0.95 * budget[c]);
    }
    r
}

/// The partition diff of the canonical graphs of one and of two regions.
pub fn partition_report(one: &Graph, two: &Graph) -> PartitionReport {
    let d = diff(one, two);
    let first = d.first().map_or("identical".to_string(), |c| match c {
        Change::Added { what, id } => format!("added:{what}:{id:016x}"),
        Change::Removed { what, id } => format!("removed:{what}:{id:016x}"),
        Change::Field { what, id, field } => format!("field:{what}:{id:016x}:{field}"),
    });
    PartitionReport { changes: d.len(), topology: d.iter().filter(|c| c.is_topology()).count(), first }
}

impl G2Report {
    /// The columns (name, value, class), in a fixed order; a part the input did not name prints no column.
    pub fn columns(&self) -> Vec<(String, String, GateClass)> {
        use GateClass::*;
        let mut f: Vec<(String, String, GateClass)> = Vec::new();
        let mut push = |n: &str, v: String, c: GateClass| f.push((n.to_string(), v, c));
        if let Some(g) = &self.graph {
            let viol: usize = g.violations.values().sum();
            push("g_nodes", g.nodes.to_string(), Correctness);
            push("g_reaches_h1", g.reaches_h1.to_string(), Correctness);
            push("g_reaches_l", g.reaches_l.to_string(), Correctness);
            push("g_violations", viol.to_string(), Correctness);
            push(
                "g_violations_by_kind",
                if g.violations.is_empty() {
                    "-".into()
                } else {
                    g.violations.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join("/")
                },
                Correctness,
            );
            push("g_missing_owner", g.missing_owner.to_string(), Correctness);
            push("g_sources", g.sources.to_string(), Correctness);
            push("g_sources_to_outlet", g.sources_to_outlet.to_string(), Correctness);
            push("g_sources_to_outlet_pct", format!("{:.2}", 100.0 * g.sources_to_outlet as f64 / g.sources.max(1) as f64), Correctness);
            push("g_sources_open", g.sources_open.to_string(), Correctness);
            push("g_trunk_reaches", g.trunk_reaches.to_string(), Correctness);
            push("g_trunk_breaks", g.trunk_breaks.to_string(), Correctness);
            push("g_conservation_breaks", g.violations.get("conservation_break").copied().unwrap_or(0).to_string(), Correctness);
            push("g_bed_rises", g.bed_rises.to_string(), Correctness);
            push("g_bed_rise_reaches", g.bed_rise_reaches.to_string(), Correctness);
            push("g_bed_residual_max_m", format!("{:.1}", g.bed_residual_max_m), Correctness);
            push("g_exc_rise_over_budget", g.exc_rise_over_budget.to_string(), Correctness);
            push("g_exc_unresolved_endpoint", g.exc_unresolved_endpoint.to_string(), Correctness);
            push("g_exc_budget_residual", g.exc_budget_residual.to_string(), Correctness);
            push("g_tributaries", g.tributaries.to_string(), Correctness);
            push("g_tributaries_attached", g.tributaries_attached.to_string(), Correctness);
            push("g_channel_km", format!("{:.1}", g.channel_km), Geomorphology);
            push("g_density_km_per_km2", format!("{:.4}", g.density_km_per_km2), Geomorphology);
            push(
                "g_streams_by_order",
                if g.streams.is_empty() { "-".into() } else { g.streams.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("/") },
                Geomorphology,
            );
            push("g_rb_orders", g.rb_orders.to_string(), Geomorphology);
            push("g_rb", format!("{:.3}", g.rb), Geomorphology);
            push("g_basins", g.basins.to_string(), Correctness);
            push("g_basins_protected", g.basins_protected.to_string(), Correctness);
            push("g_basins_endorheic", g.basins_endorheic.to_string(), Correctness);
            push("g_basin_parent_missing", g.basin_parent_missing.to_string(), Correctness);
            push("g_basin_spill_missing", g.basin_spill_missing.to_string(), Correctness);
            push("g_basin_spill_missing_edge", g.basin_spill_missing_edge.to_string(), Correctness);
            push("g_basin_order_bad", g.basin_order_bad.to_string(), Correctness);
        }
        for (i, a) in self.agreement.iter().enumerate() {
            let p = format!("agree{}", i + 1);
            push(&format!("{p}_km2"), format!("{}", a.threshold_km2), Correctness);
            push(&format!("{p}_routed"), a.routed.to_string(), Correctness);
            push(&format!("{p}_routed_pct"), format!("{:.2}", 100.0 * a.routed_agree as f64 / a.routed.max(1) as f64), Correctness);
            push(&format!("{p}_graph"), a.graph.to_string(), Correctness);
            push(&format!("{p}_graph_pct"), format!("{:.2}", 100.0 * a.graph_agree as f64 / a.graph.max(1) as f64), Correctness);
            push(&format!("{p}_routed_kept"), a.routed_kept.to_string(), Correctness);
            push(&format!("{p}_routed_above_head"), a.routed_above_head.to_string(), Correctness);
            push(&format!("{p}_graph_kept"), a.graph_kept.to_string(), Correctness);
        }
        if let Some(k) = &self.kept {
            push("kept_basins", k.basins.to_string(), Correctness);
            push("kept_cells", k.cells.to_string(), Correctness);
            push("kept_steps", k.steps.to_string(), Correctness);
            push("kept_uphill_steps", k.uphill.to_string(), Correctness);
        }
        if let Some(b) = &self.budget {
            for s in &b.stages {
                let n = &s.name;
                push(&format!("b_{n}_lowered"), s.lowered.to_string(), Preservation);
                push(&format!("b_{n}_lowered_pct"), format!("{:.2}", 100.0 * s.lowered as f64 / b.land.max(1) as f64), Preservation);
                push(&format!("b_{n}_p95_m"), format!("{:.1}", s.lower_p95_m), Preservation);
                push(&format!("b_{n}_max_m"), format!("{:.1}", s.lower_max_m), Preservation);
                push(&format!("b_{n}_over50"), s.over_cut.to_string(), Preservation);
                push(&format!("b_{n}_filled"), s.filled.to_string(), Preservation);
                push(&format!("b_{n}_fill_max_m"), format!("{:.1}", s.fill_max_m), Preservation);
                push(&format!("b_{n}_fill_over"), s.fill_over.to_string(), Preservation);
            }
            push("b_land", b.land.to_string(), Preservation);
            push("b_overrun", b.overrun.to_string(), Preservation);
            push("b_at_95", b.at_95.to_string(), Preservation);
            push("b_at_95_pct", format!("{:.3}", 100.0 * b.at_95 as f64 / b.land.max(1) as f64), Preservation);
            push("b_overrun_s0", b.overrun_s0.to_string(), Preservation);
            push("b_at_95_s0", b.at_95_s0.to_string(), Preservation);
            push("b_residual_records", b.residual_records.map_or("-".into(), |v| v.to_string()), Correctness);
            push("b_residual_reaches", b.residual_reaches.map_or("-".into(), |v| v.to_string()), Correctness);
            if let Some((count, volume)) = b.kc_patches {
                push("b_kc_patches", count.to_string(), Preservation);
                push("b_kc_patch_km3", format!("{:.4}", volume / 1.0e9), Preservation);
            }
        }
        if let Some(p) = &self.partition {
            push("part_changes", p.changes.to_string(), Partition);
            push("part_topology", p.topology.to_string(), Partition);
            push("part_first", p.first.clone(), Partition);
        }
        f
    }

    /// The printed lines, one per part.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(g) = &self.graph {
            let viol: usize = g.violations.values().sum();
            out.push(format!(
                "RIVER-GATE graph [correctness]: {} nodes, {} H1 and {} L reaches, {} basins; {viol} violations ({}); {} endpoints owned elsewhere; {} of {} sources reach an outlet ({} end open); {} of {} trunk reaches break; {} conservation breaks; bed: {} rises outside exceptions, {} reaches rise above their running minimum (largest {:.1} m); exceptions: {} rise_over_budget, {} unresolved_endpoint, {} budget_residual; {} of {} tributaries attached; basins: {} protected, {} endorheic, {} parents and {} spill nodes missing ({} more outside the counted cells), {} out of order",
                g.nodes,
                g.reaches_h1,
                g.reaches_l,
                g.basins,
                if g.violations.is_empty() { "none".to_string() } else { g.violations.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ") },
                g.missing_owner,
                g.sources_to_outlet,
                g.sources,
                g.sources_open,
                g.trunk_breaks,
                g.trunk_reaches,
                g.violations.get("conservation_break").copied().unwrap_or(0),
                g.bed_rises,
                g.bed_rise_reaches,
                g.bed_residual_max_m,
                g.exc_rise_over_budget,
                g.exc_unresolved_endpoint,
                g.exc_budget_residual,
                g.tributaries_attached,
                g.tributaries,
                g.basins_protected,
                g.basins_endorheic,
                g.basin_parent_missing,
                g.basin_spill_missing,
                g.basin_spill_missing_edge,
                g.basin_order_bad
            ));
            out.push(format!(
                "RIVER-GATE graph hierarchy [geomorphology]: {:.1} km of rasterised L channel over {:.1} km2 of land inside, density {:.4} km/km2; Strahler streams by order {}; R_b {} (against BUILD-059's bar)",
                g.channel_km,
                g.land_km2,
                g.density_km_per_km2,
                if g.streams.is_empty() { "none".into() } else { g.streams.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" / ") },
                if g.rb_orders >= 2 { format!("{:.3} over orders 1 to {}", g.rb, g.rb_orders) } else { "none".into() }
            ));
        }
        for a in &self.agreement {
            out.push(format!(
                "RIVER-GATE graph/raster agreement at {} km2 [correctness]: {:.2} % of {} routed river cells have a graph cell within 1 cell; {:.2} % of {} graph cells have a routed river cell within 1 cell; apart: {} routed and {} graph cells in kept-basin interiors, {} routed cells above a graph head",
                a.threshold_km2,
                100.0 * a.routed_agree as f64 / a.routed.max(1) as f64,
                a.routed,
                100.0 * a.graph_agree as f64 / a.graph.max(1) as f64,
                a.graph,
                a.routed_kept,
                a.graph_kept,
                a.routed_above_head
            ));
        }
        if let Some(k) = &self.kept {
            out.push(format!(
                "RIVER-GATE kept basins [correctness]: {} kept basins hold {} cells inside; {} river steps in them, {} uphill (apart from the uphill bar)",
                k.basins, k.cells, k.steps, k.uphill
            ));
        }
        if let Some(b) = &self.budget {
            let st: Vec<String> = b
                .stages
                .iter()
                .map(|s| {
                    format!(
                        "{} lowers {} cells (p95 {:.1} m, max {:.1} m), fills {} (max {:.1} m, {} over the budget)",
                        s.name, s.lowered, s.lower_p95_m, s.lower_max_m, s.filled, s.fill_max_m, s.fill_over
                    )
                })
                .collect();
            out.push(format!(
                "RIVER-GATE budget [preservation; diagnostic fields, the f64 validator is the correctness gate]: over {} land cells inside, the budget from S0's relief: {}; vs S_KH (ks, kv, kc) {} cells over the budget, {} at 95 % of it or more; vs S0 (kh added) {} over, {} at 95 % or more; residual records {}, residual reaches {}",
                b.land,
                st.join("; "),
                b.overrun,
                b.at_95,
                b.overrun_s0,
                b.at_95_s0,
                b.residual_records.map_or("not given".into(), |v| v.to_string()),
                b.residual_reaches.map_or("no graph".into(), |v| v.to_string())
            ));
        }
        if let Some(p) = &self.partition {
            out.push(format!(
                "RIVER-GATE partition [partition]: one region against two: {} changes, {} of them topology; first {}",
                p.changes, p.topology, p.first
            ));
        }
        out
    }
}

/// The H1 trunk reaches of a canonical graph as the gate's trunks (BUILD-060 `--start`, lead #171): each H1-level reach's vertices, upstream first, as the window's L cell coordinates
/// of the H1 cell centres (`coarse` L cells per H1 cell; an H1 id is `(gy << 32) | gx` in global H1 cells). A reach with a vertex outside the window is cut there; a reach left with fewer than
/// two vertices is dropped.
pub fn h1_trunks(g: &Graph, window: &Window, coarse: usize) -> Vec<Vec<(f64, f64)>> {
    let c = coarse as f64;
    g.reaches
        .iter()
        .filter(|r| r.level == Level::H1)
        .filter_map(|r| {
            let v: Vec<(f64, f64)> = r
                .vertices
                .iter()
                .map(|&id| {
                    (
                        (id & 0xffff_ffff) as f64 * c + (c - 1.0) / 2.0 - window.x0 as f64,
                        (id >> 32) as f64 * c + (c - 1.0) / 2.0 - window.y0 as f64,
                    )
                })
                .take_while(|&(x, y)| x >= 0.0 && y >= 0.0 && x < window.w as f64 && y < window.h as f64)
                .collect();
            (v.len() >= 2).then_some(v)
        })
        .collect()
}

/// A candidate's surface on the window's L grid (BUILD-060 `--surface`): `n` little-endian f64 heights, row-major; refused when the byte count differs or a height is not finite.
pub fn read_surface(path: &str, n: usize) -> Result<Vec<f64>, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.len() != 8 * n {
        return Err(format!("{path}: {} bytes, the window's {n} cells of f64 are {}", b.len(), 8 * n));
    }
    let z: Vec<f64> = b.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect();
    match z.iter().position(|v| !v.is_finite()) {
        Some(c) => Err(format!("{path}: cell {c} is not finite")),
        None => Ok(z),
    }
}

/// A canonical river graph file (GEN-189).
pub fn read_graph(path: &str) -> Result<Graph, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Graph::parse(&text).map_err(|e| format!("{path}: {e}"))
}

/// KC's patches of a stage directory (lead #172): `kc-patches.csv`, a header `cells,volume_m3` and one line per patch; none when the file is missing. A line that is not two numbers
/// is refused naming the file and the line.
pub fn read_kc_patches(dir: &Path) -> Result<Option<(usize, f64)>, String> {
    let p = dir.join("kc-patches.csv");
    if !p.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let (mut count, mut volume) = (0usize, 0.0f64);
    for (k, line) in text.lines().enumerate().skip(1).filter(|(_, l)| !l.trim().is_empty()) {
        let v = line.split(',').nth(1).and_then(|x| x.trim().parse::<f64>().ok()).ok_or(format!(
            "{}: line {}: not `cells,volume_m3`",
            p.display(),
            k + 1
        ))?;
        count += 1;
        volume += v;
    }
    Ok(Some((count, volume)))
}

/// The spectrum bars (plan section 4.1, lead #172): the relief spectrum slope `beta` within the benchmark's range ([`BETA_MIN`] to [`BETA_MAX`], SPIKE-119's 25 GLO-30 and 3DEP
/// tiles), as two bars (too steep, too rough), geomorphology. The comparison with B's loss is the table's (BUILD-060).
pub fn spectrum_bars(beta: f64) -> Vec<Bar> {
    vec![
        Bar { name: "bar_spectrum_steep".into(), value: beta, limit: BETA_MIN, at_most: false, class: GateClass::Geomorphology },
        Bar { name: "bar_spectrum_rough".into(), value: beta, limit: BETA_MAX, at_most: true, class: GateClass::Geomorphology },
    ]
}

/// The benchmark's spectrum range (SPIKE-119's benchmark.csv: 25 GLO-30 and 3DEP tiles).
pub const BETA_MIN: f64 = -4.13;
pub const BETA_MAX: f64 = -2.05;

/// The per-stage budget fields of a directory (BUILD-060): `<stage>-lower.u16` and `<stage>-fill.u16` for stage ks, kv, kc (little-endian u16 of 0.1 m, row-major on the window's L grid; a missing
/// file is a stage that did not run, or a stage without fills), and `residuals.csv` (a header and one line per residual record), when present.
pub fn read_stages(dir: &Path, n: usize) -> Result<(Vec<StageField>, Option<usize>), String> {
    let read = |name: &str| -> Result<Option<Vec<u16>>, String> {
        let p = dir.join(name);
        if !p.exists() {
            return Ok(None);
        }
        let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if b.len() != 2 * n {
            return Err(format!("{}: {} bytes, the window's {n} cells of u16 are {}", p.display(), b.len(), 2 * n));
        }
        Ok(Some(b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect()))
    };
    let mut out = Vec::new();
    for st in ["kh", "ks", "kv", "kc"] {
        if let Some(lower) = read(&format!("{st}-lower.u16"))? {
            out.push(StageField { name: st.to_string(), lower, fill: read(&format!("{st}-fill.u16"))? });
        }
    }
    if out.is_empty() {
        return Err(format!("{}: no kh-lower.u16, ks-lower.u16, kv-lower.u16 or kc-lower.u16", dir.display()));
    }
    let res = dir.join("residuals.csv");
    let records = if res.exists() {
        Some(
            std::fs::read_to_string(&res)
                .map_err(|e| format!("{}: {e}", res.display()))?
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .count(),
        )
    } else {
        None
    };
    Ok((out, records))
}

/// The G2 bars (owner decision 1, 2026-11-17; lead #168): the trunk corridor share (k_c 2, snapped), uphill river steps, graph/raster agreement at each threshold, graph violations and
/// missing basin spill nodes, each pass or fail.
pub const UPHILL_MAX_PCT: f64 = 1.0;
pub const AGREE_MIN_PCT: f64 = 90.0;
/// The corridor bar outside mountain windows, and on a mountain window (M1).
pub const CORRIDOR_MIN_PCT: f64 = 97.5;
pub const CORRIDOR_MIN_PCT_MOUNTAIN: f64 = 95.0;

/// One bar: a value against its limit, `at_most` or at least.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub name: String,
    pub value: f64,
    pub limit: f64,
    pub at_most: bool,
    pub class: GateClass,
}

impl Bar {
    pub fn pass(&self) -> bool {
        if self.at_most {
            self.value <= self.limit
        } else {
            self.value >= self.limit
        }
    }

    /// The table cell: `PASS:<value>` or `FAIL:<value>` (two decimals; no space).
    pub fn cell(&self) -> String {
        format!("{}:{:.2}", if self.pass() { "PASS" } else { "FAIL" }, self.value)
    }
}

/// The bars of a row with a graph (none without): `uphill_pct` and `corridor_pct` from the raster columns, the corridor bar `corridor_min_pct`.
pub fn g2_bars(g2: &G2Report, uphill_pct: f64, corridor_pct: f64, corridor_min_pct: f64) -> Vec<Bar> {
    use GateClass::*;
    let Some(g) = &g2.graph else { return Vec::new() };
    let bar = |name: String, value: f64, limit: f64, at_most: bool, class: GateClass| Bar { name, value, limit, at_most, class };
    let mut out = vec![
        bar("bar_corridor".into(), corridor_pct, corridor_min_pct, false, Preservation),
        bar("bar_uphill".into(), uphill_pct, UPHILL_MAX_PCT, true, Correctness),
    ];
    for a in &g2.agreement {
        let pct = 100.0 * a.routed_agree as f64 / a.routed.max(1) as f64;
        out.push(bar(format!("bar_agree_{}km2", a.threshold_km2), pct, AGREE_MIN_PCT, false, Correctness));
    }
    out.push(bar("bar_violations".into(), g.violations.values().sum::<usize>() as f64, 0.0, true, Correctness));
    out.push(bar("bar_spill_missing".into(), g.basin_spill_missing as f64, 0.0, true, Correctness));
    out
}

/// The bars' line: `RIVER-GATE bars [lead #168]: name PASS value (<= or >= limit); ...; n of m pass`.
pub fn bars_line(bars: &[Bar]) -> Option<String> {
    if bars.is_empty() {
        return None;
    }
    let parts: Vec<String> = bars
        .iter()
        .map(|b| {
            format!(
                "{} {} {:.2} ({} {})",
                b.name,
                if b.pass() { "PASS" } else { "FAIL" },
                b.value,
                if b.at_most { "<=" } else { ">=" },
                b.limit
            )
        })
        .collect();
    Some(format!(
        "RIVER-GATE bars [owner decision 1]: {}; {} of {} pass",
        parts.join("; "),
        bars.iter().filter(|b| b.pass()).count(),
        bars.len()
    ))
}
