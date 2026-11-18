//! Tier A, TEST-137 and BUILD-060: the river gate's G2 columns on a hand graph in a 12 x 12 window at global L cell (100, 200). Sources S1 (101, 201) and S2 (101, 205) meet at the
//! confluence C (103, 203), which runs to the sea outlet O (106, 203); a tributary T from S3 (104, 207) ends at a `trib_junction` J on C -> O's vertex (105, 203).
use planet_cache::river_gate::{measure, GateInput, GateParams};
use planet_cache::river_gate_graph::{
    agreement, budget_field, budget_report, graph_report, partition_report, BudgetReport, G2Input, G2Report, GateClass, StageField, Window,
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
    let (none, recv) = (vec![false; 144], vec![u32::MAX; 144]);
    let a = agreement(&g, &W, &area, &all, &all, &none, &recv, 1.0e6);
    // seven routed cells, the far one (110, 210) unmatched; twelve graph cells, those of S2's first cell and three of T's away from a routed cell
    assert_eq!((a.routed, a.routed_agree, a.graph, a.graph_agree), (7, 6, 12, 8), "{a:?}");
    assert_eq!(agreement(&g, &W, &area, &all, &all, &none, &recv, 5.0e6).routed, 0);
}

// spec: TEST-137, BUILD-060
#[test]
fn agreement_counts_kept_interiors_and_cells_above_a_graph_head_apart() {
    // lead #166: routed cells in a kept basin's interior and routed cells above a graph head leave both shares and are counted apart
    let g = graph();
    let all = vec![true; 144];
    let cell = |x: usize, y: usize| (y - 200) * 12 + (x - 100);
    let mut area = vec![0.0; 144];
    let mut recv = vec![u32::MAX; 144];
    // above S3's head (104, 207): (104, 210) -> (104, 209) -> (104, 208), the last within one cell of the head
    // a side channel (108, 201) -> (107, 201) -> (106, 202), the last within one cell of C -> O away from every head
    for (a, b) in [((104, 210), (104, 209)), ((104, 209), (104, 208)), ((108, 201), (107, 201)), ((107, 201), (106, 202))] {
        recv[cell(a.0, a.1)] = cell(b.0, b.1) as u32;
    }
    for (x, y) in [(104, 210), (104, 209), (104, 208), (108, 201), (107, 201), (106, 202), (110, 210)] {
        area[cell(x, y)] = 2.0e6;
    }
    let mut kept = vec![false; 144];
    kept[cell(110, 210)] = true;
    kept[cell(104, 206)] = true;
    let a = agreement(&g, &W, &area, &all, &all, &kept, &recv, 1.0e6);
    // (104, 208) and (106, 202) agree; (104, 210) and (104, 209) lie above the head; (108, 201) and (107, 201) are misses; (110, 210) is kept
    assert_eq!((a.routed, a.routed_agree, a.routed_above_head, a.routed_kept), (4, 2, 2, 1), "{a:?}");
    // T's cell (104, 206) is in the kept interior: one graph cell leaves the graph share
    assert_eq!((a.graph, a.graph_kept), (11, 1), "{a:?}");
}

// spec: TEST-137, BUILD-060
#[test]
fn a_missing_spill_node_of_a_basin_in_the_margin_is_counted_apart() {
    let mut g = graph();
    let b = |id: u64, floor: (u64, u64)| Basin {
        id,
        level: Level::L,
        parent: None,
        floor_cell: gid(floor.0, floor.1),
        floor_m: 1.0,
        spill_cell: gid(floor.0 + 1, floor.1),
        spill_m: 2.0,
        spill_node: 0,
        outlet: Outlet::Active,
        represented: Represented::None,
        owner: Domain::Landmass,
    };
    g.basins = vec![b(1, (108, 208)), b(2, (110, 210))];
    // the counted cells: all but the last column and row hold both floors
    let inside: Vec<bool> = (0..144).map(|c| c % 12 < 11 && c / 12 < 11).collect();
    let r = graph_report(&g, &W, &inside, &[true; 144], DX);
    assert_eq!((r.basin_spill_missing, r.basin_spill_missing_edge), (2, 0), "{r:?}");
    // two columns and rows fewer: the second floor (110, 210) lies in the margin
    let inside: Vec<bool> = (0..144).map(|c| c % 12 < 10 && c / 12 < 10).collect();
    let r = graph_report(&g, &W, &inside, &[true; 144], DX);
    assert_eq!((r.basin_spill_missing, r.basin_spill_missing_edge), (1, 1), "{r:?}");
}

// spec: TEST-137, BUILD-060
#[test]
fn a_kept_basins_interior_is_the_depression_holding_its_floor_and_its_river_steps_are_counted_apart() {
    use planet_cache::river_gate_graph::{kept_interiors, kept_steps};
    let mut g = graph();
    let cell = |x: usize, y: usize| (y - 200) * 12 + (x - 100);
    // a bowl at (108, 208): floor 1 m, its ring 5 m, the rest 20 m; spill 6 m
    let mut z = vec![20.0; 144];
    for y in 207..=209 {
        for x in 107..=109 {
            z[cell(x, y)] = 5.0;
        }
    }
    z[cell(108, 208)] = 1.0;
    let basin = |floor: (u64, u64), spill_m: f64, represented: Represented| Basin {
        id: floor.0 * 1000 + floor.1,
        level: Level::L,
        parent: None,
        floor_cell: gid(floor.0, floor.1),
        floor_m: 1.0,
        spill_cell: gid(110, 208),
        spill_m,
        spill_node: g.nodes[0].id,
        outlet: Outlet::Active,
        represented,
        owner: Domain::Landmass,
    };
    // a kept basin, a drained one (not kept) and a kept one whose floor lies in no depression of the measured surface
    g.basins = vec![
        basin((108, 208), 6.0, Represented::ProtectedSurface),
        basin((102, 210), 30.0, Represented::None),
        basin((102, 209), 10.0, Represented::ProtectedSurface),
    ];
    // the gate's depressions: the bowl (label 0) and a pit at (102, 210) (label 1); (102, 209) is dry
    let mut label = vec![u32::MAX; 144];
    for y in 207..=209 {
        for x in 107..=109 {
            label[cell(x, y)] = 0;
        }
    }
    label[cell(102, 210)] = 1;
    let (kept, basins) = kept_interiors(&g, &W, &label);
    assert_eq!((basins, kept.iter().filter(|&&k| k).count()), (1, 9));
    assert!(kept[cell(107, 207)] && !kept[cell(106, 208)]);
    // river steps: into the floor, out over the rim (rising 4 m), and one outside on the plain rising 1 m
    let mut river = vec![false; 144];
    let mut recv = vec![u32::MAX; 144];
    for (a, b) in
        [((106, 208), (107, 208)), ((107, 208), (108, 208)), ((108, 208), (109, 208)), ((109, 208), (110, 208)), ((102, 203), (102, 204))]
    {
        river[cell(a.0, a.1)] = true;
        river[cell(b.0, b.1)] = true;
        recv[cell(a.0, a.1)] = cell(b.0, b.1) as u32;
    }
    z[cell(102, 204)] = 21.0;
    let all = vec![true; 144];
    let k = kept_steps(&kept, basins, &recv, &river, &all, &z);
    // four steps touch the interior; (108, 208) -> (109, 208) rises 4 m and (109, 208) -> (110, 208) 15 m: two uphill; the plain's step is not theirs
    assert_eq!((k.basins, k.cells, k.steps, k.uphill), (1, 9, 4, 2), "{k:?}");
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
    // without a kh field the use against S0 is the use against S_KH
    assert_eq!((r.overrun_s0, r.at_95_s0), (2, 2));
}

// spec: BUILD-060
#[test]
fn the_budget_use_is_against_s_kh_without_kh_and_against_s0_with_it() {
    // reading 21 (lead #168): ks, kv, kc count against S_KH; kh (S0 against KH) only in the use against S0
    let budget = vec![10.0; 4];
    let all = vec![true; 4];
    let kh = StageField { name: "kh".into(), lower: vec![50, 0, 95, 0], fill: Some(vec![0, 30, 0, 0]) };
    let ks = StageField { name: "ks".into(), lower: vec![60, 0, 0, 0], fill: None };
    let r = budget_report(&[kh, ks], &budget, &all, &all, None, None);
    // cell 0: 6 m vs S_KH, 11 m vs S0 (over); cell 2: 0 m vs S_KH, 9.5 m vs S0 (at 95 %)
    assert_eq!((r.overrun, r.at_95, r.overrun_s0, r.at_95_s0), (0, 0, 1, 2), "{r:?}");
    assert_eq!(r.stages[0].name, "kh");
    let cols = G2Report { budget: Some(r), ..G2Report::default() }.columns();
    let get = |n: &str| cols.iter().find(|c| c.0 == n).map(|c| c.1.clone());
    assert_eq!(get("b_overrun_s0"), Some("1".into()));
    assert!(get("b_reference").is_none(), "reading 28: the budget field is always S0's, so no column names it");
    assert_eq!(get("b_kh_lowered"), Some("2".into()));
}

// spec: BUILD-060
#[test]
fn the_g2_bars_pass_or_fail_against_the_owners_limits() {
    use planet_cache::river_gate_graph::{bars_line, g2_bars, Agreement, GraphReport, CORRIDOR_MIN_PCT, CORRIDOR_MIN_PCT_MOUNTAIN};
    // no graph, no bars (B's row)
    assert!(g2_bars(&G2Report::default(), 0.0, 100.0, CORRIDOR_MIN_PCT).is_empty());
    let gr = GraphReport { basin_spill_missing: 3, ..GraphReport::default() };
    let g2 = G2Report {
        graph: Some(gr),
        agreement: vec![
            Agreement { threshold_km2: 1.0, routed: 100, routed_agree: 90, ..Agreement::default() },
            Agreement { threshold_km2: 75.0, routed: 100, routed_agree: 89, ..Agreement::default() },
        ],
        ..G2Report::default()
    };
    let bars = g2_bars(&g2, 1.0, 97.4, CORRIDOR_MIN_PCT);
    let pass = |n: &str| bars.iter().find(|b| b.name == n).map(|b| b.pass()).unwrap();
    // at the limits: uphill 1.0 % and agreement 90 % pass; 89 % and a corridor of 97.4 % fail; no violations pass; 3 missing spill nodes fail
    assert!(pass("bar_uphill") && pass("bar_agree_1km2") && pass("bar_violations"));
    assert!(!pass("bar_agree_75km2") && !pass("bar_corridor") && !pass("bar_spill_missing"));
    assert!(g2_bars(&g2, 1.0, 97.4, CORRIDOR_MIN_PCT_MOUNTAIN).iter().find(|b| b.name == "bar_corridor").unwrap().pass(), "95 % on M1");
    assert_eq!(bars.iter().find(|b| b.name == "bar_spill_missing").unwrap().cell(), "FAIL:3.00");
    let line = bars_line(&bars).unwrap();
    assert!(line.contains("bar_uphill PASS 1.00 (<= 1)") && line.ends_with("3 of 6 pass"), "{line}");
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
    let input = G2Input { window: W, graph: Some(&g), stages: None, residual_records: None, partition: Some((&g, &g)), kc_patches: None };
    let rg = run(Some(&input));
    let line = rg.fields_line();
    assert!(line.contains(" g_nodes=6 ") && line.contains(" part_first=identical") && line.contains(" agree1_km2=1 "), "{line}");
    let classes = rg.classes_line().expect("a classes line");
    assert!(
        classes.contains("g_nodes=correctness") && classes.contains("g_rb=geomorphology") && classes.contains("part_changes=partition"),
        "{classes}"
    );
    assert!(rg.lines().iter().any(|l| l.starts_with("RIVER-GATE graph [correctness]")));
    // the graph has no kept basin: the kept columns are zero, and the uphill bar is the uphill share (lead #166)
    assert!(
        line.contains(" kept_steps=0 ") && line.contains(" kept_uphill_steps=0 ") && line.contains(" agree1_routed_above_head="),
        "{line}"
    );
    assert!(rg.lines().iter().any(|l| l.starts_with("RIVER-GATE kept basins [correctness]")));
    // a 6 m pit at (104, 209) holding a kept basin's floor: the gate's own basin flood finds it, and only it is the interior
    let mut zp = z.clone();
    zp[9 * 12 + 4] = 24.0;
    let mut gk = g.clone();
    gk.basins = vec![Basin {
        id: 9,
        level: Level::L,
        parent: None,
        floor_cell: gid(104, 209),
        floor_m: 24.0,
        spill_cell: gid(105, 209),
        spill_m: 30.0,
        spill_node: gk.nodes[0].id,
        outlet: Outlet::Active,
        represented: Represented::ProtectedSurface,
        owner: Domain::Landmass,
    }];
    let input_k = G2Input { window: W, graph: Some(&gk), stages: None, residual_records: None, partition: None, kc_patches: None };
    let outlet_p = grid.outlets(&zp, 0.0);
    let rk = measure(
        &GateInput {
            grid: &grid,
            start: &zp,
            surface: &zp,
            outlet: &outlet_p,
            inside: &all,
            region: &all,
            h1: &[],
            seed: 7,
            sea_level_m: 0.0,
            outside_drains: false,
            trunks: &[],
            g2: Some(&input_k),
        },
        &GateParams::default(),
    );
    let lk = rk.fields_line();
    assert!(lk.contains(" kept_basins=1 ") && lk.contains(" kept_cells=1 "), "{lk}");
    // a row with a graph prints the G2 bars (lead #168), each with its class; B's row prints none
    assert!(line.contains(" bar_violations=PASS:0.00") && line.contains(" bar_uphill="), "{line}");
    assert!(classes.contains("bar_corridor=preservation") && classes.contains("bar_spill_missing=correctness"), "{classes}");
    assert!(rg.lines().iter().any(|l| l.starts_with("RIVER-GATE bars [owner decision 1]")));
    assert!(b.bars().is_empty() && !b.lines().iter().any(|l| l.starts_with("RIVER-GATE bars")));
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
    assert!(read_stages(&dir, 4).unwrap_err().contains("no kh-lower.u16, ks-lower.u16, kv-lower.u16 or kc-lower.u16"));
    // KS lowers only, KV lowers and fills, KC did not run; three residual records under a header
    std::fs::write(dir.join("ks-lower.u16"), u16s(&[0, 5, 300, 0])).unwrap();
    std::fs::write(dir.join("kv-lower.u16"), u16s(&[1, 0, 0, 0])).unwrap();
    std::fs::write(dir.join("kv-fill.u16"), u16s(&[0, 0, 0, 7])).unwrap();
    std::fs::write(dir.join("residuals.csv"), "cell,stage,needed_m,available_m,feature\n1,ks,1,1,a\n2,ks,1,1,b\n3,kv,1,1,c\n\n").unwrap();
    let (st, rec) = read_stages(&dir, 4).unwrap();
    assert_eq!(st.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["ks", "kv"]);
    assert_eq!((st[0].lower.clone(), st[0].fill.is_none(), st[1].fill.clone()), (vec![0, 5, 300, 0], true, Some(vec![0, 0, 0, 7])));
    assert_eq!(rec, Some(3));
    // KH's field (reading 21) is read first, in stage order
    std::fs::write(dir.join("kh-lower.u16"), u16s(&[2, 0, 0, 0])).unwrap();
    let (st, _) = read_stages(&dir, 4).unwrap();
    assert_eq!(st.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["kh", "ks", "kv"]);
    std::fs::remove_file(dir.join("kh-lower.u16")).unwrap();
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

// spec: BUILD-060
#[test]
fn the_runs_own_trunks_are_its_graphs_h1_reaches_at_their_h1_cell_centres_cut_at_the_window() {
    use planet_cache::river_gate_graph::h1_trunks;
    // H1 cells of 8 L cells; the window starts at L (16, 8): H1 cell (3, 2) is at L cell centre (27.5, 19.5), i.e. (11.5, 11.5) in the window
    let win = Window { x0: 16, y0: 8, w: 32, h: 32 };
    let h1 = |x: u64, y: u64| (y << 32) | x;
    let node = |x: u64, y: u64| {
        let anchor = h1(x, y);
        Node {
            id: node_id(BODY, NodeKind::Source, Level::H1, anchor),
            kind: NodeKind::Source,
            level: Level::H1,
            anchor,
            parent: None,
            owner: Domain::Landmass,
            marker: None,
        }
    };
    let (a, b, c) = (node(3, 2), node(4, 3), node(9, 3));
    let reach = |up: &Node, down: &Node, cells: &[(u64, u64)], level: Level| Reach {
        id: reach_id(BODY, level, up.id, down.id),
        level,
        up: up.id,
        down: down.id,
        vertices: cells.iter().map(|&(x, y)| h1(x, y)).collect(),
        parent: None,
        owner: Domain::Landmass,
        attrs: attrs(&vec![1.0; cells.len()], &vec![0; cells.len()], 1),
        exceptions: Vec::new(),
    };
    let inside = reach(&a, &b, &[(3, 2), (4, 3)], Level::H1);
    // its third vertex lies past the window's right edge (H1 x 6 is L 51.5 - 16 = 35.5 >= 32): cut there
    let cut = reach(&b, &c, &[(4, 3), (5, 3), (6, 3), (9, 3)], Level::H1);
    let l_reach = reach(&a, &c, &[(20, 10), (21, 10)], Level::L);
    let g = Graph { body: BODY, nodes: vec![a, b, c], reaches: vec![inside, cut, l_reach], basins: Vec::new() };
    let t = h1_trunks(&g, &win, 8);
    assert_eq!(t.len(), 2, "the L reach is no trunk");
    assert_eq!(t[0], vec![(11.5, 11.5), (19.5, 19.5)]);
    assert_eq!(t[1], vec![(19.5, 19.5), (27.5, 19.5)], "cut at the first vertex outside the window");
}

// spec: BUILD-060
#[test]
fn each_stages_deep_lowering_kc_patches_and_the_spectrum_bars_are_reported() {
    use planet_cache::river_gate_graph::{read_kc_patches, spectrum_bars, BETA_MAX, BETA_MIN, OVER_CUT_M};
    // lead #172: a stage's cells lowered by more than 50 m (the field's 0.1 m steps: 500 is exactly 50 m and not counted)
    let budget = vec![50.0; 4];
    let all = vec![true; 4];
    let kv = StageField { name: "kv".into(), lower: vec![501, 500, 10, 0], fill: None };
    let r = budget_report(&[kv], &budget, &all, &all, None, None);
    assert_eq!((OVER_CUT_M, r.stages[0].over_cut), (50.0, 1));
    let cols = G2Report { budget: Some(BudgetReport { kc_patches: Some((3, 2.5e9)), ..r }), ..G2Report::default() }.columns();
    let get = |n: &str| cols.iter().find(|c| c.0 == n).map(|c| c.1.clone());
    assert_eq!(
        (get("b_kv_over50"), get("b_kc_patches"), get("b_kc_patch_km3")),
        (Some("1".into()), Some("3".into()), Some("2.5000".into()))
    );
    // the patch file: a header and one line per patch; a missing file is none; a bad line is refused naming the file
    let dir = std::env::temp_dir().join(format!("river-gate-kc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(read_kc_patches(&dir).unwrap(), None);
    std::fs::write(dir.join("kc-patches.csv"), "cells,volume_m3\n12,1000.5\n3,20\n\n").unwrap();
    assert_eq!(read_kc_patches(&dir).unwrap(), Some((2, 1020.5)));
    std::fs::write(dir.join("kc-patches.csv"), "cells,volume_m3\n12\n").unwrap();
    assert!(read_kc_patches(&dir).unwrap_err().contains("kc-patches.csv"));
    let _ = std::fs::remove_dir_all(&dir);
    // the spectrum's benchmark range: inside passes both bars; steeper than the range fails the first; rougher fails the second
    let pass = |beta: f64| spectrum_bars(beta).iter().map(|b| b.pass()).collect::<Vec<_>>();
    assert_eq!((pass(-3.5), pass(BETA_MIN - 0.01), pass(BETA_MAX + 0.01)), (vec![true, true], vec![false, true], vec![true, false]));
    assert_eq!((pass(BETA_MIN), pass(BETA_MAX)), (vec![true, true], vec![true, true]), "the range's ends pass");
}
