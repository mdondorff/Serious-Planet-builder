//! Tier A, TEST-137 and BUILD-060: the river gate's G2 columns on a hand graph in a 12 x 12 window at global L cell (100, 200). Sources S1 (101, 201) and S2 (101, 205) meet at the
//! confluence C (103, 203), which runs to the sea outlet O (106, 203); a tributary T from S3 (104, 207) ends at a `trib_junction` J on C -> O's vertex (105, 203).
use planet_cache::river_gate::{measure, GateInput, GateParams};
use planet_cache::river_gate_graph::{
    agreement, budget_field, budget_report, graph_report, partition_report, G2Input, G2Report, GateClass, StageField, Window,
};
use planet_generators::bake::river_graph::*;
use planet_generators::bake::Grid;

const BODY: u64 = 0x5EED;
const W: Window = Window { x0: 100, y0: 200, w: 12, h: 12 };
const DX: f64 = 100.0;

fn gid(x: u64, y: u64) -> u64 {
    (y << 32) | x
}

fn node(kind: NodeKind, x: u64, y: u64) -> Node {
    let anchor = gid(x, y);
    Node { id: node_id(BODY, kind, Level::L, anchor), kind, level: Level::L, anchor, parent: None, owner: Domain::Landmass, marker: None }
}

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

fn reach(up: &Node, down: &Node, cells: &[(u64, u64)], at: Attrs) -> Reach {
    Reach {
        id: reach_id(BODY, Level::L, up.id, down.id),
        level: Level::L,
        up: up.id,
        down: down.id,
        vertices: cells.iter().map(|&(x, y)| gid(x, y)).collect(),
        parent: None,
        owner: Domain::Landmass,
        attrs: at,
        exceptions: Vec::new(),
    }
}

fn graph() -> Graph {
    let (s1, s2, c, o, s3) = (
        node(NodeKind::Source, 101, 201),
        node(NodeKind::Source, 101, 205),
        node(NodeKind::Confluence, 103, 203),
        node(NodeKind::OutletSea, 106, 203),
        node(NodeKind::Source, 104, 207),
    );
    let r1 = reach(&s1, &c, &[(101, 201), (102, 202), (103, 203)], attrs(&[1.0e6, 2.0e6, 5.0e6], &[20, 19, 18], 1));
    let r2 = reach(&s2, &c, &[(101, 205), (102, 204), (103, 203)], attrs(&[1.0e6, 2.0e6, 5.0e6], &[22, 20, 18], 1));
    let r3 = reach(&c, &o, &[(103, 203), (104, 203), (105, 203), (106, 203)], attrs(&[5.0e6, 6.0e6, 8.0e6, 9.0e6], &[18, 17, 16, 0], 2));
    let mut j = node(NodeKind::TribJunction, 105, 203);
    j.marker = Some(Marker { on_reach: r3.id, at_vertex: 2 });
    j.id = marker_id(BODY, j.kind, Level::L, j.anchor, r3.id);
    let t = reach(
        &s3,
        &j,
        &[(104, 207), (104, 206), (105, 205), (105, 204), (105, 203)],
        attrs(&[1.0e6, 1.5e6, 2.0e6, 2.5e6, 2.6e6], &[30, 25, 20, 18, 16], 1),
    );
    let mut g = Graph { body: BODY, nodes: vec![s1, s2, c, o, s3, j], reaches: vec![r1, r2, r3, t], basins: Vec::new() };
    g.sort();
    g
}

// spec: TEST-137, BUILD-060
#[test]
fn the_graph_report_follows_chains_through_markers_and_counts_strahler_streams_on_the_rasterised_graph() {
    let g = graph();
    let all = vec![true; 144];
    let r = graph_report(&g, &W, &all, &all, DX);
    assert!(r.violations.is_empty(), "the hand graph is valid: {:?}", r.violations);
    assert_eq!((r.nodes, r.reaches_h1, r.reaches_l, r.basins, r.missing_owner), (6, 0, 4, 0, 0));
    // S3's chain ends at the marker J and goes on along C -> O to the sea.
    assert_eq!((r.sources, r.sources_to_outlet, r.sources_open), (3, 3, 0));
    assert_eq!((r.trunk_reaches, r.trunk_breaks, r.tributaries, r.tributaries_attached), (0, 0, 4, 4));
    assert_eq!((r.bed_rises, r.bed_rise_reaches, r.bed_residual_max_m), (0, 0, 0.0));
    // Streams: three of order 1 (S1, S2, S3) and one of order 2 starting at C; one order below the highest leaves no R_b.
    assert_eq!((r.streams.clone(), r.rb_orders, r.rb), (vec![3, 1], 1, 0.0));
    // Length from each owned cell to its next vertex: r1 and r2 2 diagonals each, r3 3 straight steps, T 3 straight and 1 diagonal; over 144 cells of 0.01 km2.
    let km = (5.0 * std::f64::consts::SQRT_2 + 6.0) * DX / 1000.0;
    assert!((r.channel_km - km).abs() < 1e-12 && (r.land_km2 - 1.44).abs() < 1e-12, "{r:?}");
    assert!((r.density_km_per_km2 - km / 1.44).abs() < 1e-12);
    // A rise on T counts as a bed rise outside exceptions, and its size is the residual.
    let mut bad = g.clone();
    let k = bad.reaches.iter().position(|x| x.vertices.len() == 5).expect("T");
    bad.reaches[k].attrs.bed = vec![30, 25, 26, 18, 16];
    let rb = graph_report(&bad, &W, &all, &all, DX);
    assert_eq!((rb.bed_rises, rb.bed_rise_reaches, rb.bed_residual_max_m), (1, 1, 0.5), "{rb:?}");
    // An exception is counted by its reason.
    bad.reaches[k].exceptions = vec![Exception { reason: Reason::RiseOverBudget, value: 0.5 }];
    assert_eq!(graph_report(&bad, &W, &all, &all, DX).exc_rise_over_budget, 1);
}

// spec: TEST-137, BUILD-060
#[test]
fn agreement_counts_both_shares_within_one_cell() {
    let g = graph();
    let all = vec![true; 144];
    let mut area = vec![0.0; 144];
    let cell = |x: usize, y: usize| (y - 200) * 12 + (x - 100);
    for (x, y) in [(101, 201), (102, 202), (103, 203), (104, 203), (105, 203), (106, 203), (110, 210)] {
        area[cell(x, y)] = 2.0e6;
    }
    let a = agreement(&g, &W, &area, &all, &all, 1.0e6);
    // seven routed cells, the far one (110, 210) unmatched; twelve graph cells, those of S2's first cell and three of T's away from a routed cell
    assert_eq!((a.routed, a.routed_agree, a.graph, a.graph_agree), (7, 6, 12, 8), "{a:?}");
    assert_eq!(agreement(&g, &W, &area, &all, &all, 5.0e6).routed, 0);
}

// spec: TEST-137, BUILD-060
#[test]
fn the_budget_field_and_report_follow_the_contract() {
    // relief_1km per 8-cell block: a step at x = 8 lies within 5 cells of the first two blocks' centres (x 4 and 12), so relief is 100, 100, 0 and the p50 100: a 50 m budget.
    let (w, h) = (24usize, 8usize);
    let s0: Vec<f64> = (0..w * h).map(|c| if c % w >= 8 { 100.0 } else { 0.0 }).collect();
    assert!(budget_field(&s0, w, h, 8, DX, 25_000.0).iter().all(|&b| b == 50.0));
    // a flat S0 has the 10 m floor
    let budget = budget_field(&[5.0; 16], 4, 4, 8, DX, 25_000.0);
    assert!(budget.iter().all(|&b| b == 10.0));
    let all = vec![true; 16];
    let mut ks = vec![0u16; 16];
    (ks[0], ks[1], ks[2]) = (50, 100, 120);
    let mut kv = vec![0u16; 16];
    kv[1] = 1;
    let mut fill = vec![0u16; 16];
    fill[3] = 150;
    let stages = [StageField { name: "ks".into(), lower: ks, fill: None }, StageField { name: "kv".into(), lower: kv, fill: Some(fill) }];
    let r = budget_report(&stages, &budget, &all, &all, Some(2), None);
    // cell 1 uses 10.1 m and cell 2 12 m of a 10 m budget: two overruns, both at 95 % or more; cell 3 fills 15 m
    assert_eq!((r.land, r.overrun, r.at_95, r.residual_records, r.residual_reaches), (16, 2, 2, Some(2), None), "{r:?}");
    assert_eq!(
        (r.stages[0].lowered, r.stages[0].lower_max_m, r.stages[1].lowered, r.stages[1].filled, r.stages[1].fill_over),
        (3, 12.0, 1, 1, 1)
    );
}

// spec: TEST-137, BUILD-060
#[test]
fn the_partition_diff_names_identical_graphs_and_the_first_change() {
    let g = graph();
    assert_eq!((partition_report(&g, &g).changes, partition_report(&g, &g).first.as_str()), (0, "identical"));
    let mut h = g.clone();
    let k = h.reaches.iter().position(|x| x.vertices.len() == 4).expect("C -> O");
    h.reaches[k].attrs.q_w[3] *= 2.0;
    let p = partition_report(&g, &h);
    assert_eq!((p.changes, p.topology), (1, 0));
    assert_eq!(p.first, format!("field:reach:{:016x}:q_w", h.reaches[k].id));
    let mut t = g.clone();
    t.reaches.retain(|x| x.vertices.len() != 5);
    assert!(partition_report(&g, &t).topology >= 1);
}

// spec: TEST-137, BUILD-060
#[test]
fn the_gate_prints_g2_columns_with_their_classes_only_when_given_g2_inputs() {
    let g = graph();
    let grid = Grid::new(12, 12, DX);
    // a plane falling to the east, sea from x = 11
    let z: Vec<f64> = (0..144).map(|c| if c % 12 >= 11 { -1.0 } else { 50.0 - 4.0 * (c % 12) as f64 }).collect();
    let all = vec![true; 144];
    let outlet = grid.outlets(&z, 0.0);
    let run = |g2: Option<&G2Input>| {
        measure(
            &GateInput {
                grid: &grid,
                start: &z,
                surface: &z,
                outlet: &outlet,
                inside: &all,
                region: &all,
                h1: &[],
                seed: 7,
                sea_level_m: 0.0,
                outside_drains: false,
                trunks: &[],
                g2,
            },
            &GateParams::default(),
        )
    };
    // B's row: no G2 input, no G2 column, no classes line
    let b = run(None);
    assert!(b.g2.is_none() && b.classes_line().is_none() && !b.fields_line().contains("g_nodes="));
    let input = G2Input { window: W, graph: Some(&g), stages: None, residual_records: None, partition: Some((&g, &g)) };
    let rg = run(Some(&input));
    let line = rg.fields_line();
    assert!(line.contains(" g_nodes=6 ") && line.contains(" part_first=identical") && line.contains(" agree1_km2=1 "), "{line}");
    let classes = rg.classes_line().expect("a classes line");
    assert!(
        classes.contains("g_nodes=correctness") && classes.contains("g_rb=geomorphology") && classes.contains("part_changes=partition"),
        "{classes}"
    );
    assert!(rg.lines().iter().any(|l| l.starts_with("RIVER-GATE graph [correctness]")));
    // every G2 column has one class
    let cols = rg.g2.as_ref().map(G2Report::columns).unwrap_or_default();
    assert!(cols.iter().all(|(_, _, c)| *c != GateClass::Incremental));
    assert_eq!(cols.iter().map(|c| &c.0).collect::<std::collections::BTreeSet<_>>().len(), cols.len(), "column names are unique");
}

// spec: TEST-137, BUILD-060
#[test]
fn the_stage_and_graph_readers_take_the_files_the_spec_names_and_refuse_a_wrong_size() {
    use planet_cache::river_gate_graph::{read_graph, read_stages};
    let dir = std::env::temp_dir().join(format!("river-gate-g2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let u16s = |v: &[u16]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    // no stage file at all is refused
    assert!(read_stages(&dir, 4).unwrap_err().contains("no ks-lower.u16"));
    // KS lowers only, KV lowers and fills, KC did not run; three residual records under a header
    std::fs::write(dir.join("ks-lower.u16"), u16s(&[0, 5, 300, 0])).unwrap();
    std::fs::write(dir.join("kv-lower.u16"), u16s(&[1, 0, 0, 0])).unwrap();
    std::fs::write(dir.join("kv-fill.u16"), u16s(&[0, 0, 0, 7])).unwrap();
    std::fs::write(dir.join("residuals.csv"), "cell,stage,needed_m,available_m,feature\n1,ks,1,1,a\n2,ks,1,1,b\n3,kv,1,1,c\n\n").unwrap();
    let (st, rec) = read_stages(&dir, 4).unwrap();
    assert_eq!(st.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["ks", "kv"]);
    assert_eq!((st[0].lower.clone(), st[0].fill.is_none(), st[1].fill.clone()), (vec![0, 5, 300, 0], true, Some(vec![0, 0, 0, 7])));
    assert_eq!(rec, Some(3));
    // a field of another size is refused, naming the file
    assert!(read_stages(&dir, 5).unwrap_err().contains("ks-lower.u16"));
    // a graph file round-trips its canonical form; another header is refused
    let g = graph();
    let p = dir.join("g.txt");
    std::fs::write(&p, g.canonical()).unwrap();
    assert_eq!(read_graph(p.to_str().unwrap()).unwrap(), g);
    std::fs::write(&p, "river-graph 1 body=0\n").unwrap();
    assert!(read_graph(p.to_str().unwrap()).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: TEST-137, BUILD-060
#[test]
fn the_surface_reader_takes_the_windows_f64_cells_and_refuses_a_wrong_size_or_a_non_finite_height() {
    use planet_cache::river_gate_graph::read_surface;
    let dir = std::env::temp_dir().join(format!("river-gate-surface-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let p = dir.join("s.f64");
    let path = p.to_str().unwrap();
    let z = [1.5f64, -2.0, 1.0e4, 0.0];
    std::fs::write(&p, z.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    assert_eq!(read_surface(path, 4).unwrap(), z.to_vec());
    assert!(read_surface(path, 5).unwrap_err().contains("32 bytes"));
    std::fs::write(&p, [1.0f64, f64::NAN, 2.0, 3.0].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    assert!(read_surface(path, 4).unwrap_err().contains("cell 1 is not finite"));
    let _ = std::fs::remove_dir_all(&dir);
}
