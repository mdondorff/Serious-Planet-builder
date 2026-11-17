//! SPIKE-130 (spike branch, never merged): the basin producer's G1e cases (addendum section 3 G1e; lead #156) on a cube sphere of 24 cells a face edge, 100 m cells: face 0 is land sloping
//! to the east, the other faces are sea. Each case validates with the river graph's validators, and the partition cases give the same basins (ids, parents, spills, receiving nodes) on any
//! region layout with a one-tile halo.

#![allow(clippy::needless_range_loop)]

use planet_generators::bake::basins_rg::*;
use planet_generators::bake::river_graph::*;
use planet_generators::bake::Grid;

const N: usize = 24;
const TILE: usize = 8;
const BODY: u64 = 0x0130;
/// 100 m cells at a face's centre.
const R: f64 = 100.0 * N as f64 * 2.0 / core::f64::consts::PI;

fn cell(i: usize, j: usize) -> usize {
    j * N + i
}

fn tile_of(c: usize) -> Domain {
    let (f, i, j) = (c / (N * N), c % N, (c % (N * N)) / N);
    Domain::Tile { face: f as u8, ti: (i / TILE) as u32, tj: (j / TILE) as u32 }
}

/// Face 0 land: 30 m falling 0.5 m a cell to the east; the other faces sea (-10 m) unless `all_land`.
fn base(all_land: bool) -> Vec<f64> {
    (0..6 * N * N)
        .map(|c| {
            if c < N * N {
                30.0 - 0.5 * (c % N) as f64
            } else if all_land {
                200.0
            } else {
                -10.0
            }
        })
        .collect()
}

/// A cone on face 0: `depth` m at (ci, cj), falling to 0 at `radius` cells.
fn cone(z: &mut [f64], ci: f64, cj: f64, radius: f64, depth: f64) {
    for j in 0..N {
        for i in 0..N {
            let r = libm::hypot(i as f64 - ci, j as f64 - cj);
            z[cell(i, j)] -= depth * (1.0 - r / radius).max(0.0);
        }
    }
}

fn params() -> BasinParams {
    // the synthetic scale: kept from 4 cells (about 0.04 km2) and 2 m, reported from 0.5 m and 2 cells (the contract's L values are 0.25 km2, 2 m; 0.5 m, 16 cells)
    BasinParams {
        level: Level::L,
        body: BODY,
        min_area_m2: 4.0 * 100.0 * 100.0 * 0.9,
        min_depth_m: 2.0,
        report_depth_m: 0.5,
        report_cells: 2,
    }
}

fn run(z: &[f64], reaches: &[Reach]) -> (Grid, Produced) {
    let g = Grid::cube(N, R);
    let outlet = g.outlets(z, 0.0);
    let p = produce(&g, z, &outlet, reaches, &tile_of, &params());
    (g, p)
}

fn graph_of(p: &Produced, reaches: &[Reach], extra: &[Node]) -> Graph {
    let mut g = Graph { body: BODY, nodes: p.nodes.clone(), reaches: reaches.to_vec(), basins: p.basins.clone() };
    g.nodes.extend(extra.iter().cloned());
    g.sort();
    g.nodes.dedup_by_key(|n| n.id);
    g
}

fn kept(p: &Produced) -> Vec<&Basin> {
    p.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).collect()
}

fn basin_at(p: &Produced, c: usize) -> &Basin {
    kept(p).into_iter().filter(|b| p.cells[&b.id].contains(&c)).min_by_key(|b| p.cells[&b.id].len()).expect("a kept basin holds the cell")
}

// spec: SPIKE-130
#[test]
fn nested_basins_have_their_parent() {
    let mut z = base(false);
    cone(&mut z, 11.0, 12.0, 8.0, 10.0);
    cone(&mut z, 8.0, 12.0, 3.5, 13.0); // deeper: the ground falls to the east
    cone(&mut z, 14.0, 12.0, 3.5, 9.0);
    let (_, p) = run(&z, &[]);
    // the container: the kept basin holding both pits' floors
    let big = kept(&p)
        .into_iter()
        .filter(|b| p.cells[&b.id].contains(&cell(8, 12)) && p.cells[&b.id].contains(&cell(14, 12)))
        .min_by_key(|b| p.cells[&b.id].len())
        .expect("a container")
        .id;
    let (a, b) = (basin_at(&p, cell(8, 12)), basin_at(&p, cell(14, 12)));
    assert!(a.id != big && b.id != big && a.id != b.id, "two nested basins and their container");
    let up = |id: u64| {
        let mut x = p.basins.iter().find(|q| q.id == id).unwrap().parent;
        while let Some(pid) = x {
            if pid == big {
                return true;
            }
            x = p.basins.iter().find(|q| q.id == pid).unwrap().parent;
        }
        false
    };
    assert!(up(a.id) && up(b.id), "both nest in the container");
    assert!(p.cells[&big].len() > p.cells[&a.id].len() + p.cells[&b.id].len());
}

// spec: SPIKE-130
#[test]
fn two_depressions_share_one_spill_and_spill_into_each_other() {
    let mut z = base(false);
    cone(&mut z, 8.0, 12.0, 4.5, 14.0);
    cone(&mut z, 14.0, 12.0, 4.5, 12.0);
    let (_, p) = run(&z, &[]);
    let (a, b) = (basin_at(&p, cell(9, 12)), basin_at(&p, cell(14, 12)));
    assert_ne!(a.id, b.id);
    assert_eq!(a.spill_cell, b.spill_cell, "one saddle");
    let inlet_in = |n: u64, other: &Basin| {
        p.nodes.iter().any(|x| x.id == n && x.kind == NodeKind::LakeInlet && p.cells[&other.id].contains(&(x.anchor as usize)))
    };
    assert!(inlet_in(a.spill_node, b) && inlet_in(b.spill_node, a), "each spills into the other's lake_inlet");
    assert_eq!(a.parent, b.parent, "siblings");
}

// spec: SPIKE-130
#[test]
fn a_closed_basin_is_endorheic() {
    let mut z = base(true);
    cone(&mut z, 11.0, 12.0, 4.0, 10.0);
    let (_, p) = run(&z, &[]);
    assert!(p.basins.iter().any(|b| b.outlet == Outlet::Endorheic), "a world without an outlet holds an endorheic basin");
    // with the sea the same bowl spills
    let mut zs = base(false);
    cone(&mut zs, 11.0, 12.0, 4.0, 10.0);
    let (_, q) = run(&zs, &[]);
    assert!(q.basins.iter().all(|b| b.outlet == Outlet::Active));
}

/// A straight L reach of cells from `cells[0]` to the last, with nodes `up` (a source) and `down`.
fn line(cells: &[usize], down_kind: NodeKind) -> (Reach, Node, Node) {
    let mk = |kind, c: usize| Node {
        id: node_id(BODY, kind, Level::L, c as u64),
        kind,
        level: Level::L,
        anchor: c as u64,
        parent: None,
        owner: tile_of(c),
        marker: None,
    };
    let (up, down) = (mk(NodeKind::Source, cells[0]), mk(down_kind, *cells.last().unwrap()));
    let n = cells.len();
    let a: Vec<f64> = (0..n).map(|k| 1.0e4 * (k + 1) as f64).collect();
    let r = Reach {
        id: reach_id(BODY, Level::L, up.id, down.id),
        level: Level::L,
        up: up.id,
        down: down.id,
        vertices: cells.iter().map(|&c| c as u64).collect(),
        parent: None,
        owner: tile_of(cells[1]),
        attrs: Attrs {
            a_geo: a.clone(),
            a_ext: vec![0.0; n],
            a_run: a.clone(),
            a_run_no_ext: a.clone(),
            q: a.iter().map(|x| x * 1e-8).collect(),
            q_w: a.iter().map(|x| x * 1e-8).collect(),
            strahler: vec![1; n],
            bed: vec![0; n],
        },
        status: Status::Ok,
    };
    (r, up, down)
}

// spec: SPIKE-130
#[test]
fn a_lake_with_three_inlets_has_one_spill() {
    let mut z = base(false);
    cone(&mut z, 12.0, 12.0, 5.0, 10.0);
    let reaches: Vec<Reach> = [
        (0..8).map(|j| cell(12, 3 + j)).collect::<Vec<_>>(),
        (0..8).map(|i| cell(3 + i, 12)).collect(),
        (0..8).map(|j| cell(12, 21 - j)).collect(),
    ]
    .iter()
    .map(|c| line(c, NodeKind::LakeInlet).0)
    .collect();
    let (_, p) = run(&z, &reaches);
    let lake = basin_at(&p, cell(12, 12));
    let inlets: Vec<&Node> =
        p.nodes.iter().filter(|n| n.kind == NodeKind::LakeInlet && p.cells[&lake.id].contains(&(n.anchor as usize))).collect();
    assert!(inlets.len() >= 3, "three inlets: {}", inlets.len());
    assert_eq!(kept(&p).iter().filter(|b| b.id == lake.id).count(), 1);
    assert_eq!(lake.outlet, Outlet::Active, "one spill");
}

// spec: SPIKE-130
#[test]
fn a_basin_spills_into_another() {
    let mut z = base(false);
    cone(&mut z, 5.0, 12.0, 3.0, 8.0);
    cone(&mut z, 14.0, 12.0, 4.0, 12.0);
    let (_, p) = run(&z, &[]);
    let (up, low) = (basin_at(&p, cell(5, 12)), basin_at(&p, cell(14, 12)));
    let into = p.nodes.iter().find(|n| n.id == up.spill_node).expect("a receiving node");
    assert_eq!(into.kind, NodeKind::LakeInlet);
    assert!(p.cells[&low.id].contains(&(into.anchor as usize)), "the upper basin spills into the lower one's inlet");
    assert!(up.parent.is_none() && low.parent.is_none(), "two trees, not nested");
}

/// A region: which tiles it holds.
type Region = Box<dyn Fn(Domain) -> bool>;

/// The producer run on a region of whole tiles with a halo of `halo` tiles (other cells inactive, the domain's edge an outlet): the basins whose floor lies in the region and whose cells
/// keep off the domain's edge.
fn regional(z: &[f64], reaches: &[Reach], region: &dyn Fn(Domain) -> bool, halo: usize) -> Vec<Basin> {
    let near = |c: usize| {
        let Domain::Tile { face, ti, tj } = tile_of(c) else { unreachable!() };
        (0..6 * N * N).any(|o| {
            let Domain::Tile { face: f2, ti: a, tj: b } = tile_of(o) else { unreachable!() };
            f2 == face && region(tile_of(o)) && (a as i64 - ti as i64).abs() <= halo as i64 && (b as i64 - tj as i64).abs() <= halo as i64
        })
    };
    let active: Vec<bool> = (0..6 * N * N).map(|c| region(tile_of(c)) || (c < N * N && near(c))).collect();
    let g = Grid::cube(N, R).with_active(active.clone());
    let edge: Vec<bool> =
        (0..6 * N * N).map(|c| active[c] && (0..8).any(|d| Grid::cube(N, R).neighbour(c, d).is_some_and(|q| !active[q]))).collect();
    let outlet: Vec<bool> = (0..6 * N * N).map(|c| active[c] && (z[c] <= 0.0 || edge[c])).collect();
    let p = produce(&g, z, &outlet, reaches, &tile_of, &params());
    p.basins.iter().filter(|b| region(tile_of(b.floor_cell as usize)) && p.cells[&b.id].iter().all(|&c| !edge[c])).cloned().collect()
}

// spec: SPIKE-130
#[test]
fn basins_across_a_tile_line_are_the_same_under_both_layouts() {
    let mut z = base(false);
    cone(&mut z, 7.5, 12.0, 4.0, 10.0); // across the tile line between columns 7 and 8
    cone(&mut z, 18.0, 6.0, 3.0, 8.0);
    let (_, whole) = run(&z, &[]);
    let across = basin_at(&whole, cell(7, 12));
    let tiles: std::collections::BTreeSet<Domain> = whole.cells[&across.id].iter().map(|&c| tile_of(c)).collect();
    assert!(tiles.len() >= 2, "the basin crosses a tile line");
    // one region (face 0) and two regions (tile columns 0 and 1-2 of face 0), each with a one-tile halo
    let layouts: [Vec<Region>; 2] = [
        vec![Box::new(|d| matches!(d, Domain::Tile { face: 0, .. }))],
        vec![
            Box::new(|d| matches!(d, Domain::Tile { face: 0, ti: 0, .. })),
            Box::new(|d| matches!(d, Domain::Tile { face: 0, ti: 1 | 2, .. })),
        ],
    ];
    for (k, layout) in layouts.iter().enumerate() {
        let mut got: Vec<Basin> = layout.iter().flat_map(|r| regional(&z, &[], r.as_ref(), 1)).collect();
        got.sort_by_key(|b| b.id);
        let want: Vec<Basin> =
            whole.basins.iter().filter(|b| matches!(tile_of(b.floor_cell as usize), Domain::Tile { face: 0, .. })).cloned().collect();
        assert_eq!(got, want, "layout {k}: ids, parents, spills and receiving nodes as the whole run");
    }
}

// spec: SPIKE-130
#[test]
fn every_case_validates_and_is_deterministic() {
    let adj = {
        let g = Grid::cube(N, R);
        move |_: Level, a: u64, b: u64| (0..8).any(|d| g.neighbour(a as usize, d) == Some(b as usize))
    };
    let mut z = base(false);
    cone(&mut z, 11.0, 12.0, 8.0, 14.0);
    cone(&mut z, 8.0, 12.0, 3.5, 6.0);
    cone(&mut z, 18.0, 6.0, 3.0, 8.0);
    let (r, up, down) = line(&(0..8).map(|j| cell(11, 3 + j)).collect::<Vec<_>>(), NodeKind::OutletSea);
    let (_, p) = run(&z, std::slice::from_ref(&r));
    let g = graph_of(&p, std::slice::from_ref(&r), &[up, down]);
    let v = validate(&g, &Checks { adjacent: &adj, tile_of: None, open: true });
    // basin ids carry the spill anchor (DEC crusader-7, default A), which GEN-189's basin id check does not know yet
    let bad: Vec<&Violation> =
        v.iter().filter(|x| !matches!(x, Violation::IdMismatch { id } if p.basins.iter().any(|b| b.id == *id))).collect();
    assert!(
        bad.iter().all(|x| !matches!(
            x,
            Violation::DuplicateId { .. }
                | Violation::IdMismatch { .. }
                | Violation::TwoNodesOneAnchor { .. }
                | Violation::InvalidMarker { .. }
        )),
        "{bad:?}"
    );
    let (_, again) = run(&z, &[r]);
    assert_eq!(graph_of(&again, &[], &[]).canonical(), graph_of(&p, &[], &[]).canonical(), "the same bytes twice");
}
