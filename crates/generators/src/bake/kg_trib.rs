//! SPIKE-133 (spike branch spike/g1d, never merged): KG-trib extractor-1 of the river-graph plan (G1d; plan section 3.2, G0 contracts sections 2 to 5): the L tributaries of a domain (the
//! whole body, or a region's tiles plus a halo) as GEN-189 nodes and reaches, behind the reach contract.
//!
//! 1. Routing: the canonical flood plus clipped MFD (GEN-141, round rules) on the stage input (KS0; `S0` with KS off), the outlets the sea, the window's exits and the trunk images' L cells (trunk
//!    cells are forced receivers: a tributary ends where it reaches a trunk, no burn).
//! 2. Channel cells: the clipped-MFD area plus the window's entry seed `a_ext` (propagated down the D8 receivers) at least `A_crit`, and every D8 receiver of a channel cell (closure, so the
//!    network is a tree); cells of kept basins are never channel cells.
//! 3. Nodes: `source` (a head), `lake_spill` (a head fed by a kept basin), `confluence` (two or more channel donors), `tile_crossing` (the last vertex inside a tile core a chain leaves, when it is
//!    no other node), and the ends: `trib_junction` on a trunk vertex, `outlet_sea` on the first sea cell, `lake_inlet` on the first cell of a kept basin; a chain reaching a window exit is open.
//! 4. Attributes: the D8 accumulation of true cell areas, with (`a_geo`, `a_run`) and without (`a_run_no_ext`) the seed; `a_ext` the seed at its entry vertex; `q = q_w = a_run r`; Strahler by the
//!    standard rule, one order a reach; the bed the running minimum of the 0.5 m codes, lifted to the end's bed (a junction carries the trunk's), one value at a node. A reach ending on a trunk
//!    takes the trunk's bed on its last vertex; a reach reaching a trunk's last vertex ends at the trunk's down node.
//! 5. Ids: GEN-189's (node: body, kind, level, anchor; reach: body, level, up, down); owners: a tile of the anchor (`trib_junction`: the landmass, as the trunk), a reach the tile of its second vertex.
//!
//! Single-threaded, fixed orders (CON-14).
// spike only (never merged): kept as run, so the runs keep their bits; main gets its own form through an OpenSpec change
#![allow(clippy::needless_range_loop)]

use super::river_graph::*;
use super::route::{catchment_roots, route_rules_filled, RoundRules, NONE};
use super::Grid;
use std::collections::BTreeMap;

/// The extractor's parameters (plan section 3.4).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub body: u64,
    /// Channel initiation (m2 of clipped-MFD area).
    pub a_crit_m2: f64,
    /// Uniform runoff (m/s): `q = a_run r`.
    pub runoff: f64,
    pub seed: u64,
    pub eps: f64,
    /// Initiation on the clipped-MFD area (the plan's extractor-1); false: on the D8 area (the comparison of the results note).
    pub mfd_init: bool,
}

impl Params {
    pub fn plan(body: u64, runoff: f64) -> Params {
        Params { body, a_crit_m2: 1.0e6, runoff, seed: 1, eps: 1e-3, mfd_init: true }
    }
}

/// An L trunk image the tributaries end on.
#[derive(Clone, Debug)]
pub struct Trunk {
    pub id: u64,
    pub vertices: Vec<usize>,
    pub attrs: Attrs,
    /// The trunk's down node: a tributary reaching the trunk's last vertex ends there (a junction never sits on a reach's last vertex).
    pub down: Node,
}

/// The inputs of one domain.
pub struct Input<'a> {
    pub grid: &'a Grid,
    pub z: &'a [f64],
    pub sea: &'a [bool],
    /// The window's exits (outlets on its border); all false for the whole body.
    pub exit: &'a [bool],
    pub trunks: &'a [Trunk],
    /// The cells of kept (protected) basins.
    pub kept: &'a [bool],
    /// `a_ext` at the window's entry cells (m2), zero elsewhere.
    pub seed_ext: &'a [f64],
    pub tile: &'a dyn Fn(usize) -> Domain,
    /// The global cell id of a grid cell (the anchors and vertices of the graph).
    pub gid: &'a dyn Fn(usize) -> u64,
    /// Keep the per-cell fields `filled`, `root` and `init` (the fixture's halo tests); a window leaves them empty.
    pub keep_fields: bool,
}

/// What a run made.
pub struct Extracted {
    pub nodes: Vec<Node>,
    pub reaches: Vec<Reach>,
    pub channel: Vec<bool>,
    /// The D8 receivers of the routing (`NONE`: an outlet).
    pub receiver: Vec<u32>,
    /// The flooded surface the routing ran on, and the D8 catchment root of each cell (the clip of the MFD area).
    pub filled: Vec<f64>,
    pub root: Vec<u32>,
    /// The D8 accumulation where the flood raised a cell (debug: the share of channel cells in filled depressions).
    pub raised_channel: usize,
    /// Clipped-MFD area plus the propagated seed (the initiation measure), and the D8 areas with and without the seed.
    pub init: Vec<f64>,
    pub d8: Vec<f64>,
    pub d8_no_ext: Vec<f64>,
    /// Channel cells whose D8 area is below `A_crit` (initiated by MFD), and heads whose D8 area is at least `A_crit`.
    pub mfd_only: usize,
    /// Reaches whose last vertex sits on a trunk with a smaller `a_geo` than the vertex before (the trunk's given area below the tributary's).
    pub trunk_area_breaks: usize,
    /// Chains that reach a window exit (open; their reaches outside the core).
    pub open_ends: usize,
}

/// The bed code (GEN-170): 0.5 m, rounded down.
fn code(z: f64) -> i32 {
    libm::floor(z / 0.5) as i32
}

/// Runs the extractor on one domain.
pub fn extract(inp: &Input, p: &Params) -> Extracted {
    let g = inp.grid;
    let n = g.len();
    // trunk cells: (trunk index, vertex index), the first claim by trunk id order
    let mut on_trunk: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    let mut order_t: Vec<usize> = (0..inp.trunks.len()).collect();
    order_t.sort_by_key(|&t| inp.trunks[t].id);
    for t in order_t {
        for (k, &c) in inp.trunks[t].vertices.iter().enumerate() {
            // the claim of a cell several trunks pass: a trunk's first vertex (a junction there shares the anchor with that trunk's up node, reading 1), then an interior vertex, then
            // a last vertex (the tributary ends at that trunk's down node); ties by trunk id
            let rank = |t: usize, k: usize| {
                if k == 0 {
                    0
                } else if k + 1 == inp.trunks[t].vertices.len() {
                    2
                } else {
                    1
                }
            };
            match on_trunk.get(&c) {
                Some(&(t0, k0)) if rank(t0, k0) <= rank(t, k) => {}
                _ => {
                    on_trunk.insert(c, (t, k));
                }
            }
        }
    }
    let mut is_trunk = vec![false; n];
    for &c in on_trunk.keys() {
        is_trunk[c] = true;
    }
    let outlet: Vec<bool> = (0..n).map(|c| g.is_active(c) && (inp.sea[c] || inp.exit[c] || is_trunk[c])).collect();
    let (r, filled) = route_rules_filled(g, inp.z, &outlet, p.eps, p.seed, 0.0, None, RoundRules::ROUND);
    let root = if inp.keep_fields { catchment_roots(&r.receiver, &r.order) } else { Vec::new() };
    // D8 accumulations (an outlet adds no area of its own) and the seed, donors first
    let mut d8_no_ext: Vec<f64> = (0..n).map(|c| if g.is_active(c) && !outlet[c] { g.cell_area(c) } else { 0.0 }).collect();
    let mut ext: Vec<f64> = inp.seed_ext.to_vec();
    for &c in r.order.iter().rev() {
        let to = r.receiver[c as usize];
        if to != NONE {
            let (a, e) = (d8_no_ext[c as usize], ext[c as usize]);
            d8_no_ext[to as usize] += a;
            ext[to as usize] += e;
        }
    }
    let d8: Vec<f64> = (0..n).map(|c| d8_no_ext[c] + ext[c]).collect();
    let init_at = |c: usize| if p.mfd_init { r.area[c] + ext[c] } else { d8[c] };
    let init: Vec<f64> = if inp.keep_fields { (0..n).map(init_at).collect() } else { Vec::new() };
    // channel cells, donors first: the threshold, or a channel donor (closure); never an outlet or a kept basin cell
    let mut channel = vec![false; n];
    // the donors of each cell as one flat list (CSR), in cell order
    let mut start = vec![0u32; n + 1];
    for &to in &r.receiver {
        if to != NONE {
            start[to as usize + 1] += 1;
        }
    }
    for c in 0..n {
        start[c + 1] += start[c];
    }
    let mut fill = start.clone();
    let mut flat = vec![0u32; start[n] as usize];
    for (c, &to) in r.receiver.iter().enumerate() {
        if to != NONE {
            flat[fill[to as usize] as usize] = c as u32;
            fill[to as usize] += 1;
        }
    }
    drop(fill);
    let donors = |c: usize| flat[start[c] as usize..start[c + 1] as usize].iter().map(|&d| d as usize);
    for &c in r.order.iter().rev() {
        let c = c as usize;
        if !g.is_active(c) || outlet[c] || inp.kept[c] {
            continue;
        }
        channel[c] = init_at(c) >= p.a_crit_m2 || donors(c).any(|d| channel[d]);
    }
    let mfd_only = (0..n).filter(|&c| channel[c] && d8[c] < p.a_crit_m2).count();
    // Strahler and the bed, donors first: the bed the running minimum (one value at a confluence: the lowest inflow)
    let mut strahler = vec![0u8; n];
    let mut bed = vec![i32::MAX; n];
    for &c in r.order.iter().rev() {
        let c = c as usize;
        if !channel[c] {
            continue;
        }
        let ins: Vec<usize> = donors(c).filter(|&d| channel[d]).collect();
        let top = ins.iter().map(|&d| strahler[d]).max().unwrap_or(0);
        strahler[c] = if ins.is_empty() {
            1
        } else if ins.iter().filter(|&&d| strahler[d] == top).count() >= 2 {
            top + 1
        } else {
            top
        };
        bed[c] = ins.iter().map(|&d| bed[d]).min().unwrap_or(i32::MAX).min(code(inp.z[c]));
    }
    // the end of each channel cell's chain and its bed: a trunk's bed at the junction lifts the chain above it (receivers first)
    let end_bed = |t: usize| -> Option<i32> { on_trunk.get(&t).map(|&(ti, k)| inp.trunks[ti].attrs.bed[k]) };
    let mut lift = vec![i32::MIN; n];
    for &c in r.order.iter() {
        let c = c as usize;
        if !channel[c] {
            continue;
        }
        let to = r.receiver[c] as usize;
        let below = if channel[to] { lift[to].max(bed[to]) } else { end_bed(to).unwrap_or(i32::MIN) };
        lift[c] = below;
    }
    let bed_at = |c: usize| bed[c].max(lift[c]);
    // node kinds of channel cells
    let tile = inp.tile;
    let kind_at = |c: usize| -> Option<NodeKind> {
        let ins = donors(c).filter(|&d| channel[d]).count();
        if ins == 0 {
            Some(if donors(c).any(|d| inp.kept[d]) { NodeKind::LakeSpill } else { NodeKind::Source })
        } else if ins >= 2 {
            Some(NodeKind::Confluence)
        } else if tile(r.receiver[c] as usize) != tile(c) {
            Some(NodeKind::TileCrossing)
        } else {
            None
        }
    };
    let gid = inp.gid;
    let node = |kind: NodeKind, c: usize, owner: Domain| Node {
        id: node_id(p.body, kind, Level::L, gid(c)),
        kind,
        level: Level::L,
        anchor: gid(c),
        parent: None,
        owner,
        marker: None,
    };
    let mut nodes: BTreeMap<u64, Node> = BTreeMap::new();
    let mut reaches: Vec<Reach> = Vec::new();
    let (mut trunk_area_breaks, mut open_ends) = (0usize, 0usize);
    for c0 in 0..n {
        if !channel[c0] {
            continue;
        }
        let Some(k0) = kind_at(c0) else { continue };
        let up = node(k0, c0, tile(c0));
        let mut cells = vec![c0];
        let mut x = c0;
        // the end node, or None for an open chain
        let down: Option<Node> = loop {
            let to = r.receiver[x];
            if to == NONE {
                break None;
            }
            let t = to as usize;
            // a chain leaving the tile core from a cell that is no node is cut there (the crossing cell is that node, handled when the walk starts there)
            cells.push(t);
            if channel[t] {
                if let Some(k) = kind_at(t) {
                    break Some(node(k, t, tile(t)));
                }
                x = t;
                continue;
            }
            if let Some(&(ti, k)) = on_trunk.get(&t) {
                if k + 1 == inp.trunks[ti].vertices.len() {
                    break Some(inp.trunks[ti].down.clone());
                }
                let mut j = node(NodeKind::TribJunction, t, Domain::Landmass);
                j.marker = Some(Marker { on_reach: inp.trunks[ti].id, at_vertex: k as u32 });
                j.id = node_id_of(p.body, &j);
                break Some(j);
            }
            if inp.sea[t] {
                break Some(node(NodeKind::OutletSea, t, tile(t)));
            }
            if inp.kept[t] {
                break Some(node(NodeKind::LakeInlet, t, tile(t)));
            }
            break None;
        };
        let Some(down) = down else {
            open_ends += 1;
            continue;
        };
        // attributes; the last vertex on a trunk takes the trunk's
        let last = *cells.last().unwrap();
        let st = strahler[c0];
        let mut a = Attrs {
            a_geo: cells.iter().map(|&c| d8[c]).collect(),
            a_ext: cells.iter().map(|&c| inp.seed_ext[c]).collect(),
            a_run: cells.iter().map(|&c| d8[c]).collect(),
            a_run_no_ext: cells.iter().map(|&c| d8_no_ext[c]).collect(),
            q: cells.iter().map(|&c| d8[c] * p.runoff).collect(),
            q_w: cells.iter().map(|&c| d8[c] * p.runoff).collect(),
            strahler: vec![st; cells.len()],
            bed: cells.iter().map(|&c| if channel[c] { bed_at(c) } else { code(inp.z[c]).min(bed_at(cells[cells.len() - 2])) }).collect(),
        };
        // on a trunk the last vertex shares the trunk's bed; its areas and discharges stay the tributary's own (the D8 inflow there): the trunk's are the landmass pass's
        if let Some(&(ti, k)) = on_trunk.get(&last) {
            let ta = &inp.trunks[ti].attrs;
            let m = cells.len() - 1;
            a.bed[m] = ta.bed[k];
            if m > 0 && ta.a_geo[k] < a.a_geo[m - 1] {
                trunk_area_breaks += 1;
            }
        }
        let rid = reach_id(p.body, Level::L, up.id, down.id);
        reaches.push(Reach {
            id: rid,
            level: Level::L,
            up: up.id,
            down: down.id,
            owner: tile(cells[1]),
            vertices: cells.iter().map(|&c| gid(c)).collect(),
            parent: None,
            attrs: a,
            exceptions: Vec::new(),
        });
        nodes.entry(up.id).or_insert(up);
        nodes.entry(down.id).or_insert(down);
    }
    reaches.sort_by_key(|x| x.id);
    let raised_channel = (0..n).filter(|&c| channel[c] && filled[c] > inp.z[c]).count();
    let filled = if inp.keep_fields { filled } else { Vec::new() };
    Extracted {
        nodes: nodes.into_values().collect(),
        reaches,
        channel,
        receiver: r.receiver.clone(),
        filled,
        root,
        raised_channel,
        init,
        d8,
        d8_no_ext,
        mfd_only,
        trunk_area_breaks,
        open_ends,
    }
}

/// What the landmass pass changed.
#[derive(Debug, Default)]
pub struct Accumulated {
    pub reaches: usize,
    /// Heads where the L inflow exceeds the H1 seed (the head then takes the inflow).
    pub heads_over_seed: usize,
    /// Trunk reaches the pass could not order (a cycle through trunk nodes; left as they were).
    pub unordered: usize,
}

/// Reading 18: the landmass pass over the combined graph. Every L trunk reach in `trunks` gets `a_geo`, `a_run`, `a_run_no_ext`, `q`, `q_w` accumulated at L: a vertex carries the vertex
/// before it plus `inflow(cell)` (its own cell and what drains into it directly: tributaries and hillslope, with and without the halo seed); a node shares one value with every reach there, the sum
/// of its inflowing trunk reaches' second-to-last vertices plus its own inflow; a head with no inflowing trunk takes the larger of its H1 seed (its present value) and its own inflow.
/// `q = q_w = a_run runoff` (no basin loss on a crossed basin, reading 16).
pub fn accumulate_trunks(
    g: &mut Graph,
    trunks: &std::collections::BTreeSet<u64>,
    inflow: &dyn Fn(u64) -> (f64, f64),
    runoff: f64,
) -> Accumulated {
    let mut out = Accumulated::default();
    let idx: Vec<usize> = (0..g.reaches.len()).filter(|&i| trunks.contains(&g.reaches[i].id)).collect();
    out.reaches = idx.len();
    // the reaches into each node, and the order: a reach after every trunk reach ending at its up node (Kahn, ties by reach id)
    let mut into: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for &i in &idx {
        into.entry(g.reaches[i].down).or_default().push(i);
    }
    let mut waiting: BTreeMap<usize, usize> = idx.iter().map(|&i| (i, into.get(&g.reaches[i].up).map_or(0, |v| v.len()))).collect();
    let mut ready: std::collections::BTreeSet<(u64, usize)> =
        idx.iter().filter(|i| waiting[i] == 0).map(|&i| (g.reaches[i].id, i)).collect();
    let mut order: Vec<usize> = Vec::new();
    while let Some(&(id, i)) = ready.iter().next() {
        ready.remove(&(id, i));
        order.push(i);
        let down = g.reaches[i].down;
        for &j in &idx {
            if g.reaches[j].up == down {
                let w = waiting.get_mut(&j).unwrap();
                *w -= 1;
                if *w == 0 {
                    ready.insert((g.reaches[j].id, j));
                }
            }
        }
    }
    out.unordered = idx.len() - order.len();
    for &i in &order {
        let up = g.reaches[i].up;
        let cells = g.reaches[i].vertices.clone();
        // the head: the node's value from its inflowing trunk reaches, or the seed against the own inflow
        let (own0, own0_no) = inflow(cells[0]);
        let ins: Vec<usize> = into.get(&up).cloned().unwrap_or_default();
        let (mut v, mut v_no) = if ins.is_empty() {
            let a = &g.reaches[i].attrs;
            let (seed, seed_no) = (a.a_geo[0], a.a_run_no_ext[0]);
            if own0 > seed {
                out.heads_over_seed += 1;
            }
            (seed.max(own0), seed_no.max(own0_no))
        } else {
            let pre = |r: &Reach, f: fn(&Attrs) -> &Vec<f64>| {
                let x = f(&r.attrs);
                x[x.len().saturating_sub(2)]
            };
            let s: f64 = ins.iter().map(|&j| pre(&g.reaches[j], |a| &a.a_geo)).sum::<f64>() + own0;
            let s_no: f64 = ins.iter().map(|&j| pre(&g.reaches[j], |a| &a.a_run_no_ext)).sum::<f64>() + own0_no;
            // the inflowing reaches end with the node's value
            for &j in &ins {
                let a = &mut g.reaches[j].attrs;
                let m = a.a_geo.len() - 1;
                a.a_geo[m] = s;
                a.a_run[m] = s;
                a.a_run_no_ext[m] = s_no;
                a.q[m] = s * runoff;
                a.q_w[m] = s * runoff;
            }
            (s, s_no)
        };
        let a = &mut g.reaches[i].attrs;
        let n = cells.len();
        for k in 0..n {
            if k > 0 {
                let (o, o_no) = inflow(cells[k]);
                v += o;
                v_no += o_no;
            }
            a.a_geo[k] = v;
            a.a_run[k] = v;
            a.a_run_no_ext[k] = v_no;
            a.q[k] = v * runoff;
            a.q_w[k] = v * runoff;
        }
    }
    out
}
