//! SPIKE-133 (spike branch spike/g1d, never merged): KG-trib extractor-1 (G1d) on the G0 fixture (TEST-139). The trunks are the fixture's expected graph's (TEST-140: the L trunk image and its
//! beds); KS is off, the stage input is `S0` (labelled). The L basins are SPIKE-130's producer's on `S0`. `G1D_PRINT` prints the report.

#![allow(clippy::needless_range_loop)]

use crate::g0_fixture::fixture::*;
use crate::g0_graph::expected;
use planet_generators::bake::basins_rg::{produce, BasinParams, Produced};
use planet_generators::bake::kg_trib::{extract, Extracted, Input, Params, Trunk};
use planet_generators::bake::river_graph::*;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use std::collections::{BTreeMap, BTreeSet};

/// The fixture's runoff (m/s), as the expected graph's.
const RUNOFF: f64 = 1.0e-8;

fn tile(c: usize) -> Domain {
    let (f, ti, tj) = tile_of_l(c);
    Domain::Tile { face: f as u8, ti: ti as u32, tj: tj as u32 }
}

fn adjacency() -> impl Fn(Level, u64, u64) -> bool {
    let (gh, gl) = (Grid::cube(H1_N, RADIUS_M), Grid::cube(L_N, RADIUS_M));
    move |level, a, b| {
        let g = if level == Level::H1 { &gh } else { &gl };
        (0..8).any(|d| g.neighbour(a as usize, d) == Some(b as usize))
    }
}

/// The fixture, its expected graph's trunks and the whole-body inputs.
pub(crate) struct Case {
    pub(crate) fx: Fixture,
    pub(crate) graph: Graph,
    pub(crate) trunks: Vec<Trunk>,
    pub(crate) sea: Vec<bool>,
}

pub(crate) fn case(threads: usize) -> Case {
    let fx = build(threads);
    let b = expected(&fx);
    let sea: Vec<bool> = fx.s0.iter().map(|&z| z <= SEA_LEVEL_M).collect();
    let trunks: Vec<Trunk> = b
        .graph
        .reaches
        .iter()
        .filter(|r| r.level == Level::L && r.parent.is_some())
        .map(|r| Trunk {
            id: r.id,
            vertices: r.vertices.iter().map(|&c| c as usize).collect(),
            attrs: r.attrs.clone(),
            down: b.graph.nodes.iter().find(|n| n.id == r.down).expect("the trunk's down node").clone(),
        })
        .collect();
    Case { fx, graph: b.graph, trunks, sea }
}

fn trunk_reaches(c: &Case) -> Vec<Reach> {
    c.graph.reaches.iter().filter(|r| r.level == Level::L && r.parent.is_some()).cloned().collect()
}

/// The kept-basin cells of a producer run.
fn kept_of(p: &Produced, n: usize) -> Vec<bool> {
    let mut kept = vec![false; n];
    for b in p.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface) {
        for &c in &p.cells[&b.id] {
            kept[c] = true;
        }
    }
    kept
}

/// One domain's run: basins on it (trunks only), the extractor, then the basins again with the tributaries as spill receivers and inlets.
pub(crate) struct Run {
    pub(crate) ex: Extracted,
    pub(crate) basins: Produced,
}

pub(crate) fn run_domain(c: &Case, grid: &Grid, exit: &[bool], seed_ext: &[f64]) -> Run {
    run_domain_with(c, grid, exit, seed_ext, true)
}

pub(crate) fn run_domain_with(c: &Case, grid: &Grid, exit: &[bool], seed_ext: &[f64], mfd_init: bool) -> Run {
    run_domain_weighted(c, grid, exit, seed_ext, mfd_init, None)
}

pub(crate) fn run_domain_weighted(
    c: &Case,
    grid: &Grid,
    exit: &[bool],
    seed_ext: &[f64],
    mfd_init: bool,
    runoff_weight: Option<&[f64]>,
) -> Run {
    let n = c.fx.s0.len();
    let outlet: Vec<bool> = (0..n).map(|x| grid.is_active(x) && (c.sea[x] || exit[x])).collect();
    let trunks_r = trunk_reaches(c);
    let p = BasinParams::l(c.graph.body);
    let first = produce(grid, &c.fx.s0, &outlet, &trunks_r, &tile, &p);
    let kept = kept_of(&first, n);
    let inp = Input {
        grid,
        z: &c.fx.s0,
        sea: &c.sea,
        exit,
        trunks: &c.trunks,
        kept: &kept,
        seed_ext,
        tile: &tile,
        gid: &|x| x as u64,
        keep_fields: true,
        runoff_weight,
    };
    let ex = extract(&inp, &Params { mfd_init, ..Params::plan(c.graph.body, RUNOFF) });
    let mut all = trunks_r;
    all.extend(ex.reaches.iter().cloned());
    let basins = produce(grid, &c.fx.s0, &outlet, &all, &tile, &p);
    Run { ex, basins }
}

/// The whole graph: the expected graph's H1 part, its L trunk with its nodes and exception markers, the extracted tributaries and the producer's L basins (and the nodes they name).
pub(crate) fn assemble(c: &Case, run: &Run) -> Graph {
    let mut g = Graph { body: c.graph.body, ..Graph::default() };
    let trunk_ids: BTreeSet<u64> = c.trunks.iter().map(|t| t.id).collect();
    let trunk_nodes: BTreeSet<u64> = c.graph.reaches.iter().filter(|r| trunk_ids.contains(&r.id)).flat_map(|r| [r.up, r.down]).collect();
    g.nodes = c
        .graph
        .nodes
        .iter()
        .filter(|n| n.level == Level::H1 || trunk_nodes.contains(&n.id) || n.kind == NodeKind::Exception)
        .cloned()
        .collect();
    g.reaches = c.graph.reaches.iter().filter(|r| r.level == Level::H1 || trunk_ids.contains(&r.id)).cloned().collect();
    g.basins = c.graph.basins.iter().filter(|b| b.level == Level::H1).cloned().collect();
    // the H1 basin's junction on the H1 trunk stays; the L tributaries, their nodes, the basins and their nodes
    let mut by_id: BTreeMap<u64, Node> = g.nodes.drain(..).map(|n| (n.id, n)).collect();
    for n in run.ex.nodes.iter().chain(run.basins.nodes.iter()) {
        by_id.entry(n.id).or_insert_with(|| n.clone());
    }
    g.nodes = by_id.into_values().collect();
    g.reaches.extend(run.ex.reaches.iter().cloned());
    g.basins.extend(run.basins.basins.iter().cloned());
    g.sort();
    g
}

fn report(c: &Case, run: &Run, g: &Graph) -> Vec<String> {
    let l_reaches: Vec<&Reach> = g.reaches.iter().filter(|r| r.level == Level::L && r.parent.is_none()).collect();
    let count = |k: NodeKind| g.nodes.iter().filter(|n| n.level == Level::L && n.kind == k).count();
    let len_m: f64 =
        l_reaches.iter().map(|r| r.vertices.len().saturating_sub(1) as f64).sum::<f64>() * RADIUS_M * core::f64::consts::FRAC_PI_2
            / L_N as f64;
    let land_km2: f64 = (0..c.fx.s0.len()).filter(|&x| !c.sea[x]).map(|x| Grid::cube(L_N, RADIUS_M).cell_area(x)).sum::<f64>() / 1e6;
    let max_order = l_reaches.iter().map(|r| r.attrs.strahler[0]).max().unwrap_or(0);
    let cell_m = RADIUS_M * core::f64::consts::FRAC_PI_2 / L_N as f64;
    let mut lens: Vec<f64> = l_reaches.iter().map(|r| (r.vertices.len() - 1) as f64 * cell_m).collect();
    lens.sort_by(f64::total_cmp);
    let q = |v: &[f64], f: f64| v.get(((v.len() as f64 * f) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0.0);
    let mut by_order = String::new();
    for o in 1..=max_order {
        let mut a: Vec<f64> =
            l_reaches.iter().filter(|r| r.attrs.strahler[0] == o).map(|r| r.attrs.a_geo[r.vertices.len() - 2] / 1e6).collect();
        a.sort_by(f64::total_cmp);
        by_order.push_str(&format!(" order {o}: {} reaches, end area p50 {:.2} km2;", a.len(), q(&a, 0.5)));
    }
    vec![
        format!(
            "L tributary reaches {}; channel cells {}; channel length {:.1} km; land {:.1} km2; density {:.2} km/km2",
            l_reaches.len(),
            run.ex.channel.iter().filter(|&&x| x).count(),
            len_m / 1e3,
            land_km2,
            len_m / 1e3 / land_km2
        ),
        format!(
            "nodes: source {}, confluence {}, tile_crossing {}, trib_junction {}, outlet_sea {}, lake_inlet {}, lake_spill {}",
            count(NodeKind::Source),
            count(NodeKind::Confluence),
            count(NodeKind::TileCrossing),
            count(NodeKind::TribJunction),
            count(NodeKind::OutletSea),
            count(NodeKind::LakeInlet),
            count(NodeKind::LakeSpill)
        ),
        format!(
            "max Strahler {max_order}; MFD-only channel cells {}; trunk area breaks {}; open ends {}",
            run.ex.mfd_only, run.ex.trunk_area_breaks, run.ex.open_ends
        ),
        format!(
            "reach length p50 {:.0} m, p90 {:.0} m, max {:.0} m;{by_order}",
            q(&lens, 0.5),
            q(&lens, 0.9),
            lens.last().copied().unwrap_or(0.0)
        ),
        format!(
            "L basins: kept {}, reported {}; unresolved spills {}",
            run.basins.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).count(),
            run.basins.basins.iter().filter(|b| b.represented == Represented::None).count(),
            run.basins.unresolved.len()
        ),
    ]
}

// SPIKE-133 (spike prototype; not a spec requirement)
#[test]
fn the_whole_body_extraction_validates() {
    let c = case(4);
    let gl = Grid::cube(L_N, RADIUS_M);
    let n = c.fx.s0.len();
    let run = run_domain(&c, &gl, &vec![false; n], &vec![0.0; n]);
    let g = assemble(&c, &run);
    let mut lines = report(&c, &run, &g);
    // the comparison: initiation on the D8 area
    let d8 = run_domain_with(&c, &gl, &vec![false; n], &vec![0.0; n], false);
    let gd = assemble(&c, &d8);
    lines.push("-- initiation on the D8 area (comparison):".into());
    lines.extend(report(&c, &d8, &gd));
    let vd = validate(&gd, &Checks { adjacent: &adjacency(), tile_of: Some(&|x: u64| tile(x as usize)), open: false });
    lines.push(format!("D8 initiation: {} violations", vd.len()));
    if let Some(dir) = std::env::var_os("G1D_DUMP") {
        dump(&std::path::Path::new(&dir).join("d8"), &c, &d8, &gd);
    }
    if std::env::var_os("G1D_PRINT").is_some() {
        for l in &lines {
            println!("{l}");
        }
    }
    if let Some(dir) = std::env::var_os("G1D_DUMP") {
        dump(std::path::Path::new(&dir), &c, &run, &g);
    }
    let adj = adjacency();
    let tiles = |x: u64| tile(x as usize);
    let v = validate(&g, &Checks { adjacent: &adj, tile_of: Some(&tiles), open: false });
    for x in &v {
        if let Violation::TwoNodesOneAnchor { anchor, .. } = x {
            for nd in g.nodes.iter().filter(|nd| nd.anchor == *anchor && nd.level == Level::L) {
                let on = nd.marker.map(|m| g.reaches.iter().find(|r| r.id == m.on_reach).map(|r| (r.parent.is_some(), r.up, m.at_vertex)));
                eprintln!("node {:?} {:x} marker {:?} on {:?}", nd.kind, nd.id, nd.marker, on);
            }
            for r in g.reaches.iter().filter(|r| r.vertices.contains(anchor)) {
                eprintln!(
                    "reach {:x} up {:x} down {:x} trunk {} at {}",
                    r.id,
                    r.up,
                    r.down,
                    r.parent.is_some(),
                    r.vertices.iter().position(|v| v == anchor).unwrap()
                );
            }
        }
    }
    assert!(v.is_empty(), "{} violations, first {:?}\n{}", v.len(), &v[..v.len().min(12)], lines.join("\n"));
}

// ---- regions, halos and the H1 seed

/// The production halo (plan section 3.2: 13 km), in H1 cells (814 m): 16.
const HALO_H1: usize = 16;

/// H1's routing on `z_s` (round rules) and its D8 area `A_d` (true cell areas).
pub(crate) struct H1Flow {
    pub(crate) receiver: Vec<u32>,
    pub(crate) a_d: Vec<f64>,
}

pub(crate) fn h1_flow(c: &Case) -> H1Flow {
    let gh = Grid::cube(H1_N, RADIUS_M);
    let outlet = gh.outlets(&c.fx.z_s, SEA_LEVEL_M);
    let r = route_rules(&gh, &c.fx.z_s, &outlet, 1e-3, 1, 0.0, None, RoundRules::ROUND);
    let mut a: Vec<f64> = (0..gh.len()).map(|x| if outlet[x] { 0.0 } else { gh.cell_area(x) }).collect();
    for &x in r.order.iter().rev() {
        let to = r.receiver[x as usize];
        if to != NONE {
            let v = a[x as usize];
            a[to as usize] += v;
        }
    }
    H1Flow { receiver: r.receiver, a_d: a }
}

/// A window: the given H1 cells and `halo` rings of H1 cells around them.
fn window(core: &[bool], halo: usize) -> Vec<bool> {
    let gh = Grid::cube(H1_N, RADIUS_M);
    let mut w = core.to_vec();
    for _ in 0..halo {
        let prev = w.clone();
        for x in 0..w.len() {
            if !prev[x] && (0..8).any(|d| gh.neighbour(x, d).is_some_and(|q| prev[q])) {
                w[x] = true;
            }
        }
    }
    w
}

/// One window's run: the window's border cells are exits, except the entry cells: for each H1 cell outside whose H1 receiver lies inside, the lowest border L cell of that receiver next to
/// it (ties by id), seeded with the outside cell's `A_d` when `seeded`.
pub(crate) fn window_run(c: &Case, h1: &H1Flow, win_h1: &[bool], seeded: bool) -> (Run, Vec<bool>) {
    window_run_with(c, h1, win_h1, seeded, true)
}

pub(crate) fn window_run_with(c: &Case, h1: &H1Flow, win_h1: &[bool], seeded: bool, mfd_init: bool) -> (Run, Vec<bool>) {
    let n = c.fx.s0.len();
    let full = Grid::cube(L_N, RADIUS_M);
    let active: Vec<bool> = (0..n).map(|x| win_h1[h1_of_l(x)]).collect();
    let border: Vec<bool> = (0..n).map(|x| active[x] && (0..8).any(|d| full.neighbour(x, d).is_some_and(|q| !active[q]))).collect();
    let on_trunk: BTreeSet<usize> = c.trunks.iter().flat_map(|t| t.vertices.iter().copied()).collect();
    let mut seed = vec![0.0; n];
    let mut entry = vec![false; n];
    for q in 0..win_h1.len() {
        let to = h1.receiver[q];
        if win_h1[q] || to == NONE || !win_h1[to as usize] {
            continue;
        }
        let best = (0..n)
            .filter(|&x| h1_of_l(x) == to as usize && border[x] && !c.sea[x] && !on_trunk.contains(&x))
            .filter(|&x| (0..8).any(|d| full.neighbour(x, d).is_some_and(|y| h1_of_l(y) == q)))
            .min_by(|&a, &b| c.fx.s0[a].total_cmp(&c.fx.s0[b]).then(a.cmp(&b)));
        if let Some(x) = best {
            entry[x] = true;
            if seeded {
                seed[x] += h1.a_d[q];
            }
        }
    }
    let exit: Vec<bool> = (0..n).map(|x| border[x] && !entry[x]).collect();
    let grid = Grid::cube(L_N, RADIUS_M).with_active(active.clone());
    (run_domain_with(c, &grid, &exit, &seed, mfd_init), active)
}

fn tile_region(layout: &Layout, d: Domain) -> Option<usize> {
    let per = TILE_L / L_PER_H1;
    match d {
        Domain::Landmass => None,
        Domain::Tile { face, ti, tj } => {
            Some(layout.region_of_h1[face as usize * H1_N * H1_N + (tj as usize * per) * H1_N + ti as usize * per] as usize)
        }
    }
}

// SPIKE-133 (spike prototype; not a spec requirement)
#[test]
fn the_topology_is_the_same_under_both_layouts() {
    let c = case(4);
    let n = c.fx.s0.len();
    let whole = run_domain(&c, &Grid::cube(L_N, RADIUS_M), &vec![false; n], &vec![0.0; n]);
    let h1 = h1_flow(&c);
    let ids = |rs: &[Reach]| -> BTreeSet<u64> { rs.iter().map(|r| r.id).collect() };
    let want = ids(&whole.ex.reaches);
    for layout in layouts() {
        let mut got: Vec<Reach> = Vec::new();
        let mut lines = Vec::new();
        for r in 0..layout.regions {
            let core: Vec<bool> = (0..6 * H1_N * H1_N).map(|x| layout.region_of_h1[x] as usize == r).collect();
            let (run, _) = window_run(&c, &h1, &window(&core, HALO_H1), true);
            got.extend(run.ex.reaches.iter().filter(|x| tile_region(&layout, x.owner) == Some(r)).cloned());
            lines.push(format!("{} region {r}: {} reaches owned, {} open ends", layout.name, got.len(), run.ex.open_ends));
        }
        let have = ids(&got);
        assert_eq!(have.len(), got.len(), "{}: no reach in two regions", layout.name);
        let attr_diff = got.iter().filter(|x| whole.ex.reaches.iter().any(|w| w.id == x.id && w != *x)).count();
        if std::env::var_os("G1D_PRINT").is_some() {
            println!(
                "{}\n{}: same ids {}; missing {}; extra {}; attributes differ on {}",
                lines.join("\n"),
                layout.name,
                have == want,
                want.difference(&have).count(),
                have.difference(&want).count(),
                attr_diff
            );
        }
        assert_eq!(have, want, "{}: the same reaches as the whole body", layout.name);
    }
}

/// The cells whose clipped-MFD area takes some area from outside `win` (L mask): the taint of every cell outside it spreads to its D8 receiver and to every lower neighbour of the filled surface in
/// its D8 catchment (the clip), highest cells first.
fn tainted(ex: &Extracted, win: &[bool], sea: &[bool]) -> Vec<bool> {
    let g = Grid::cube(L_N, RADIUS_M);
    let n = ex.filled.len();
    let mut t: Vec<bool> = (0..n).map(|x| !win[x] && !sea[x]).collect();
    let mut by_height: Vec<usize> = (0..n).collect();
    by_height.sort_by(|&a, &b| ex.filled[b].total_cmp(&ex.filled[a]).then(b.cmp(&a)));
    for x in by_height {
        if !t[x] {
            continue;
        }
        if ex.receiver[x] != NONE {
            t[ex.receiver[x] as usize] = true;
        }
        for d in 0..8 {
            if let Some(y) = g.neighbour(x, d) {
                if ex.filled[y] < ex.filled[x] && ex.root[y] == ex.root[x] {
                    t[y] = true;
                }
            }
        }
    }
    t
}

/// The reduced halo of the halo cases (labelled: the fixture's body is 78 km round, so the 13 km halo covers almost all of it): 2 H1 cells (1.6 km).
const HALO_CASES_H1: usize = 2;

// SPIKE-133 (spike prototype; not a spec requirement)
#[test]
fn catchments_at_the_halo_s_reach_and_sources_outside_a_tile() {
    let c = case(4);
    let n = c.fx.s0.len();
    let whole = run_domain(&c, &Grid::cube(L_N, RADIUS_M), &vec![false; n], &vec![0.0; n]);
    let h1 = h1_flow(&c);
    let l_mask = |w: &[bool]| -> Vec<bool> { (0..n).map(|x| w[h1_of_l(x)]).collect() };
    // classes by how far a reach's catchment reaches beyond its tile: deep inside the window, just below the halo's reach, just above it, far above
    let names = ["inside", "just below", "just above", "far above"];
    let mut stats = [[0usize; 5]; 4]; // per class: reaches, present unseeded, present seeded, seeded a_geo within 1 %, unseeded a_geo within 1 %
                                      // per class: the relative a_geo error at the reach's end, unseeded and seeded, where both runs hold the reach with the same vertices
    let mut errs: [(Vec<f64>, Vec<f64>); 4] = Default::default();
    let (mut conf_total, mut conf_found, mut conf_found_unseeded) = (0usize, 0usize, 0usize);
    let per = TILE_L / L_PER_H1;
    for face in [0usize, 2] {
        for tj in 0..TILES_PER_FACE {
            for ti in 0..TILES_PER_FACE {
                let t = Domain::Tile { face: face as u8, ti: ti as u32, tj: tj as u32 };
                let core: Vec<bool> = (0..6 * H1_N * H1_N)
                    .map(|x| {
                        let (f, i, j) = (x / (H1_N * H1_N), x % H1_N, (x % (H1_N * H1_N)) / H1_N);
                        f == face && i / per == ti && j / per == tj
                    })
                    .collect();
                let mine: Vec<&Reach> = whole.ex.reaches.iter().filter(|r| r.owner == t).collect();
                if mine.is_empty() {
                    continue;
                }
                let rings: Vec<Vec<bool>> =
                    (0..=HALO_CASES_H1 + 1).map(|k| tainted(&whole.ex, &l_mask(&window(&core, k)), &c.sea)).collect();
                let win = window(&core, HALO_CASES_H1);
                let (plain, _) = window_run(&c, &h1, &win, false);
                let (seeded, _) = window_run(&c, &h1, &win, true);
                let find = |run: &Run, id: u64| run.ex.reaches.iter().find(|r| r.id == id).cloned();
                for r in &mine {
                    // the catchment of the end (a confluence exists only with its other inflows)
                    let at = *r.vertices.last().unwrap() as usize;
                    let class = if !rings[HALO_CASES_H1 - 1][at] {
                        0
                    } else if !rings[HALO_CASES_H1][at] {
                        1
                    } else if !rings[HALO_CASES_H1 + 1][at] {
                        2
                    } else {
                        3
                    };
                    let k = r.vertices.len() - 2;
                    let close = |x: &Reach| (x.attrs.a_geo[k] - r.attrs.a_geo[k]).abs() <= 0.01 * r.attrs.a_geo[k];
                    let (p, s) = (find(&plain, r.id), find(&seeded, r.id));
                    if class == 0 && p.is_none() && std::env::var_os("G1D_DIAG").is_some() {
                        let kind = |run: &Run, id: u64| run.ex.nodes.iter().find(|x| x.id == id).map(|x| (x.kind, x.anchor));
                        let v0 = r.vertices[0] as usize;
                        let other = plain.ex.reaches.iter().find(|x| x.vertices[0] == r.vertices[0]);
                        eprintln!(
                            "MISS tile {t:?} up {:?} down {:?} len {}; window: channel {} init {:.0} vs {:.0} rcv {} vs {}; window reach from it: {:?}",
                            kind(&whole, r.up),
                            kind(&whole, r.down),
                            r.vertices.len(),
                            plain.ex.channel[v0],
                            plain.ex.init[v0],
                            whole.ex.init[v0],
                            plain.ex.receiver[v0],
                            whole.ex.receiver[v0],
                            other.map(|x| (kind(&plain, x.down), x.vertices.len()))
                        );
                    }
                    stats[class][0] += 1;
                    stats[class][1] += usize::from(p.is_some());
                    stats[class][2] += usize::from(s.is_some());
                    stats[class][3] += usize::from(s.as_ref().is_some_and(|x| x.vertices == r.vertices && close(x)));
                    stats[class][4] += usize::from(p.as_ref().is_some_and(|x| x.vertices == r.vertices && close(x)));
                    if let (Some(x), Some(y)) = (&p, &s) {
                        if x.vertices == r.vertices && y.vertices == r.vertices {
                            errs[class].0.push((x.attrs.a_geo[k] - r.attrs.a_geo[k]).abs() / r.attrs.a_geo[k]);
                            errs[class].1.push((y.attrs.a_geo[k] - r.attrs.a_geo[k]).abs() / r.attrs.a_geo[k]);
                        }
                    }
                }
                // confluences in this tile fed by a source outside it
                for nd in whole.ex.nodes.iter().filter(|nd| nd.kind == NodeKind::Confluence && nd.owner == t) {
                    let mut stack = vec![nd.id];
                    let mut outside = false;
                    while let Some(id) = stack.pop() {
                        for r in whole.ex.reaches.iter().filter(|r| r.down == id) {
                            let up = whole.ex.nodes.iter().find(|x| x.id == r.up).unwrap();
                            if up.kind == NodeKind::Source && up.owner != t {
                                outside = true;
                            }
                            stack.push(r.up);
                        }
                    }
                    if outside {
                        conf_total += 1;
                        conf_found += usize::from(seeded.ex.nodes.iter().any(|x| x.id == nd.id));
                        conf_found_unseeded += usize::from(plain.ex.nodes.iter().any(|x| x.id == nd.id));
                    }
                }
            }
        }
    }
    let mut lines = vec![format!("halo {HALO_CASES_H1} H1 cells (reduced, labelled); tiles of faces 0 and 2")];
    for (k, s) in stats.iter().enumerate() {
        lines.push(format!(
            "{:<10}: reaches {:>4}; present unseeded {:>4}, seeded {:>4}; same vertices and a_geo within 1 %: unseeded {:>4}, seeded {:>4}",
            names[k], s[0], s[1], s[2], s[4], s[3]
        ));
        let p50 = |v: &[f64]| {
            let mut v = v.to_vec();
            v.sort_by(f64::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        let max = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
        lines.push(format!(
            "            a_geo error at the end ({} reaches): p50 {:.3} / max {:.3} unseeded, p50 {:.3} / max {:.3} seeded",
            errs[k].0.len(),
            p50(&errs[k].0),
            max(&errs[k].0),
            p50(&errs[k].1),
            max(&errs[k].1)
        ));
    }
    lines.push(format!(
        "confluences fed by a source outside their tile: {conf_total}; found unseeded {conf_found_unseeded}, seeded {conf_found}"
    ));
    if std::env::var_os("G1D_PRINT").is_some() {
        println!("{}", lines.join("\n"));
    }
    let report = lines.join("\n");
    assert!(stats[0][0] > 0 && stats[1][0] > 0 && stats[2][0] > 0, "every class near the halo has cases\n{report}");
    // the clip reads the neighbours' D8 roots (results note, finding 3): most catchments inside the window are found
    assert!(stats[0][1] * 5 >= stats[0][0] * 4, "a catchment inside the window is mostly found\n{report}");
    assert!(stats[2][2] + stats[3][2] >= stats[2][1] + stats[3][1], "the seed finds at least what the plain run finds\n{report}");
    assert!(conf_total > 0, "a confluence fed by a source outside its tile\n{report}");
}

// SPIKE-133 (spike prototype; not a spec requirement)
#[test]
fn the_network_attaches_conserves_orders_and_is_deterministic() {
    let c = case(4);
    let n = c.fx.s0.len();
    let whole = run_domain(&c, &Grid::cube(L_N, RADIUS_M), &vec![false; n], &vec![0.0; n]);
    let g = assemble(&c, &whole);
    // the same bytes on 1 and 4 threads
    let c1 = case(1);
    let w1 = run_domain(&c1, &Grid::cube(L_N, RADIUS_M), &vec![false; n], &vec![0.0; n]);
    assert_eq!(assemble(&c1, &w1).canonical(), g.canonical(), "the same bytes on 1 and 4 threads");
    let tribs: Vec<&Reach> = whole.ex.reaches.iter().collect();
    // no dangling or duplicate reaches: ids and vertex chains unique, every end a node of the graph
    let ids: BTreeSet<u64> = tribs.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), tribs.len(), "no duplicate reach id");
    let chains: BTreeSet<&Vec<u64>> = tribs.iter().map(|r| &r.vertices).collect();
    assert_eq!(chains.len(), tribs.len(), "no duplicate vertex chain");
    let nodes: BTreeMap<u64, &Node> = g.nodes.iter().map(|x| (x.id, x)).collect();
    assert!(tribs.iter().all(|r| nodes.contains_key(&r.up) && nodes.contains_key(&r.down)), "no dangling end");
    // conservation and Strahler at every confluence: one shared value at least the inflows one vertex upstream; the standard rule
    let mut confluences = 0;
    for nd in g.nodes.iter().filter(|x| x.level == Level::L && x.kind == NodeKind::Confluence) {
        let ins: Vec<&&Reach> = tribs.iter().filter(|r| r.down == nd.id).collect();
        let out: Vec<&&Reach> = tribs.iter().filter(|r| r.up == nd.id).collect();
        assert!(ins.len() >= 2 && out.len() == 1, "a confluence has two or more inflows and one outflow");
        let sum: f64 = ins.iter().map(|r| r.attrs.a_geo[r.vertices.len() - 2]).sum();
        assert!(out[0].attrs.a_geo[0] >= sum * (1.0 - 1e-9), "conservation at {:x}", nd.id);
        assert!(ins.iter().all(|r| r.attrs.a_geo[r.vertices.len() - 1] == out[0].attrs.a_geo[0]), "one shared value at {:x}", nd.id);
        let top = ins.iter().map(|r| r.attrs.strahler[0]).max().unwrap();
        let want = if ins.iter().filter(|r| r.attrs.strahler[0] == top).count() >= 2 { top + 1 } else { top };
        assert_eq!(out[0].attrs.strahler[0], want, "Strahler at {:x}", nd.id);
        confluences += 1;
    }
    assert!(confluences > 0);
    for r in &tribs {
        let up = nodes[&r.up];
        if matches!(up.kind, NodeKind::Source | NodeKind::LakeSpill) {
            assert_eq!(r.attrs.strahler[0], 1, "a head is order 1");
        }
        if up.kind == NodeKind::TileCrossing {
            let before = tribs.iter().find(|x| x.down == r.up).expect("a crossing has its inflow");
            assert_eq!(before.attrs.strahler[0], r.attrs.strahler[0], "the order holds across a tile crossing");
        }
    }
    // the fixture's features: the expected graph's tributary nodes the extractor reproduces (the explicit confluence, the junctions, W's outlet, the tile crossings)
    let exp_nodes: Vec<&Node> = c
        .graph
        .nodes
        .iter()
        .filter(|x| {
            x.level == Level::L
                && matches!(x.kind, NodeKind::Confluence | NodeKind::TribJunction | NodeKind::OutletSea | NodeKind::TileCrossing)
        })
        .collect();
    let found = exp_nodes.iter().filter(|x| nodes.contains_key(&x.id)).count();
    let exp_cells: BTreeSet<u64> =
        c.graph.reaches.iter().filter(|r| r.level == Level::L && r.parent.is_none()).flat_map(|r| r.vertices.iter().copied()).collect();
    let covered = exp_cells
        .iter()
        .filter(|&&x| whole.ex.channel[x as usize] || c.trunks.iter().any(|t| t.vertices.contains(&(x as usize))) || c.sea[x as usize])
        .count();
    let lines = [
        format!("expected tributary nodes reproduced: {found} of {}", exp_nodes.len()),
        format!("expected tributary cells on the extracted network: {covered} of {}", exp_cells.len()),
        format!("confluences checked: {confluences}"),
    ];
    if std::env::var_os("G1D_PRINT").is_some() {
        println!("{}", lines.join("\n"));
        for x in exp_nodes.iter().filter(|x| !nodes.contains_key(&x.id)) {
            println!("missing expected {:?} at {} ({:?})", x.kind, x.anchor, x.marker);
        }
    }
    let conf = exp_nodes.iter().find(|x| x.kind == NodeKind::Confluence).unwrap();
    assert!(nodes.contains_key(&conf.id), "the explicit confluence\n{}", lines.join("\n"));
}

/// Writes the map's CSVs: the reaches' vertices (reach, order, lon, lat), the nodes (kind, lon, lat), the trunk, the kept basins' cells and the coast.
fn dump(dir: &std::path::Path, c: &Case, run: &Run, g: &Graph) {
    use planet_generators::bake::h0::GlobalGrid;
    std::fs::create_dir_all(dir).unwrap();
    let l = GlobalGrid::new(L_N, RADIUS_M);
    let ll = |x: u64| lon_lat(l.direction(x as u32));
    let mut s = String::from("reach,order,trunk,lon,lat\n");
    for (k, r) in g.reaches.iter().filter(|r| r.level == Level::L).enumerate() {
        for &x in &r.vertices {
            let (lo, la) = ll(x);
            s.push_str(&format!("{k},{},{},{lo:.4},{la:.4}\n", r.attrs.strahler[0], u8::from(r.parent.is_some())));
        }
    }
    std::fs::write(dir.join("reaches.csv"), s).unwrap();
    let mut s = String::from("kind,lon,lat\n");
    for nd in g.nodes.iter().filter(|x| x.level == Level::L) {
        let (lo, la) = ll(nd.anchor);
        s.push_str(&format!("{:?},{lo:.4},{la:.4}\n", nd.kind));
    }
    std::fs::write(dir.join("nodes.csv"), s).unwrap();
    let kept = kept_of(&run.basins, c.fx.s0.len());
    let mut s = String::from("what,lon,lat\n");
    let gl = Grid::cube(L_N, RADIUS_M);
    for x in 0..c.fx.s0.len() {
        let coast = !c.sea[x] && (0..8).any(|d| gl.neighbour(x, d).is_some_and(|q| c.sea[q]));
        if kept[x] || coast {
            let (lo, la) = ll(x as u64);
            s.push_str(&format!("{},{lo:.4},{la:.4}\n", if kept[x] { "kept" } else { "coast" }));
        }
    }
    std::fs::write(dir.join("cells.csv"), s).unwrap();
}

// SPIKE-133 (spike prototype; not a spec requirement)
#[test]
fn d8_initiation_at_the_halo_s_reach() {
    // the comparison of the results note: the same halo cases with initiation on the D8 area (classes by the D8 catchment of the reach's end)
    let c = case(4);
    let n = c.fx.s0.len();
    let whole = run_domain_with(&c, &Grid::cube(L_N, RADIUS_M), &vec![false; n], &vec![0.0; n], false);
    let h1 = h1_flow(&c);
    let per = TILE_L / L_PER_H1;
    let mut stats = [[0usize; 2]; 2]; // inside the window / beyond it: reaches, present
    for face in [0usize, 2] {
        for tj in 0..TILES_PER_FACE {
            for ti in 0..TILES_PER_FACE {
                let t = Domain::Tile { face: face as u8, ti: ti as u32, tj: tj as u32 };
                let core: Vec<bool> = (0..6 * H1_N * H1_N)
                    .map(|x| {
                        let (f, i, j) = (x / (H1_N * H1_N), x % H1_N, (x % (H1_N * H1_N)) / H1_N);
                        f == face && i / per == ti && j / per == tj
                    })
                    .collect();
                let mine: Vec<&Reach> = whole.ex.reaches.iter().filter(|r| r.owner == t).collect();
                if mine.is_empty() {
                    continue;
                }
                let win = window(&core, HALO_CASES_H1);
                let wl: Vec<bool> = (0..n).map(|x| win[h1_of_l(x)]).collect();
                // the D8 catchment's taint: along the receivers only
                let mut taint: Vec<bool> = (0..n).map(|x| !wl[x] && !c.sea[x]).collect();
                for x in 0..n {
                    if !taint[x] || wl[x] {
                        continue;
                    }
                    let mut y = x;
                    while whole.ex.receiver[y] != NONE {
                        y = whole.ex.receiver[y] as usize;
                        if taint[y] && wl[y] {
                            break;
                        }
                        taint[y] = true;
                    }
                }
                let (plain, _) = window_run_with(&c, &h1, &win, false, false);
                for r in &mine {
                    let k = usize::from(taint[*r.vertices.last().unwrap() as usize]);
                    stats[k][0] += 1;
                    stats[k][1] += usize::from(plain.ex.reaches.iter().any(|x| x.id == r.id));
                }
            }
        }
    }
    let line = format!(
        "D8 initiation, halo {HALO_CASES_H1}: catchment inside the window {} reaches, {} found; beyond it {} reaches, {} found",
        stats[0][0], stats[0][1], stats[1][0], stats[1][1]
    );
    if std::env::var_os("G1D_PRINT").is_some() {
        println!("{line}");
    }
    assert!(stats[0][0] > 0, "{line}");
}

/// The tributaries ending at a trunk junction whose trunk carries less `a_geo` there than they bring (the vertex before the junction).
fn junction_breaks(g: &Graph) -> usize {
    let nodes: BTreeMap<u64, &Node> = g.nodes.iter().map(|x| (x.id, x)).collect();
    g.reaches
        .iter()
        .filter(|r| r.parent.is_none() && r.level == Level::L)
        .filter(|r| {
            let Some(m) = nodes.get(&r.down).and_then(|n| n.marker) else { return false };
            let Some(t) = g.reaches.iter().find(|x| x.id == m.on_reach) else { return false };
            t.attrs.a_geo[m.at_vertex as usize] < r.attrs.a_geo[r.vertices.len() - 2]
        })
        .count()
}

// SPIKE-133 (spike prototype; not a spec requirement)
#[test]
fn the_landmass_pass_accumulates_the_trunk_from_its_tributaries() {
    // reading 18: the L trunk's areas and discharge from the combined graph; H1's values only seed its head
    use planet_generators::bake::kg_trib::accumulate_trunks;
    let c = case(4);
    let n = c.fx.s0.len();
    let gl = Grid::cube(L_N, RADIUS_M);
    let run = run_domain(&c, &gl, &vec![false; n], &vec![0.0; n]);
    let mut g = assemble(&c, &run);
    let before = junction_breaks(&g);
    let trunks: BTreeSet<u64> = c.trunks.iter().map(|t| t.id).collect();
    let inflow = |x: u64| {
        let x = x as usize;
        (gl.cell_area(x) + run.ex.d8[x], gl.cell_area(x) + run.ex.d8_no_ext[x])
    };
    let acc = accumulate_trunks(&mut g, &trunks, &inflow, RUNOFF);
    assert_eq!(acc.unordered, 0);
    assert_eq!(junction_breaks(&g), 0, "no tributary brings more than its trunk carries (before the pass: {before})");
    let adj = adjacency();
    let tiles = |x: u64| tile(x as usize);
    assert_eq!(validate(&g, &Checks { adjacent: &adj, tile_of: Some(&tiles), open: false }), vec![], "the graph still validates");
    // the trunk at its mouth carries at least its tributaries' areas
    let t = g.reaches.iter().find(|r| trunks.contains(&r.id)).unwrap();
    let into: f64 = g
        .reaches
        .iter()
        .filter(|r| g.nodes.iter().any(|x| x.id == r.down && x.marker.is_some_and(|m| m.on_reach == t.id)))
        .map(|r| r.attrs.a_geo[r.vertices.len() - 2])
        .sum();
    assert!(*t.attrs.a_geo.last().unwrap() >= into, "{} against {into}", t.attrs.a_geo.last().unwrap());
    if std::env::var_os("G1D_PRINT").is_some() {
        println!(
            "landmass pass: {} trunk reaches; junction breaks {before} before, 0 after; heads over their seed {}; mouth a_geo {:.1} km2 (expected graph's {:.1} km2)",
            acc.reaches,
            acc.heads_over_seed,
            t.attrs.a_geo.last().unwrap() / 1e6,
            c.trunks.iter().find(|x| x.id == t.id).unwrap().attrs.a_geo.last().unwrap() / 1e6
        );
    }
}

// SPIKE-133 (spike prototype; lead #189)
#[test]
fn a_per_cell_runoff_accumulates_as_area_times_runoff_and_is_conserved_at_every_node() {
    // lead #189 (spikes #182: a per-vertex runoff ratio broke conservation at 40,866 nodes on R1h): a runoff weight from 0.25 to 2.0 across the fixture, accumulated as area x
    // weight down the D8 tree and in the landmass pass, so a_run and q sum exactly at every node; the graph validates and a_run differs from a_geo
    use planet_generators::bake::kg_trib::accumulate_trunks_weighted;
    let c = case(4);
    let n = c.fx.s0.len();
    let gl = Grid::cube(L_N, RADIUS_M);
    let weight: Vec<f64> = (0..n).map(|x| 0.25 + 1.75 * ((x * 7919) % 13) as f64 / 12.0).collect();
    let run = run_domain_weighted(&c, &gl, &vec![false; n], &vec![0.0; n], true, Some(&weight));
    assert_eq!(run.ex.run.len(), n, "the weighted accumulation is kept");
    let mut g = assemble(&c, &run);
    let trunks: BTreeSet<u64> = c.trunks.iter().map(|t| t.id).collect();
    let inflow = |x: u64| {
        let x = x as usize;
        let own = gl.cell_area(x);
        (own + run.ex.d8[x], own * weight[x] + run.ex.run[x], own * weight[x] + run.ex.run_no_ext[x])
    };
    let acc = accumulate_trunks_weighted(&mut g, &trunks, &inflow, RUNOFF);
    assert_eq!(acc.unordered, 0);
    let adj = adjacency();
    let tiles = |x: u64| tile(x as usize);
    let v = validate(&g, &Checks { adjacent: &adj, tile_of: Some(&tiles), open: false });
    assert!(!v.iter().any(|x| matches!(x, Violation::ConservationBreak { .. })), "no conservation break: {:?}", &v[..v.len().min(8)]);
    assert!(v.is_empty(), "the graph validates: {:?}", &v[..v.len().min(8)]);
    let l: Vec<&Reach> = g.reaches.iter().filter(|r| r.level == Level::L).collect();
    assert!(l.iter().any(|r| r.attrs.a_run.iter().zip(&r.attrs.a_geo).any(|(a, b)| (a - b).abs() > 1.0)), "the weight changes a_run");
    assert!(
        l.iter().all(|r| r.attrs.q.iter().zip(&r.attrs.a_run).all(|(q, a)| (q - a * RUNOFF).abs() <= 1e-12 * a.abs().max(1.0))),
        "q = a_run runoff"
    );
    // without a weight the extract keeps a_run = a_geo (the bits of before)
    let plain = run_domain(&c, &gl, &vec![false; n], &vec![0.0; n]);
    assert!(plain.ex.run.is_empty());
    assert!(plain.ex.reaches.iter().all(|r| r.attrs.a_run == r.attrs.a_geo));
}
