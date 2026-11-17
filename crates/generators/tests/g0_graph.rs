//! Tier A, TEST-140 and GEN-189: the G0 fixture's expected river graph (TEST-139's terrain; G0 contracts section 11), built by hand in the contract's types. The topology is the expectation
//! file's; the vertices are today's D8 receiver chains (round rules) on `z_s` at H1 and on `S0` at L, the areas their D8 accumulation of true cell areas. No KS, KG, KV or KC algorithm runs.
//!
//! - H1: the one trunk reach (the cells at or over 75 km2 of D8 area) from its source to the first sea cell, and the closed basin with its spill into a `trib_junction` on it.
//! - L: the trunk's image (anchors by the snap rule, the outlet on the first sea cell, contract section 12); the upper course and T1 meet at the explicit confluence and end at a `trib_junction`
//!   at the trunk's vertex 0; T2 ends at a `trib_junction` on the trunk; river W ends in the sea; every tributary is cut at the tile lines it crosses (`tile_crossing`); the ridge crossing makes the
//!   trunk `exception(rise_over_budget)` with `exception` markers where its bed rises; the basin's image is protected.
//!
//! The graph validates, serialises and round-trips, builds the same bytes on any thread count, and runs under the fixture's two partition layouts: every tile-owned item lies in exactly one
//! region's slice, the landmass items in every slice, and every slice validates as an open slice.

#![allow(clippy::needless_range_loop)]

use crate::g0_fixture::fixture::*;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::h0::GlobalGrid;
use planet_generators::bake::river_graph::*;
use planet_generators::bake::route::{route_rules, RoundRules, Routing, NONE};
use planet_generators::bake::Grid;
use std::collections::BTreeMap;

const EPS: f64 = 1e-3;
const BODY: u64 = 0x0060_F1C7_0000_0139;
/// The H1 trunk threshold (contract section 1: trunk reaches from 75 km2).
const TRUNK_M2: f64 = 75.0e6;
/// The fixture's runoff (m/s): `q = a_run r`.
const RUNOFF: f64 = 1.0e-8;
/// The bed code (GEN-170): 0.5 m.
fn code(z: f64) -> i32 {
    i32::from(planet_generators::bake::channel_graph::bed_code(z))
}

fn path(r: &Routing, from: usize) -> Vec<usize> {
    let mut out = vec![from];
    let mut c = from;
    while r.receiver[c] != NONE {
        c = r.receiver[c] as usize;
        out.push(c);
    }
    out
}

/// The D8 accumulation of true cell areas (an outlet cell adds none of its own) in the routing's topological order.
fn accumulate(g: &Grid, r: &Routing, outlet: &[bool]) -> Vec<f64> {
    let mut acc: Vec<f64> = (0..g.len()).map(|c| if outlet[c] { 0.0 } else { g.cell_area(c) }).collect();
    for &c in r.order.iter().rev() {
        let to = r.receiver[c as usize];
        if to != NONE {
            let a = acc[c as usize];
            acc[to as usize] += a;
        }
    }
    acc
}

fn tile_domain(c: usize) -> Domain {
    let (f, ti, tj) = tile_of_l(c);
    Domain::Tile { face: f as u8, ti: ti as u32, tj: tj as u32 }
}

/// What the build keeps besides the graph.
struct Built {
    graph: Graph,
    trunk_l: Vec<usize>,
    trunk_l_id: u64,
    budget_at_rise: f64,
    rise_m: f64,
}

/// The fixture's expected graph.
fn expected(fx: &Fixture) -> Built {
    let h1 = GlobalGrid::new(H1_N, RADIUS_M);
    let l = GlobalGrid::new(L_N, RADIUS_M);
    let (gh, gl) = (Grid::cube(H1_N, RADIUS_M), Grid::cube(L_N, RADIUS_M));
    let mut g = Graph { body: BODY, ..Graph::default() };

    // ---- H1: the trunk at or over 75 km2, on z_s
    let outlet_h = gh.outlets(&fx.z_s, SEA_LEVEL_M);
    let rh = route_rules(&gh, &fx.z_s, &outlet_h, EPS, 1, 0.0, None, RoundRules::ROUND);
    let acc_h = accumulate(&gh, &rh, &outlet_h);
    let whole = path(&rh, cell_at(H1_N, TRUNK_LON.0 + 0.5, TRUNK_LAT));
    let k0 = whole.iter().position(|&c| acc_h[c] >= TRUNK_M2).expect("the trunk reaches 75 km2");
    let trunk_h: Vec<usize> = whole[k0..].to_vec();
    let node_h = |kind, c: usize| Node {
        id: node_id(BODY, kind, Level::H1, c as u64),
        kind,
        level: Level::H1,
        anchor: c as u64,
        parent: None,
        owner: Domain::Landmass,
        marker: None,
    };
    let (src_h, out_h) = (node_h(NodeKind::Source, trunk_h[0]), node_h(NodeKind::OutletSea, *trunk_h.last().unwrap()));
    let attrs_of = |acc: &[f64], cells: &[usize], z: &[f64], strahler: u8| {
        let a: Vec<f64> = cells.iter().map(|&c| acc[c]).collect();
        let mut bed = Vec::with_capacity(cells.len());
        for &c in cells {
            let b = code(z[c]);
            bed.push(bed.last().map_or(b, |&p: &i32| p.min(b)));
        }
        Attrs {
            a_geo: a.clone(),
            a_ext: vec![0.0; a.len()],
            a_run: a.clone(),
            a_run_no_ext: a.clone(),
            q: a.iter().map(|x| x * RUNOFF).collect(),
            q_w: a.iter().map(|x| x * RUNOFF).collect(),
            strahler: vec![strahler; a.len()],
            bed,
        }
    };
    let reach_h = Reach {
        id: reach_id(BODY, Level::H1, src_h.id, out_h.id),
        level: Level::H1,
        up: src_h.id,
        down: out_h.id,
        vertices: trunk_h.iter().map(|&c| c as u64).collect(),
        parent: None,
        owner: Domain::Landmass,
        attrs: attrs_of(&acc_h, &trunk_h, &fx.z_s, 1),
        status: Status::Ok,
    };
    // the H1 basin: the flood's raised cells around the bowl, its spill the first cell of the floor's receiver path outside it, received by a trib_junction on the trunk
    let basin_of = |grid: &Grid, z: &[f64], outlet: &[bool], r: &Routing, centre: usize| {
        let filled = priority_flood_with(grid, z, outlet, EPS, false);
        let mut inside = vec![false; z.len()];
        let mut stack = vec![centre];
        inside[centre] = true;
        let mut floor = centre;
        while let Some(c) = stack.pop() {
            if (z[c], c) < (z[floor], floor) {
                floor = c;
            }
            for q in (0..8).filter_map(|d| grid.neighbour(c, d)) {
                if !inside[q] && filled[q] > z[q] + 0.01 {
                    inside[q] = true;
                    stack.push(q);
                }
            }
        }
        let spill_path = path(r, floor);
        let k = spill_path.iter().position(|&c| !inside[c]).expect("the basin spills");
        (floor, spill_path[k], spill_path[k..].to_vec())
    };
    let (floor_h, spill_h, spill_path_h) = basin_of(&gh, &fx.z_s, &outlet_h, &rh, cell_at(H1_N, BOWL.0, BOWL.1));
    let join_h = spill_path_h.iter().position(|c| trunk_h.contains(c)).expect("the basin's spill reaches the trunk");
    let at_h = trunk_h.iter().position(|&c| c == spill_path_h[join_h]).unwrap();
    let mut junction_h = node_h(NodeKind::TribJunction, trunk_h[at_h]);
    junction_h.marker = Some(Marker { on_reach: reach_h.id, at_vertex: at_h as u32 });
    let basin_h = Basin {
        id: basin_id(BODY, Level::H1, floor_h as u64),
        level: Level::H1,
        parent: None,
        floor_cell: floor_h as u64,
        floor_m: fx.z_s[floor_h],
        spill_cell: spill_h as u64,
        spill_m: fx.z_s[spill_h],
        spill_node: junction_h.id,
        outlet: Outlet::Active,
        represented: Represented::ProtectedSurface,
        owner: Domain::Landmass,
    };

    // ---- L: routing on S0
    let outlet_l = gl.outlets(&fx.s0, SEA_LEVEL_M);
    let rl = route_rules(&gl, &fx.s0, &outlet_l, EPS, 1, 0.0, None, RoundRules::ROUND);
    let acc_l = accumulate(&gl, &rl, &outlet_l);
    // the snap rule: the lowest L cell of S0 within one H1 cell (814 m) of the H1 anchor's centre, ties by the global cell id
    let snap = |hc: usize| {
        let centre = h1.direction(hc as u32);
        let reach = H1_CELL_M / RADIUS_M;
        let mut best: Option<usize> = None;
        let mut near = vec![hc];
        near.extend((0..8).filter_map(|d| gh.neighbour(hc, d)));
        for n in near {
            let (f, i, j) = (n / (H1_N * H1_N), n % H1_N, (n % (H1_N * H1_N)) / H1_N);
            for dj in 0..L_PER_H1 {
                for di in 0..L_PER_H1 {
                    let c = f * L_N * L_N + (j * L_PER_H1 + dj) * L_N + i * L_PER_H1 + di;
                    let d = l.direction(c as u32);
                    if libm::acos(d.dot(centre).clamp(-1.0, 1.0)) <= reach && best.is_none_or(|b| (fx.s0[c], c) < (fx.s0[b], b)) {
                        best = Some(c);
                    }
                }
            }
        }
        best.expect("an L cell within one H1 cell")
    };
    let src_l_cell = snap(trunk_h[0]);
    let trunk_l = path(&rl, src_l_cell);
    let node_l = |kind, c: usize, owner: Domain| Node {
        id: node_id(BODY, kind, Level::L, c as u64),
        kind,
        level: Level::L,
        anchor: c as u64,
        parent: None,
        owner,
        marker: None,
    };
    let mut src_l = node_l(NodeKind::Source, src_l_cell, Domain::Landmass);
    src_l.parent = Some(src_h.id);
    let mut out_l = node_l(NodeKind::OutletSea, *trunk_l.last().unwrap(), Domain::Landmass);
    out_l.parent = Some(out_h.id);
    let trunk_l_id = reach_id(BODY, Level::L, src_l.id, out_l.id);
    // the budget at L (contract section 6): relief_1km per H1 cell over the L cells within 5 (Chebyshev) of its centre L cell; relief10 the p50 within 25 km; budget = min(50, max(10, relief10 / 2))
    let ring = |c: usize, k: usize| {
        let mut seen = vec![c];
        let mut front = vec![c];
        for _ in 0..k {
            let mut next = Vec::new();
            for &x in &front {
                for q in (0..8).filter_map(|d| gl.neighbour(x, d)) {
                    if !seen.contains(&q) {
                        seen.push(q);
                        next.push(q);
                    }
                }
            }
            front = next;
        }
        seen
    };
    let relief_1km: Vec<f64> = (0..6 * H1_N * H1_N)
        .map(|hc| {
            let (f, i, j) = (hc / (H1_N * H1_N), hc % H1_N, (hc % (H1_N * H1_N)) / H1_N);
            let centre = f * L_N * L_N + (j * L_PER_H1 + L_PER_H1 / 2) * L_N + i * L_PER_H1 + L_PER_H1 / 2;
            let zs: Vec<f64> = ring(centre, 5).into_iter().map(|c| fx.s0[c]).collect();
            zs.iter().copied().fold(f64::NEG_INFINITY, f64::max) - zs.iter().copied().fold(f64::INFINITY, f64::min)
        })
        .collect();
    let budget_h = |hc: usize| {
        let centre = h1.direction(hc as u32);
        let mut near: Vec<f64> = (0..relief_1km.len())
            .filter(|&o| libm::acos(h1.direction(o as u32).dot(centre).clamp(-1.0, 1.0)) * RADIUS_M <= 25_000.0)
            .map(|o| relief_1km[o])
            .collect();
        near.sort_by(f64::total_cmp);
        (0.5 * near[near.len() / 2]).clamp(10.0, 50.0)
    };
    // the trunk's bed at L: the running minimum, cut at most the budget (KS off): where S0 rises more, the bed rises and the reach is exception(rise_over_budget)
    let mut bed: Vec<i32> = Vec::new();
    let (mut low, mut rise, mut budget_at_rise, mut over) = (f64::INFINITY, 0.0f64, 0.0, false);
    for &c in &trunk_l {
        let budget = budget_h(h1_of_l(c));
        low = low.min(fx.s0[c]);
        if fx.s0[c] - low > rise {
            (rise, budget_at_rise) = (fx.s0[c] - low, budget);
        }
        // a vertex whose rise over the bed exceeds its budget keeps the excess: the bed rises there
        over |= fx.s0[c] - low > budget;
        let floor = low.max(fx.s0[c] - budget);
        low = floor;
        bed.push(code(floor));
    }
    let mut trunk_attrs = attrs_of(&acc_l, &trunk_l, &fx.s0, 2);
    trunk_attrs.bed = bed.clone();
    let mut exceptions: Vec<Node> = Vec::new();
    for k in 1..bed.len() {
        if bed[k] > bed[k - 1] {
            let mut m = node_l(NodeKind::Exception, trunk_l[k], Domain::Landmass);
            m.marker = Some(Marker { on_reach: trunk_l_id, at_vertex: k as u32 });
            exceptions.push(m);
        }
    }
    let status = if over { Status::Exception { reason: Reason::RiseOverBudget, value: rise } } else { Status::Ok };
    let trunk_reach_l = Reach {
        id: trunk_l_id,
        level: Level::L,
        up: src_l.id,
        down: out_l.id,
        vertices: trunk_l.iter().map(|&c| c as u64).collect(),
        parent: Some(reach_h.id),
        owner: Domain::Landmass,
        attrs: trunk_attrs,
        status,
    };
    // tributaries: L receiver chains cut at tile lines (a tile_crossing on the last vertex inside the core a chain leaves), ending at `end` (a node id) on `end_cell`
    let mut nodes_l: Vec<Node> = vec![src_l.clone(), out_l.clone()];
    nodes_l.extend(exceptions);
    let mut reaches_l: Vec<Reach> = vec![trunk_reach_l.clone()];
    let mut junctions: BTreeMap<usize, Node> = BTreeMap::new();
    let mut junction_at = |k: usize| {
        junctions
            .entry(k)
            .or_insert_with(|| {
                let mut j = node_l(NodeKind::TribJunction, trunk_l[k], Domain::Landmass);
                j.marker = Some(Marker { on_reach: trunk_l_id, at_vertex: k as u32 });
                j
            })
            .id
    };
    // a chain's reaches: `cells` from its first node `up` to its end node, cut at tile lines; the bed is the running minimum of the whole chain from `start_bed` (if given), held at least at
    // `end.1` and equal to it on the last vertex (a junction's bed is the trunk's there); order `strahler`
    let chain = |cells: &[usize],
                 up: Node,
                 start_bed: Option<i32>,
                 end: (u64, Option<i32>),
                 strahler: u8,
                 nodes: &mut Vec<Node>,
                 reaches: &mut Vec<Reach>| {
        let mut bed: Vec<i32> = Vec::with_capacity(cells.len());
        for &c in cells {
            let b = code(fx.s0[c]);
            bed.push(match bed.last() {
                Some(&p) => p.min(b),
                None => start_bed.unwrap_or(b),
            });
        }
        if let Some(e) = end.1 {
            for x in bed.iter_mut() {
                *x = (*x).max(e);
            }
            *bed.last_mut().unwrap() = e;
        }
        let mut cuts: Vec<usize> = vec![0];
        for k in 1..cells.len() {
            if tile_of_l(cells[k]) != tile_of_l(cells[k - 1]) && k - 1 > *cuts.last().unwrap() {
                cuts.push(k - 1);
            }
        }
        if *cuts.last().unwrap() != cells.len() - 1 {
            cuts.push(cells.len() - 1);
        }
        let mut up = up;
        nodes.push(up.clone());
        for w in cuts.windows(2) {
            let part = &cells[w[0]..=w[1]];
            let last = w[1] == cells.len() - 1;
            let down =
                if last { None } else { Some(node_l(NodeKind::TileCrossing, part[part.len() - 1], tile_domain(part[part.len() - 1]))) };
            let down_id = down.as_ref().map_or(end.0, |n| n.id);
            let mut a = attrs_of(&acc_l, part, &fx.s0, strahler);
            a.bed = bed[w[0]..=w[1]].to_vec();
            reaches.push(Reach {
                id: reach_id(BODY, Level::L, up.id, down_id),
                level: Level::L,
                up: up.id,
                down: down_id,
                vertices: part.iter().map(|&c| c as u64).collect(),
                parent: None,
                owner: tile_domain(part[1]),
                attrs: a,
                status: Status::Ok,
            });
            if let Some(n) = down {
                nodes.push(n.clone());
                up = n;
            }
        }
    };
    // the upper course and T1 meet at the explicit confluence; their joined chain ends at the trib_junction at the trunk's vertex 0
    let upper = path(&rl, cell_at(L_N, TRUNK_LON.0 + 0.25, TRUNK_LAT));
    let k_src = upper.iter().position(|&c| c == src_l_cell).expect("the upper course reaches the trunk's L source");
    let t1 = path(&rl, cell_at(L_N, -21.0, 27.0));
    let kc = t1.iter().position(|c| upper[..k_src].contains(c)).expect("T1 meets the upper course above the trunk's head");
    let conf_cell = t1[kc];
    let ku = upper.iter().position(|&c| c == conf_cell).unwrap();
    let conf = node_l(NodeKind::Confluence, conf_cell, tile_domain(conf_cell));
    let j0 = junction_at(0);
    let trunk_bed0 = trunk_reach_l.attrs.bed[0];
    // the confluence's bed: one value, the lower of the two inflows' running minima at it
    let run_min = |cells: &[usize]| cells.iter().map(|&c| code(fx.s0[c])).min().unwrap();
    let conf_bed = run_min(&upper[..=ku]).min(run_min(&t1[..=kc])).max(trunk_bed0);
    let mut nodes_t: Vec<Node> = Vec::new();
    let mut reaches_t: Vec<Reach> = Vec::new();
    chain(
        &upper[..=ku],
        node_l(NodeKind::Source, upper[0], tile_domain(upper[0])),
        None,
        (conf.id, Some(conf_bed)),
        1,
        &mut nodes_t,
        &mut reaches_t,
    );
    chain(
        &t1[..=kc],
        node_l(NodeKind::Source, t1[0], tile_domain(t1[0])),
        None,
        (conf.id, Some(conf_bed)),
        1,
        &mut nodes_t,
        &mut reaches_t,
    );
    chain(&upper[ku..=k_src], conf.clone(), Some(conf_bed), (j0, Some(trunk_bed0)), 2, &mut nodes_t, &mut reaches_t);
    // T2 ends at a trib_junction on the trunk
    let t2 = path(&rl, cell_at(L_N, 39.5, -27.0));
    let k2 = t2.iter().position(|c| trunk_l.contains(c)).expect("T2 reaches the trunk");
    let at2 = trunk_l.iter().position(|&c| c == t2[k2]).unwrap();
    let j2 = junction_at(at2);
    chain(
        &t2[..=k2],
        node_l(NodeKind::Source, t2[0], tile_domain(t2[0])),
        None,
        (j2, Some(trunk_reach_l.attrs.bed[at2])),
        1,
        &mut nodes_t,
        &mut reaches_t,
    );
    // river W ends in the sea
    let w = path(&rl, cell_at(L_N, -15.5, TRUNK_LAT));
    let w_out = node_l(NodeKind::OutletSea, *w.last().unwrap(), tile_domain(*w.last().unwrap()));
    nodes_t.push(w_out.clone());
    chain(&w, node_l(NodeKind::Source, w[0], tile_domain(w[0])), None, (w_out.id, None), 1, &mut nodes_t, &mut reaches_t);
    // the L basin: the H1 basin's image, protected; its spill received by a trib_junction on the trunk
    let (floor_l, spill_l, spill_path_l) = basin_of(&gl, &fx.s0, &outlet_l, &rl, cell_at(L_N, BOWL.0, BOWL.1));
    let kj = spill_path_l.iter().position(|c| trunk_l.contains(c)).expect("the basin's spill reaches the trunk at L");
    let atb = trunk_l.iter().position(|&c| c == spill_path_l[kj]).unwrap();
    let jb = junction_at(atb);
    let basin_l = Basin {
        id: basin_id(BODY, Level::L, floor_l as u64),
        level: Level::L,
        parent: Some(basin_h.id),
        floor_cell: floor_l as u64,
        floor_m: fx.s0[floor_l],
        spill_cell: spill_l as u64,
        spill_m: fx.s0[spill_l],
        spill_node: jb,
        outlet: Outlet::Active,
        represented: Represented::ProtectedSurface,
        owner: Domain::Landmass,
    };
    nodes_t.push(conf);
    nodes_l.extend(nodes_t);
    nodes_l.extend(junctions.into_values());
    reaches_l.extend(reaches_t);
    // no duplicates from shared start nodes (a chain pushes its up node; the confluence starts one chain and ends two)
    nodes_l.sort_by_key(|n| n.id);
    nodes_l.dedup_by_key(|n| n.id);

    g.nodes = vec![src_h, out_h, junction_h];
    g.nodes.extend(nodes_l);
    g.reaches = vec![reach_h];
    g.reaches.extend(reaches_l);
    g.basins = vec![basin_h, basin_l];
    g.sort();
    Built { graph: g, trunk_l, trunk_l_id, budget_at_rise, rise_m: rise }
}

fn ids_of(s: &Graph) -> Vec<u64> {
    s.nodes.iter().map(|n| n.id).chain(s.reaches.iter().map(|r| r.id)).chain(s.basins.iter().map(|b| b.id)).collect()
}

fn adjacency() -> impl Fn(Level, u64, u64) -> bool {
    let (gh, gl) = (Grid::cube(H1_N, RADIUS_M), Grid::cube(L_N, RADIUS_M));
    move |level, a, b| {
        let g = if level == Level::H1 { &gh } else { &gl };
        (0..8).any(|d| g.neighbour(a as usize, d) == Some(b as usize))
    }
}

// spec: TEST-140, GEN-189
#[test]
fn the_fixture_graph_validates_round_trips_and_is_the_same_on_any_thread_count() {
    let fx = build(4);
    let b = expected(&fx);
    let g = &b.graph;
    let adj = adjacency();
    let tiles = |c: u64| tile_domain(c as usize);
    assert_eq!(validate(g, &Checks { adjacent: &adj, tile_of: Some(&tiles), open: false }), vec![], "the expected graph validates");
    assert_eq!(missing_owners(g), vec![], "a whole graph names no missing owner");
    let text = g.canonical();
    assert_eq!(Graph::parse(&text).expect("parses").canonical(), text, "round trip");
    assert_eq!(expected(&build(1)).graph.canonical(), text, "the same bytes on 1 and 4 threads");
    // the features in the graph
    let count = |k: NodeKind, level: Level| g.nodes.iter().filter(|n| n.kind == k && n.level == level).count();
    assert_eq!(g.reaches.iter().filter(|r| r.level == Level::H1).count(), 1, "one H1 trunk reach");
    assert_eq!(count(NodeKind::Confluence, Level::L), 1, "the explicit confluence");
    assert!(count(NodeKind::TileCrossing, Level::L) >= 1, "a tributary cut at a tile line");
    assert_eq!(count(NodeKind::OutletSea, Level::L), 2, "the trunk's and W's mouths");
    assert_eq!(g.basins.len(), 2, "the basin at H1 and its L image");
    let trunk = g.reaches.iter().find(|r| r.id == b.trunk_l_id).unwrap();
    assert!(
        matches!(trunk.status, Status::Exception { reason: Reason::RiseOverBudget, value } if value > b.budget_at_rise),
        "the antecedent ridge: a rise of {:.1} m over a budget of {:.1} m",
        b.rise_m,
        b.budget_at_rise
    );
    // the status is exception exactly where the reach carries exception markers (its bed rises)
    let marked = g.nodes.iter().any(|n| n.kind == NodeKind::Exception && n.marker.is_some_and(|m| m.on_reach == trunk.id));
    assert!(marked, "the trunk's bed rises at the ridge, marked");
    assert!(
        g.reaches.iter().filter(|r| r.id != trunk.id).all(|r| r.status == Status::Ok && r.attrs.bed.windows(2).all(|w| w[1] <= w[0])),
        "no other reach is exceptional"
    );
    let faces: std::collections::BTreeSet<usize> = b.trunk_l.iter().map(|&c| c / (L_N * L_N)).collect();
    assert_eq!(faces.len(), 2, "the trunk crosses a cube-face edge (gated at H1, reported at L)");
    // the explicit confluence conserves: one shared value, at least the inflows
    let conf = g.nodes.iter().find(|n| n.kind == NodeKind::Confluence).unwrap();
    let inflows: Vec<&Reach> = g.reaches.iter().filter(|r| r.down == conf.id).collect();
    assert_eq!(inflows.len(), 2);
    // the rasterisation owns every L vertex cell, a junction cell to the downstream reach
    let raster = rasterise(g);
    for r in g.reaches.iter().filter(|r| r.level == Level::L) {
        for c in &r.vertices {
            assert!(raster.contains_key(c), "cell {c} of reach {:x}", r.id);
        }
    }
    let j0 = g
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::TribJunction && n.level == Level::L && n.marker.is_some_and(|m| m.at_vertex == 0))
        .expect("the junction at the trunk's head");
    assert_eq!(raster[&j0.anchor].0, trunk.id, "the trunk's head cell belongs to the trunk");
    if std::env::var_os("G0_GRAPH_PRINT").is_some() {
        println!(
            "nodes {} reaches {} basins {}; tile crossings {}; trunk L {} vertices, rise {:.1} m, budget {:.1} m",
            g.nodes.len(),
            g.reaches.len(),
            g.basins.len(),
            count(NodeKind::TileCrossing, Level::L),
            b.trunk_l.len(),
            b.rise_m,
            b.budget_at_rise
        );
    }
}

// spec: TEST-140, GEN-189
#[test]
fn the_fixture_graph_runs_under_both_partition_layouts() {
    let fx = build(4);
    let g = expected(&fx).graph;
    let adj = adjacency();
    let per = TILE_L / L_PER_H1;
    for layout in layouts() {
        let region_of = |d: Domain| match d {
            Domain::Landmass => None,
            Domain::Tile { face, ti, tj } => {
                let h = face as usize * H1_N * H1_N + (tj as usize * per) * H1_N + ti as usize * per;
                Some(layout.region_of_h1[h] as usize)
            }
        };
        let slices: Vec<Graph> = (0..layout.regions).map(|r| slice(&g, &|d| region_of(d).is_none_or(|x| x == r))).collect();
        for (r, s) in slices.iter().enumerate() {
            let tiles = |c: u64| tile_domain(c as usize);
            assert_eq!(
                validate(s, &Checks { adjacent: &adj, tile_of: Some(&tiles), open: true }),
                vec![],
                "{} region {r} validates as an open slice",
                layout.name
            );
            // what the slice names but does not hold is owned by the other region
            for (_, id) in missing_owners(s) {
                assert!(ids_of(&g).contains(&id) && !ids_of(s).contains(&id), "{} region {r}: {id:x}", layout.name);
            }
        }
        // every tile-owned item in exactly one slice, the landmass items in every slice, nothing else
        let ids = |s: &Graph| -> Vec<u64> {
            s.nodes.iter().map(|n| n.id).chain(s.reaches.iter().map(|r| r.id)).chain(s.basins.iter().map(|b| b.id)).collect()
        };
        for n in &g.nodes {
            let holding = slices.iter().filter(|s| s.nodes.iter().any(|x| x.id == n.id)).count();
            assert_eq!(holding, if n.owner == Domain::Landmass { layout.regions } else { 1 }, "{} node {:x}", layout.name, n.id);
        }
        for r in &g.reaches {
            let holding = slices.iter().filter(|s| s.reaches.iter().any(|x| x.id == r.id)).count();
            assert_eq!(holding, if r.owner == Domain::Landmass { layout.regions } else { 1 }, "{} reach {:x}", layout.name, r.id);
        }
        let total: usize = slices.iter().map(|s| ids(s).len()).sum();
        let landmass = ids(&slice(&g, &|d| d == Domain::Landmass)).len();
        assert_eq!(total, ids(&g).len() + (layout.regions - 1) * landmass);
        // the shared (landmass) part is the same bytes in every slice
        let shared: Vec<String> = slices.iter().map(|s| slice(s, &|d| d == Domain::Landmass).canonical()).collect();
        assert!(shared.windows(2).all(|w| w[0] == w[1]), "{}", layout.name);
    }
    // the two-region cut runs through feature 6: a tributary reach owned east of the cut ends at a tile_crossing owned west of it, or the reverse
    let lay = &layouts()[1];
    let region = |d: Domain| match d {
        Domain::Tile { face, ti, tj } => lay.region_of_h1[face as usize * H1_N * H1_N + (tj as usize * per) * H1_N + ti as usize * per],
        Domain::Landmass => 9,
    };
    let crossing = g
        .reaches
        .iter()
        .any(|r| g.nodes.iter().any(|n| n.id == r.up && n.kind == NodeKind::TileCrossing && region(n.owner) != region(r.owner)));
    assert!(crossing, "a tributary reach crosses the two-region cut at a tile_crossing");
}
