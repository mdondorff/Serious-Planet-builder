//! SPIKE-133 (spike branch spike/g1d, never merged): KG-trib extractor-1 (G1d) on the G0 fixture (TEST-139). The trunks are the fixture's expected graph's (TEST-140: the L trunk image and its
//! beds); KS is off, the stage input is `S0` (labelled). The L basins are SPIKE-130's producer's on `S0`. `G1D_PRINT` prints the report.

#![allow(clippy::needless_range_loop)]

use crate::g0_fixture::fixture::*;
use crate::g0_graph::expected;
use planet_generators::bake::basins_rg::{produce, BasinParams, Produced};
use planet_generators::bake::kg_trib::{extract, Extracted, Input, Params, Trunk};
use planet_generators::bake::river_graph::*;
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
        .map(|r| Trunk { id: r.id, vertices: r.vertices.iter().map(|&c| c as usize).collect(), attrs: r.attrs.clone() })
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
    let n = c.fx.s0.len();
    let outlet: Vec<bool> = (0..n).map(|x| grid.is_active(x) && (c.sea[x] || exit[x])).collect();
    let trunks_r = trunk_reaches(c);
    let p = BasinParams::l(c.graph.body);
    let first = produce(grid, &c.fx.s0, &outlet, &trunks_r, &tile, &p);
    let kept = kept_of(&first, n);
    let inp = Input { grid, z: &c.fx.s0, sea: &c.sea, exit, trunks: &c.trunks, kept: &kept, seed_ext, tile: &tile };
    let ex = extract(&inp, &Params::plan(c.graph.body, RUNOFF));
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
            "L basins: kept {}, reported {}; unresolved spills {}",
            run.basins.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).count(),
            run.basins.basins.iter().filter(|b| b.represented == Represented::None).count(),
            run.basins.unresolved.len()
        ),
    ]
}

// spec: SPIKE-133
#[test]
fn the_whole_body_extraction_validates() {
    let c = case(4);
    let gl = Grid::cube(L_N, RADIUS_M);
    let n = c.fx.s0.len();
    let run = run_domain(&c, &gl, &vec![false; n], &vec![0.0; n]);
    let g = assemble(&c, &run);
    let lines = report(&c, &run, &g);
    if std::env::var_os("G1D_PRINT").is_some() {
        for l in &lines {
            println!("{l}");
        }
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
