//! Tier A, TEST-140: the canonical river graph's validators (GEN-189; G0 contracts section 8), each with an injected failure, the canonical serialisation's round trip, the diff, the
//! rasterisation and the budget validator (section 6). A small hand graph at L on a cube sphere of 4 cells a face edge (face 0): sources S1 (0, 0) and S2 (0, 2) meet at the confluence C (1, 1),
//! which runs to the sea outlet O (3, 1); a tributary T from S3 (2, 3) ends at a `trib_junction` J on C -> O's vertex (2, 1).

use planet_generators::bake::river_graph::*;
use planet_generators::bake::Grid;

const N: usize = 4;
const BODY: u64 = 0x5EED;

fn cell(i: usize, j: usize) -> u64 {
    (j * N + i) as u64
}

fn adjacency() -> impl Fn(Level, u64, u64) -> bool {
    let g = Grid::cube(N, 1000.0);
    move |_, a, b| (0..8).any(|d| g.neighbour(a as usize, d) == Some(b as usize))
}

fn node(kind: NodeKind, anchor: u64) -> Node {
    Node {
        id: node_id(BODY, kind, Level::L, anchor),
        kind,
        level: Level::L,
        anchor,
        parent: None,
        owner: Domain::Tile { face: 0, ti: 0, tj: 0 },
        marker: None,
    }
}

/// Per-vertex attributes from the vertex areas (m2) and beds (codes): every area field and the discharges follow `a` (runoff 1e-8 m/s), order 1 unless given.
fn attrs(a: &[f64], bed: &[i32], strahler: u8) -> Attrs {
    Attrs {
        a_geo: a.to_vec(),
        a_ext: vec![0.0; a.len()],
        a_run: a.to_vec(),
        a_run_no_ext: a.to_vec(),
        q: a.iter().map(|x| x * 1e-8).collect(),
        q_w: a.iter().map(|x| x * 1e-8).collect(),
        strahler: vec![strahler; a.len()],
        bed: bed.to_vec(),
    }
}

fn reach(up: &Node, down: &Node, vertices: Vec<u64>, at: Attrs) -> Reach {
    Reach {
        id: reach_id(BODY, Level::L, up.id, down.id),
        level: Level::L,
        up: up.id,
        down: down.id,
        vertices,
        parent: None,
        owner: Domain::Tile { face: 0, ti: 0, tj: 0 },
        attrs: at,
        status: Status::Ok,
    }
}

/// The valid hand graph.
fn base() -> Graph {
    let (s1, s2, c, o, s3) = (
        node(NodeKind::Source, cell(0, 0)),
        node(NodeKind::Source, cell(0, 2)),
        node(NodeKind::Confluence, cell(1, 1)),
        node(NodeKind::OutletSea, cell(3, 1)),
        node(NodeKind::Source, cell(2, 3)),
    );
    let r1 = reach(&s1, &c, vec![cell(0, 0), cell(1, 1)], attrs(&[1.0e4, 3.0e4], &[20, 18], 1));
    let r2 = reach(&s2, &c, vec![cell(0, 2), cell(1, 1)], attrs(&[1.0e4, 3.0e4], &[22, 18], 1));
    let r3 = reach(&c, &o, vec![cell(1, 1), cell(2, 1), cell(3, 1)], attrs(&[3.0e4, 5.0e4, 6.0e4], &[18, 16, 0], 2));
    let mut j = node(NodeKind::TribJunction, cell(2, 1));
    j.marker = Some(Marker { on_reach: r3.id, at_vertex: 1 });
    let t = reach(&s3, &j, vec![cell(2, 3), cell(2, 2), cell(2, 1)], attrs(&[1.0e4, 1.5e4, 1.6e4], &[30, 20, 16], 1));
    let basin = Basin {
        id: basin_id(BODY, Level::L, cell(3, 3)),
        level: Level::L,
        parent: None,
        floor_cell: cell(3, 3),
        floor_m: 4.0,
        spill_cell: cell(3, 2),
        spill_m: 9.5,
        spill_node: j.id,
        outlet: Outlet::Active,
        represented: Represented::ProtectedSurface,
        owner: Domain::Tile { face: 0, ti: 0, tj: 0 },
    };
    let mut g = Graph { body: BODY, nodes: vec![s1, s2, c, o, s3, j], reaches: vec![r1, r2, r3, t], basins: vec![basin] };
    g.sort();
    g
}

fn check(g: &Graph) -> Vec<Violation> {
    let adj = adjacency();
    validate(g, &adj, false)
}

fn reach_mut(g: &mut Graph, up: u64) -> &mut Reach {
    let id = node_id(BODY, NodeKind::Source, Level::L, up);
    let id = if g.reaches.iter().any(|r| r.up == id) { id } else { node_id(BODY, NodeKind::Confluence, Level::L, up) };
    g.reaches.iter_mut().find(|r| r.up == id).expect("a reach from that anchor")
}

// spec: TEST-140
#[test]
fn the_hand_graph_validates_and_round_trips() {
    let g = base();
    assert_eq!(check(&g), vec![]);
    let text = g.canonical();
    let back = Graph::parse(&text).expect("the canonical form parses");
    assert_eq!(back, g);
    assert_eq!(back.canonical(), text, "same graph, same bytes");
    // the order of the input does not matter
    let mut shuffled = g.clone();
    shuffled.nodes.reverse();
    shuffled.reaches.rotate_left(2);
    assert_eq!(shuffled.canonical(), text);
    assert!(text.lines().nth(1).unwrap().starts_with("node "), "nodes first:\n{text}");
}

// spec: TEST-140
#[test]
fn each_injected_failure_is_found() {
    let adj = adjacency();
    let found = |g: &Graph, pred: &dyn Fn(&Violation) -> bool| validate(g, &adj, false).iter().any(pred);
    // a cycle: the outlet becomes a confluence draining back into S1's reach start
    let mut g = base();
    let c = node_id(BODY, NodeKind::Confluence, Level::L, cell(1, 1));
    let s1 = node_id(BODY, NodeKind::Source, Level::L, cell(0, 0));
    let r = reach_mut(&mut g, cell(1, 1));
    r.down = s1;
    assert!(found(&g, &|v| matches!(v, Violation::Cycle { .. } | Violation::InvalidEdge { why: "down is a source", .. })));
    // duplicate id
    let mut g = base();
    let dup = g.nodes[0].clone();
    g.nodes.push(dup);
    assert!(found(&g, &|v| matches!(v, Violation::DuplicateId { .. })));
    // two nodes on one (level, anchor)
    let mut g = base();
    g.nodes.push(node(NodeKind::LakeSpill, cell(1, 1)));
    assert!(found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })));
    // a marker on another node's anchor; allowed only at vertex 0 of the reach that node starts (DEC crusader-6, default A)
    let mut g = base();
    let r3 = reach_mut(&mut g, cell(1, 1)).id;
    let mut m = node(NodeKind::TribJunction, cell(1, 1));
    m.marker = Some(Marker { on_reach: r3, at_vertex: 0 });
    g.nodes.push(m.clone());
    assert!(!found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "a junction at vertex 0 shares the up node's anchor");
    let mut g = base();
    m.marker = Some(Marker { on_reach: r3, at_vertex: 1 });
    g.nodes.push(m);
    assert!(found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })));
    // an unresolved endpoint
    let mut g = base();
    g.nodes.retain(|n| n.kind != NodeKind::OutletSea);
    assert!(found(&g, &|v| matches!(v, Violation::UnresolvedEndpoint { .. })));
    // a chain without an outlet: the outlet kind becomes a lake spill (not an outlet, no outgoing reach)
    let mut g = base();
    for n in g.nodes.iter_mut().filter(|n| n.kind == NodeKind::OutletSea) {
        n.kind = NodeKind::LakeSpill;
    }
    assert!(found(&g, &|v| matches!(v, Violation::ChainWithoutOutlet { .. })));
    // a confluence with one inflow
    let mut g = base();
    let s2 = node_id(BODY, NodeKind::Source, Level::L, cell(0, 2));
    g.reaches.retain(|r| r.up != s2);
    g.nodes.retain(|n| n.id != s2);
    assert!(found(&g, &|v| matches!(v, Violation::ConfluenceOneInflow { node } if *node == c)));
    // a non-outlet node with zero, and with two, outgoing reaches
    let mut g = base();
    g.nodes.push(node(NodeKind::Source, cell(3, 0)));
    assert!(found(&g, &|v| matches!(v, Violation::OutgoingCount { outgoing: 0, .. })));
    let mut g = base();
    let extra = g.reaches.iter().find(|r| r.up == s1).unwrap().clone();
    let mut extra2 = extra.clone();
    extra2.id ^= 1;
    g.reaches.push(extra2);
    assert!(found(&g, &|v| matches!(v, Violation::OutgoingCount { outgoing: 2, .. })));
    // invalid edges: from an outlet, into a source, across levels, a broken chain
    let o = node_id(BODY, NodeKind::OutletSea, Level::L, cell(3, 1));
    let mut g = base();
    let mut back = g.reaches.iter().find(|r| r.down == o).unwrap().clone();
    back.id ^= 7;
    back.up = o;
    back.vertices = vec![cell(3, 1), cell(2, 1)];
    back.attrs = attrs(&[6.0e4, 6.0e4], &[0, 0], 1);
    g.reaches.push(back);
    assert!(found(&g, &|v| matches!(v, Violation::InvalidEdge { why: "up is an outlet", .. })));
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == o).unwrap().level = Level::H1;
    assert!(found(&g, &|v| matches!(v, Violation::InvalidEdge { why: "down at another level", .. })));
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).vertices = vec![cell(1, 1), cell(3, 1), cell(3, 1)];
    assert!(found(&g, &|v| matches!(v, Violation::InvalidEdge { why: "vertices not an 8-connected chain", .. })));
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).vertices = vec![cell(1, 1), cell(2, 1), cell(3, 2)];
    assert!(found(&g, &|v| matches!(v, Violation::InvalidEdge { why: "chain does not end at down's anchor", .. })));
    // a bed rise on codes, allowed only at an exception marker
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, 19, 0];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 1, .. })));
    let r3 = reach_mut(&mut g, cell(1, 1)).id;
    let mut ex = node(NodeKind::Exception, cell(2, 0));
    ex.marker = Some(Marker { on_reach: r3, at_vertex: 1 });
    g.nodes.push(ex);
    assert!(!found(&g, &|v| matches!(v, Violation::BedRise { .. })), "an exception marker allows the rise");
    // a node's bed is shared by its reaches
    let mut g = base();
    reach_mut(&mut g, cell(0, 0)).attrs.bed = vec![20, 19];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 0, .. })));
    // conservation: an area falling along a reach; a confluence below its inflows; a value not shared at the node
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.q[2] = 1.0e-5;
    assert!(found(&g, &|v| matches!(v, Violation::ConservationBreak { field: "q", .. })));
    let mut g = base();
    for j in [0, 2] {
        reach_mut(&mut g, cell(0, j)).attrs.a_geo = vec![2.0e4, 3.0e4];
    }
    assert!(found(&g, &|v| matches!(v, Violation::ConservationBreak { at, field: "a_geo" } if *at == c)), "3e4 below 2e4 + 2e4");
    let mut g = base();
    reach_mut(&mut g, cell(0, 0)).attrs.q_w[1] = 3.5e-4;
    assert!(found(&g, &|v| matches!(v, Violation::ConservationBreak { at, field: "q_w" } if *at == c)));
}

// spec: TEST-140
#[test]
fn two_reaches_owning_one_cell_are_found_and_the_rasterisation_owns_each_cell_once() {
    let g = base();
    let raster = rasterise(&g);
    let claims: Vec<(u64, u64)> = raster.iter().map(|(&c, &(r, _))| (c, r)).collect();
    assert_eq!(validate_raster(&claims), vec![]);
    // the junction cell (1, 1) belongs to the downstream reach C -> O; the shared cell (2, 1) to the larger q_w (C -> O carries 5e-4 against T's 1.6e-4)
    let down = reach_mut(&mut g.clone(), cell(1, 1)).id;
    assert_eq!(raster[&cell(1, 1)].0, down);
    assert_eq!(raster[&cell(2, 1)], (down, 1));
    let mut bad = claims.clone();
    bad.push((cell(2, 1), reach_mut(&mut g.clone(), cell(2, 3)).id));
    assert!(matches!(validate_raster(&bad)[..], [Violation::CellOwnedTwice { cell: c, .. }] if c == cell(2, 1)));
}

// spec: TEST-140
#[test]
fn the_diff_separates_topology_from_attributes() {
    let g = base();
    assert_eq!(diff(&g, &g), vec![]);
    // attribute-only: the same ids, one field named
    let mut a = g.clone();
    reach_mut(&mut a, cell(1, 1)).attrs.q_w[2] = 7.0e-4;
    let d = diff(&g, &a);
    assert_eq!(d.len(), 1);
    assert!(matches!(&d[0], Change::Field { what: "reach", field, .. } if field == "q_w") && !d[0].is_topology());
    // topology: a kind change on an anchor retires the node id and the reaches that end there
    let mut t = g.clone();
    let o = t.nodes.iter_mut().find(|n| n.kind == NodeKind::OutletSea).unwrap();
    let old = o.id;
    o.kind = NodeKind::LakeInlet;
    o.id = node_id(BODY, NodeKind::LakeInlet, Level::L, o.anchor);
    let new = o.id;
    let r = t.reaches.iter_mut().find(|r| r.down == old).unwrap();
    r.down = new;
    r.id = reach_id(BODY, Level::L, r.up, new);
    let d = diff(&g, &t);
    assert!(d.iter().all(Change::is_topology), "{d:?}");
    assert_eq!(d.iter().filter(|c| matches!(c, Change::Removed { .. })).count(), 2);
    assert_eq!(d.iter().filter(|c| matches!(c, Change::Added { .. })).count(), 2);
}

// spec: TEST-140
#[test]
fn the_budget_validator_finds_overruns_and_clamps_without_residuals() {
    let g = base();
    let s0 = vec![100.0, 100.0, 100.0];
    let budget = vec![10.0, 10.0, 10.0];
    // KS lowers 6 m, KV 4 m (exactly the budget), KC fills 3 m: valid
    let ks = vec![94.0, 100.0, 100.0];
    let kv = vec![90.0, 100.0, 100.0];
    let kc = vec![90.0, 103.0, 100.0];
    let stages = |ks: &[f64], kv: &[f64], kc: &[f64]| -> Vec<(&'static str, Vec<f64>, bool)> {
        vec![("ks", ks.to_vec(), false), ("kv", kv.to_vec(), true), ("kc", kc.to_vec(), true)]
    };
    let run = |st: &[(&'static str, Vec<f64>, bool)], wanted: Option<&[f64]>, res: &[Residual], g: &Graph| {
        let s: Vec<Stage> = st
            .iter()
            .map(|(name, out, fills)| Stage { name, out, wanted_lowering: if *name == "kv" { wanted } else { None }, fills: *fills })
            .collect();
        validate_budget(&s0, &budget, &s, res, g)
    };
    assert_eq!(run(&stages(&ks, &kv, &kc), None, &[], &g), vec![]);
    // an injected overrun: KV lowers 0.5 m more
    let kv2 = vec![89.5, 100.0, 100.0];
    let v = run(&stages(&ks, &kv2, &kv2), None, &[], &g);
    assert!(matches!(v[..], [Violation::BudgetOverrun { cell: 0, stage: "kv", .. }]), "{v:?}");
    // a fill over the budget
    let kc2 = vec![90.0, 111.0, 100.0];
    assert!(run(&stages(&ks, &kv, &kc2), None, &[], &g).iter().any(|v| matches!(v, Violation::BudgetOverrun { cell: 1, stage: "kc", .. })));
    // an injected clamp without a residual: KV wanted 7 m at cell 0 and got 4 m
    let wanted = vec![7.0, 0.0, 0.0];
    let v = run(&stages(&ks, &kv, &kc), Some(&wanted), &[], &g);
    assert!(matches!(v[..], [Violation::ClampWithoutResidual { cell: 0, stage: "kv" }]), "{v:?}");
    // with its residual the clamp passes once the feature is flagged budget_residual
    let mut flagged = g.clone();
    let t = reach_mut(&mut flagged, cell(2, 3));
    t.status = Status::Exception { reason: Reason::BudgetResidual, value: 3.0 };
    let feature = t.id;
    let res = [Residual { cell: 0, stage: "kv", needed_m: 7.0, available_m: 4.0, feature }];
    assert_eq!(run(&stages(&ks, &kv, &kc), Some(&wanted), &res, &flagged), vec![]);
    assert!(matches!(run(&stages(&ks, &kv, &kc), Some(&wanted), &res, &g)[..], [Violation::ResidualFeatureNotFlagged { .. }]));
}

// spec: TEST-140
#[test]
fn an_open_slice_reports_no_missing_owner_as_a_violation() {
    let g = base();
    // the slice without the outlet's owner: the outlet's reach ends at an id the slice does not hold
    let o = g.nodes.iter().find(|n| n.kind == NodeKind::OutletSea).unwrap().id;
    let mut s = g.clone();
    s.nodes.retain(|n| n.id != o);
    let adj = adjacency();
    assert!(validate(&s, &adj, true).is_empty(), "{:?}", validate(&s, &adj, true));
    assert!(validate(&s, &adj, false).iter().any(|v| matches!(v, Violation::UnresolvedEndpoint { node, .. } if *node == o)));
}
