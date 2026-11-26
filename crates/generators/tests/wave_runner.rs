//! Spike only (crusader #208 step 2, lead 2026-11-25): the wave runner on TEST-139's fixture. A stage is a hook with a declared form; the runner owns the regions, their search domains
//! and the barriers, so a rule change inside a stage needs no runner change. Forms:
//! - `Local { halo }`: tile-local by a halo (KH, KS, KV);
//! - `Join`: per round, each region decides what it owns in its domain (its tiles' `CubePatch`es), a search that reaches the domain's border is a candidate, the join decides the candidates
//!   on the whole grid in (spill level, floor id) order, each region applies its own cells; a barrier after each round (#205-#207);
//! - `Owner`: the routing tile by tile (GEN-143), each unit (a catchment) decided whole by one owner (K5, the routing).
//!
//! The grid is `Grid::cube` (GEN-144): its index is the cube id `face n² + j n + i`, so a tie by the index is a tie by a partition-free id, and on one face window it is today's order.
//! The stages call the chain's kernels (`drain::sel_*`, `solve_rounds_memo`, `partitioned_route`) and take the chain's switches through their parameters and the drain's statics.

use super::g0_fixture::fixture::*;
use planet_core::{CubePatch, Face, GlobalCell};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::drain::{sel_apply, sel_decide, sel_round, SearchDomain, SelDecision, SelParams, SelSwitches};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
use planet_generators::bake::round::{solve_rounds_memo, solve_rounds_with, Drivers, RoundCache};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use std::collections::BTreeMap;

/// How a stage is partitioned.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)] // `Local` serves KH, KS and KV in L-A and L-B (the next steps)
pub enum Form {
    Local {
        halo: usize,
    },
    Join,
    Owner,
    /// decided once over the whole grid (a wave's global decide; its production scale is G3-6's, R13)
    Global,
}

/// A TEST-151 layout on a cube grid of `n` cells an edge: the region of each cell, each region's domain (its tiles' patches) and the domain's border.
pub struct Regions {
    pub name: String,
    pub count: usize,
    /// The tile in this grid's cells.
    pub tile: usize,
    pub region_of: Vec<u8>,
    pub inside: Vec<Vec<bool>>,
    pub border: Vec<Vec<bool>>,
}

impl Regions {
    /// `per`: L cells per cell of this grid (1 at L, `L_PER_H1` at H1). `naive`: no border (the region decides whatever its domain gives; the falsification).
    pub fn new(g: &Grid, l: &PartitionLayout, per: usize, naive: bool) -> Regions {
        let n = g.layout().0;
        let (tile, halo) = (l.tile / per, l.halo / per);
        let cube_id = |p: usize| if g.is_compact() { g.cube_id_of(p) } else { p };
        let index = |c: GlobalCell| {
            let id = (c.face().0 as usize * n + c.j() as usize) * n + c.i() as usize;
            if g.is_compact() {
                g.compact_index_of(id)
            } else {
                Some(id)
            }
        };
        let region_of: Vec<u8> = (0..g.len())
            .map(|p| {
                let id = cube_id(p);
                let (f, r) = (id / (n * n), id % (n * n));
                l.region(f, (r % n) / tile, (r / n) / tile)
            })
            .collect();
        let mut inside = vec![vec![false; g.len()]; l.regions];
        for f in 0..6 {
            for tj in 0..n / tile {
                for ti in 0..n / tile {
                    let r = l.region(f, ti, tj) as usize;
                    let p =
                        CubePatch::new(Face(f as u8), n as i64, (ti * tile) as i64, (tj * tile) as i64, tile as i64, halo as i64).unwrap();
                    for &c in p.cells() {
                        if let Some(q) = index(c) {
                            inside[r][q] = true;
                        }
                    }
                }
            }
        }
        let border = inside
            .iter()
            .map(|dom| (0..dom.len()).map(|p| !naive && dom[p] && (0..8).any(|d| g.neighbour(p, d).is_some_and(|q| !dom[q]))).collect())
            .collect();
        Regions { name: l.name.clone(), count: l.regions, tile, region_of, inside, border }
    }
}

/// What the stages read and write: the surface and named fields (bits), plus the trace of every decision (compared across layouts) and notes (not compared: they depend on the layout).
#[derive(Default)]
pub struct State {
    pub z: Vec<f64>,
    pub fields: BTreeMap<&'static str, Vec<u64>>,
    pub trace: Vec<String>,
    pub notes: Vec<String>,
}

pub struct Ctx<'a> {
    pub g: &'a Grid,
    pub outlet: &'a [bool],
    pub regions: Option<&'a Regions>,
    pub threads: usize,
}

/// A stage hook.
pub trait Stage {
    fn name(&self) -> &'static str;
    fn form(&self) -> Form;
    fn run(&self, cx: &Ctx, st: &mut State);
}

/// The selective drain (`drain_selective_floors`'s rule) as a `Join` stage: base1 and sel-post at H1 (`Drain::SELECTIVE_50`), band6 at L.
pub struct Sel {
    pub name: &'static str,
    pub max_rounds: usize,
    pub search: usize,
    /// Kept cells (none: empty).
    pub keep: Vec<bool>,
    /// The cut and fill cap of a depression, read at its floor.
    pub cap: Vec<f64>,
    /// The per-cell floor (none: empty).
    pub cell_lo: Vec<f64>,
    /// The drain's switches, per call (R16).
    pub switches: SelSwitches,
}

impl Sel {
    /// `Drain::SELECTIVE_50` on `n` cells: 12 rounds, search 2^20, caps 50 m, no kept cells, no floors.
    pub fn selective_50(name: &'static str, n: usize) -> Sel {
        Sel {
            name,
            max_rounds: 12,
            search: 1 << 20,
            keep: Vec::new(),
            cap: vec![50.0; n],
            cell_lo: Vec::new(),
            switches: SelSwitches::default(),
        }
    }
}

impl Stage for Sel {
    fn name(&self) -> &'static str {
        self.name
    }

    fn form(&self) -> Form {
        Form::Join
    }

    fn run(&self, cx: &Ctx, st: &mut State) {
        let g = cx.g;
        let keep = |p: usize| !self.keep.is_empty() && self.keep[p];
        let area = |p: usize| g.cell_area(p);
        let cap = |p: usize| self.cap[p];
        let lo = |p: usize| if self.cell_lo.is_empty() { f64::NEG_INFINITY } else { self.cell_lo[p] };
        let p = SelParams {
            keep: &keep,
            area: &area,
            max_rounds: self.max_rounds,
            search: self.search,
            cut_cap: &cap,
            fill_cap: &cap,
            cell_lo: &lo,
            switches: self.switches,
        };
        for round in 0..self.max_rounds {
            let r = sel_round(g, &st.z, cx.outlet, &p);
            if r.active.is_empty() {
                break;
            }
            // local phase
            let (mut decided, mut cand) = (Vec::new(), Vec::new());
            for &k in &r.active {
                let dom = cx.regions.map(|rg| {
                    let o = rg.region_of[r.comps[k].1] as usize;
                    SearchDomain { inside: &rg.inside[o], border: &rg.border[o] }
                });
                match sel_decide(g, &st.z, cx.outlet, &r, k, &p, dom.as_ref()) {
                    SelDecision::Candidate => cand.push(k),
                    d => decided.push((k, d)),
                }
            }
            // join: (spill level, floor id) on the whole grid
            let floor_of = |k: usize| r.comps[k].1;
            let level_of = |k: usize| r.level[floor_of(k)];
            cand.sort_by(|&a, &b| level_of(a).total_cmp(&level_of(b)).then(floor_of(a).cmp(&floor_of(b))));
            st.notes.push(format!("{} round {round}: {} candidates of {}", self.name, cand.len(), r.active.len()));
            for &k in &cand {
                decided.push((k, sel_decide(g, &st.z, cx.outlet, &r, k, &p, None)));
            }
            decided.sort_by_key(|&(k, _)| floor_of(k));
            for (k, d) in &decided {
                st.trace.push(format!("{} round {round} floor {} {}", self.name, g.global_id(floor_of(*k)), by_id(g, d)));
            }
            if !decided.iter().any(|(_, d)| matches!(d, SelDecision::Fill | SelDecision::Breach { .. })) {
                break;
            }
            // apply: each region its own cells (a barrier: the next round reads the whole applied surface)
            let z0 = st.z.clone();
            match cx.regions {
                None => sel_apply(&z0, &mut st.z, &r, &decided, &|_| true),
                Some(rg) => {
                    for o in 0..rg.count as u8 {
                        sel_apply(&z0, &mut st.z, &r, &decided, &|c| rg.region_of[c] == o);
                    }
                }
            }
        }
    }
}

/// A decision with its cells as global ids (the same on the full and the compact cube grid).
fn by_id(g: &Grid, d: &SelDecision) -> String {
    match d {
        SelDecision::Breach { path, cut, volume } => {
            let ids: Vec<u64> = path.iter().map(|&c| g.global_id(c as usize)).collect();
            format!("Breach {{ path: {ids:?}, cut: {cut:?}, volume: {volume:?} }}")
        }
        other => format!("{other:?}"),
    }
}

/// K5 (`solve_rounds_with`, 4 rounds of 5 steps, as `RegionSpec`) as an `Owner` stage: partitioned, `solve_rounds_memo` at the layout's tile.
pub struct K5 {
    pub rounds: usize,
    pub seed: u64,
}

pub fn k5_params(rounds: usize) -> CarveParams {
    CarveParams {
        steps: 5 * rounds,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    }
}

impl Stage for K5 {
    fn name(&self) -> &'static str {
        "k5"
    }

    fn form(&self) -> Form {
        Form::Owner
    }

    fn run(&self, cx: &Ctx, st: &mut State) {
        let p = k5_params(self.rounds);
        match cx.regions {
            None => {
                solve_rounds_with(cx.g, &mut st.z, cx.outlet, &p, self.rounds, &cx.threads, self.seed, &Drivers::default());
            }
            Some(rg) => {
                let mut cache = RoundCache::without_memory();
                solve_rounds_memo(
                    cx.g,
                    &mut st.z,
                    cx.outlet,
                    &p,
                    self.rounds,
                    &cx.threads,
                    self.seed,
                    &Drivers::default(),
                    rg.tile,
                    &mut cache,
                );
            }
        }
    }
}

/// Today's K6 (`--h1 k6`, `planet_cache::rg_h1::h1_from`, the chain's code): the routing with the discharge weight (`Owner`: GEN-143's route when partitioned), then the hydrology,
/// the channel graph and the trunks once over the whole H1 grid (`Global`). The fixture has no climate: a discharge weight of 1, a constant runoff and no evaporation.
pub struct K6 {
    pub seed: u64,
    /// Runoff (m/s) on every cell.
    pub runoff: f32,
}

impl Stage for K6 {
    fn name(&self) -> &'static str {
        "k6"
    }

    fn form(&self) -> Form {
        Form::Global
    }

    fn run(&self, cx: &Ctx, st: &mut State) {
        let g = cx.g;
        let eps = k5_params(1).epsilon;
        let weight = vec![1.0f32; g.len()];
        let (routing, filled) = match cx.regions {
            None => (route_rules(g, &st.z, cx.outlet, eps, self.seed, 0.0, Some(&weight), RoundRules::ROUND), None),
            Some(rg) => {
                let pr = partitioned_route_weighted(
                    g,
                    &st.z,
                    cx.outlet,
                    eps,
                    self.seed,
                    rg.tile,
                    RoundRules::ROUND,
                    Some(&weight),
                    &mut PartCache::without_memory(),
                );
                // G3-6 step 3 (a): the route's filled surface is the canonical flood bit for bit, so the partitioned K6 takes it (no second flood)
                (pr.routing, Some(pr.filled))
            }
        };
        drop(weight);
        let runoff = vec![self.runoff; g.len()];
        // the reference is today's form (a second flood, a zero evaporation field, a new d8 field); the partitioned run takes the route's filled surface (a), no evaporation
        // field (d) and the filled buffer as d8 (f): the layouts' bit-identity checks them against it
        let h1 = match filled {
            None => {
                let filled = priority_flood_with(g, &st.z, cx.outlet, eps, true);
                let evap = vec![0.0f32; g.len()];
                planet_cache::rg_h1::h1_from(g, &st.z, &filled, routing, cx.outlet, &runoff, &evap, Some(&[]), false)
            }
            Some(filled) => planet_cache::rg_h1::h1_from_owned(g, &st.z, filled, routing, cx.outlet, &runoff, &[], Some(&[]), false),
        };
        st.fields
            .insert("receiver", h1.routing.receiver.iter().map(|&v| if v == NONE { u64::MAX } else { g.global_id(v as usize) }).collect());
        st.fields.insert("area", h1.routing.area.iter().map(|v| v.to_bits()).collect());
        st.fields.insert("d8", h1.d8.iter().map(|v| v.to_bits()).collect());
        st.fields.insert("qw", h1.hydro.qw.iter().map(|v| v.to_bits()).collect());
        for (k, cells) in h1.trunk_cells.iter().enumerate() {
            let ids: Vec<u64> = cells.iter().map(|&c| g.global_id(c as usize)).collect();
            st.trace.push(format!("k6 trunk {} segment {} cells {ids:?}", k, h1.trunk_seg[k]));
        }
        st.notes.push(format!("k6: {} basins, {} trunk reaches", h1.hydro.basins.len(), h1.trunk_cells.len()));
    }
}

/// Runs the stages in order, a barrier between stages.
pub fn run_wave(stages: &[&dyn Stage], cx: &Ctx, z0: &[f64]) -> State {
    let mut st = State { z: z0.to_vec(), ..State::default() };
    for s in stages {
        let before = st.trace.len();
        s.run(cx, &mut st);
        st.notes.push(format!("{} ({:?}): {} decisions", s.name(), s.form(), st.trace.len() - before));
    }
    st
}

/// The first difference between two states, or none.
pub fn differs(a: &State, b: &State) -> Option<String> {
    if let Some(p) = (0..a.z.len()).find(|&p| a.z[p].to_bits() != b.z[p].to_bits()) {
        return Some(format!("z at cell {p}: {} against {}", a.z[p], b.z[p]));
    }
    for (k, v) in &a.fields {
        if b.fields.get(k) != Some(v) {
            return Some(format!("field {k}"));
        }
    }
    if a.trace != b.trace {
        let i = (0..a.trace.len().min(b.trace.len())).find(|&i| a.trace[i] != b.trace[i]).unwrap_or(a.trace.len().min(b.trace.len()));
        return Some(format!("trace at {i}: {:?} against {:?}", a.trace.get(i), b.trace.get(i)));
    }
    None
}

// spike only (crusader #208 steps 2 and 3): wave 1 at H1 through the runner on the five layouts (base1, K5, sel-post, K6)
#[test]
fn wave_1_is_the_same_bits_on_every_layout() {
    let inp = super::cross_seam_band6::input_h1();
    let g = Grid::cube(H1_N, RADIUS_M);
    let z0 = inp.z0.clone();
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let n = z0.len();
    let base1 = Sel::selective_50("base1", n);
    let k5 = K5 { rounds: 4, seed: 0x6b35 };
    let post = Sel::selective_50("sel-post", n);
    let k6 = K6 { seed: 0x6b35, runoff: 1.0e-6 };
    let stages: [&dyn Stage; 4] = [&base1, &k5, &post, &k6];
    let cx = Ctx { g: &g, outlet: &outlet, regions: None, threads: 4 };
    let t = std::time::Instant::now();
    let reference = run_wave(&stages, &cx, &z0);
    let moved = reference.z.iter().zip(&z0).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    eprintln!(
        "#208 wave 1 reference: {} decisions, {moved} cells changed, {:.2} s; {:?}",
        reference.trace.len(),
        t.elapsed().as_secs_f64(),
        reference.notes
    );
    assert!(reference.trace.iter().any(|t| t.starts_with("base1") && t.contains("Breach")), "base1 breaches the planted pits");
    assert!(reference.trace.iter().any(|t| t.starts_with("k6 trunk")), "K6 finds trunks on the fixture");
    let mut naive_differs = Vec::new();
    for l in partition_layouts() {
        let rg = Regions::new(&g, &l, L_PER_H1, false);
        let cx = Ctx { g: &g, outlet: &outlet, regions: Some(&rg), threads: 4 };
        let t = std::time::Instant::now();
        let st = run_wave(&stages, &cx, &z0);
        let secs = t.elapsed().as_secs_f64();
        let cands: Vec<&String> = st.notes.iter().filter(|s| s.contains("candidates") && !s.contains(": 0 candidates")).collect();
        eprintln!("#208 wave 1 {}: {secs:.2} s, rounds with candidates {cands:?}", rg.name);
        assert_eq!(differs(&reference, &st), None, "{}: wave 1", l.name);
        let rn = Regions::new(&g, &l, L_PER_H1, true);
        let cxn = Ctx { g: &g, outlet: &outlet, regions: Some(&rn), threads: 4 };
        if differs(&reference, &run_wave(&stages, &cxn, &z0)).is_some() {
            naive_differs.push(l.name.clone());
        }
    }
    eprintln!("#208 wave 1 falsification: without the candidate rule the wave differs on {naive_differs:?}");
    assert!(!naive_differs.is_empty(), "without the candidate rule some layout must differ");
}

// spike only (crusader #208 step 4): band6 at L through the drain library on #206's planted cases (a kept pit, a floor-blocked trench, an interacting pair across a seam)
#[test]
fn band6_through_the_drain_library_is_the_same_bits_on_every_layout() {
    let inp = super::cross_seam_band6::input();
    let g = Grid::cube(L_N, RADIUS_M);
    let n = inp.z0.len();
    // band6 as rg_chain runs it: 8 rounds, search 4096, the outlet floors on (lead #191), the per-cell floors, the kept cells; a constant 50 m budget (#206's scope)
    let band6 = Sel {
        name: "band6",
        max_rounds: 8,
        search: 4096,
        keep: inp.keep.clone(),
        cap: vec![50.0; n],
        cell_lo: inp.cell_lo.clone(),
        switches: SelSwitches { outlet_floors: true, ..SelSwitches::default() },
    };
    let stages: [&dyn Stage; 1] = [&band6];
    let cx = Ctx { g: &g, outlet: &inp.outlet, regions: None, threads: 4 };
    let reference = run_wave(&stages, &cx, &inp.z0);
    let breaches = reference.trace.iter().filter(|t| t.contains("Breach")).count();
    let rounds = reference
        .trace
        .iter()
        .filter_map(|t| t.split(" round ").nth(1)?.split(' ').next()?.parse::<usize>().ok())
        .max()
        .map_or(0, |r| r + 1);
    eprintln!("#208 band6 L reference: {} decisions, {breaches} breaches over {rounds} rounds", reference.trace.len());
    assert!(breaches >= 6 && rounds >= 2, "the plants breach, the interacting pair in a later round");
    let mut naive_differs = Vec::new();
    for l in partition_layouts() {
        let rg = Regions::new(&g, &l, 1, false);
        let st = run_wave(&stages, &Ctx { g: &g, outlet: &inp.outlet, regions: Some(&rg), threads: 4 }, &inp.z0);
        let cands: Vec<&String> = st.notes.iter().filter(|s| s.contains("candidates") && !s.contains(": 0 candidates")).collect();
        eprintln!("#208 band6 L {}: rounds with candidates {cands:?}", rg.name);
        assert_eq!(differs(&reference, &st), None, "{}: band6", rg.name);
        let rn = Regions::new(&g, &l, 1, true);
        if differs(&reference, &run_wave(&stages, &Ctx { g: &g, outlet: &inp.outlet, regions: Some(&rn), threads: 4 }, &inp.z0)).is_some() {
            naive_differs.push(rn.name.clone());
        }
    }
    eprintln!("#208 band6 L falsification: without the candidate rule band6 differs on {naive_differs:?}");
    assert!(!naive_differs.is_empty(), "without the candidate rule some layout must differ");
}

/// KH in the h2 form (`kh::read`, the flow-diagonal triangulation of z_s; G0 contracts reading 19) as a `Local { halo: 0 }` stage at L: each L cell reads z_s's H1 block of its own face
/// (`kh_g0`'s reading) plus its residual `r = B - C1(z_b)`. Its H1 inputs (z_s, its routing and splits) are wave 1's outputs, read whole after wave 1's barrier. The chain's default h5 form
/// (h2 plus the curve zone's blend) is glue in rg_chain's L-stage block and comes with its move (DEC crusader-49).
pub struct KhH2 {
    /// z_s at H1, wave 1's output.
    pub zs: Vec<f64>,
    /// z_b at H1 and B at L (the residual's inputs).
    pub zb: Vec<f64>,
    pub b: Vec<f64>,
}

impl Stage for KhH2 {
    fn name(&self) -> &'static str {
        "kh-h2"
    }

    fn form(&self) -> Form {
        Form::Local { halo: 0 }
    }

    fn run(&self, cx: &Ctx, st: &mut State) {
        use planet_core::Face;
        use planet_generators::bake::h0::GlobalGrid;
        use planet_generators::bake::k7::RegionField;
        use planet_generators::bake::kh;
        let nn = H1_N * H1_N;
        let gh = Grid::cube(H1_N, RADIUS_M);
        let outlet = gh.outlets(&self.zs, SEA_LEVEL_M);
        let (recv, area) = kh::routing(&gh, &self.zs, &outlet);
        let splits: Vec<Vec<u8>> = (0..6)
            .map(|f| {
                let rv = |c: usize| {
                    let r = recv[f * nn + c];
                    (r != NONE && r as usize / nn == f).then(|| r as usize % nn)
                };
                kh::split(&self.zs[f * nn..(f + 1) * nn], H1_N, H1_N, &rv, &|c| area[f * nn + c]).0
            })
            .collect();
        let l = GlobalGrid::new(L_N, RADIUS_M);
        let zb: Vec<RegionField> = (0..6)
            .map(|f| {
                RegionField::new(Face(f as u8), H1_N, (0, 0), H1_N, H1_N, self.zb[f * nn..(f + 1) * nn].iter().map(|&x| x as f32).collect())
            })
            .collect();
        let (ln, lr) = (L_N * L_N, L_PER_H1 as f64);
        let cell = |p: usize| {
            let (f, i, j) = (p / ln, p % L_N, (p % ln) / L_N);
            let r = self.b[p] - zb[f].at(l.direction(p as u32));
            kh::read(
                &self.zs[f * nn..(f + 1) * nn],
                H1_N,
                H1_N,
                &splits[f],
                (i as f64 - (lr - 1.0) / 2.0) / lr,
                (j as f64 - (lr - 1.0) / 2.0) / lr,
            ) + r
        };
        let mut out = vec![f64::NAN; cx.g.len()];
        // each region its own cells (a tile-local stage with no halo: a cell reads only the whole H1 fields)
        match cx.regions {
            None => (0..out.len()).for_each(|p| out[p] = cell(p)),
            Some(rg) => {
                for o in 0..rg.count as u8 {
                    for p in (0..out.len()).filter(|&p| rg.region_of[p] == o) {
                        out[p] = cell(p);
                    }
                }
            }
        }
        st.z = out;
    }
}

// spike only (crusader #208 step 4b): the first cross-wave composite, wave 1 at H1 then KH at L, on the five layouts
#[test]
fn wave_1_then_kh_is_the_same_bits_on_every_layout() {
    let inp = super::cross_seam_band6::input_h1();
    let fx = build(4);
    let gh = Grid::cube(H1_N, RADIUS_M);
    let gl = Grid::cube(L_N, RADIUS_M);
    let outlet: Vec<bool> = inp.z0.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let n = inp.z0.len();
    let (base1, post) = (Sel::selective_50("base1", n), Sel::selective_50("sel-post", n));
    let k5 = K5 { rounds: 4, seed: 0x6b35 };
    let k6 = K6 { seed: 0x6b35, runoff: 1.0e-6 };
    let wave1: [&dyn Stage; 4] = [&base1, &k5, &post, &k6];
    let chain = |l: Option<&PartitionLayout>| {
        let rh = l.map(|l| Regions::new(&gh, l, L_PER_H1, false));
        let w1 = run_wave(&wave1, &Ctx { g: &gh, outlet: &outlet, regions: rh.as_ref(), threads: 4 }, &inp.z0);
        // the barrier: L-A reads wave 1's outputs whole
        let kh = KhH2 { zs: w1.z.clone(), zb: inp.z0.clone(), b: fx.b.clone() };
        let rl = l.map(|l| Regions::new(&gl, l, 1, false));
        let ol: Vec<bool> = fx.b.iter().map(|&h| h <= SEA_LEVEL_M).collect();
        let la = run_wave(&[&kh], &Ctx { g: &gl, outlet: &ol, regions: rl.as_ref(), threads: 4 }, &fx.b);
        (w1, la)
    };
    let (w1, la) = chain(None);
    assert!(la.z.iter().all(|v| v.is_finite()), "every L cell read");
    let dev = la.z.iter().zip(&fx.s0).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    eprintln!("#208 wave 1 -> KH: {} wave-1 decisions; |S_KH - S0| max {dev:.2} m", w1.trace.len());
    for l in partition_layouts() {
        let (a, b) = chain(Some(&l));
        assert_eq!(differs(&w1, &a), None, "{}: wave 1", l.name);
        assert_eq!(differs(&la, &b), None, "{}: KH", l.name);
    }
    eprintln!("#208 wave 1 -> KH: bit-identical on all five layouts");
}

/// The compact grid of a cube surface's land (GEN-147): every land cell and every sea cell next to land (the outlets land drains to), indexed by the rank of its cube id. Returns the grid
/// and each compact cell's cube id.
pub fn compact_land(n: usize, outlet: &[bool]) -> (Grid, Vec<usize>) {
    let full = Grid::cube(n, RADIUS_M);
    let mut bits = vec![0u64; (6 * n * n).div_ceil(64)];
    for p in 0..6 * n * n {
        if !outlet[p] || (0..8).any(|d| full.neighbour(p, d).is_some_and(|q| !outlet[q])) {
            bits[p / 64] |= 1 << (p % 64);
        }
    }
    let g = Grid::compact_cube(n, RADIUS_M, bits);
    let ids = (0..g.len()).map(|p| g.cube_id_of(p)).collect();
    (g, ids)
}

// spike only (crusader, G3-6 layout step 1, lead 2026-11-25): wave 1 on the compact grid is today's wave 1 on the full cube, on every layout
#[test]
fn wave_1_on_the_compact_grid_is_today_s_wave_1() {
    let inp = super::cross_seam_band6::input_h1();
    let gh = Grid::cube(H1_N, RADIUS_M);
    let z0 = inp.z0.clone();
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let (gc, ids) = compact_land(H1_N, &outlet);
    let zc0: Vec<f64> = ids.iter().map(|&i| z0[i]).collect();
    let oc: Vec<bool> = ids.iter().map(|&i| outlet[i]).collect();
    eprintln!("#G3-6.1 the fixture's H1: compact {} of {} cells ({:.1} %)", gc.len(), z0.len(), 100.0 * gc.len() as f64 / z0.len() as f64);
    let wave = |g: &Grid| {
        let n = g.len();
        (
            Sel::selective_50("base1", n),
            K5 { rounds: 4, seed: 0x6b35 },
            Sel::selective_50("sel-post", n),
            K6 { seed: 0x6b35, runoff: 1.0e-6 },
        )
    };
    let (fb, fk5, fp, fk6) = wave(&gh);
    let (cb, ck5, cp, ck6) = wave(&gc);
    let full_stages: [&dyn Stage; 4] = [&fb, &fk5, &fp, &fk6];
    let comp_stages: [&dyn Stage; 4] = [&cb, &ck5, &cp, &ck6];
    let layouts = partition_layouts();
    for l in std::iter::once(None).chain(layouts.iter().map(Some)) {
        let (rf, rc) = (l.map(|l| Regions::new(&gh, l, L_PER_H1, false)), l.map(|l| Regions::new(&gc, l, L_PER_H1, false)));
        let full = run_wave(&full_stages, &Ctx { g: &gh, outlet: &outlet, regions: rf.as_ref(), threads: 4 }, &z0);
        let comp = run_wave(&comp_stages, &Ctx { g: &gc, outlet: &oc, regions: rc.as_ref(), threads: 4 }, &zc0);
        let name = l.map_or("unpartitioned", |l| l.name.as_str());
        let mut held = vec![false; z0.len()];
        for (k, &i) in ids.iter().enumerate() {
            held[i] = true;
            assert_eq!(comp.z[k].to_bits(), full.z[i].to_bits(), "{name}: z at cube cell {i}");
            for (f, v) in &comp.fields {
                assert_eq!(v[k], full.fields[f][i], "{name}: {f} at cube cell {i}");
            }
        }
        assert!(
            (0..z0.len()).all(|i| held[i] || full.z[i].to_bits() == z0[i].to_bits()),
            "{name}: a cell the compact grid leaves out is unchanged in the full run"
        );
        assert_eq!(comp.trace, full.trace, "{name}: the decisions by global id");
    }
    eprintln!("#G3-6.1 wave 1 on the compact grid = today's wave 1 on the full cube: unpartitioned and on all five layouts");
}
