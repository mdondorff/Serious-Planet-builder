//! SPIKE-130 (spike branch spike/basins-g1e, never merged): a basin producer on the river graph's contract types (GEN-189; G0 contracts section 4 and its readings), at H1 or L.
//!
//! 1. The depression hierarchy (SPIKE-123's sweep on any grid): cells in increasing (height, id) order with a union-find; a cell with no processed neighbour starts a leaf; a cell joining two
//!    live depressions is their saddle and their parent's start; a depression meeting the outlet set (an outlet cell, or land already drained) is linked there: its spill cell and target.
//!    A tree never linked (no outlet in the domain) is closed: endorheic.
//! 2. Basins: the hierarchy nodes from `min_area_m2` (their subtree's true area) and `min_depth_m` (spill minus floor) are kept, `protected_surface`; smaller ones from `report_depth_m` and
//!    `report_cells` are reported, `represented: none`; the rest are no basin. A basin's parent is its nearest basin ancestor.
//! 3. The receiving node of a spill: the routed path (round rules on the raw surface's receivers) from the spill cell, to the first cell of another basin (its `lake_inlet`, anchored there) or
//!    of a given reach (a `trib_junction` on it); a path reaching an outlet first is `unresolved` (the contract has no external outlet).
//! 4. Inlets: a given reach entering a kept basin gets a `lake_inlet` on its first vertex inside it (several inlets, one spill).
//! 5. Ids: GEN-189's `basin_id` (floor and spill anchors, reading 4; the floor the lowest cell of the subtree, ties by the global cell id).
//!
//! Single-threaded, fixed orders (CON-14). No water balance.
// spike only (never merged): kept as run, so the runs keep their bits; main gets its own form through an OpenSpec change
#![allow(clippy::needless_range_loop, clippy::redundant_closure)]

use super::river_graph::*;
use super::route::{route_rules, RoundRules, NONE};
use super::Grid;
use std::collections::BTreeMap;

/// The node of a cell that drains to the outlets (not in any depression).
pub const DRAINED: u32 = u32::MAX;
const UNSEEN: u32 = u32::MAX - 1;

/// A depression of the hierarchy.
#[derive(Clone, Debug)]
pub struct Depression {
    pub kids: Vec<u32>,
    pub parent: u32,
    pub floor_cell: usize,
    pub floor_m: f64,
    /// The spill cell and height (`None` for a closed tree), and the node the overflow enters (`DRAINED`: the outlet side).
    pub spill: Option<(usize, f64, u32)>,
    /// The cells flooded in this node (not its children's).
    pub own: Vec<usize>,
}

/// The hierarchy and the node each cell is flooded in.
pub struct Hierarchy {
    pub nodes: Vec<Depression>,
    pub node_of: Vec<u32>,
}

fn find(uf: &mut [u32], mut a: u32) -> u32 {
    while uf[a as usize] != a {
        let p = uf[uf[a as usize] as usize];
        uf[a as usize] = p;
        a = p;
    }
    a
}

/// Builds the depression hierarchy of `z` on `grid` (its 8 neighbours) with `outlet` the outlet set.
pub fn hierarchy(grid: &Grid, z: &[f64], outlet: &[bool]) -> Hierarchy {
    let n = z.len();
    let mut order: Vec<u32> = (0..n as u32).filter(|&c| grid.is_active(c as usize)).collect();
    order.sort_unstable_by(|&a, &b| z[a as usize].total_cmp(&z[b as usize]).then(a.cmp(&b)));
    let mut node_of = vec![UNSEEN; n];
    let mut nodes: Vec<Depression> = Vec::new();
    let mut uf: Vec<u32> = Vec::new();
    let mut top: Vec<u32> = Vec::new();
    for &c in &order {
        let c = c as usize;
        let nbrs: Vec<usize> = (0..8).filter_map(|d| grid.neighbour(c, d)).collect();
        if outlet[c] {
            node_of[c] = DRAINED;
            for &q in &nbrs {
                let k = node_of[q];
                if k == UNSEEN || k == DRAINED {
                    continue;
                }
                let r = find(&mut uf, k);
                let t = top[r as usize] as usize;
                if nodes[t].spill.is_none() {
                    nodes[t].spill = Some((c, z[c], DRAINED));
                }
            }
            continue;
        }
        let mut roots: Vec<u32> = Vec::new();
        let mut drained_side: Option<usize> = None;
        for &q in &nbrs {
            let k = node_of[q];
            if k == UNSEEN {
                continue;
            }
            if k == DRAINED {
                drained_side.get_or_insert(q);
                continue;
            }
            let r = find(&mut uf, k);
            if nodes[top[r as usize] as usize].spill.is_some() {
                drained_side.get_or_insert(q);
                continue;
            }
            if !roots.contains(&r) {
                roots.push(r);
            }
        }
        roots.sort_unstable_by_key(|&r| top[r as usize]);
        if let Some(side) = drained_side {
            let t = node_of[side];
            for &r in &roots {
                let nd = &mut nodes[top[r as usize] as usize];
                nd.spill = Some((c, z[c], t));
            }
            node_of[c] = DRAINED;
            continue;
        }
        match roots.len() {
            0 => {
                let id = nodes.len() as u32;
                nodes.push(Depression { kids: Vec::new(), parent: DRAINED, floor_cell: c, floor_m: z[c], spill: None, own: vec![c] });
                uf.push(id);
                top.push(id);
                node_of[c] = id;
            }
            1 => {
                let t = top[roots[0] as usize];
                nodes[t as usize].own.push(c);
                node_of[c] = t;
            }
            _ => {
                let mut acc = roots[0];
                for &r in &roots[1..] {
                    let (ta, tb) = (top[acc as usize], top[r as usize]);
                    let id = nodes.len() as u32;
                    for t in [ta, tb] {
                        let nd = &mut nodes[t as usize];
                        nd.spill = Some((c, z[c], id));
                        nd.parent = id;
                    }
                    let (a, b) = (&nodes[ta as usize], &nodes[tb as usize]);
                    let (floor_cell, floor_m) = if (a.floor_m, a.floor_cell) <= (b.floor_m, b.floor_cell) {
                        (a.floor_cell, a.floor_m)
                    } else {
                        (b.floor_cell, b.floor_m)
                    };
                    nodes.push(Depression { kids: vec![ta, tb], parent: DRAINED, floor_cell, floor_m, spill: None, own: Vec::new() });
                    uf.push(id);
                    top.push(id);
                    uf[acc as usize] = id;
                    uf[r as usize] = id;
                    acc = id;
                }
                let t = top[acc as usize];
                nodes[t as usize].own.push(c);
                node_of[c] = t;
            }
        }
    }
    for v in node_of.iter_mut() {
        if *v == UNSEEN {
            *v = DRAINED;
        }
    }
    Hierarchy { nodes, node_of }
}

/// The producer's parameters.
#[derive(Clone, Copy, Debug)]
pub struct BasinParams {
    pub level: Level,
    pub body: u64,
    /// Kept: from this area (m2, the subtree's true cell areas) and depth (spill minus floor, m).
    pub min_area_m2: f64,
    pub min_depth_m: f64,
    /// Reported (`represented: none`): from this depth and cell count (BUILD-055's basin rule).
    pub report_depth_m: f64,
    pub report_cells: usize,
    /// The sea level (m): a spill path ending on an outlet at or below it receives an `outlet_sea` there (reading 6).
    pub sea_level_m: f64,
}

impl BasinParams {
    /// The contract's L basins: from 0.25 km2 and 2 m; reported from 0.5 m and 16 cells.
    pub fn l(body: u64) -> BasinParams {
        BasinParams {
            level: Level::L,
            body,
            min_area_m2: 0.25e6,
            min_depth_m: 2.0,
            report_depth_m: 0.5,
            report_cells: 16,
            sea_level_m: 0.0,
        }
    }
}

/// The cells of the basins: one flat array in which every hierarchy node's subtree is a contiguous range (its own cells, then its children's), so a nested basin shares its
/// cells with its parent instead of copying them (order #159). `cells[&id]` is a basin's cells, in that order (not sorted).
#[derive(Default)]
pub struct Cells {
    flat: Vec<usize>,
    range: BTreeMap<u64, (usize, usize)>,
}

impl std::ops::Index<&u64> for Cells {
    type Output = [usize];
    fn index(&self, id: &u64) -> &[usize] {
        let (a, b) = self.range[id];
        &self.flat[a..b]
    }
}

/// What the producer made: basins and the nodes they name (`lake_inlet`s, `trib_junction`s), the hierarchy node of each basin, the basins' cells, and the spills it could not resolve.
pub struct Produced {
    pub basins: Vec<Basin>,
    pub nodes: Vec<Node>,
    pub node_of_basin: BTreeMap<u64, u32>,
    pub cells: Cells,
    pub unresolved: Vec<u64>,
    /// Kept-size depressions that drain under reading 22 (reported, not protected); 0 without a budget.
    pub drained: usize,
    /// Depressions kept as lakes because a reach's over-budget rise crosses them (crusader #172); 0 without forced cells.
    pub forced: usize,
}

/// A grid's cells against global cell ids: `gid` the global id of a grid cell, `local` the grid cell of a global id (`None` outside the grid).
pub struct CellIds<'a> {
    pub gid: &'a dyn Fn(usize) -> u64,
    pub local: &'a dyn Fn(u64) -> Option<usize>,
}

/// Produces the basins of `z` on `grid` (`outlet` the outlet set), with the given reaches at the same level as spill receivers and inlets. `owner` names a cell's domain. On a grid whose cell
/// index is its global id (a cube grid, the G0 fixture); a window uses [`produce_ids`].
pub fn produce(grid: &Grid, z: &[f64], outlet: &[bool], reaches: &[Reach], owner: &dyn Fn(usize) -> Domain, p: &BasinParams) -> Produced {
    let n = z.len();
    produce_ids(
        grid,
        z,
        outlet,
        reaches,
        owner,
        p,
        &CellIds { gid: &|c| c as u64, local: &|id| ((id as usize) < n).then_some(id as usize) },
    )
}

/// [`produce`] on a window (explorer, 2026-11-17): the reaches' vertices are global ids (read through `ids.local`; a vertex outside the grid is skipped), and every anchor, basin cell and id
/// the producer writes is global (`ids.gid`). `Produced::cells` stays in grid cells.
pub fn produce_ids(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    reaches: &[Reach],
    owner: &dyn Fn(usize) -> Domain,
    p: &BasinParams,
    ids: &CellIds,
) -> Produced {
    produce_core(grid, z, outlet, reaches, owner, p, ids, None, &std::collections::BTreeSet::new(), &[])
}

/// [`produce_ids`] with the grid cells of markers already on the reaches (spikes #142, spike only): a reach's `lake_inlet` goes on its first vertex inside the kept basin that holds no
/// marker (a marker shares an anchor with a non-marker only as its reach's up node at vertex 0, reading 1); none when the reach leaves the basin first.
#[allow(clippy::too_many_arguments)]
pub fn produce_ids_avoiding(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    reaches: &[Reach],
    owner: &dyn Fn(usize) -> Domain,
    p: &BasinParams,
    ids: &CellIds,
    markers: &std::collections::BTreeSet<usize>,
) -> Produced {
    produce_core(grid, z, outlet, reaches, owner, p, ids, None, markers, &[])
}

/// [`produce_drain`] with the markers of [`produce_ids_avoiding`] (explorer, the rg-chain merge of spikes #142 and crusader #161; spike only).
#[allow(clippy::too_many_arguments)]
pub fn produce_drain_avoiding(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    reaches: &[Reach],
    owner: &dyn Fn(usize) -> Domain,
    p: &BasinParams,
    ids: &CellIds,
    remaining: &[f64],
    markers: &std::collections::BTreeSet<usize>,
) -> Produced {
    produce_core(grid, z, outlet, reaches, owner, p, ids, Some(remaining), markers, &[])
}

/// [`produce_drain_avoiding`] with forced lakes (crusader #172, owner 2026-11-20, spike only): each `(cell, level)` names the hollow a reach's over-budget rise crosses, the
/// cell its bed rises from and the bed it rises to; the smallest depression holding the cell whose spill reaches `level - 0.5 m` is kept (a lake, its water at its spill),
/// whatever its size or whether reading 22 would drain it. A cell in no depression forces nothing.
#[allow(clippy::too_many_arguments)]
pub fn produce_drain_avoiding_forced(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    reaches: &[Reach],
    owner: &dyn Fn(usize) -> Domain,
    p: &BasinParams,
    ids: &CellIds,
    remaining: &[f64],
    markers: &std::collections::BTreeSet<usize>,
    forced: &[(usize, f64)],
) -> Produced {
    produce_core(grid, z, outlet, reaches, owner, p, ids, Some(remaining), markers, forced)
}

/// The bound of reading 22's breach test: settled cells per depression.
const BREACH_CAP: usize = 20_000;

/// [`produce_ids`] under reading 22 (owner, #161): a depression the kept rule names (area and depth) is protected only when neither its fill (every cell raised to the spill) nor
/// a bounded least-cost breach ([`breach_path`]) fits `remaining` (the lowering budget left per cell); every other one drains and is reported (`represented: none`).
#[allow(clippy::too_many_arguments)]
pub fn produce_drain(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    reaches: &[Reach],
    owner: &dyn Fn(usize) -> Domain,
    p: &BasinParams,
    ids: &CellIds,
    remaining: &[f64],
) -> Produced {
    produce_core(grid, z, outlet, reaches, owner, p, ids, Some(remaining), &std::collections::BTreeSet::new(), &[])
}

#[allow(clippy::too_many_arguments)]
fn produce_core(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    reaches: &[Reach],
    owner: &dyn Fn(usize) -> Domain,
    p: &BasinParams,
    ids: &CellIds,
    remaining: Option<&[f64]>,
    markers: &std::collections::BTreeSet<usize>,
    forced: &[(usize, f64)],
) -> Produced {
    let gid = ids.gid;
    let hy = hierarchy(grid, z, outlet);
    let nn = hy.nodes.len();
    // crusader #172: the forced nodes, each the smallest depression holding its cell whose spill reaches the rise's level less one bed code
    let mut force = vec![false; nn];
    for &(c, level) in forced {
        let mut k = hy.node_of[c];
        while k != DRAINED && k != UNSEEN {
            let nd = &hy.nodes[k as usize];
            if nd.spill.is_none_or(|sp| sp.1 >= level - 0.5) {
                force[k as usize] = true;
                break;
            }
            k = nd.parent;
        }
    }
    // subtrees as ranges of one flat array (order #159: no per-subtree copies): sizes children first (a node's id is larger than its children's), then the starts parents first,
    // a node's own cells first and its children after them in id order
    let mut size = vec![0usize; nn];
    for k in 0..nn {
        size[k] = hy.nodes[k].own.len() + hy.nodes[k].kids.iter().map(|&kid| size[kid as usize]).sum::<usize>();
    }
    let mut first = vec![usize::MAX; nn];
    let mut next = 0usize;
    for k in (0..nn).rev() {
        if first[k] == usize::MAX {
            // a root of the hierarchy
            first[k] = next;
            next += size[k];
        }
        let mut at = first[k] + hy.nodes[k].own.len();
        for &kid in &hy.nodes[k].kids {
            first[kid as usize] = at;
            at += size[kid as usize];
        }
    }
    let mut flat = vec![0usize; next];
    for k in 0..nn {
        flat[first[k]..first[k] + hy.nodes[k].own.len()].copy_from_slice(&hy.nodes[k].own);
    }
    let span = |k: usize| &flat[first[k]..first[k] + size[k]];
    // a subtree's cells sorted, into a reused buffer (the area sums in cell order, as before: the same bits)
    let sorted_into = |k: usize, buf: &mut Vec<usize>| {
        buf.clear();
        buf.extend_from_slice(span(k));
        buf.sort_unstable();
    };
    let depth = |k: usize| hy.nodes[k].spill.map_or(f64::INFINITY, |s| s.1) - hy.nodes[k].floor_m;
    let mut buf: Vec<usize> = Vec::new();
    #[derive(Clone, Copy, PartialEq)]
    enum Keep {
        No,
        Kept,
        Reported,
    }
    let mut keep: Vec<Keep> = Vec::with_capacity(nn);
    let mut drained = 0usize;
    for k in 0..nn {
        let deep = depth(k) >= p.min_depth_m;
        let kept = deep && {
            sorted_into(k, &mut buf);
            buf.iter().map(|&c| grid.cell_area(c)).sum::<f64>() >= p.min_area_m2
        };
        // reading 22: a kept-size depression drains (reported) when its fill or a bounded breach fits the budget left; only the rest are protected
        let drains = kept
            && remaining.is_some_and(|rem| {
                let nd = &hy.nodes[k];
                let fill = nd.spill.is_some_and(|sp| span(k).iter().all(|&c| sp.1 - z[c] <= rem[c].max(0.0) + 1e-9));
                fill || {
                    // `buf` holds this subtree's cells sorted
                    let sub_k = &buf;
                    breach_path(
                        grid,
                        z,
                        outlet,
                        nd.floor_cell,
                        nd.floor_m,
                        rem,
                        &|c| sub_k.binary_search(&c).is_ok(),
                        &|_| false,
                        BREACH_CAP,
                    )
                    .is_some()
                }
            });
        drained += usize::from(drains);
        keep.push(if (kept && !drains) || force[k] {
            Keep::Kept
        } else if depth(k) >= p.report_depth_m && size[k] >= p.report_cells {
            Keep::Reported
        } else {
            Keep::No
        });
    }
    // the basin of each cell: the deepest kept (or reported) node whose subtree holds it
    let mut basin_of_cell: Vec<u32> = vec![NONE; z.len()];
    for k in 0..nn {
        if keep[k] != Keep::No {
            for &c in span(k) {
                if basin_of_cell[c] == NONE {
                    basin_of_cell[c] = k as u32;
                }
            }
        }
    }
    let basin_at = |c: usize| (basin_of_cell[c] != NONE).then_some(basin_of_cell[c]);
    let id_of = |k: usize| basin_id(p.body, p.level, gid(hy.nodes[k].floor_cell), hy.nodes[k].spill.map(|sp| gid(sp.0)));
    let mut nodes: BTreeMap<u64, Node> = BTreeMap::new();
    let inlet = |c: usize, nodes: &mut BTreeMap<u64, Node>| {
        let n = Node {
            id: node_id(p.body, NodeKind::LakeInlet, p.level, gid(c)),
            kind: NodeKind::LakeInlet,
            level: p.level,
            anchor: gid(c),
            parent: None,
            owner: owner(c),
            marker: None,
        };
        let id = n.id;
        nodes.entry(id).or_insert(n);
        id
    };
    // the vertex index of every reach cell (first reach by id, first vertex)
    let mut sorted_reaches: Vec<&Reach> = reaches.iter().filter(|r| r.level == p.level).collect();
    sorted_reaches.sort_by_key(|r| r.id);
    // (explorer, R1h: a spill onto a reach's last vertex names that reach's down node, a junction never sits there; a claim on a last vertex yields to one inside another reach)
    // (explorer, R1h partition diff: a marker is owned by the reach it sits on, so every stage gives it one owner)
    let mut on_reach: BTreeMap<usize, (u64, u32, Option<u64>, Domain)> = BTreeMap::new();
    let mut node_cells: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for r in &sorted_reaches {
        let last = r.vertices.len().saturating_sub(1);
        for (k, &c) in r.vertices.iter().enumerate() {
            let Some(c) = (ids.local)(c) else { continue };
            if k == 0 || k == last {
                node_cells.insert(c);
            }
            let claim = (r.id, k as u32, (k == last).then_some(r.down), r.owner);
            match on_reach.get(&c) {
                None => {
                    on_reach.insert(c, claim);
                }
                Some(&(_, _, Some(_), _)) if k != last => {
                    on_reach.insert(c, claim);
                }
                Some(_) => {}
            }
        }
    }
    // the spill paths: routed on the outlets, or in a world without one (every tree closed) on its lowest cell
    let lowest: Vec<bool>;
    let route_outlet = if outlet.iter().any(|&o| o) {
        outlet
    } else {
        let m = (0..z.len()).filter(|&c| grid.is_active(c)).min_by(|&a, &b| z[a].total_cmp(&z[b]).then(a.cmp(&b))).unwrap_or(0);
        lowest = (0..z.len()).map(|c| c == m).collect();
        &lowest
    };
    let routing = route_rules(grid, z, route_outlet, 1e-3, p.body, 0.0, None, RoundRules::ROUND);
    let mut out = Produced {
        basins: Vec::new(),
        nodes: Vec::new(),
        node_of_basin: BTreeMap::new(),
        cells: Cells::default(),
        unresolved: Vec::new(),
        drained,
        forced: force.iter().filter(|&&f| f).count(),
    };
    for k in 0..nn {
        if keep[k] == Keep::No {
            continue;
        }
        let nd = &hy.nodes[k];
        let mut parent = nd.parent;
        while parent != DRAINED && keep[parent as usize] == Keep::No {
            parent = hy.nodes[parent as usize].parent;
        }
        let id = id_of(k);
        sorted_into(k, &mut buf);
        let sub_k = &buf;
        let (outlet_kind, spill_cell, spill_m, spill_node) = match nd.spill {
            None => (Outlet::Endorheic, nd.floor_cell, f64::INFINITY, 0),
            Some((sc, sm, _)) => {
                // the overflow leaves over the spill cell to its lowest neighbour outside this basin (ties by id), then follows the routed path until another basin's cell or a reach
                // vertex
                let outside = (0..8)
                    .filter_map(|d| grid.neighbour(sc, d))
                    .filter(|q| sub_k.binary_search(q).is_err())
                    .min_by(|&a, &b| z[a].total_cmp(&z[b]).then(a.cmp(&b)));
                let mut c = outside.unwrap_or(sc);
                let mut target = 0u64;
                let mut steps = 0usize;
                loop {
                    if let Some(&(r, v, end, reach_owner)) = on_reach.get(&c) {
                        if let Some(down) = end {
                            target = down;
                            break;
                        }
                        let j = Node {
                            id: marker_id(p.body, NodeKind::TribJunction, p.level, gid(c), r),
                            kind: NodeKind::TribJunction,
                            level: p.level,
                            anchor: gid(c),
                            parent: None,
                            owner: reach_owner,
                            marker: Some(Marker { on_reach: r, at_vertex: v }),
                        };
                        target = j.id;
                        nodes.entry(j.id).or_insert(j);
                        break;
                    }
                    if let Some(b) = basin_at(c) {
                        let b = b as usize;
                        // another basin: not this one, not one nested in it, not an ancestor it merges into (the water goes on into the sibling)
                        let mut anc = hy.nodes[k].parent;
                        let mut is_ancestor = false;
                        while anc != DRAINED {
                            if anc as usize == b {
                                is_ancestor = true;
                                break;
                            }
                            anc = hy.nodes[anc as usize].parent;
                        }
                        if b != k && sub_k.binary_search(&c).is_err() && !is_ancestor {
                            target = inlet(c, &mut nodes);
                            break;
                        }
                    }
                    let next = routing.receiver[c];
                    steps += 1;
                    // reading 6 (owner, #161): a spill path that reaches the sea before any reach or basin is received by an `outlet_sea` on its first sea cell; a window's exit
                    // stays unresolved (a window is no domain)
                    if next == NONE && route_outlet[c] && z[c] <= p.sea_level_m {
                        let o = Node {
                            id: node_id(p.body, NodeKind::OutletSea, p.level, gid(c)),
                            kind: NodeKind::OutletSea,
                            level: p.level,
                            anchor: gid(c),
                            parent: None,
                            owner: owner(c),
                            marker: None,
                        };
                        target = o.id;
                        nodes.entry(o.id).or_insert(o);
                        break;
                    }
                    if next == NONE || steps > z.len() {
                        break;
                    }
                    c = next as usize;
                }
                if target == 0 {
                    out.unresolved.push(id);
                }
                (Outlet::Active, sc, sm, target)
            }
        };
        out.basins.push(Basin {
            id,
            level: p.level,
            parent: (parent != DRAINED).then(|| id_of(parent as usize)),
            floor_cell: gid(nd.floor_cell),
            floor_m: nd.floor_m,
            spill_cell: gid(spill_cell),
            spill_m,
            spill_node,
            outlet: outlet_kind,
            represented: if keep[k] == Keep::Kept { Represented::ProtectedSurface } else { Represented::None },
            owner: owner(nd.floor_cell),
        });
        out.node_of_basin.insert(id, k as u32);
        out.cells.range.insert(id, (first[k], first[k] + size[k]));
    }
    // inlets: each given reach entering a kept basin gets a lake_inlet on its first vertex inside it
    // (SPIKE-133: a reach that starts inside a kept basin does not enter it there: from its second vertex, and only from outside that basin)
    // spikes #142: the markers this producer made (spill trib_junctions) are passed like the given ones
    let own_markers: std::collections::BTreeSet<usize> =
        nodes.values().filter(|x| x.kind.is_marker()).filter_map(|x| (ids.local)(x.anchor)).collect();
    for r in &sorted_reaches {
        for k in 1..r.vertices.len() {
            let (Some(c), prev) = ((ids.local)(r.vertices[k]), (ids.local)(r.vertices[k - 1])) else { continue };
            // a reach entering at its last vertex ends at its own node there: the inlet only when that node is this cell's lake_inlet (the same id); a node's cell takes no inlet beside
            // it (explorer, R1h)
            let own_inlet = node_id(p.body, NodeKind::LakeInlet, p.level, gid(c)) == r.down;
            if (k + 1 == r.vertices.len() && !own_inlet) || (node_cells.contains(&c) && !(k + 1 == r.vertices.len() && own_inlet)) {
                break;
            }
            if let Some(b) = basin_at(c) {
                if keep[b as usize] == Keep::Kept && prev.and_then(|q| basin_at(q)) != Some(b) {
                    // spikes #142: past the marker cells, while inside the basin; never onto another node's cell or the last vertex (unless its own inlet)
                    let mut j = k;
                    while j < r.vertices.len() {
                        let Some(cj) = (ids.local)(r.vertices[j]) else { break };
                        if basin_at(cj) != Some(b) {
                            break;
                        }
                        let own = node_id(p.body, NodeKind::LakeInlet, p.level, gid(cj)) == r.down;
                        if (j + 1 == r.vertices.len() && !own) || (node_cells.contains(&cj) && !(j + 1 == r.vertices.len() && own)) {
                            break;
                        }
                        if !markers.contains(&cj) && !own_markers.contains(&cj) {
                            inlet(cj, &mut nodes);
                            break;
                        }
                        j += 1;
                    }
                    break;
                }
            }
        }
    }
    out.basins.sort_by_key(|b| b.id);
    out.nodes = nodes.into_values().collect();
    out.cells.flat = flat;
    out
}

/// The L images of H1 basins (contract section 2.1): for each H1 basin, the L basin holding the lowest L cell (of `z_l`, ties by id) among `l_cells_of(h1 floor cell)` that lies in a kept
/// L basin; the image's `parent` becomes the H1 basin id and its owner the landmass. Returns (H1 basin id, L basin id) pairs.
pub fn link_images(h1: &Produced, l: &mut Produced, z_l: &[f64], l_cells_of: &dyn Fn(usize) -> Vec<usize>) -> Vec<(u64, u64)> {
    let mut cell_basin: BTreeMap<usize, u64> = BTreeMap::new();
    // the deepest L basin holding each cell: cells lists are subtree cells, so a nested basin is visited... keep the smallest basin per cell
    let mut size: BTreeMap<u64, usize> = BTreeMap::new();
    for b in l.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface) {
        size.insert(b.id, l.cells[&b.id].len());
        for &c in &l.cells[&b.id] {
            match cell_basin.get(&c) {
                Some(o) if size[o] <= l.cells[&b.id].len() => {}
                _ => {
                    cell_basin.insert(c, b.id);
                }
            }
        }
    }
    let mut pairs = Vec::new();
    for hb in &h1.basins {
        let mut cells: Vec<usize> = l_cells_of(hb.floor_cell as usize).into_iter().filter(|c| cell_basin.contains_key(c)).collect();
        cells.sort_by(|&a, &b| z_l[a].total_cmp(&z_l[b]).then(a.cmp(&b)));
        if let Some(&c) = cells.first() {
            let lid = cell_basin[&c];
            if let Some(lb) = l.basins.iter_mut().find(|b| b.id == lid) {
                lb.parent = Some(hb.id);
                lb.owner = Domain::Landmass;
                pairs.push((hb.id, lid));
            }
        }
    }
    pairs
}

/// A bounded least-cost breach (reading 22, owner #161): from `start` (a depression's lowest cell), the path whose cells, cut to the floor level `floor`, give the least cut volume
/// (`max(0, z - floor)` times the cell area, summed), stepping only onto cells whose cut fits `remaining` and that `blocked` does not hold, to the first cell outside `inside` lying at or
/// below the floor, or an outlet. `None` when no such path exists within `cap` settled cells. Dijkstra, ties by the cell id: the same path on any machine.
#[allow(clippy::too_many_arguments)]
pub fn breach_path(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    start: usize,
    floor: f64,
    remaining: &[f64],
    inside: &dyn Fn(usize) -> bool,
    blocked: &dyn Fn(usize) -> bool,
    cap: usize,
) -> Option<(f64, Vec<usize>)> {
    use std::cmp::Reverse;
    use std::collections::{BinaryHeap, HashMap};
    let cut = |c: usize| (z[c] - floor).max(0.0);
    let mut best: HashMap<usize, (f64, usize)> = HashMap::new();
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    best.insert(start, (0.0, usize::MAX));
    heap.push(Reverse((0, start)));
    let mut settled = 0usize;
    while let Some(Reverse((key, c))) = heap.pop() {
        let (cost, _) = best[&c];
        if key != cost.to_bits() {
            continue;
        }
        settled += 1;
        if settled > cap {
            return None;
        }
        if c != start && (outlet[c] || (!inside(c) && z[c] <= floor)) {
            let mut path = vec![c];
            let mut x = c;
            while let Some(&(_, from)) = best.get(&x) {
                if from == usize::MAX {
                    break;
                }
                path.push(from);
                x = from;
            }
            path.reverse();
            return Some((cost, path));
        }
        for d in 0..8 {
            let Some(q) = grid.neighbour(c, d) else { continue };
            if blocked(q) || cut(q) > remaining[q].max(0.0) {
                continue;
            }
            let nc = cost + cut(q) * grid.cell_area(q);
            if best.get(&q).is_none_or(|&(b, _)| nc < b) {
                best.insert(q, (nc, c));
                heap.push(Reverse((nc.to_bits(), q)));
            }
        }
    }
    None
}

/// What [`floor_inlet_beds`] did (lead #179a): the vertices raised and the metres raised over them (reading 9's wanted fill, recorded), the carries stopped at a depression's
/// edge (a residual each: the reach, its vertex, the metres still wanted there), and the bed rises left along the L reaches.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InletFloor {
    pub raised: usize,
    pub raised_m: f64,
    pub residuals: Vec<(u64, usize, f64)>,
    pub rises: usize,
}

/// Lead #179 and #179a (A3, owner decision 2, 2026-11-21; spike): every L vertex's graph bed on a kept basin's rim (the cells beside its cells, 8-connected) or inside it is
/// floored at the basin's spill (the highest spill where basins meet; bed codes of 0.5 m, rounded up), so no line inside or on a kept rim lies under the spill (reading 35 test
/// (a)). The floor is carried upstream along a reach only over vertices inside a kept basin's depression (its cells); it stops at the first vertex at or above it, and where the
/// next vertex upstream lies outside the depression and stands lower, the carry stops with a residual on that reach. `local` maps a vertex id to its grid cell (`None` outside).
pub fn floor_inlet_beds(
    grid: &Grid,
    basins: &[Basin],
    cells: &Cells,
    reaches: &mut [Reach],
    local: &dyn Fn(u64) -> Option<usize>,
) -> InletFloor {
    let (mut floor, mut inside): (BTreeMap<usize, f64>, BTreeMap<usize, f64>) = (BTreeMap::new(), BTreeMap::new());
    for b in basins.iter().filter(|b| b.represented == Represented::ProtectedSurface && b.spill_m.is_finite()) {
        for &c in &cells[&b.id] {
            let f = inside.entry(c).or_insert(f64::NEG_INFINITY);
            *f = f.max(b.spill_m);
            for q in std::iter::once(c).chain((0..8).filter_map(|k| grid.neighbour(c, k))) {
                let f = floor.entry(q).or_insert(f64::NEG_INFINITY);
                *f = f.max(b.spill_m);
            }
        }
    }
    let mut out = InletFloor::default();
    for r in reaches.iter_mut().filter(|r| r.level == Level::L) {
        let n = r.vertices.len();
        let before = r.attrs.bed.clone();
        let mut lifted = vec![false; n];
        for k in 0..n {
            let Some(&fl) = local(r.vertices[k]).and_then(|c| floor.get(&c)) else { continue };
            if 0.5 * f64::from(r.attrs.bed[k]) < fl {
                r.attrs.bed[k] = (2.0 * fl).ceil() as i32;
                lifted[k] = true;
            }
        }
        // the carry: upstream of a lifted vertex, a lower bed inside a kept depression takes the one below it; outside the depression it stops with a residual
        for k in (0..n.saturating_sub(1)).rev() {
            if !lifted[k + 1] || r.attrs.bed[k] >= r.attrs.bed[k + 1] {
                continue;
            }
            if local(r.vertices[k]).is_some_and(|c| inside.contains_key(&c)) {
                r.attrs.bed[k] = r.attrs.bed[k + 1];
                lifted[k] = true;
            } else if !lifted[k] {
                out.residuals.push((r.id, k, 0.5 * f64::from(r.attrs.bed[k + 1] - r.attrs.bed[k])));
            }
        }
        for k in 0..n {
            if r.attrs.bed[k] != before[k] {
                out.raised += 1;
                out.raised_m += 0.5 * f64::from(r.attrs.bed[k] - before[k]);
            }
        }
        out.rises += r.attrs.bed.windows(2).filter(|x| x[1] > x[0]).count();
    }
    out
}

/// Lead #179b (DEC crusader-28): each stopped carry of [`floor_inlet_beds`] is recorded as the contract's residual (section 6): an `exception` marker node at the vertex where
/// the bed rises (the stopped vertex's downstream neighbour), so the validator accepts exactly that rise, and `exception(budget_residual)` on the reach carrying the metres
/// wanted (the largest where a reach has several). A rise at a reach's last vertex sits at its down node, where no marker may stand: it is left unflagged. Returns the rises
/// flagged and those left.
pub fn mark_inlet_residuals(body: u64, nodes: &mut Vec<Node>, reaches: &mut [Reach], residuals: &[(u64, usize, f64)]) -> (usize, usize) {
    let have: std::collections::BTreeSet<u64> = nodes.iter().map(|n| n.id).collect();
    let mut added = std::collections::BTreeSet::new();
    let (mut flagged, mut left) = (0usize, 0usize);
    for &(rid, k, need) in residuals {
        let Some(r) = reaches.iter_mut().find(|r| r.id == rid) else {
            left += 1;
            continue;
        };
        let v = k + 1;
        if v + 1 >= r.vertices.len() {
            left += 1;
            continue;
        }
        let anchor = r.vertices[v];
        let id = marker_id(body, NodeKind::Exception, r.level, anchor, rid);
        if !have.contains(&id) && added.insert(id) {
            nodes.push(Node {
                id,
                kind: NodeKind::Exception,
                level: r.level,
                anchor,
                parent: None,
                owner: r.owner,
                marker: Some(Marker { on_reach: rid, at_vertex: v as u32 }),
            });
        }
        match r.exceptions.iter_mut().find(|e| e.reason == Reason::BudgetResidual) {
            Some(e) => e.value = e.value.max(need),
            None => r.exceptions.push(Exception { reason: Reason::BudgetResidual, value: need }),
        }
        flagged += 1;
    }
    (flagged, left)
}
