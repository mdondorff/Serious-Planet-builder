//! Spike only (crusader #205, lead 2026-11-25; design note `lanes/digest/proposals/cross-seam-breaches-2026-11-25.md`): the first proof of partition-invariant
//! breaches on TEST-139's fixture, for KC's breach rule. KC's search reads the surface before its cuts, and its cuts lower a separate copy by a minimum. So every
//! depression's decision depends only on the input, and the order of resolution does not change the result.
//! The proof: a reference over the whole cube as one grid (cube adjacency by `GlobalCell`) against each of TEST-151's five layouts, in three phases:
//! - local: each region searches the depressions it owns (the canonical floor in its tiles' cores) inside its domain, the tiles' `CubePatch`es (cores plus halo);
//! - join: the candidates in (spill level, floor `GlobalCell`) order, each searched over the whole grid;
//! - apply: each region writes its own cells from all records.
//!
//! A search is a candidate when it pops a cell on the domain's border, or reaches an exit while a border cell's tentative cost is not above the exit's.
//! Scope of the proof: the depressions (cells, level, fill volume) come from the planet-wide fill level, a shared input (GEN-121 and GEN-191 give it alike on any
//! tiling); a constant 50 m budget and no protections. Off by default: nothing in the chain reads it.

use super::g0_fixture::fixture::*;
use planet_core::{CubePatch, Face, GlobalCell};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};

const N: i64 = L_N as i64;
/// KC's budget for a cut or a fill (m), the contract's 50 m ceiling.
const BUDGET_M: f64 = 50.0;
/// KC's search cap (settled cells), `BREACH_CAP`.
const CAP: usize = 20_000;
const TOL: f64 = 1e-3;

fn cell(p: usize) -> GlobalCell {
    let nn = L_N * L_N;
    GlobalCell::new(Face((p / nn) as u8), N, (p % L_N) as i64, ((p % nn) / L_N) as i64).unwrap()
}

fn index(c: GlobalCell) -> usize {
    (c.face().0 as usize * L_N + c.j() as usize) * L_N + c.i() as usize
}

/// An f64 as an order-preserving key.
pub(super) fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b >> 63 == 1 {
        !b
    } else {
        b | (1 << 63)
    }
}

/// The cube grid of `n` cells a face edge, in the fixture's order (face, row, column), with its 8-neighbourhood and `GlobalCell` ids.
pub(super) struct Grid {
    pub(super) n: usize,
    pub(super) nb: Vec<Vec<usize>>,
    pub(super) raw: Vec<u64>,
}

impl Grid {
    /// The L grid.
    pub(super) fn new() -> Grid {
        Grid::at(L_N)
    }

    pub(super) fn at(n: usize) -> Grid {
        let g = Grid { n, nb: Vec::new(), raw: Vec::new() };
        let len = 6 * n * n;
        Grid {
            nb: (0..len).map(|p| g.cell(p).neighbours8(n as i64).into_iter().map(|c| g.index(c)).collect()).collect(),
            raw: (0..len).map(|p| g.cell(p).raw()).collect(),
            ..g
        }
    }

    pub(super) fn cell(&self, p: usize) -> GlobalCell {
        let nn = self.n * self.n;
        GlobalCell::new(Face((p / nn) as u8), self.n as i64, (p % self.n) as i64, ((p % nn) / self.n) as i64).unwrap()
    }

    pub(super) fn index(&self, c: GlobalCell) -> usize {
        (c.face().0 as usize * self.n + c.j() as usize) * self.n + c.i() as usize
    }
}

/// The planet-wide fill level: a priority flood from the sea over the cube's 8-neighbourhood, ties by `GlobalCell`.
pub(super) fn fill_level(g: &Grid, z: &[f64], outlet: &[bool]) -> Vec<f64> {
    let mut level = vec![f64::INFINITY; z.len()];
    let mut heap = BinaryHeap::new();
    for p in (0..z.len()).filter(|&p| outlet[p]) {
        level[p] = z[p];
        heap.push(Reverse((key(z[p]), g.raw[p], p)));
    }
    while let Some(Reverse((_, _, p))) = heap.pop() {
        for &q in &g.nb[p] {
            if level[q].is_infinite() {
                level[q] = z[q].max(level[p]);
                heap.push(Reverse((key(level[q]), g.raw[q], q)));
            }
        }
    }
    level
}

/// A depression: its cells, its canonical floor (least height, then `GlobalCell`), its level, whether KC's fill fits the budget, and the fill's volume.
struct Depression {
    cells: HashSet<usize>,
    floor: usize,
    level: f64,
    fill_ok: bool,
    fill_vol: f64,
}

fn depressions(g: &Grid, z: &[f64], level: &[f64], area: &[f64]) -> Vec<Depression> {
    let mut seen = vec![false; z.len()];
    let mut out = Vec::new();
    for p0 in 0..z.len() {
        if seen[p0] || level[p0] - z[p0] <= TOL {
            continue;
        }
        let mut cells = vec![p0];
        seen[p0] = true;
        let mut k = 0;
        while k < cells.len() {
            for &q in &g.nb[cells[k]] {
                if !seen[q] && level[q] - z[q] > TOL {
                    seen[q] = true;
                    cells.push(q);
                }
            }
            k += 1;
        }
        cells.sort_unstable();
        let floor = *cells.iter().min_by(|&&a, &&b| z[a].total_cmp(&z[b]).then(g.raw[a].cmp(&g.raw[b]))).unwrap();
        let fill_ok = cells.iter().all(|&c| level[c] - z[c] <= BUDGET_M);
        let fill_vol: f64 = cells.iter().map(|&c| (level[c] - z[c]) * area[c]).sum();
        out.push(Depression { cells: cells.into_iter().collect(), floor, level: level[floor], fill_ok, fill_vol });
    }
    out
}

/// A breach decision: the depression's floor, the path (indices) and the cut volume.
#[derive(Clone, Debug, PartialEq)]
struct Record {
    floor: usize,
    path: Vec<usize>,
    vol: f64,
}

/// What a search found: a path with its volume, none (no exit within the cap or the budget), or a candidate for the join.
enum Found {
    Path(Vec<usize>, f64),
    None,
    Candidate,
}

/// The domain a search may enter, with its border (domain cells that have a neighbour outside it).
struct Domain {
    cells: HashSet<usize>,
    border: HashSet<usize>,
}

/// KC's breach search (`basins_rg::breach_path`'s rule): least `max(0, z - floor) * area` summed, entering only cells whose cut fits the budget, to the first cell
/// outside the depression at or below the floor, or an outlet; ties by (cost, `GlobalCell`). `domain` None: the whole cube.
fn search(g: &Grid, z: &[f64], outlet: &[bool], area: &[f64], d: &Depression, domain: Option<&Domain>) -> Found {
    let floor = z[d.floor];
    let cut = |c: usize| (z[c] - floor).max(0.0);
    let mut best: HashMap<usize, (f64, usize)> = HashMap::new();
    let mut heap = BinaryHeap::new();
    best.insert(d.floor, (0.0, usize::MAX));
    heap.push(Reverse((key(0.0), g.raw[d.floor], d.floor)));
    let mut settled = 0usize;
    let mut border_min = f64::INFINITY;
    let mut done: HashSet<usize> = HashSet::new();
    while let Some(Reverse((k, _, c))) = heap.pop() {
        let (cost, _) = best[&c];
        if key(cost) != k || !done.insert(c) {
            continue;
        }
        if domain.is_some_and(|dm| dm.border.contains(&c)) {
            return Found::Candidate;
        }
        settled += 1;
        if settled > CAP {
            return Found::None;
        }
        if c != d.floor && (outlet[c] || (!d.cells.contains(&c) && z[c] <= floor)) {
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
            let vol = path.iter().map(|&p| cut(p) * area[p]).sum();
            return Found::Path(path, vol);
        }
        for &q in &g.nb[c] {
            if domain.is_some_and(|dm| !dm.cells.contains(&q)) || cut(q) > BUDGET_M {
                continue;
            }
            let nc = cost + cut(q) * area[q];
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

/// KC's decision on a found path: breach when its volume is below the fill's, or the fill does not fit.
fn decide(d: &Depression, found: Found) -> Option<Record> {
    match found {
        Found::Path(path, vol) if vol < d.fill_vol || !d.fill_ok => Some(Record { floor: d.floor, path, vol }),
        _ => None,
    }
}

/// The cut surface: every record lowers its path's cells (not the outlets) to its floor, by a minimum, so the order does not matter.
fn apply(z: &[f64], outlet: &[bool], records: &[Record], owned: &dyn Fn(usize) -> bool) -> Vec<f64> {
    let mut kc = z.to_vec();
    for r in records {
        let floor = z[r.floor];
        for &c in &r.path {
            if owned(c) && !outlet[c] && kc[c] > floor {
                kc[c] = floor;
            }
        }
    }
    kc
}

struct Setup {
    g: Grid,
    z: Vec<f64>,
    outlet: Vec<bool>,
    area: Vec<f64>,
    deps: Vec<Depression>,
    /// The planted pits, in `PLANTS` order.
    pits: Vec<BTreeSet<usize>>,
}

impl Setup {
    /// The plant whose pit holds a depression's floor, or "fixture".
    fn name(&self, d: &Depression) -> &'static str {
        self.pits.iter().position(|p| p.contains(&d.floor)).map_or("fixture", |k| PLANTS[k].name)
    }
}

/// A planted breach: a pit (centre lon and lat, radius in degrees) at `PIT_FLOOR_M` in a `PLATEAU_M` plateau, drained by a trench to a sea cell at the end point.
/// The trench falls from the floor + 45 m at the pit to the floor, then the sea cell: within KC's budget only along the trench, so the breach follows it.
struct Plant {
    name: &'static str,
    pit: (f64, f64),
    radius: f64,
    end: (f64, f64),
}

const PLATEAU_M: f64 = 2000.0;
const PIT_FLOOR_M: f64 = 20.0;
const TRENCH_HEAD_M: f64 = 45.0;

/// The forced cases: long breaches across the two-region seam (longitude -15), a cube edge (longitude 45) and near a three-face corner (45, 35.26), a short one
/// across a tile edge inside the halo, and a candidate whose fill wins (a small pit).
const PLANTS: [Plant; 5] = [
    Plant { name: "region-seam", pit: (-17.5, -20.0), radius: 2.5, end: (5.0, -20.0) },
    Plant { name: "cube-edge", pit: (38.0, -25.0), radius: 2.5, end: (62.0, -25.0) },
    Plant { name: "corner", pit: (35.0, 28.0), radius: 2.5, end: (55.0, 42.0) },
    Plant { name: "in-halo", pit: (11.0, -35.0), radius: 1.5, end: (20.0, -35.0) },
    Plant { name: "fill-wins", pit: (-17.0, 25.0), radius: 0.6, end: (5.0, 25.0) },
];

/// The cells within `r` degrees of a point (longitude scaled by the latitude's cosine).
fn disk(lon: f64, lat: f64, r: f64) -> BTreeSet<usize> {
    disk_n(L_N, lon, lat, r)
}

/// [`disk`] on the grid of `n` cells a face edge.
pub(super) fn disk_n(n: usize, lon: f64, lat: f64, r: f64) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    let k = (r / 0.05).ceil() as i64;
    for a in -k..=k {
        for b in -k..=k {
            let (dl, dt) = (a as f64 * 0.05, b as f64 * 0.05);
            if dl * dl + dt * dt <= r * r {
                out.insert(cell_at(n, lon + dl / libm::cos((lat + dt).to_radians()), lat + dt));
            }
        }
    }
    out
}

/// Plants the cases into `z`; returns each plant's pit and trench (pit edge to sea cell).
fn plant(g: &Grid, z: &mut [f64]) -> Vec<(BTreeSet<usize>, Vec<usize>)> {
    let mut trenches = Vec::new();
    for pl in &PLANTS {
        let steps = 4000;
        let at = |k: usize| {
            let t = k as f64 / steps as f64;
            (pl.pit.0 + t * (pl.end.0 - pl.pit.0), pl.pit.1 + t * (pl.end.1 - pl.pit.1))
        };
        for k in (0..=steps).step_by(20) {
            let (lo, la) = at(k);
            for c in disk(lo, la, pl.radius.max(1.0) + 2.0) {
                z[c] = PLATEAU_M;
            }
        }
        let pit = disk(pl.pit.0, pl.pit.1, pl.radius);
        let mut path: Vec<usize> = Vec::new();
        for k in 0..=steps {
            let (lo, la) = at(k);
            let c = cell_at(L_N, lo, la);
            if !pit.contains(&c) && path.last() != Some(&c) {
                if let Some(&prev) = path.last() {
                    if !g.nb[prev].contains(&c) {
                        let bridge = *g.nb[prev].iter().find(|q| g.nb[c].contains(q)).expect("a bridge cell between trench cells");
                        path.push(bridge);
                    }
                }
                path.push(c);
            }
        }
        for &c in &pit {
            z[c] = PIT_FLOOR_M;
        }
        let n = path.len() - 1;
        for (k, &c) in path.iter().enumerate() {
            z[c] = if k == n { SEA_LEVEL_M - 1.0 } else { PIT_FLOOR_M + TRENCH_HEAD_M * (1.0 - k as f64 / n as f64) };
        }
        trenches.push((pit, path));
    }
    trenches
}

fn setup() -> Setup {
    let g = Grid::new();
    let mut z = build(4).s0;
    let trenches = plant(&g, &mut z);
    for (pl, (_p, t)) in PLANTS.iter().zip(&trenches) {
        eprintln!("#205 plant {}: pit {} cells, trench {} cells", pl.name, _p.len(), t.len());
    }
    let pits = trenches.into_iter().map(|(p, _)| p).collect();
    let outlet: Vec<bool> = z.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    assert!(outlet.iter().any(|&o| o), "the fixture has sea");
    let area: Vec<f64> = (0..z.len()).map(|p| cell(p).area(N, RADIUS_M)).collect();
    let level = fill_level(&g, &z, &outlet);
    let deps = depressions(&g, &z, &level, &area);
    Setup { g, z, outlet, area, deps, pits }
}

/// The reference: every depression searched over the whole cube.
fn reference(s: &Setup) -> (Vec<Record>, Vec<f64>) {
    let mut recs: Vec<Record> = s.deps.iter().filter_map(|d| decide(d, search(&s.g, &s.z, &s.outlet, &s.area, d, None))).collect();
    recs.sort_by_key(|r| s.g.raw[r.floor]);
    let kc = apply(&s.z, &s.outlet, &recs, &|_| true);
    (recs, kc)
}

/// What a layout's partitioned run gives, and what it costs.
struct Run {
    records: Vec<Record>,
    kc: Vec<f64>,
    candidates: Vec<&'static str>,
    candidate_breaches: usize,
    join_regions: Vec<usize>,
    domain_cells: Vec<usize>,
}

/// `naive`: decide in the region whatever its domain gives (no candidate rule), the falsification of the rule.
fn partitioned(s: &Setup, l: &PartitionLayout, naive: bool) -> Run {
    let t = TILES_PER_FACE;
    let region_of = |p: usize| {
        let c = cell(p);
        l.region(c.face().0 as usize, c.i() as usize / l.tile, c.j() as usize / l.tile)
    };
    let mut local: Vec<Record> = Vec::new();
    let mut candidates: Vec<&Depression> = Vec::new();
    let mut domain_cells = Vec::new();
    for r in 0..l.regions as u8 {
        // the domain: the region's tiles' patches (cores and halo, across edges and corners by the fold)
        let mut cells: HashSet<usize> = HashSet::new();
        for f in 0..6 {
            for tj in 0..t {
                for ti in 0..t {
                    if l.region(f, ti, tj) == r {
                        let p = CubePatch::new(Face(f as u8), N, (ti * l.tile) as i64, (tj * l.tile) as i64, l.tile as i64, l.halo as i64)
                            .unwrap();
                        cells.extend(p.cells().iter().map(|&c| index(c)));
                    }
                }
            }
        }
        let border: HashSet<usize> =
            if naive { HashSet::new() } else { cells.iter().copied().filter(|&p| s.g.nb[p].iter().any(|q| !cells.contains(q))).collect() };
        domain_cells.push(cells.len());
        let dom = Domain { cells, border };
        for d in s.deps.iter().filter(|d| region_of(d.floor) == r) {
            match search(&s.g, &s.z, &s.outlet, &s.area, d, Some(&dom)) {
                Found::Candidate => candidates.push(d),
                found => local.extend(decide(d, found)),
            }
        }
    }
    // the join: the candidates in (spill level, floor GlobalCell) order, each over the whole grid
    candidates.sort_by(|a, b| a.level.total_cmp(&b.level).then(s.g.raw[a.floor].cmp(&s.g.raw[b.floor])));
    let names: Vec<&'static str> = candidates.iter().map(|d| s.name(d)).collect();
    let mut joined = Vec::new();
    let mut join_regions = Vec::new();
    for d in &candidates {
        if let Some(rec) = decide(d, search(&s.g, &s.z, &s.outlet, &s.area, d, None)) {
            let touched: BTreeSet<u8> = rec.path.iter().map(|&p| region_of(p)).collect();
            join_regions.push(touched.len());
            joined.push(rec);
        }
    }
    let candidate_breaches = joined.len();
    let mut records: Vec<Record> = local.into_iter().chain(joined).collect();
    records.sort_by_key(|r| s.g.raw[r.floor]);
    // apply: each region writes its own cells from every record
    let mut kc = s.z.clone();
    for r in 0..l.regions as u8 {
        let part = apply(&s.z, &s.outlet, &records, &|p| region_of(p) == r);
        for p in (0..kc.len()).filter(|&p| region_of(p) == r) {
            kc[p] = part[p];
        }
    }
    Run { records, kc, candidates: names, candidate_breaches, join_regions, domain_cells }
}

// spike only (crusader #205): the first partition row of the cross-seam design
#[test]
fn kcs_breaches_are_the_same_bits_on_every_layout() {
    let s = setup();
    let (ref_recs, ref_kc) = reference(&s);
    let cut_cells = ref_kc.iter().zip(&s.z).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    eprintln!("#205 fixture: {} depressions, {} breached in the reference, {cut_cells} cells cut", s.deps.len(), ref_recs.len());
    assert!(!ref_recs.is_empty(), "the fixture has breaches to resolve");
    let breached: Vec<&str> = ref_recs.iter().map(|r| s.name(s.deps.iter().find(|d| d.floor == r.floor).unwrap())).collect();
    eprintln!("#205 reference breaches by plant: {breached:?}");
    for pl in &PLANTS[..4] {
        assert!(breached.contains(&pl.name), "the plant {} is breached in the reference", pl.name);
    }
    assert!(!breached.contains(&"fill-wins"), "the small pit is filled, not breached");
    let mut naive_differs = Vec::new();
    for l in partition_layouts() {
        let naive = partitioned(&s, &l, true);
        if naive.records != ref_recs {
            naive_differs.push(l.name.clone());
        }
        let run = partitioned(&s, &l, false);
        let same_kc = run.kc.iter().zip(&ref_kc).all(|(a, b)| a.to_bits() == b.to_bits());
        eprintln!(
            "#205 {}: {} regions, candidates {:?} ({} breach after the join, regions touched {:?}), domain cells {:?} (max {} B at 16 B a cell), records {} (reference {}), KC bit-identical {same_kc}",
            l.name,
            l.regions,
            run.candidates,
            run.candidate_breaches,
            run.join_regions,
            run.domain_cells,
            16 * run.domain_cells.iter().max().unwrap(),
            run.records.len(),
            ref_recs.len()
        );
        assert_eq!(run.records, ref_recs, "{}: the sorted breach records", l.name);
        assert!(same_kc, "{}: KC's output", l.name);
        if l.regions > 1 && l.name != "shifted-seam" {
            assert!(!run.candidates.is_empty(), "{}: a planted breach crosses its seam", l.name);
        }
    }
    eprintln!("#205 falsification: deciding in the region without the candidate rule changes the records on {naive_differs:?}");
    assert!(!naive_differs.is_empty(), "without the candidate rule some layout must differ, else the plants test nothing");
}
