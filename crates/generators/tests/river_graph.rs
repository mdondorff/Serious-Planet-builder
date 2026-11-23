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
        exceptions: Vec::new(),
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
    j.id = marker_id(BODY, j.kind, Level::L, j.anchor, r3.id);
    let t = reach(&s3, &j, vec![cell(2, 3), cell(2, 2), cell(2, 1)], attrs(&[1.0e4, 1.5e4, 1.6e4], &[30, 20, 16], 1));
    let basin = Basin {
        id: basin_id(BODY, Level::L, cell(3, 3), Some(cell(3, 2))),
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

/// Two tiles a face edge on the 4-cell face: columns and rows 0-1 and 2-3.
fn tile_of(c: u64) -> Domain {
    let (f, i, j) = (c as usize / (N * N), c as usize % N, (c as usize % (N * N)) / N);
    Domain::Tile { face: f as u8, ti: (i / 2) as u32, tj: (j / 2) as u32 }
}

fn check_with(g: &Graph, tiles: bool, open: bool) -> Vec<Violation> {
    let adj = adjacency();
    let t = tile_of;
    validate(g, &Checks { adjacent: &adj, tile_of: if tiles { Some(&t) } else { None }, open })
}

fn check(g: &Graph) -> Vec<Violation> {
    check_with(g, false, false)
}

fn found(g: &Graph, pred: &dyn Fn(&Violation) -> bool) -> bool {
    check(g).iter().any(pred)
}

fn id(kind: NodeKind, i: usize, j: usize) -> u64 {
    node_id(BODY, kind, Level::L, cell(i, j))
}

fn reach_mut(g: &mut Graph, up: u64) -> &mut Reach {
    let src = node_id(BODY, NodeKind::Source, Level::L, up);
    let up_id = if g.reaches.iter().any(|r| r.up == src) { src } else { node_id(BODY, NodeKind::Confluence, Level::L, up) };
    g.reaches.iter_mut().find(|r| r.up == up_id).expect("a reach from that anchor")
}

/// Recomputes every id from its fields, and the references to them.
fn rehash(g: &mut Graph) {
    // to a fixed point: a marker's id reads its reach's id, a reach's id its ends' ids
    for round in 0.. {
        assert!(round < 32, "rehash reaches a fixed point");
        let before = g.clone();
        let mut map = std::collections::BTreeMap::new();
        for n in g.nodes.iter_mut() {
            let new = node_id_of(BODY, n);
            map.insert(n.id, new);
            n.id = new;
        }
        let mut rmap = std::collections::BTreeMap::new();
        for r in g.reaches.iter_mut() {
            r.up = *map.get(&r.up).unwrap_or(&r.up);
            r.down = *map.get(&r.down).unwrap_or(&r.down);
            let new = reach_id(BODY, r.level, r.up, r.down);
            rmap.insert(r.id, new);
            r.id = new;
        }
        for n in g.nodes.iter_mut() {
            if let Some(m) = n.marker.as_mut() {
                m.on_reach = *rmap.get(&m.on_reach).unwrap_or(&m.on_reach);
            }
        }
        for b in g.basins.iter_mut() {
            b.spill_node = *map.get(&b.spill_node).unwrap_or(&b.spill_node);
        }
        if *g == before {
            break;
        }
    }
    g.sort();
}

// spec: TEST-140, GEN-189
#[test]
fn the_hand_graph_validates_and_round_trips() {
    let g = base();
    assert_eq!(check(&g), vec![]);
    let text = g.canonical();
    let back = Graph::parse(&text).expect("the canonical form parses");
    assert_eq!(back, g);
    assert_eq!(back.canonical(), text, "same graph, same bytes");
    let mut shuffled = g.clone();
    shuffled.nodes.reverse();
    shuffled.reaches.rotate_left(2);
    assert_eq!(shuffled.canonical(), text);
    assert_eq!(check(&shuffled), check(&g), "the validator does not read the input order");
    assert!(text.lines().nth(1).unwrap().starts_with("node "), "nodes first:\n{text}");
    // the parser refuses what the canonical form never writes
    for (from, to) in
        [("outlet=active", "outlet=open"), ("represented=protected_surface", "represented=lake"), ("owner=tile:0:", "owner=tile:300:")]
    {
        assert!(Graph::parse(&text.replacen(from, to, 1)).is_err(), "{to}");
    }
}

// spec: TEST-140
#[test]
fn each_injected_structural_failure_is_found() {
    let c = id(NodeKind::Confluence, 1, 1);
    let o = id(NodeKind::OutletSea, 3, 1);
    let s1 = id(NodeKind::Source, 0, 0);
    let r3 = reach_mut(&mut base(), cell(1, 1)).id;
    // a cycle: C's outflow goes to a new confluence X at (2, 0), which drains back into C
    let mut g = base();
    let x = node(NodeKind::Confluence, cell(2, 0));
    let cx = reach_mut(&mut g, cell(1, 1)).clone();
    g.reaches.retain(|r| r.up != c);
    let to_x = Reach { vertices: vec![cell(1, 1), cell(2, 0)], down: x.id, attrs: attrs(&[3.0e4, 4.0e4], &[18, 18], 2), ..cx.clone() };
    let back = Reach { up: x.id, down: c, vertices: vec![cell(2, 0), cell(1, 1)], attrs: attrs(&[4.0e4, 4.0e4], &[18, 18], 2), ..cx };
    g.nodes.push(x);
    g.reaches.extend([to_x, back]);
    rehash(&mut g);
    assert!(found(&g, &|v| matches!(v, Violation::Cycle { .. })));
    // a duplicate id, and an id that is not the hash of its fields
    let mut g = base();
    let dup = g.nodes[0].clone();
    g.nodes.push(dup);
    assert!(found(&g, &|v| matches!(v, Violation::DuplicateId { .. })));
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == o).unwrap().kind = NodeKind::LakeInlet;
    assert!(found(&g, &|v| matches!(v, Violation::IdMismatch { id } if *id == o)), "a kind changed under a kept id");
    // a basin's id holds its floor and its spill (reading 4): a parent sharing its floor with its deepest child has another id; a spill moved under a kept id is a mismatch
    let (floor, spill) = (cell(3, 3), cell(3, 2));
    assert_ne!(basin_id(BODY, Level::L, floor, Some(spill)), basin_id(BODY, Level::L, floor, Some(cell(2, 2))));
    assert_ne!(basin_id(BODY, Level::L, floor, None), basin_id(BODY, Level::L, floor, Some(0)), "no spill cell is not cell 0");
    let mut g = base();
    g.basins[0].spill_cell = cell(2, 2);
    let b = g.basins[0].id;
    assert!(found(&g, &|v| matches!(v, Violation::IdMismatch { id } if *id == b)), "a spill moved under a kept id");
    let mut g = base();
    let mut parent = g.basins[0].clone();
    parent.spill_cell = cell(1, 3);
    parent.spill_m = 12.0;
    parent.id = basin_id(BODY, Level::L, floor, Some(cell(1, 3)));
    g.basins[0].parent = Some(parent.id);
    g.basins.push(parent);
    assert!(
        !found(&g, &|v| matches!(v, Violation::DuplicateId { .. } | Violation::IdMismatch { .. })),
        "a parent on its child's floor validates"
    );
    // a basin with no spill cell (+inf) validates with the id of no spill; a NaN or -inf spill, or a non-finite floor, is not finite
    let mut g = base();
    g.basins[0].spill_m = f64::INFINITY;
    g.basins[0].id = basin_id(BODY, Level::L, floor, None);
    assert_eq!(check(&g), vec![], "no spill cell");
    for (spill, floor_m, field) in [(f64::NAN, 4.0, "spill_m"), (f64::NEG_INFINITY, 4.0, "spill_m"), (9.5, f64::NAN, "floor_m")] {
        let mut g = base();
        (g.basins[0].spill_m, g.basins[0].floor_m) = (spill, floor_m);
        g.basins[0].id = basin_id(BODY, Level::L, floor, spill_anchor(&g.basins[0]));
        let b = g.basins[0].id;
        assert!(found(&g, &|v| matches!(v, Violation::NotFinite { at, field: f } if *at == b && *f == field)), "{field} {spill} {floor_m}");
    }
    // two nodes on one (level, anchor)
    let mut g = base();
    g.nodes.push(node(NodeKind::LakeSpill, cell(1, 1)));
    assert!(found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })));
    // a marker on a node's anchor: allowed at vertex 0 of the reach that node starts, not at the end of an inflow
    let mut m = node(NodeKind::TribJunction, cell(1, 1));
    m.marker = Some(Marker { on_reach: r3, at_vertex: 0 });
    let mut g = base();
    g.nodes.push(m.clone());
    assert!(!found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "a junction at vertex 0 shares the up node's anchor");
    let mut g = base();
    m.marker = Some(Marker { on_reach: reach_mut(&mut g, cell(0, 0)).id, at_vertex: 1 });
    g.nodes.push(m);
    assert!(
        found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })),
        "a marker at the end of an inflow on the confluence's anchor"
    );
    // markers of different kinds may share an anchor; two of one kind on one reach may not; two of one kind on different reaches may (reading 8)
    let mut g = base();
    let mut e = node(NodeKind::Exception, cell(2, 1));
    e.marker = Some(Marker { on_reach: r3, at_vertex: 1 });
    g.nodes.push(e);
    assert!(!found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "an exception marker beside a junction");
    let mut j2 = node(NodeKind::TribJunction, cell(2, 1));
    j2.id ^= 3;
    j2.marker = Some(Marker { on_reach: r3, at_vertex: 1 });
    g.nodes.push(j2);
    assert!(found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "two junctions on one vertex");
    // a junction on its own reach's last vertex joins nothing
    let mut g = base();
    let t_id = g.reaches.iter().find(|r| r.up == id(NodeKind::Source, 2, 3)).unwrap().id;
    let mut j3 = node(NodeKind::TribJunction, cell(2, 1));
    j3.marker = Some(Marker { on_reach: t_id, at_vertex: 2 });
    j3.id = marker_id(BODY, j3.kind, Level::L, j3.anchor, t_id);
    g.nodes.push(j3);
    assert!(found(&g, &|v| matches!(v, Violation::InvalidMarker { why: "a junction at its reach's last vertex", .. })));
    // a second reach X through (2, 1) (from (1, 0) to a sea outlet at (3, 0)): markers of one kind on two reaches share the anchor (reading 8), but tributaries ending there end at one
    // junction
    let with_x = || {
        let mut g = base();
        let (sx, ox) = (node(NodeKind::Source, cell(1, 0)), node(NodeKind::OutletSea, cell(3, 0)));
        let x = reach(&sx, &ox, vec![cell(1, 0), cell(2, 1), cell(3, 0)], attrs(&[1.0e4, 2.0e4, 3.0e4], &[30, 16, 0], 1));
        let xid = x.id;
        g.nodes.extend([sx, ox]);
        g.reaches.push(x);
        (g, xid)
    };
    let (mut g, xid) = with_x();
    for on in [r3, xid] {
        let mut e = node(NodeKind::Exception, cell(2, 1));
        e.marker = Some(Marker { on_reach: on, at_vertex: 1 });
        e.id = marker_id(BODY, e.kind, Level::L, e.anchor, on);
        g.nodes.push(e);
    }
    assert!(!found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "exception markers on one anchor, on two reaches");
    let (mut g, xid) = with_x();
    let mut j4 = node(NodeKind::TribJunction, cell(2, 1));
    j4.marker = Some(Marker { on_reach: xid, at_vertex: 1 });
    j4.id = marker_id(BODY, j4.kind, Level::L, j4.anchor, xid);
    g.nodes.push(j4.clone());
    assert!(!found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "a junction without inflow beside the one with T");
    let sy = node(NodeKind::Source, cell(1, 2));
    g.reaches.push(reach(&sy, &j4, vec![cell(1, 2), cell(2, 1)], attrs(&[1.0e4, 2.0e4], &[30, 16], 1)));
    g.nodes.push(sy);
    assert!(found(&g, &|v| matches!(v, Violation::TwoNodesOneAnchor { .. })), "two junctions with inflows on one anchor");
    // markers off their reach: no reach, out of range, another vertex; a marker kind without, and a non-marker with, the field
    for (mk, why) in [
        (Marker { on_reach: 42, at_vertex: 1 }, "its reach does not exist"),
        (Marker { on_reach: r3, at_vertex: 9 }, "at_vertex out of range"),
        (Marker { on_reach: r3, at_vertex: 2 }, "not at its reach's vertex"),
    ] {
        let mut g = base();
        g.nodes.iter_mut().find(|n| n.kind == NodeKind::TribJunction).unwrap().marker = Some(mk);
        assert!(found(&g, &|v| matches!(v, Violation::InvalidMarker { why: w, .. } if *w == why)), "{why}");
    }
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.kind == NodeKind::TribJunction).unwrap().marker = None;
    assert!(found(&g, &|v| matches!(v, Violation::InvalidMarker { why: "a marker kind without its reach", .. })));
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == o).unwrap().marker = Some(Marker { on_reach: r3, at_vertex: 2 });
    assert!(found(&g, &|v| matches!(v, Violation::InvalidMarker { why: "a marker field on a non-marker", .. })));
    // an unresolved endpoint
    let mut g = base();
    g.nodes.retain(|n| n.id != o);
    assert!(found(&g, &|v| matches!(v, Violation::UnresolvedEndpoint { node, .. } if *node == o)));
    // a chain without an outlet, followed through a junction: the outlet becomes a lake spill
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == o).unwrap().kind = NodeKind::LakeSpill;
    rehash(&mut g);
    let t_src = id(NodeKind::Source, 2, 3);
    assert!(
        found(&g, &|v| matches!(v, Violation::ChainWithoutOutlet { node } if *node == t_src)),
        "the tributary's chain runs on through its junction"
    );
    // a confluence with one inflow; two inflows into a non-confluence
    let mut g = base();
    let s2 = id(NodeKind::Source, 0, 2);
    g.reaches.retain(|r| r.up != s2);
    g.nodes.retain(|n| n.id != s2);
    assert!(found(&g, &|v| matches!(v, Violation::ConfluenceOneInflow { node } if *node == c)));
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == c).unwrap().kind = NodeKind::TileCrossing;
    rehash(&mut g);
    assert!(found(&g, &|v| matches!(v, Violation::InflowsToNonConfluence { .. })));
    // a non-outlet node with zero, and with two, outgoing reaches
    let mut g = base();
    g.nodes.push(node(NodeKind::Source, cell(3, 0)));
    assert!(found(&g, &|v| matches!(v, Violation::OutgoingCount { outgoing: 0, .. })));
    let mut g = base();
    let mut extra = g.reaches.iter().find(|r| r.up == s1).unwrap().clone();
    extra.id ^= 1;
    g.reaches.push(extra);
    assert!(found(&g, &|v| matches!(v, Violation::OutgoingCount { outgoing: 2, .. })));
}

// spec: TEST-140
#[test]
fn each_injected_edge_failure_is_found() {
    let o = id(NodeKind::OutletSea, 3, 1);
    let s1 = id(NodeKind::Source, 0, 0);
    let s3 = id(NodeKind::Source, 2, 3);
    let edge = |g: &Graph, why: &'static str| found(g, &|v| matches!(v, Violation::InvalidEdge { why: w, .. } if *w == why));
    // up is an outlet; down is a source
    let mut g = base();
    let mut back = g.reaches.iter().find(|r| r.down == o).unwrap().clone();
    (back.up, back.down, back.vertices) = (o, s3, vec![cell(3, 1), cell(2, 2), cell(2, 3)]);
    back.attrs = attrs(&[6.0e4, 6.0e4, 6.0e4], &[0, 0, 0], 1);
    g.reaches.push(back);
    assert!(edge(&g, "up is an outlet") && edge(&g, "down is a source"));
    // up is a marker
    let mut g = base();
    let j = g.nodes.iter().find(|n| n.kind == NodeKind::TribJunction).unwrap().clone();
    let mut from_j = g.reaches.iter().find(|r| r.down == o).unwrap().clone();
    (from_j.up, from_j.vertices) = (j.id, vec![cell(2, 1), cell(3, 1)]);
    from_j.attrs = attrs(&[5.0e4, 6.0e4], &[16, 0], 2);
    g.reaches.push(from_j);
    assert!(edge(&g, "up is a marker"));
    // a reach ending at an exception marker
    let mut g = base();
    let r3 = reach_mut(&mut g, cell(1, 1)).id;
    let mut e = node(NodeKind::Exception, cell(2, 1));
    e.marker = Some(Marker { on_reach: r3, at_vertex: 1 });
    g.reaches.iter_mut().find(|r| r.up == s3).unwrap().down = e.id;
    g.nodes.push(e);
    assert!(edge(&g, "down is an exception marker"));
    // up and down at another level
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == s1).unwrap().level = Level::H1;
    assert!(edge(&g, "up at another level"));
    let mut g = base();
    g.nodes.iter_mut().find(|n| n.id == o).unwrap().level = Level::H1;
    assert!(edge(&g, "down at another level"));
    // chains: not 8-connected, not starting at up's anchor, not ending at down's anchor; attributes not one per vertex (an empty reach is reported, not a panic)
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).vertices = vec![cell(1, 1), cell(3, 1), cell(3, 1)];
    assert!(edge(&g, "vertices not an 8-connected chain"));
    let mut g = base();
    reach_mut(&mut g, cell(0, 0)).vertices = vec![cell(0, 1), cell(1, 1)];
    assert!(edge(&g, "chain does not start at up's anchor"));
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).vertices = vec![cell(1, 1), cell(2, 1), cell(3, 2)];
    assert!(edge(&g, "chain does not end at down's anchor"));
    let mut g = base();
    let r = reach_mut(&mut g, cell(1, 1));
    r.vertices.clear();
    r.attrs = Attrs::default();
    assert!(edge(&g, "attributes not one per vertex"));
    // a tributary leaving its tile core (two tiles a face edge here): T runs from tile (1, 1) into tile (1, 0) after its first vertex; the other reaches stay in one core
    let v = check_with(&base(), true, false);
    let t = reach_mut(&mut base(), cell(2, 3)).id;
    assert_eq!(v, vec![Violation::TributaryLeavesTile { reach: t }]);
    // a trunk image (with a parent) is never cut at tiles
    let mut g = base();
    reach_mut(&mut g, cell(2, 3)).parent = Some(7);
    assert_eq!(check_with(&g, true, false), vec![]);
}

// spec: TEST-140
#[test]
fn each_injected_bed_and_conservation_failure_is_found() {
    let c = id(NodeKind::Confluence, 1, 1);
    let j = base().nodes.iter().find(|n| n.kind == NodeKind::TribJunction).unwrap().id;
    // a bed rise on codes, allowed only at an exception marker on that vertex
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, 19, 0];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 1, .. })));
    let r3 = reach_mut(&mut g, cell(1, 1)).id;
    let mut ex = node(NodeKind::Exception, cell(2, 1));
    ex.marker = Some(Marker { on_reach: r3, at_vertex: 1 });
    g.nodes.push(ex);
    assert!(!found(&g, &|v| matches!(v, Violation::BedRise { .. })), "an exception marker allows the rise");
    // the junction there now sees the trunk at 19 and T arriving at 16
    assert!(
        found(&g, &|v| matches!(v, Violation::BedNotShared { node } if *node == j)),
        "a tributary carries the trunk's bed at its junction"
    );
    // reading 41: a rise into a reach's own down node is excused only within the reach's budget_residual; a rise without the record, a larger one, or one at another
    // vertex stays a violation
    let rises = |g: &Graph| check(g).into_iter().filter(|v| matches!(v, Violation::BedRise { .. })).count();
    let adj = adjacency();
    let excused = |g: &Graph| excused_down_rises(g, &Checks { adjacent: &adj, tile_of: None, open: false });
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, 16, 17];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 2, .. })), "a rise into the down node without the record");
    assert_eq!(excused(&g), (0, 0.0));
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 0.5 }];
    assert_eq!(rises(&g), 0, "the residual records the 0.5 m rise into the down node");
    assert_eq!(excused(&g), (1, 0.5));
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 0.4 }];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 2, .. })), "a rise larger than the residual");
    assert_eq!(excused(&g), (0, 0.0));
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::RiseOverBudget, value: 5.0 }];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 2, .. })), "another reason is no residual record");
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, 19, 0];
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 5.0 }];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 1, .. })), "a rise at another vertex stays a violation");
    assert_eq!(excused(&g), (0, 0.0));
    // numerics review: a rise a marker already excuses is not counted again; a malformed reach (InvalidEdge) is not bed-checked, so nothing is excused there; codes at the
    // i32 ends do not overflow (a rise, never excused by a small residual)
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, 16, 17];
    let r3 = reach_mut(&mut g, cell(1, 1)).id;
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 0.5 }];
    let mut mk = node(NodeKind::Exception, cell(3, 1));
    mk.marker = Some(Marker { on_reach: r3, at_vertex: 2 });
    g.nodes.push(mk);
    assert_eq!(excused(&g), (0, 0.0), "the marker excuses it, not the residual");
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, 16, 17];
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 0.5 }];
    reach_mut(&mut g, cell(1, 1)).attrs.q.pop();
    assert_eq!(excused(&g), (0, 0.0), "a malformed reach is not bed-checked");
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.bed = vec![18, i32::MIN, i32::MAX];
    reach_mut(&mut g, cell(1, 1)).exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 0.0 }];
    assert!(found(&g, &|v| matches!(v, Violation::BedRise { vertex: 2, .. })), "no overflow excuses the rise");
    // a node's bed is shared by its reaches
    let mut g = base();
    reach_mut(&mut g, cell(0, 0)).attrs.bed = vec![20, 19];
    assert!(found(&g, &|v| matches!(v, Violation::BedNotShared { node } if *node == c)));
    // conservation along a reach: each field falling
    for f in ["a_geo", "a_run", "q", "q_w"] {
        let mut g = base();
        let a = &mut reach_mut(&mut g, cell(1, 1)).attrs;
        let x = match f {
            "a_geo" => &mut a.a_geo,
            "a_run" => &mut a.a_run,
            "q" => &mut a.q,
            _ => &mut a.q_w,
        };
        x[2] = x[1] * 0.5;
        assert!(found(&g, &|v| matches!(v, Violation::ConservationBreak { field, .. } if *field == f)), "{f}");
    }
    // at a confluence: below its inflows, and a value not shared
    let mut g = base();
    for jj in [0, 2] {
        reach_mut(&mut g, cell(0, jj)).attrs.a_geo = vec![2.0e4, 3.0e4];
    }
    assert!(found(&g, &|v| matches!(v, Violation::ConservationBreak { at, field: "a_geo" } if *at == c)), "3e4 below 2e4 + 2e4");
    for f in ["a_run", "q_w"] {
        let mut g = base();
        let a = &mut reach_mut(&mut g, cell(0, 0)).attrs;
        match f {
            "a_run" => a.a_run[1] = 3.5e4,
            _ => a.q_w[1] = 3.5e-4,
        }
        assert!(found(&g, &|v| matches!(v, Violation::ConservationBreak { at, field } if *at == c && *field == f)), "{f}");
    }
    // a value that is not finite
    let mut g = base();
    reach_mut(&mut g, cell(1, 1)).attrs.q_w[1] = f64::NAN;
    assert!(found(&g, &|v| matches!(v, Violation::NotFinite { field: "q_w", .. })));
}

// spec: TEST-140
#[test]
fn the_rasterisation_owns_each_cell_once_by_its_rules() {
    let g = base();
    let raster = rasterise(&g);
    for r in &g.reaches {
        for c in &r.vertices {
            assert!(raster.contains_key(c), "cell {c} of reach {:x}", r.id);
        }
    }
    let claims: Vec<(u64, u64)> = raster.iter().map(|(&c, &(r, _))| (c, r)).collect();
    assert_eq!(validate_raster(&claims), vec![]);
    // junction cells belong to the downstream reach: the confluence (1, 1) and the trib_junction (2, 1) to C -> O, whatever the q_w
    let down = reach_mut(&mut g.clone(), cell(1, 1)).id;
    assert_eq!(raster[&cell(1, 1)].0, down);
    let mut big_t = g.clone();
    for x in reach_mut(&mut big_t, cell(2, 3)).attrs.q_w.iter_mut() {
        *x = 1.0;
    }
    assert_eq!(rasterise(&big_t)[&cell(2, 1)], (down, 1), "the tributary's last vertex yields to the trunk");
    // elsewhere the larger q_w wins, ties by the lower reach id: T shares (2, 2) with a parallel reach P from S4 (3, 3) to O
    let mut p = g.clone();
    let s4 = node(NodeKind::Source, cell(3, 3));
    let o = p.nodes.iter().find(|n| n.kind == NodeKind::OutletSea).unwrap().clone();
    let r = reach(&s4, &o, vec![cell(3, 3), cell(2, 2), cell(3, 1)], attrs(&[1.0e4, 1.5e4, 1.6e4], &[30, 20, 0], 1));
    let (t_id, p_id) = (reach_mut(&mut p, cell(2, 3)).id, r.id);
    p.nodes.push(s4);
    p.reaches.push(r);
    assert_eq!(rasterise(&p)[&cell(2, 2)].0, t_id.min(p_id), "equal q_w: the lower id");
    // a signed zero is no larger: -0.0 and +0.0 tie, and the lower id wins whichever sign it carries
    for (t_q, p_q) in [(-0.0, 0.0), (0.0, -0.0)] {
        let mut z = p.clone();
        reach_mut(&mut z, cell(2, 3)).attrs.q_w[1] = t_q;
        reach_mut(&mut z, cell(3, 3)).attrs.q_w[1] = p_q;
        assert_eq!(rasterise(&z)[&cell(2, 2)].0, t_id.min(p_id), "q_w {t_q:?} against {p_q:?}");
    }
    reach_mut(&mut p, cell(3, 3)).attrs.q_w[1] = 1.0;
    assert_eq!(rasterise(&p)[&cell(2, 2)].0, p_id, "the larger q_w");
    let mut shuffled = p.clone();
    shuffled.reaches.reverse();
    assert_eq!(rasterise(&shuffled), rasterise(&p), "the same for any input order");
    // two claims on one cell
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
    // topology: a kind change on an anchor retires the node id and the reach that ends there; the junction on that reach takes the reach's new id (reading 8), so the tributary ending at it
    // is retired too
    let mut t = g.clone();
    t.nodes.iter_mut().find(|n| n.kind == NodeKind::OutletSea).unwrap().kind = NodeKind::LakeInlet;
    rehash(&mut t);
    assert_eq!(check(&t), vec![]);
    let d = diff(&g, &t);
    assert_eq!(d.iter().filter(|c| matches!(c, Change::Removed { .. })).count(), 4, "{d:?}");
    assert_eq!(d.iter().filter(|c| matches!(c, Change::Added { .. })).count(), 4, "{d:?}");
    assert!(d.iter().all(|c| c.is_topology() || matches!(c, Change::Field { field, .. } if field == "spill_node")), "{d:?}");
}

// spec: TEST-140
#[test]
fn the_budget_validator_finds_overruns_raises_and_clamps_without_residuals() {
    let g = base();
    let s0 = vec![100.0, 100.0, 100.0];
    let budget = vec![10.0, 10.0, 10.0];
    let run = |ks: &[f64], kv: &[f64], kc: &[f64], wanted: Option<&[f64]>, res: &[Residual], g: &Graph| {
        let s = [
            Stage { name: "ks", out: ks, wanted_lowering: None, wanted_fill: None, fills: false },
            Stage { name: "kv", out: kv, wanted_lowering: wanted, wanted_fill: None, fills: true },
            Stage { name: "kc", out: kc, wanted_lowering: None, wanted_fill: None, fills: true },
        ];
        validate_budget(&s0, &budget, &s, res, g)
    };
    // KS lowers 6 m, KV 4 m (exactly the budget), KC fills 3 m: valid
    let (ks, kv, kc) = ([94.0, 100.0, 100.0], [90.0, 100.0, 100.0], [90.0, 103.0, 100.0]);
    assert_eq!(run(&ks, &kv, &kc, None, &[], &g), vec![]);
    // an overrun: KV lowers 0.5 m more, KC 0.1 m more again: one violation, at KV
    let (kv2, kc2) = ([89.5, 100.0, 100.0], [89.4, 100.0, 100.0]);
    let v = run(&ks, &kv2, &kc2, None, &[], &g);
    assert!(matches!(v[..], [Violation::BudgetOverrun { cell: 0, stage: "kv", .. }]), "{v:?}");
    // a residual does not excuse a lowering overrun
    let mut flagged = g.clone();
    let t = reach_mut(&mut flagged, cell(2, 3));
    t.exceptions = vec![Exception { reason: Reason::BudgetResidual, value: 3.0 }];
    let feature = t.id;
    let res = [Residual { cell: 0, stage: "kv", needed_m: 7.0, available_m: 4.0, feature }];
    assert!(run(&ks, &kv2, &kv2, None, &res, &flagged).iter().any(|v| matches!(v, Violation::BudgetOverrun { cell: 0, stage: "kv", .. })));
    // a fill over the budget, excused only by its residual
    let kc3 = [90.0, 111.0, 100.0];
    assert!(run(&ks, &kv, &kc3, None, &[], &g).iter().any(|v| matches!(v, Violation::BudgetOverrun { cell: 1, stage: "kc", .. })));
    let res_fill = [Residual { cell: 1, stage: "kc", needed_m: 11.0, available_m: 10.0, feature }];
    assert_eq!(run(&ks, &kv, &kc3, None, &res_fill, &flagged), vec![]);
    // a lowering stage that raises
    let ks_up = [94.0, 105.0, 100.0];
    assert!(run(&ks_up, &kv, &kc, None, &[], &g).iter().any(|v| matches!(v, Violation::RaiseInLoweringStage { cell: 1, stage: "ks" })));
    // a clamp without a residual: KV wanted 7 m at cell 0 and got 4 m; with its residual it passes once the feature is flagged
    let wanted = [7.0, 0.0, 0.0];
    let v = run(&ks, &kv, &kc, Some(&wanted), &[], &g);
    assert!(matches!(v[..], [Violation::ClampWithoutResidual { cell: 0, stage: "kv" }]), "{v:?}");
    assert_eq!(run(&ks, &kv, &kc, Some(&wanted), &res, &flagged), vec![]);
    assert!(matches!(run(&ks, &kv, &kc, Some(&wanted), &res, &g)[..], [Violation::ResidualFeatureNotFlagged { .. }]));
    // a residual carries its stage's numbers: the clamp needed 7 m with 4 m left (KS used 6); the fill needed 11 m against 10 m; a residual where nothing clamped or overran fails
    let mismatch = |v: &[Violation]| v.iter().any(|x| matches!(x, Violation::ResidualMismatch { .. }));
    for (needed, available) in [(6.0, 4.0), (7.0, 10.0)] {
        let wrong = [Residual { cell: 0, stage: "kv", needed_m: needed, available_m: available, feature }];
        assert!(mismatch(&run(&ks, &kv, &kc, Some(&wanted), &wrong, &flagged)), "clamp needed {needed}, available {available}");
    }
    let wrong_fill = [Residual { cell: 1, stage: "kc", needed_m: 11.0, available_m: 4.0, feature }];
    assert!(mismatch(&run(&ks, &kv, &kc3, None, &wrong_fill, &flagged)));
    let idle = [Residual { cell: 2, stage: "kc", needed_m: 1.0, available_m: 10.0, feature }];
    assert!(matches!(run(&ks, &kv, &kc, None, &idle, &flagged)[..], [Violation::ResidualMismatch { cell: 2, stage: "kc", .. }]));
    // a residual naming no stage that ran, or a cell outside the surface
    for (cell, stage) in [(0, "kx"), (99, "kv")] {
        let stray = [Residual { cell, stage, needed_m: 1.0, available_m: 1.0, feature }];
        assert!(
            run(&ks, &kv, &kc, None, &stray, &flagged)
                .iter()
                .any(|v| matches!(v, Violation::ResidualMismatch { why: "no such cell or stage", .. })),
            "{cell} {stage}"
        );
    }
    // a duplicate residual is one more than the events
    let twice = [res[0].clone(), res[0].clone()];
    assert!(run(&ks, &kv, &kc, Some(&wanted), &twice, &flagged)
        .iter()
        .any(|v| matches!(v, Violation::ResidualMismatch { why: "more residuals than clamps and fill overruns", .. })));
    // one cell that clamps and over-fills in one stage (KV wanted 7 m at cell 0 and raised it 12 m): one residual for each event passes, a wrong one fails
    let kv_up = [106.0, 100.0, 100.0];
    let kc_up = [106.0, 103.0, 100.0];
    let both = [
        Residual { cell: 0, stage: "kv", needed_m: 7.0, available_m: 4.0, feature },
        Residual { cell: 0, stage: "kv", needed_m: 12.0, available_m: 10.0, feature },
    ];
    assert_eq!(run(&ks, &kv_up, &kc_up, Some(&wanted), &both, &flagged), vec![]);
    let wrong = [both[0].clone(), Residual { cell: 0, stage: "kv", needed_m: 11.0, available_m: 10.0, feature }];
    assert!(mismatch(&run(&ks, &kv_up, &kc_up, Some(&wanted), &wrong, &flagged)));
    // one to one: the clamp's record alone leaves the fill overrun unrecorded; the clamp's record twice records the clamp once; the fill's record alone leaves the clamp
    let v = run(&ks, &kv_up, &kc_up, Some(&wanted), &[both[0].clone()], &flagged);
    assert!(v.iter().any(|x| matches!(x, Violation::BudgetOverrun { cell: 0, stage: "kv", .. })), "{v:?}");
    let v = run(&ks, &kv_up, &kc_up, Some(&wanted), &[both[0].clone(), both[0].clone()], &flagged);
    assert!(v.iter().any(|x| matches!(x, Violation::BudgetOverrun { cell: 0, stage: "kv", .. })), "{v:?}");
    assert!(
        v.iter().any(|x| matches!(x, Violation::ResidualMismatch { why: "more residuals than clamps and fill overruns", .. })),
        "{v:?}"
    );
    let v = run(&ks, &kv_up, &kc_up, Some(&wanted), &[both[1].clone()], &flagged);
    assert!(v.iter().any(|x| matches!(x, Violation::ClampWithoutResidual { cell: 0, stage: "kv" })), "{v:?}");
    // a residual where the stage's output is not finite
    let kv_nan0 = [f64::NAN, 100.0, 100.0];
    let v = run(&ks, &kv_nan0, &kc, None, &[both[0].clone()], &flagged);
    assert!(v.iter().any(|x| matches!(x, Violation::ResidualMismatch { why: "its stage's output is not finite there", .. })), "{v:?}");
    // inputs that are not finite or do not fit
    let kv_nan = [f64::NAN, 100.0, 100.0];
    assert!(run(&ks, &kv_nan, &kc, None, &[], &g).iter().any(|v| matches!(v, Violation::BudgetInput { cell: 0, stage: "kv" })));
    let short = [90.0, 100.0];
    assert!(matches!(run(&ks, &short, &kc, None, &[], &g)[..], [Violation::BudgetInput { stage: "kv", .. }]));
}

// spec: TEST-140
#[test]
fn a_fill_held_under_its_wanted_fill_needs_a_residual() {
    // reading 9: KC wanted to fill 15 m at cell 1 and filled 10 m (its budget): a fill clamp, recorded like a lowering clamp
    let g = base();
    let s0 = vec![100.0, 100.0, 100.0];
    let budget = vec![10.0, 10.0, 10.0];
    let (ks, kv, kc) = ([94.0, 100.0, 100.0], [90.0, 100.0, 100.0], [90.0, 110.0, 100.0]);
    let run = |kc: &[f64], wanted: &[f64], res: &[Residual], g: &Graph| {
        let s = [
            Stage { name: "ks", out: &ks, wanted_lowering: None, wanted_fill: None, fills: false },
            Stage { name: "kv", out: &kv, wanted_lowering: None, wanted_fill: None, fills: true },
            Stage { name: "kc", out: kc, wanted_lowering: None, wanted_fill: Some(wanted), fills: true },
        ];
        validate_budget(&s0, &budget, &s, res, g)
    };
    let wanted = [0.0, 15.0, 0.0];
    assert_eq!(run(&kc, &wanted, &[], &g), vec![Violation::FillClampWithoutResidual { cell: 1, stage: "kc" }]);
    // a fill that reached what it wanted is no event
    assert_eq!(run(&kc, &[0.0, 10.0, 0.0], &[], &g), vec![]);
    // the residual's feature is already exceptional for another reason: the list holds both (finding 3 of SPIKE-132)
    let mut flagged = g.clone();
    let t = reach_mut(&mut flagged, cell(2, 3));
    t.exceptions =
        vec![Exception { reason: Reason::RiseOverBudget, value: 12.5 }, Exception { reason: Reason::BudgetResidual, value: 5.0 }];
    let feature = t.id;
    let res = [Residual { cell: 1, stage: "kc", needed_m: 15.0, available_m: 10.0, feature }];
    assert_eq!(run(&kc, &wanted, &res, &flagged), vec![]);
    // a feature with only the other reason is not flagged for the residual
    let mut other = g.clone();
    reach_mut(&mut other, cell(2, 3)).exceptions = vec![Exception { reason: Reason::RiseOverBudget, value: 12.5 }];
    assert!(matches!(run(&kc, &wanted, &res, &other)[..], [Violation::ResidualFeatureNotFlagged { .. }]));
    // the residual carries the wanted fill and the budget
    for (needed, available) in [(10.0, 10.0), (15.0, 4.0)] {
        let wrong = [Residual { cell: 1, stage: "kc", needed_m: needed, available_m: available, feature }];
        let v = run(&kc, &wanted, &wrong, &flagged);
        assert!(v.iter().any(|x| matches!(x, Violation::ResidualMismatch { cell: 1, stage: "kc", .. })), "{needed} {available}: {v:?}");
        assert!(v.iter().any(|x| matches!(x, Violation::FillClampWithoutResidual { cell: 1, stage: "kc" })), "{needed} {available}: {v:?}");
    }
    // one event, one residual: a second record of the fill clamp is left over
    let twice = [res[0].clone(), res[0].clone()];
    assert!(run(&kc, &wanted, &twice, &flagged)
        .iter()
        .any(|v| matches!(v, Violation::ResidualMismatch { why: "more residuals than clamps and fill overruns", .. })));
    // a fill over the budget is one event, not also a clamp: its own residual alone passes
    let kc_over = [90.0, 111.0, 100.0];
    let over = [Residual { cell: 1, stage: "kc", needed_m: 11.0, available_m: 10.0, feature }];
    assert_eq!(run(&kc_over, &wanted, &over, &flagged), vec![]);
    // a wanted fill that is not finite or does not fit
    assert!(run(&kc, &[0.0, f64::NAN, 0.0], &[], &g).iter().any(|v| matches!(v, Violation::BudgetInput { cell: 1, stage: "kc" })));
    assert!(matches!(run(&kc, &[0.0, 15.0], &[], &g)[..], [Violation::BudgetInput { stage: "kc", .. }]));
    // a negative wanted fill is an input error, not a silent pass
    assert!(run(&kc, &[0.0, -15.0, 0.0], &[], &g).iter().any(|v| matches!(v, Violation::BudgetInput { cell: 1, stage: "kc" })));
    // a stage that may not fill wants no fill
    let ks_wants_fill = [
        Stage { name: "ks", out: &ks, wanted_lowering: None, wanted_fill: Some(&[0.0, 5.0, 0.0]), fills: false },
        Stage { name: "kv", out: &kv, wanted_lowering: None, wanted_fill: None, fills: true },
    ];
    assert!(validate_budget(&s0, &budget, &ks_wants_fill, &[], &g)
        .iter()
        .any(|v| matches!(v, Violation::BudgetInput { cell: 1, stage: "ks" })));
    // a stage with a bad wanted value still uses the budget: KS 6 m, KV 4 m (its wanted fill NaN), KC 2 m is a KC overrun
    let kv_bad = [
        Stage { name: "ks", out: &[94.0], wanted_lowering: None, wanted_fill: None, fills: false },
        Stage { name: "kv", out: &[90.0], wanted_lowering: None, wanted_fill: Some(&[f64::NAN]), fills: true },
        Stage { name: "kc", out: &[88.0], wanted_lowering: Some(&[-1.0]), wanted_fill: None, fills: true },
    ];
    let v = validate_budget(&[100.0], &[10.0], &kv_bad, &[], &g);
    assert!(v.iter().any(|x| matches!(x, Violation::BudgetInput { cell: 0, stage: "kv" })), "{v:?}");
    assert!(v.iter().any(|x| matches!(x, Violation::BudgetOverrun { cell: 0, stage: "kc", .. })), "{v:?}");
}

// spec: TEST-140
#[test]
fn a_reach_carries_a_list_of_exceptions() {
    // reading 9: several reasons on one reach, written in a fixed order, one per reason
    let mut g = base();
    let t = reach_mut(&mut g, cell(2, 3));
    t.exceptions =
        vec![Exception { reason: Reason::BudgetResidual, value: 5.0 }, Exception { reason: Reason::RiseOverBudget, value: 12.5 }];
    assert_eq!(check(&g), vec![]);
    let text = g.canonical();
    assert!(text.contains("exceptions=budget_residual:"), "sorted by the reason's name:\n{text}");
    assert!(text.contains(";rise_over_budget:"), "both listed:\n{text}");
    let back = Graph::parse(&text).expect("the canonical form parses");
    assert_eq!(back.canonical(), text, "same graph, same bytes");
    let mut reordered = g.clone();
    reach_mut(&mut reordered, cell(2, 3)).exceptions.reverse();
    assert_eq!(reordered.canonical(), text, "the list's order is not part of the graph");
    assert!(back.reaches.iter().any(|r| r.has_exception(Reason::RiseOverBudget) && r.has_exception(Reason::BudgetResidual)));
    // a reason twice, a value that is not finite, a reason the contract does not name
    let mut dup = g.clone();
    let id = reach_mut(&mut dup, cell(2, 3)).id;
    reach_mut(&mut dup, cell(2, 3)).exceptions.push(Exception { reason: Reason::BudgetResidual, value: 6.0 });
    assert!(check(&dup).contains(&Violation::DuplicateException { at: id, reason: Reason::BudgetResidual }));
    let mut nan = g.clone();
    reach_mut(&mut nan, cell(2, 3)).exceptions[0].value = f64::NAN;
    assert!(check(&nan).contains(&Violation::NotFinite { at: id, field: "exceptions" }));
    assert!(Graph::parse(&text.replacen("exceptions=budget_residual:", "exceptions=too_steep:", 1)).is_err());
    assert!(Graph::parse(&text.replacen(";rise_over_budget:", ";rise_over_budget", 1)).is_err());
    // the parser takes the canonical order only, and full hex
    let at = text.find("exceptions=budget_residual:").unwrap();
    let (lo, hi) = (at + 11, at + text[at..].find(' ').unwrap());
    let parts: Vec<&str> = text[lo..hi].split(';').collect();
    let swapped = format!("{}{};{}{}", &text[..lo], parts[1], parts[0], &text[hi..]);
    assert!(Graph::parse(&swapped).is_err(), "out of order");
    let short = format!("{}budget_residual:0;{}{}", &text[..lo], parts[1], &text[hi..]);
    assert!(Graph::parse(&short).is_err(), "short hex");
    assert!(Graph::parse(&text.replacen("body=0000000000005eed", "body=0000000000005EED", 1)).is_err(), "upper-case hex");
    // the graph parsed back equals the sorted graph, also with a signed zero
    let mut sorted = reordered.clone();
    sorted.sort();
    assert_eq!(Graph::parse(&reordered.canonical()).unwrap(), sorted);
    let mut zero = g.clone();
    reach_mut(&mut zero, cell(2, 3)).exceptions[0].value = -0.0;
    let mut pos = g.clone();
    reach_mut(&mut pos, cell(2, 3)).exceptions[0].value = 0.0;
    assert_eq!(zero.canonical(), pos.canonical(), "a signed zero is written as +0.0");
}

// spec: TEST-140
#[test]
fn a_marker_id_holds_its_reach() {
    // reading 8: marker = hash(body, kind, level, anchor, on_reach)
    let g = base();
    let j = g.nodes.iter().find(|n| n.kind == NodeKind::TribJunction).unwrap();
    let on = j.marker.unwrap().on_reach;
    assert_eq!(j.id, marker_id(BODY, j.kind, Level::L, j.anchor, on));
    // reading 15: the salt of the spikes lane's G1c, so both lanes give one id
    assert_eq!(j.id, planet_generators::bake::hydro::id_hash(node_id(BODY, j.kind, Level::L, j.anchor), on, 0x8A2E_0008));
    assert_ne!(j.id, node_id(BODY, j.kind, Level::L, j.anchor), "a marker's id is not its plain node hash");
    assert_ne!(
        marker_id(BODY, j.kind, Level::L, j.anchor, on),
        marker_id(BODY, j.kind, Level::L, j.anchor, on ^ 1),
        "another reach, another id"
    );
    // the plain node hash on a marker is an id mismatch; so is the hash of another reach
    for wrong in [node_id(BODY, j.kind, Level::L, j.anchor), marker_id(BODY, j.kind, Level::L, j.anchor, on ^ 1)] {
        let mut b = g.clone();
        let jid = j.id;
        b.nodes.iter_mut().find(|n| n.id == jid).unwrap().id = wrong;
        b.reaches.iter_mut().filter(|r| r.down == jid).for_each(|r| r.down = wrong);
        assert!(found(&b, &|v| matches!(v, Violation::IdMismatch { id } if *id == wrong)));
    }
    // a non-marker's id is still its plain node hash
    assert!(g.nodes.iter().filter(|n| n.marker.is_none()).all(|n| n.id == node_id(BODY, n.kind, n.level, n.anchor)));
}

// spec: TEST-140
#[test]
fn an_open_slice_reports_its_missing_owners_not_violations() {
    let g = base();
    assert_eq!(missing_owners(&g), vec![]);
    // the slice without the outlet and C -> O: the junction's reach is owned elsewhere
    let o = id(NodeKind::OutletSea, 3, 1);
    let r3 = reach_mut(&mut g.clone(), cell(1, 1)).id;
    let mut s = g.clone();
    s.nodes.retain(|n| n.id != o);
    s.reaches.retain(|r| r.id != r3);
    assert_eq!(check_with(&s, false, true), vec![], "an open slice validates");
    let j = g.nodes.iter().find(|n| n.kind == NodeKind::TribJunction).unwrap().id;
    assert_eq!(missing_owners(&s), vec![(j, r3)]);
    assert!(!check(&s).is_empty(), "closed, the same slice is invalid");
}

// spec: TEST-140, GEN-189
#[test]
fn an_orphan_lake_inlet_is_found_and_a_referenced_one_is_not() {
    // lead #182a: a lake_inlet that no reach ends at and no basin spills to is a violation; the base basin's spill node pointed at the inlet, or a reach ending there,
    // makes it none; an open slice does not check it
    let orphan = node(NodeKind::LakeInlet, cell(3, 3));
    let mut g = base();
    g.nodes.push(orphan.clone());
    assert!(found(&g, &|v| matches!(v, Violation::OrphanLakeInlet { node } if *node == orphan.id)));
    assert!(!check_with(&g, false, true).iter().any(|v| matches!(v, Violation::OrphanLakeInlet { .. })), "an open slice does not check it");
    let mut s = g.clone();
    s.basins[0].spill_node = orphan.id;
    assert!(!found(&s, &|v| matches!(v, Violation::OrphanLakeInlet { .. })), "a basin spills to it");
    let mut r = g.clone();
    let src = node(NodeKind::Source, cell(3, 2));
    r.nodes.push(src.clone());
    r.reaches.push(reach(&src, &orphan, vec![cell(3, 2), cell(3, 3)], attrs(&[1.0e4, 1.2e4], &[12, 10], 1)));
    r.sort();
    assert!(!found(&r, &|v| matches!(v, Violation::OrphanLakeInlet { .. })), "a reach ends at it");
}
