//! Spike only (crusader #206, lead 2026-11-25): band6's breach rule (`drain::drain_selective_floors`) resolved across region seams on TEST-139's fixture, against
//! the whole cube as one grid, on TEST-151's five layouts. band6 works in rounds:
//! - each round reads the surface the earlier rounds left: its fill level and depressions;
//! - every active depression (none of its cells kept) decides a breach, a fill or nothing against that round's surface;
//! - then all fills (by max), then all cuts (by min).
//!
//! The decisions of one round are independent of each other, and a round is a barrier. So the partitioned form is per round: a local phase in each region's
//! `CubePatch`es (a search that pops a border cell, or ties one, is a candidate), the join of the candidates in (spill level, floor `GlobalCell`) order over the whole
//! grid, then each region applies its own cells. The next round reads the applied surface, the halo refreshed at the barrier.
//! Kept as in band6: the per-cell lower bound `cell_lo` (a cell the cut would take under it is not entered; a found path that would is not taken), kept depressions,
//! the cut and fill caps at the floor, the descent of `DELTA` a cell, the search limit `4 * cells + 4096`, 8 rounds.
//! Scope: a constant 50 m budget, ties by (cost, `GlobalCell`) instead of the grid index, the hump limit and outlet floors off (band6's defaults), the fill level and
//! depressions of each round a shared input (GEN-121, GEN-191). Off by default: nothing in the chain reads it.
//! base1 (crusader #207) is the same rule without floors and kept depressions, at H1 (12 rounds, search 2^20): its test is at the end.

use super::cross_seam_kc::{disk_n, fill_level, key, Grid};
use super::g0_fixture::fixture::*;
use planet_core::{CubePatch, Face};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};

/// band6's rounds and search; base1's (`Drain::SELECTIVE_50`) are 12 and 2^20.
const ROUNDS: usize = 8;
const SEARCH: usize = 4096;
const BUDGET_M: f64 = 50.0;
const DELTA: f64 = 1e-4;
const TOL: f64 = 1e-3;

/// The stage's fixed inputs.
pub(super) struct Input {
    g: Grid,
    rounds: usize,
    search: usize,
    pub(super) z0: Vec<f64>,
    outlet: Vec<bool>,
    keep: Vec<bool>,
    cell_lo: Vec<f64>,
    area: Vec<f64>,
    /// The planted cases: name, pit cells, trench cells.
    plants: Vec<(&'static str, BTreeSet<usize>, Vec<usize>)>,
}

/// A round's depression: its cells, its floor (least height, then `GlobalCell`) and its level.
struct Dep {
    cells: Vec<usize>,
    floor: usize,
    lv: f64,
}

fn depressions(g: &Grid, z: &[f64], level: &[f64]) -> (Vec<u32>, Vec<Dep>) {
    let mut label = vec![u32::MAX; z.len()];
    let mut out = Vec::new();
    for p0 in 0..z.len() {
        if label[p0] != u32::MAX || level[p0] - z[p0] <= TOL {
            continue;
        }
        let k = out.len() as u32;
        let mut cells = vec![p0];
        label[p0] = k;
        let mut i = 0;
        while i < cells.len() {
            for &q in &g.nb[cells[i]] {
                if label[q] == u32::MAX && level[q] - z[q] > TOL {
                    label[q] = k;
                    cells.push(q);
                }
            }
            i += 1;
        }
        cells.sort_unstable();
        let floor = *cells.iter().min_by(|&&a, &&b| z[a].total_cmp(&z[b]).then(g.raw[a].cmp(&g.raw[b]))).unwrap();
        out.push(Dep { lv: level[floor], cells, floor });
    }
    (label, out)
}

/// One depression's decision in a round.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// The path's cells with their targets (`floor - DELTA * i`).
    Breach(Vec<(usize, u64)>),
    Fill,
    Deep,
}

#[derive(Clone, Debug, PartialEq)]
struct Record {
    round: usize,
    floor: usize,
    kind: Kind,
}

enum Found {
    Path(Vec<usize>),
    None,
    Candidate,
}

struct Domain {
    cells: HashSet<usize>,
    border: HashSet<usize>,
}

/// band6's least-cost breach search from the floor: cost `max(0, z - floor)` summed, ties by (cost, `GlobalCell`); a cell the cut would take under its `cell_lo` is
/// not entered; the path ends at the first cell outside the depression that is an outlet or whose level is below the floor; at most `4 * cells + SEARCH` settled.
#[allow(clippy::too_many_arguments)]
fn search(inp: &Input, z: &[f64], level: &[f64], label: &[u32], k: u32, d: &Dep, domain: Option<&Domain>) -> Found {
    let g = &inp.g;
    let zf = z[d.floor];
    let mut best: HashMap<usize, (f64, usize)> = HashMap::new();
    let mut done: HashSet<usize> = HashSet::new();
    let mut heap = BinaryHeap::new();
    best.insert(d.floor, (0.0, usize::MAX));
    heap.push(Reverse((key(0.0), g.raw[d.floor], d.floor)));
    let limit = 4 * d.cells.len() + inp.search;
    let (mut settled, mut border_min) = (0usize, f64::INFINITY);
    while let Some(Reverse((kk, _, c))) = heap.pop() {
        let (cost, _) = best[&c];
        if key(cost) != kk || !done.insert(c) {
            continue;
        }
        if domain.is_some_and(|dm| dm.border.contains(&c)) {
            return Found::Candidate;
        }
        if c != d.floor && label[c] != k && (inp.outlet[c] || level[c] < zf - TOL) {
            if border_min <= cost {
                return Found::Candidate;
            }
            let mut path = vec![c];
            let mut x = c;
            while best[&x].1 != usize::MAX {
                x = best[&x].1;
                path.push(x);
            }
            path.reverse();
            return Found::Path(path);
        }
        settled += 1;
        if settled > limit {
            return if border_min.is_finite() { Found::Candidate } else { Found::None };
        }
        for &q in &g.nb[c] {
            if domain.is_some_and(|dm| !dm.cells.contains(&q)) || (!inp.outlet[q] && z[q] > zf && zf < inp.cell_lo[q]) {
                continue;
            }
            let nc = cost + (z[q] - zf).max(0.0);
            if best.get(&q).is_none_or(|&(b, _)| nc < b) {
                best.insert(q, (nc, c));
                heap.push(Reverse((key(nc), g.raw[q], q)));
                if domain.is_some_and(|dm| dm.border.contains(&q)) {
                    border_min = border_min.min(nc);
                }
            }
        }
    }
    Found::None
}

/// band6's choice: a breach whose deepest cut fits the cap and keeps every cell's floor, when the fill does not fit or costs at least as much; else a fill that fits.
fn decide(inp: &Input, z: &[f64], d: &Dep, path: Option<Vec<usize>>) -> Kind {
    let zf = z[d.floor];
    let fill_ok = d.lv - zf <= BUDGET_M;
    let fill_v: f64 = d.cells.iter().map(|&c| (d.lv - z[c]) * inp.area[c]).sum();
    if let Some(path) = path {
        let t = |i: usize| zf - DELTA * i as f64;
        let v: f64 = path.iter().enumerate().map(|(i, &c)| (z[c] - z[c].min(t(i))).max(0.0) * inp.area[c]).sum();
        let cut = path.iter().enumerate().map(|(i, &c)| z[c] - z[c].min(t(i))).fold(0.0, f64::max);
        let floors_ok = path.iter().enumerate().all(|(i, &c)| inp.outlet[c] || z[c] <= t(i) || t(i) >= inp.cell_lo[c]);
        if floors_ok && cut <= BUDGET_M && (!fill_ok || v <= fill_v) {
            return Kind::Breach(path.iter().enumerate().map(|(i, &c)| (c, t(i).to_bits())).collect());
        }
    }
    if fill_ok {
        Kind::Fill
    } else {
        Kind::Deep
    }
}

/// What a run gives: the records of every round, the surface, and per round the candidates (by plant name) and whether the round changed the surface.
pub(super) struct Run {
    records: Vec<Record>,
    pub(super) z: Vec<f64>,
    candidates: Vec<Vec<&'static str>>,
}

/// The plant whose pit holds a cell, or "fixture".
fn plant_of(inp: &Input, c: usize) -> &'static str {
    inp.plants.iter().find(|(_, pit, _)| pit.contains(&c)).map_or("fixture", |p| p.0)
}

/// band6 over the cube. `layout` None: the reference (every search over the whole grid). `naive`: the region decides whatever its domain gives.
pub(super) fn run(inp: &Input, layout: Option<&PartitionLayout>, naive: bool) -> Run {
    let g = &inp.g;
    let region_of = |p: usize| {
        layout.map_or(0, |l| {
            let c = g.cell(p);
            l.region(c.face().0 as usize, c.i() as usize / l.tile, c.j() as usize / l.tile)
        })
    };
    let regions = layout.map_or(1, |l| l.regions);
    // the regions' domains, fixed for the stage
    let domains: Vec<Option<Domain>> = (0..regions as u8)
        .map(|r| {
            let l = layout?;
            let mut cells: HashSet<usize> = HashSet::new();
            for f in 0..6 {
                for tj in 0..TILES_PER_FACE {
                    for ti in 0..TILES_PER_FACE {
                        if l.region(f, ti, tj) == r {
                            let p = CubePatch::new(
                                Face(f as u8),
                                g.n as i64,
                                (ti * l.tile) as i64,
                                (tj * l.tile) as i64,
                                l.tile as i64,
                                l.halo as i64,
                            )
                            .unwrap();
                            cells.extend(p.cells().iter().map(|&c| g.index(c)));
                        }
                    }
                }
            }
            let border = if naive {
                HashSet::new()
            } else {
                cells.iter().copied().filter(|&p| g.nb[p].iter().any(|q| !cells.contains(q))).collect()
            };
            Some(Domain { cells, border })
        })
        .collect();
    let mut z = inp.z0.clone();
    let mut records = Vec::new();
    let mut candidates = Vec::new();
    for round in 0..inp.rounds {
        let level = fill_level(g, &z, &inp.outlet);
        let (label, deps) = depressions(g, &z, &level);
        let active: Vec<usize> = (0..deps.len()).filter(|&k| !deps[k].cells.iter().any(|&c| inp.keep[c])).collect();
        // local phase
        let mut decided: Vec<(usize, Kind)> = Vec::new();
        let mut cand: Vec<usize> = Vec::new();
        for &k in &active {
            let d = &deps[k];
            match search(inp, &z, &level, &label, k as u32, d, domains[region_of(d.floor) as usize].as_ref()) {
                Found::Candidate => cand.push(k),
                Found::Path(p) => decided.push((k, decide(inp, &z, d, Some(p)))),
                Found::None => decided.push((k, decide(inp, &z, d, None))),
            }
        }
        // join: (spill level, floor GlobalCell)
        cand.sort_by(|&a, &b| deps[a].lv.total_cmp(&deps[b].lv).then(g.raw[deps[a].floor].cmp(&g.raw[deps[b].floor])));
        candidates.push(cand.iter().map(|&k| plant_of(inp, deps[k].floor)).collect());
        for &k in &cand {
            let path = match search(inp, &z, &level, &label, k as u32, &deps[k], None) {
                Found::Path(p) => Some(p),
                _ => None,
            };
            decided.push((k, decide(inp, &z, &deps[k], path)));
        }
        decided.sort_by_key(|(k, _)| g.raw[deps[*k].floor]);
        let mut changed = false;
        // apply: each region its own cells, all fills then all cuts
        let mut next = z.clone();
        for r in 0..regions as u8 {
            for (k, kind) in &decided {
                if *kind == Kind::Fill {
                    for &c in deps[*k].cells.iter().filter(|&&c| region_of(c) == r) {
                        next[c] = next[c].max(deps[*k].lv);
                    }
                }
            }
            for (_, kind) in &decided {
                if let Kind::Breach(path) = kind {
                    for &(c, t) in path.iter().filter(|(c, _)| region_of(*c) == r) {
                        next[c] = next[c].min(f64::from_bits(t));
                    }
                }
            }
        }
        for (k, kind) in decided {
            changed |= kind != Kind::Deep;
            records.push(Record { round, floor: deps[k].floor, kind });
        }
        z = next;
        if !changed {
            break;
        }
    }
    Run { records, z, candidates }
}

/// A connected path of cells along a line in longitude and latitude.
fn line(g: &Grid, a: (f64, f64), b: (f64, f64)) -> Vec<usize> {
    let steps = 4000;
    let mut path: Vec<usize> = Vec::new();
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        let c = cell_at(g.n, a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        if path.last() != Some(&c) {
            if let Some(&prev) = path.last() {
                if !g.nb[prev].contains(&c) {
                    path.push(*g.nb[prev].iter().find(|q| g.nb[c].contains(q)).expect("a bridge cell"));
                }
            }
            path.push(c);
        }
    }
    path
}

/// The cells within `r` degrees of a line.
fn around(n: usize, a: (f64, f64), b: (f64, f64), r: f64) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for k in 0..=200 {
        let t = k as f64 / 200.0;
        out.extend(disk_n(n, a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1), r));
    }
    out
}

const PLATEAU_M: f64 = 2000.0;
const FLOOR_M: f64 = 20.0;
const HEAD_M: f64 = 45.0;

/// A pit with its trench to a sea cell (see #205), in a plateau.
struct Pit {
    name: &'static str,
    at: (f64, f64),
    radius: f64,
    floor: f64,
    trenches: &'static [(f64, f64)],
}

/// The cases:
/// - region-seam, cube-edge, in-halo: #205's long and short breaches;
/// - interacting: pit A (east of the two-region seam at -15) breaches in round 1; pit B west of the seam spills into A over a 40 m sill, so B is part of A's
///   depression in round 1 and a depression of its own in round 2, whose breach reads round 1's cuts across the seam;
/// - floor-blocked: the short trench lies under a `cell_lo` of 1000 m, so the breach takes the long one across the cube edge;
/// - kept: a protected pit (`keep`) with a trench, neither breached nor filled;
/// - fill-wins: a small pit, filled.
const PITS: [Pit; 8] = [
    Pit { name: "region-seam", at: (-17.5, -20.0), radius: 2.5, floor: FLOOR_M, trenches: &[(5.0, -20.0)] },
    Pit { name: "cube-edge", at: (38.0, -25.0), radius: 2.5, floor: FLOOR_M, trenches: &[(62.0, -25.0)] },
    Pit { name: "in-halo", at: (11.0, -12.0), radius: 1.5, floor: FLOOR_M, trenches: &[(20.0, -12.0)] },
    Pit { name: "interacting-A", at: (-13.0, -38.0), radius: 2.0, floor: FLOOR_M, trenches: &[(8.0, -38.0)] },
    Pit { name: "interacting-B", at: (-18.5, -38.0), radius: 1.2, floor: 30.0, trenches: &[] },
    Pit { name: "floor-blocked", at: (35.0, 28.0), radius: 2.5, floor: FLOOR_M, trenches: &[(50.0, 33.0), (62.0, 18.0)] },
    Pit { name: "kept", at: (-17.0, 25.0), radius: 1.5, floor: FLOOR_M, trenches: &[(5.0, 25.0)] },
    Pit { name: "fill-wins", at: (-17.0, 10.0), radius: 0.6, floor: FLOOR_M, trenches: &[(5.0, 10.0)] },
];
const SILL_M: f64 = 40.0;

fn input() -> Input {
    let g = Grid::new();
    let mut z = build(4).s0;
    let n = z.len();
    let mut keep = vec![false; n];
    let mut cell_lo = vec![f64::NEG_INFINITY; n];
    let a_b = (PITS[4].at, PITS[3].at);
    // plateaus first, so no plateau covers another plant's pit or trench
    for p in &PITS {
        for &e in p.trenches {
            z_set(&mut z, &around(L_N, p.at, e, p.radius.max(1.0) + 2.0), PLATEAU_M);
        }
        z_set(&mut z, &disk_n(L_N, p.at.0, p.at.1, p.radius + 2.0), PLATEAU_M);
    }
    z_set(&mut z, &around(L_N, a_b.0, a_b.1, 2.0), PLATEAU_M);
    let mut plants = Vec::new();
    for p in &PITS {
        let pit = disk_n(L_N, p.at.0, p.at.1, p.radius);
        let mut trench = Vec::new();
        for (t, &e) in p.trenches.iter().enumerate() {
            let path: Vec<usize> = line(&g, p.at, e).into_iter().filter(|c| !pit.contains(c)).collect();
            let m = path.len() - 1;
            for (k, &c) in path.iter().enumerate() {
                z[c] = if k == m { SEA_LEVEL_M - 1.0 } else { p.floor + HEAD_M * (1.0 - k as f64 / m as f64) };
                if p.name == "floor-blocked" && t == 0 && k < m {
                    cell_lo[c] = 1000.0;
                }
            }
            trench.extend(path);
        }
        z_set(&mut z, &pit, p.floor);
        if p.name == "kept" {
            for &c in &pit {
                keep[c] = true;
            }
        }
        plants.push((p.name, pit, trench));
    }
    // B's sill into A
    let (pa, pb) = (&plants[3].1, &plants[4].1);
    let sill: Vec<usize> = line(&g, a_b.0, a_b.1).into_iter().filter(|c| !pa.contains(c) && !pb.contains(c)).collect();
    z_set(&mut z, &sill.iter().copied().collect(), SILL_M);
    plants[4].2 = sill;
    let outlet: Vec<bool> = z.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let area: Vec<f64> = (0..n).map(|p| g.cell(p).area(L_N as i64, RADIUS_M)).collect();
    Input { g, rounds: ROUNDS, search: SEARCH, z0: z, outlet, keep, cell_lo, area, plants }
}

fn z_set(z: &mut [f64], cells: &BTreeSet<usize>, v: f64) {
    for &c in cells {
        z[c] = v;
    }
}

// spike only (crusader #206): G3-4's second partition row
#[test]
fn band6s_rounds_are_the_same_bits_on_every_layout() {
    let inp = input();
    for (name, pit, trench) in &inp.plants {
        eprintln!("#206 plant {name}: pit {} cells, trench or sill {} cells", pit.len(), trench.len());
    }
    let reference = run(&inp, None, false);
    let rounds = reference.records.iter().map(|r| r.round).max().unwrap() + 1;
    let by_plant = |name: &str| -> Vec<(usize, &'static str)> {
        reference
            .records
            .iter()
            .filter(|r| plant_of(&inp, r.floor) == name)
            .map(|r| {
                (
                    r.round,
                    match r.kind {
                        Kind::Breach(_) => "breach",
                        Kind::Fill => "fill",
                        Kind::Deep => "deep",
                    },
                )
            })
            .collect()
    };
    for (name, _, _) in &inp.plants {
        eprintln!("#206 reference {name}: {:?}", by_plant(name));
    }
    let fixture = reference.records.iter().filter(|r| plant_of(&inp, r.floor) == "fixture").count();
    eprintln!("#206 reference: {} records over {rounds} rounds ({fixture} of the fixture's own depressions)", reference.records.len());
    for name in ["region-seam", "cube-edge", "in-halo", "interacting-A", "floor-blocked"] {
        assert!(by_plant(name).contains(&(0, "breach")), "{name} is breached in round 1");
    }
    assert_eq!(
        by_plant("interacting-B"),
        [(1, "breach")],
        "B is a depression of its own in round 2 only, breached over the sill into A's cut"
    );
    assert!(by_plant("kept").is_empty(), "a kept depression is not decided");
    assert!(inp.plants[6].1.iter().all(|&c| reference.z[c] == inp.z0[c]), "a kept pit keeps its cells");
    assert!(by_plant("fill-wins").iter().any(|r| r.1 == "fill"), "the small pit is filled");
    // the floor: no cut of the blocked trench
    let blocked: Vec<usize> = inp.plants[5].2.iter().copied().filter(|&c| inp.cell_lo[c] > 0.0).collect();
    assert!(blocked.iter().all(|&c| reference.z[c] == inp.z0[c]), "the breach does not cut under a cell's floor");
    let mut naive_differs = Vec::new();
    for l in partition_layouts() {
        if run(&inp, Some(&l), true).records != reference.records {
            naive_differs.push(l.name.clone());
        }
        let r = run(&inp, Some(&l), false);
        let same_z = r.z.iter().zip(&reference.z).all(|(a, b)| a.to_bits() == b.to_bits());
        eprintln!(
            "#206 {}: {} regions, candidates by round {:?}, records {} (reference {}), band6 bit-identical {same_z}",
            l.name,
            l.regions,
            r.candidates,
            r.records.len(),
            reference.records.len()
        );
        assert_eq!(r.records, reference.records, "{}: the records of every round", l.name);
        assert!(same_z, "{}: band6's output", l.name);
        if l.regions > 1 && l.name != "shifted-seam" {
            assert!(r.candidates.iter().any(|c| !c.is_empty()), "{}: a planted breach crosses its seam", l.name);
        }
    }
    eprintln!("#206 falsification: deciding in the region without the candidate rule changes the records on {naive_differs:?}");
    assert!(!naive_differs.is_empty(), "without the candidate rule some layout must differ");
}

/// base1's cases at H1 (3.75 degrees a cell): a long breach across the two-region seam and one across a cube edge, each from a pit of about 20 cells.
const PITS_H1: [Pit; 2] = [
    Pit { name: "h1-region-seam", at: (-22.0, -20.0), radius: 6.0, floor: FLOOR_M, trenches: &[(25.0, -20.0)] },
    Pit { name: "h1-cube-edge", at: (30.0, 25.0), radius: 6.0, floor: FLOOR_M, trenches: &[(72.0, 25.0)] },
];

/// base1's input: the fixture's `z_b` at H1 with [`PITS_H1`]. base1's rule is band6's without floors and kept depressions (`drain_selective`), 12 rounds, search 2^20.
pub(super) fn input_h1() -> Input {
    let g = Grid::at(H1_N);
    let mut z = build(4).z_b;
    let n = z.len();
    for p in &PITS_H1 {
        for &e in p.trenches {
            z_set(&mut z, &around(H1_N, p.at, e, p.radius + 8.0), PLATEAU_M);
        }
    }
    let mut plants = Vec::new();
    for p in &PITS_H1 {
        let pit = disk_n(H1_N, p.at.0, p.at.1, p.radius);
        let mut trench = Vec::new();
        for &e in p.trenches {
            let path: Vec<usize> = line(&g, p.at, e).into_iter().filter(|c| !pit.contains(c)).collect();
            let m = path.len() - 1;
            for (k, &c) in path.iter().enumerate() {
                z[c] = if k == m { SEA_LEVEL_M - 1.0 } else { p.floor + HEAD_M * (1.0 - k as f64 / m as f64) };
            }
            trench.extend(path);
        }
        z_set(&mut z, &pit, p.floor);
        plants.push((p.name, pit, trench));
    }
    let outlet: Vec<bool> = z.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let area: Vec<f64> = (0..n).map(|p| g.cell(p).area(H1_N as i64, RADIUS_M)).collect();
    Input { g, rounds: 12, search: 1 << 20, z0: z, outlet, keep: vec![false; n], cell_lo: vec![f64::NEG_INFINITY; n], area, plants }
}

/// A TEST-151 layout on the H1 lattice: the same tiles, `L_PER_H1` times fewer cells.
fn at_h1(l: &PartitionLayout) -> PartitionLayout {
    PartitionLayout {
        name: l.name.clone(),
        regions: l.regions,
        tile: l.tile / L_PER_H1,
        halo: l.halo / L_PER_H1,
        region_of_tile: l.region_of_tile.clone(),
    }
}

/// The form in use: each region drains its window alone, with the window edge as an outlet (`Grid::outlets`) and the cells outside it as walls; it keeps its own cells.
fn windowed(inp: &Input, l: &PartitionLayout) -> Vec<f64> {
    let g = &inp.g;
    let region_of = |p: usize| {
        let c = g.cell(p);
        l.region(c.face().0 as usize, c.i() as usize / l.tile, c.j() as usize / l.tile)
    };
    let mut out = inp.z0.clone();
    for r in 0..l.regions as u8 {
        let mut dom: HashSet<usize> = HashSet::new();
        for f in 0..6 {
            for tj in 0..TILES_PER_FACE {
                for ti in 0..TILES_PER_FACE {
                    if l.region(f, ti, tj) == r {
                        let p = CubePatch::new(
                            Face(f as u8),
                            g.n as i64,
                            (ti * l.tile) as i64,
                            (tj * l.tile) as i64,
                            l.tile as i64,
                            l.halo as i64,
                        )
                        .unwrap();
                        dom.extend(p.cells().iter().map(|&c| g.index(c)));
                    }
                }
            }
        }
        let mut w = Input {
            g: Grid::at(g.n),
            rounds: inp.rounds,
            search: inp.search,
            z0: inp.z0.clone(),
            outlet: inp.outlet.clone(),
            keep: inp.keep.clone(),
            cell_lo: inp.cell_lo.clone(),
            area: inp.area.clone(),
            plants: Vec::new(),
        };
        for p in 0..w.z0.len() {
            if !dom.contains(&p) {
                w.z0[p] = 1.0e9;
            } else if g.nb[p].iter().any(|q| !dom.contains(q)) {
                w.outlet[p] = true;
            }
        }
        let z = run(&w, None, false).z;
        for p in (0..out.len()).filter(|&p| region_of(p) == r) {
            out[p] = z[p];
        }
    }
    out
}

// spike only (crusader #207): G3-4's third partition row, base1 at H1
#[test]
fn base1_on_the_whole_h1_cube_is_the_same_bits_on_every_layout() {
    let inp = input_h1();
    for (name, pit, trench) in &inp.plants {
        eprintln!("#207 plant {name}: pit {} cells, trench {} cells", pit.len(), trench.len());
    }
    // the global form: base1 over the whole H1 cube in one piece; a region reads its cells from it, so its bits do not depend on the layout
    let reference = run(&inp, None, false);
    let rounds = reference.records.iter().map(|r| r.round).max().map_or(0, |r| r + 1);
    let breached = |name: &str| reference.records.iter().any(|r| plant_of(&inp, r.floor) == name && matches!(r.kind, Kind::Breach(_)));
    eprintln!(
        "#207 reference (whole H1 cube, {} cells): {} records over {rounds} rounds; breached h1-region-seam {}, h1-cube-edge {}",
        inp.z0.len(),
        reference.records.len(),
        breached("h1-region-seam"),
        breached("h1-cube-edge")
    );
    assert!(breached("h1-region-seam") && breached("h1-cube-edge"), "both plants breach");
    let mut window_differs = Vec::new();
    for l in partition_layouts().iter().map(at_h1) {
        // the tiled fallback (the design note's point 4): #206's per-round local phase, join and apply at H1
        let r = run(&inp, Some(&l), false);
        let same = r.z.iter().zip(&reference.z).all(|(a, b)| a.to_bits() == b.to_bits());
        let w = windowed(&inp, &l);
        let changed = w.iter().zip(&reference.z).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        if changed > 0 {
            window_differs.push(format!("{} ({changed} cells)", l.name));
        }
        eprintln!(
            "#207 {}: tile {} halo {} (H1 cells), candidates by round {:?}, tiled fallback bit-identical {same}; the windowed form differs on {changed} cells",
            l.name, l.tile, l.halo, r.candidates
        );
        assert_eq!(r.records, reference.records, "{}: the tiled fallback's records", l.name);
        assert!(same, "{}: the tiled fallback's output", l.name);
    }
    eprintln!("#207 falsification: the windowed form (the window edge an outlet) differs from the global form on {window_differs:?}");
    assert!(window_differs.len() >= 3, "the windowed form must differ wherever a plant crosses a region's window");
    // one piece at production: 6 faces of 12,288^2 H1 cells, at drain_selective's working set (z, level and dist 8 B each; label and parent 4 B; outlet 1 B)
    let cells = 6.0 * 12_288f64 * 12_288f64;
    eprintln!(
        "#207 memory of the one-piece form at production: {:.0} M cells x 33 B = {:.1} GB (a face: {:.1} GB)",
        cells / 1e6,
        cells * 33.0 / 1e9,
        cells * 33.0 / 6e9
    );
}
