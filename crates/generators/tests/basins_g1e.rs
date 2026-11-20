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
        sea_level_m: 0.0,
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

// SPIKE-130 (spike prototype; not a spec requirement)
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

// SPIKE-130 (spike prototype; not a spec requirement)
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

// SPIKE-130 (spike prototype; not a spec requirement)
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
        exceptions: Vec::new(),
    };
    (r, up, down)
}

// SPIKE-130 (spike prototype; not a spec requirement)
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

// SPIKE-130 (spike prototype; not a spec requirement)
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
    let inner: Vec<bool> = (0..6 * N * N).map(|c| region(tile_of(c)) || (c < N * N && near(c))).collect();
    // and a one-cell ring on every face (the halo crosses the cube edge by a cell), so a spill's first sea cell beyond face 0's edge is inside, as in the whole run (reading 6)
    let full = Grid::cube(N, R);
    let active: Vec<bool> = (0..6 * N * N).map(|c| inner[c] || (0..8).any(|d| full.neighbour(c, d).is_some_and(|q| inner[q]))).collect();
    let g = Grid::cube(N, R).with_active(active.clone());
    let edge: Vec<bool> =
        (0..6 * N * N).map(|c| active[c] && (0..8).any(|d| Grid::cube(N, R).neighbour(c, d).is_some_and(|q| !active[q]))).collect();
    let outlet: Vec<bool> = (0..6 * N * N).map(|c| active[c] && (z[c] <= 0.0 || edge[c])).collect();
    let p = produce(&g, z, &outlet, reaches, &tile_of, &params());
    p.basins.iter().filter(|b| region(tile_of(b.floor_cell as usize)) && p.cells[&b.id].iter().all(|&c| !edge[c])).cloned().collect()
}

// SPIKE-130 (spike prototype; not a spec requirement)
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

// SPIKE-130 (spike prototype; not a spec requirement)
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

/// The G0 fixture (TEST-139): H1 basins on `z_s` (GEN-145's kept rule: 2 m at any size), L basins on `S0` (the contract's 0.25 km2, 2 m), the L image of the H1 basin, the same bytes on
/// 1 and 4 threads, and the same L basins under both layouts (regions of whole tiles, a one-tile halo).
mod fixture_case {
    use super::*;
    use crate::g0_fixture::fixture as fx;

    // re-pinned for reading 6 (#161): spills reaching the sea now name an `outlet_sea` (were unresolved); nothing else changed
    const DIGEST_H1: u64 = 0x06d5_a7e5_5cbd_1a68;
    const DIGEST_L: u64 = 0xffc8_1b12_2021_31cc;

    fn l_tile(c: usize) -> Domain {
        let (f, ti, tj) = fx::tile_of_l(c);
        Domain::Tile { face: f as u8, ti: ti as u32, tj: tj as u32 }
    }

    fn basins(threads: usize) -> (Produced, Produced, Vec<(u64, u64)>, fx::Fixture) {
        basins_linked(threads, true)
    }

    fn basins_linked(threads: usize, link: bool) -> (Produced, Produced, Vec<(u64, u64)>, fx::Fixture) {
        let f = fx::build(threads);
        let gh = Grid::cube(fx::H1_N, fx::RADIUS_M);
        let gl = Grid::cube(fx::L_N, fx::RADIUS_M);
        // GEN-145: an artefact is under 4 cells and under 2 m; a basin of 2 m is kept at any size
        let h1p = BasinParams {
            level: Level::H1,
            body: BODY,
            min_area_m2: 0.0,
            min_depth_m: 2.0,
            report_depth_m: 0.5,
            report_cells: 4,
            sea_level_m: 0.0,
        };
        let h1 = produce(&gh, &f.z_s, &gh.outlets(&f.z_s, 0.0), &[], &|_| Domain::Landmass, &h1p);
        let mut l = produce(&gl, &f.s0, &gl.outlets(&f.s0, 0.0), &[], &l_tile, &BasinParams::l(BODY));
        let l_cells_of = |hc: usize| {
            let nn = fx::H1_N * fx::H1_N;
            let (face, i, j) = (hc / nn, hc % fx::H1_N, (hc % nn) / fx::H1_N);
            let mut out = Vec::new();
            for dj in 0..fx::L_PER_H1 {
                for di in 0..fx::L_PER_H1 {
                    out.push(face * fx::L_N * fx::L_N + (j * fx::L_PER_H1 + dj) * fx::L_N + i * fx::L_PER_H1 + di);
                }
            }
            out
        };
        let pairs = if link { link_images(&h1, &mut l, &f.s0, &l_cells_of) } else { Vec::new() };
        (h1, l, pairs, f)
    }

    /// A digest of a producer's output: its basins and nodes (canonical) and each basin's cell set (sorted).
    pub(crate) fn digest(p: &Produced) -> u64 {
        use std::hash::{Hash, Hasher};
        let g = Graph { body: BODY, nodes: p.nodes.clone(), reaches: Vec::new(), basins: p.basins.clone() };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        g.canonical().hash(&mut h);
        for b in &p.basins {
            let mut c: Vec<usize> = p.cells[&b.id].to_vec();
            c.sort_unstable();
            c.hash(&mut h);
        }
        h.finish()
    }

    // SPIKE-133 (spike prototype; not a spec requirement)
    #[test]
    fn the_producer_s_output_is_unchanged() {
        // order #159: the producer's memory rework gives the same basins, nodes and cell sets (digests recorded before the rework)
        let (h1, l, _, _) = basins(4);
        let (dh, dl) = (digest(&h1), digest(&l));
        if std::env::var_os("G1E_DIGEST").is_some() {
            println!("digest h1 {dh:#018x} l {dl:#018x}");
        }
        assert_eq!((dh, dl), (DIGEST_H1, DIGEST_L));
    }

    // SPIKE-130 (spike prototype; not a spec requirement)
    #[test]
    fn the_fixture_basin_has_its_l_image_and_is_the_same_on_any_thread_count() {
        let (h1, l, pairs, f) = basins(4);
        let bowl_h = fx::cell_at(fx::H1_N, fx::BOWL.0, fx::BOWL.1);
        let hb = h1.basins.iter().find(|b| h1.cells[&b.id].contains(&bowl_h)).expect("the bowl is an H1 basin");
        let (_, lid) = *pairs.iter().find(|(h, _)| *h == hb.id).expect("the H1 bowl has an L image");
        let lb = l.basins.iter().find(|b| b.id == lid).unwrap();
        assert_eq!((lb.parent, lb.owner, lb.represented), (Some(hb.id), Domain::Landmass, Represented::ProtectedSurface));
        // TEST-139's recorded L bowl: spill 431.845 m; its floor key is S0 at the bowl's centre cell (372.143 m), the basin's floor the lowest cell, lower
        assert!((lb.spill_m - 431.845).abs() < 0.01 && lb.floor_m <= 372.143, "floor {:.3} spill {:.3}", lb.floor_m, lb.spill_m);
        let kept = l.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).count();
        if std::env::var_os("G1E_PRINT").is_some() {
            println!(
                "H1 basins {}, L basins {} ({} kept), images {}, unresolved {}",
                h1.basins.len(),
                l.basins.len(),
                kept,
                pairs.len(),
                l.unresolved.len()
            );
        }
        let (h1b, lb1, _, f1) = basins(1);
        assert_eq!(f1.s0.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), f.s0.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
        let canon = |p: &Produced| Graph { body: BODY, nodes: p.nodes.clone(), reaches: vec![], basins: p.basins.clone() }.canonical();
        assert_eq!((canon(&h1b), canon(&lb1)), (canon(&h1), canon(&l)), "the same bytes on 1 and 4 threads");
    }

    // SPIKE-130 (spike prototype; not a spec requirement)
    #[test]
    fn the_fixture_l_basins_are_the_same_under_both_layouts() {
        // the whole run unlinked: an L image's parent and owner come from the landmass pass, not from a region
        let (_, whole, _, f) = basins_linked(4, false);
        let gl = Grid::cube(fx::L_N, fx::RADIUS_M);
        let n = 6 * fx::L_N * fx::L_N;
        let region_of = |lay: &fx::Layout, c: usize| lay.region_of_h1[fx::h1_of_l(c)] as usize;
        for lay in fx::layouts() {
            let mut got: Vec<Basin> = Vec::new();
            for r in 0..lay.regions {
                // the region's tiles plus a one-tile halo on the same face
                let tiles_in: std::collections::BTreeSet<(usize, usize, usize)> =
                    (0..n).filter(|&c| region_of(&lay, c) == r).map(fx::tile_of_l).collect();
                let near = |c: usize| {
                    let (face, ti, tj) = fx::tile_of_l(c);
                    tiles_in.iter().any(|&(f2, a, b)| f2 == face && a.abs_diff(ti) <= 1 && b.abs_diff(tj) <= 1)
                };
                let active: Vec<bool> = (0..n).map(near).collect();
                let g = Grid::cube(fx::L_N, fx::RADIUS_M).with_active(active.clone());
                let edge: Vec<bool> = (0..n).map(|c| active[c] && (0..8).any(|d| gl.neighbour(c, d).is_some_and(|q| !active[q]))).collect();
                let outlet: Vec<bool> = (0..n).map(|c| active[c] && (f.s0[c] <= 0.0 || edge[c])).collect();
                let p = produce(&g, &f.s0, &outlet, &[], &l_tile, &BasinParams::l(BODY));
                got.extend(
                    p.basins
                        .iter()
                        .filter(|b| region_of(&lay, b.floor_cell as usize) == r && p.cells[&b.id].iter().all(|&c| !edge[c]))
                        .cloned(),
                );
            }
            got.sort_by_key(|b| b.id);
            let want: Vec<&Basin> = whole.basins.iter().filter(|b| got.iter().any(|g| g.floor_cell == b.floor_cell)).collect();
            let missing = whole.basins.len() - want.len();
            assert_eq!(got.iter().collect::<Vec<_>>(), want, "{}: every basin a region resolves equals the whole run's", lay.name);
            if std::env::var_os("G1E_PRINT").is_some() {
                println!(
                    "{}: {} of {} L basins resolved inside a region, {missing} need a wider halo",
                    lay.name,
                    got.len(),
                    whole.basins.len()
                );
            }
        }
    }
}

// SPIKE-130 (spike prototype; not a spec requirement)
#[test]
fn a_basin_pass_without_the_tributaries_puts_inlets_on_their_node_cells() {
    // lead #160: a pass that does not see a tributary may put a lake_inlet on the cell where that tributary ends (the chain runner merged its first pass's nodes, 99 on R1h); the
    // validator finds it, and the pass that sees every reach puts none there
    let mut z = base(false);
    cone(&mut z, 12.0, 12.0, 5.0, 10.0);
    let r = line(&(0..8).map(|j| cell(12, 3 + j)).collect::<Vec<_>>(), NodeKind::OutletSea).0;
    let (_, first) = run(&z, std::slice::from_ref(&r));
    let x = first
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::LakeInlet && r.vertices.contains(&n.anchor))
        .expect("the reach enters the lake")
        .anchor as usize;
    // a tributary ending at that cell (a confluence there)
    let (i, j) = (x % N, x / N);
    let (t, t_up, t_down) = line(&(0..4).map(|k| cell(i - 3 + k, j)).collect::<Vec<_>>(), NodeKind::Confluence);
    let adj = |_: Level, a: u64, b: u64| {
        let g = Grid::cube(N, R);
        (0..8).any(|d| g.neighbour(a as usize, d) == Some(b as usize))
    };
    let at_x = |v: &[Violation]| v.iter().any(|v| matches!(v, Violation::TwoNodesOneAnchor { anchor, .. } if *anchor == x as u64));
    let merged = graph_of(&first, &[r.clone(), t.clone()], &[t_up.clone(), t_down.clone()]);
    assert!(
        at_x(&validate(&merged, &Checks { adjacent: &adj, tile_of: None, open: true })),
        "the first pass's inlet beside the confluence"
    );
    let (_, second) = run(&z, &[r.clone(), t.clone()]);
    assert!(
        !second.nodes.iter().any(|n| n.kind == NodeKind::LakeInlet && n.anchor == x as u64),
        "the pass that sees the tributary puts no inlet on its node cell"
    );
    let g = graph_of(&second, &[r, t], &[t_up, t_down]);
    assert!(!at_x(&validate(&g, &Checks { adjacent: &adj, tile_of: None, open: true })));
}

// SPIKE-130 (spike prototype; not a spec requirement)
#[test]
fn a_spill_reaching_the_sea_is_received_by_an_outlet_sea() {
    // reading 6 (owner, #161): a basin with no reach on its spill path spills to an `outlet_sea` on the first sea cell, so no spill is missing
    let mut z = base(false);
    cone(&mut z, 16.0, 12.0, 4.0, 10.0);
    let (_, p) = run(&z, &[]);
    let lake = basin_at(&p, cell(16, 12));
    let node = p.nodes.iter().find(|n| n.id == lake.spill_node).expect("the spill names a node the producer holds");
    assert_eq!(node.kind, NodeKind::OutletSea);
    assert!(z[node.anchor as usize] <= 0.0, "on a sea cell");
    assert!(p.unresolved.is_empty(), "no spill missing: {:?}", p.unresolved);
}

// SPIKE-130 (spike prototype; not a spec requirement)
#[test]
fn a_depression_is_protected_only_when_neither_fill_nor_breach_fits() {
    // reading 22 (owner, #161): a kept-size depression drains (reported) when its fill or a bounded least-cost breach fits the budget left; only the rest are protected
    use planet_generators::bake::basins_rg::{produce_drain, CellIds};
    let mut z = base(false);
    cone(&mut z, 12.0, 12.0, 5.0, 10.0);
    let g = Grid::cube(N, R);
    let outlet = g.outlets(&z, 0.0);
    let ids = CellIds { gid: &|c| c as u64, local: &|id| Some(id as usize) };
    let at = |p: &Produced| basin_at(p, cell(12, 12)).represented;
    let (_, plain) = run(&z, &[]);
    assert_eq!(at(&plain), Represented::ProtectedSurface, "without a budget the size rule protects it");
    let roomy = produce_drain(&g, &z, &outlet, &[], &tile_of, &params(), &ids, &vec![20.0; z.len()]);
    assert!(roomy.drained >= 1);
    assert!(
        roomy.basins.iter().filter(|b| roomy.cells[&b.id].contains(&cell(12, 12))).all(|b| b.represented != Represented::ProtectedSurface),
        "a 10 m depression with 20 m left drains"
    );
    let tight = produce_drain(&g, &z, &outlet, &[], &tile_of, &params(), &ids, &vec![1.0; z.len()]);
    assert_eq!(at(&tight), Represented::ProtectedSurface, "neither a fill nor a breach fits 1 m: protected");
    assert_eq!(tight.drained, 0);
}

// SPIKE-130 (spike prototype; not a spec requirement)
#[test]
fn a_hollow_an_over_budget_rise_crosses_is_kept_as_a_lake() {
    // crusader #172 (owner, 2026-11-20): a 10 m hollow drains under reading 22 with 20 m of budget left; forced by a rise across it (the cell the bed rises from, the bed it
    // rises to), it is kept whatever its size or budget; a forced cell on open ground forces nothing
    use planet_generators::bake::basins_rg::{produce_drain_avoiding_forced, CellIds};
    let mut z = base(false);
    cone(&mut z, 12.0, 12.0, 5.0, 10.0);
    let g = Grid::cube(N, R);
    let outlet = g.outlets(&z, 0.0);
    let ids = CellIds { gid: &|c| c as u64, local: &|id| Some(id as usize) };
    let markers = std::collections::BTreeSet::new();
    let roomy = vec![20.0; z.len()];
    let holds = |p: &Produced| kept(p).iter().any(|b| p.cells[&b.id].contains(&cell(12, 12)));
    let plain = produce_drain_avoiding_forced(&g, &z, &outlet, &[], &tile_of, &params(), &ids, &roomy, &markers, &[]);
    assert!(!holds(&plain) && plain.forced == 0, "the hollow drains under reading 22 on its own");
    let level = z[cell(12, 12)] + 5.0;
    let forced = produce_drain_avoiding_forced(&g, &z, &outlet, &[], &tile_of, &params(), &ids, &roomy, &markers, &[(cell(12, 12), level)]);
    assert!(holds(&forced) && forced.forced == 1, "forced by the rise, it is a kept lake");
    let b = basin_at(&forced, cell(12, 12));
    assert!(b.spill_m >= level - 0.5 && b.floor_m <= z[cell(12, 12)], "the lake's spill reaches the rise: {} against {level}", b.spill_m);
    let open = produce_drain_avoiding_forced(&g, &z, &outlet, &[], &tile_of, &params(), &ids, &roomy, &markers, &[(cell(3, 3), 40.0)]);
    assert_eq!((open.forced, kept(&open).len()), (0, kept(&plain).len()), "a cell in no depression forces nothing");
    assert_eq!(
        forced.basins,
        produce_drain_avoiding_forced(&g, &z, &outlet, &[], &tile_of, &params(), &ids, &roomy, &markers, &[(cell(12, 12), level)]).basins,
        "deterministic"
    );
}
