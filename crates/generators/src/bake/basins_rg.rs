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
}

impl BasinParams {
    /// The contract's L basins: from 0.25 km2 and 2 m; reported from 0.5 m and 16 cells.
    pub fn l(body: u64) -> BasinParams {
        BasinParams { level: Level::L, body, min_area_m2: 0.25e6, min_depth_m: 2.0, report_depth_m: 0.5, report_cells: 16 }
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
    let gid = ids.gid;
    let hy = hierarchy(grid, z, outlet);
    let nn = hy.nodes.len();
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
    let keep: Vec<Keep> = (0..nn)
        .map(|k| {
            let deep = depth(k) >= p.min_depth_m;
            if deep && {
                sorted_into(k, &mut buf);
                buf.iter().map(|&c| grid.cell_area(c)).sum::<f64>() >= p.min_area_m2
            } {
                Keep::Kept
            } else if depth(k) >= p.report_depth_m && size[k] >= p.report_cells {
                Keep::Reported
            } else {
                Keep::No
            }
        })
        .collect();
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
    let mut on_reach: BTreeMap<usize, (u64, u32)> = BTreeMap::new();
    for r in &sorted_reaches {
        for (k, &c) in r.vertices.iter().enumerate() {
            if let Some(c) = (ids.local)(c) {
                on_reach.entry(c).or_insert((r.id, k as u32));
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
    let mut out =
        Produced { basins: Vec::new(), nodes: Vec::new(), node_of_basin: BTreeMap::new(), cells: Cells::default(), unresolved: Vec::new() };
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
                    if let Some(&(r, v)) = on_reach.get(&c) {
                        let mut j = Node {
                            id: marker_id(p.body, NodeKind::TribJunction, p.level, gid(c), r),
                            kind: NodeKind::TribJunction,
                            level: p.level,
                            anchor: gid(c),
                            parent: None,
                            owner: owner(c),
                            marker: Some(Marker { on_reach: r, at_vertex: v }),
                        };
                        j.owner = owner(c);
                        target = j.id;
                        nodes.entry(j.id).or_insert(j);
                        break;
                    }
                    let next = routing.receiver[c];
                    steps += 1;
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
    for r in &sorted_reaches {
        for k in 1..r.vertices.len() {
            let (Some(c), prev) = ((ids.local)(r.vertices[k]), (ids.local)(r.vertices[k - 1])) else { continue };
            if let Some(b) = basin_at(c) {
                if keep[b as usize] == Keep::Kept && prev.and_then(|q| basin_at(q)) != Some(b) {
                    inlet(c, &mut nodes);
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
