//! The round solver (GEN-142, A3 section 6.2): one region-wide routing under the round rules (GEN-141), then the breach and the implicit sweeps of every catchment from that
//! catchment's key inputs alone, on any thread, in any order. The result is the bits of the whole-region carve (`carve` with the same rules, G-ROUND), on any thread count (G-DET).
//! With drivers (GEN-146): a class carve factor, an uplift per cell and the runoff weights of the carve, as `carve_rock` reads them. With a cache ([`solve_rounds_memo`], GEN-146): the routing
//! is the partitioned route with memoised tile floods (GEN-143) and a catchment whose key inputs were seen is not carved again, so the re-solve of an edit is the solve of the edited
//! definition (G-EQ) with only the work of what changed (G-INC).
//! Behind a switch: nothing in pass 1 or pass 2 calls it.

use super::carve::{breach_limit, distance, CarveParams};
use super::memo::Memo;
use super::partition::{partitioned_route_weighted, Hash, PartCache};
use super::probe;
use super::route::{catchment_roots, route_rules, MfdClip, Routing, NONE};
use super::{assert_finite, Grid};
use planet_core::par::{self, Par};
use std::sync::Arc;

/// What a catchment's round reads (A3 section 6.5, the catchment-round key): its members in the region's topological order (outlet first), the receiver of every member as a
/// position in that list, `A_d`, the distance to the receiver, the taper of the uplift, the class factor and the uplift of each member, whether the receiver is an outlet of the
/// region, and the surface at the start of the round.
#[derive(Clone, Debug, PartialEq)]
pub struct CatchmentInputs {
    pub z: Vec<f64>,
    pub receiver: Vec<u32>,
    pub area: Vec<f64>,
    pub dist: Vec<f64>,
    pub taper: Vec<f32>,
    pub receiver_is_outlet: Vec<bool>,
    /// The class factor of the sweep of each member (1 without drivers).
    pub scale: Vec<f64>,
    /// The uplift of a step at each member before the taper: `u_over_k k_dt class`, or `k_dt U` with an uplift per cell.
    pub up_unit: Vec<f64>,
    /// The catchment's outlet is a window cut (the caller set `CarveParams::window_cut` and the outlet is on the border of the grid): its height is no base for the breach (GEN-172).
    pub exit_window_cut: bool,
}

/// What a carve reads besides the heights (the drivers of `carve_rock` without a layered rock): the class factor, the uplift per cell (the ratio `U / K` at K 1) and the runoff weights.
#[derive(Clone, Copy, Debug, Default)]
pub struct Drivers<'a> {
    pub k_scale: Option<&'a [f64]>,
    pub uplift: Option<&'a [f32]>,
    pub runoff: Option<&'a [f32]>,
}

/// The breach and `steps` sweeps of one catchment of the round (the loops of `carve_rock` without layers), on the inputs alone.
pub fn catchment_round(inp: &mut CatchmentInputs, p: &CarveParams, steps: usize) {
    let window_cut = inp.exit_window_cut;
    let floor = p.sea_level + p.epsilon;
    let z = &mut inp.z;
    let base = if p.round_rules.clip_mfd != super::route::MfdClip::Local {
        None
    } else {
        super::carve::exit_base(p.sea_level, 0..z.len(), &inp.receiver, z, &|_| window_cut)
    };
    let mut round_start: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
    for c in (0..z.len()).rev() {
        let rec = inp.receiver[c];
        if rec == NONE || inp.receiver_is_outlet[c] {
            continue;
        }
        let rec = rec as usize;
        let mut want = (z[c] - p.breach_slope * inp.dist[c]).max(floor);
        if let Some(b) = &base {
            want = want.max(b[rec]);
        }
        if z[rec] > want {
            let start = *round_start.entry(rec).or_insert(z[rec]);
            z[rec] = want.max(z[rec] - breach_limit(p, z[rec])).max(start - breach_limit(p, start));
        }
    }
    for _ in 0..steps {
        for c in 0..z.len() {
            let rec = inp.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            let f = p.k_dt * inp.scale[c] * libm::sqrt(inp.area[c]) / inp.dist[c];
            let hr = z[rec];
            let t = f64::from(inp.taper[c]);
            let up = inp.up_unit[c] * t;
            let hn = (z[c] + up + f * hr) / (1.0 + f);
            let new = if p.keep_pits && !p.incise_only && z[c] < hr { (z[c] + up).max(floor) } else { hn.max(hr + p.epsilon).max(floor) };
            z[c] = if p.incise_only { new.min(z[c]) } else { new };
        }
    }
}

/// One round's routing cut into catchments: the members of each in the region order.
pub struct Catchments {
    pub members: Vec<Vec<u32>>,
    /// The position of each cell in its catchment's list.
    pub local: Vec<u32>,
}

/// Cuts a routing into catchments (one per outlet, in the order the outlets appear in the region order).
pub fn cut(r: &Routing) -> Catchments {
    cut_in(None, r)
}

/// [`cut`] over the cells that exist on `grid` (a masked grid, a landmass of the cube: the cells outside the mask are in no catchment).
pub fn cut_in(grid: Option<&Grid>, r: &Routing) -> Catchments {
    let root = catchment_roots(&r.receiver, &r.order);
    let mut idx = vec![u32::MAX; r.receiver.len()];
    let mut members: Vec<Vec<u32>> = Vec::new();
    let mut local = vec![0u32; r.receiver.len()];
    for &c in &r.order {
        let c = c as usize;
        if grid.is_some_and(|g| !g.is_active(c)) {
            continue;
        }
        if r.receiver[c] == NONE {
            idx[c] = members.len() as u32;
            members.push(Vec::new());
        }
        let k = idx[root[c] as usize] as usize;
        local[c] = members[k].len() as u32;
        members[k].push(c as u32);
    }
    Catchments { members, local }
}

/// The inputs of catchment `k` read from the region's state, without drivers (`p` gives the uniform uplift of a step).
#[allow(clippy::too_many_arguments)]
pub fn gather(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    r: &Routing,
    cuts: &Catchments,
    taper: &[f32],
    p: &CarveParams,
    k: usize,
) -> CatchmentInputs {
    gather_with(grid, z, outlet, r, cuts, taper, &Drivers::default(), p, k)
}

/// The inputs of catchment `k` with the drivers (`p` gives the uniform uplift `u_over_k k_dt` of a cell without an uplift field).
#[allow(clippy::too_many_arguments)]
pub fn gather_with(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    r: &Routing,
    cuts: &Catchments,
    taper: &[f32],
    drivers: &Drivers,
    p: &CarveParams,
    k: usize,
) -> CatchmentInputs {
    let cells = &cuts.members[k];
    let uplift = p.u_over_k * p.k_dt;
    let mut inp = CatchmentInputs {
        z: Vec::with_capacity(cells.len()),
        receiver: Vec::with_capacity(cells.len()),
        area: Vec::with_capacity(cells.len()),
        dist: Vec::with_capacity(cells.len()),
        taper: Vec::with_capacity(cells.len()),
        receiver_is_outlet: Vec::with_capacity(cells.len()),
        scale: Vec::with_capacity(cells.len()),
        up_unit: Vec::with_capacity(cells.len()),
        exit_window_cut: false,
    };
    for &c in cells {
        let c = c as usize;
        let rec = r.receiver[c];
        let class = drivers.k_scale.map_or(1.0, |f| f[c]);
        inp.z.push(z[c]);
        inp.area.push(r.area[c]);
        inp.taper.push(taper.get(c).copied().unwrap_or(1.0));
        inp.scale.push(class);
        inp.up_unit.push(match drivers.uplift {
            Some(u) => p.k_dt * f64::from(u[c]),
            None => uplift * class,
        });
        if rec == NONE {
            inp.receiver.push(NONE);
            inp.exit_window_cut |= p.window_cut && grid.is_edge(c) && z[c] > p.sea_level;
            inp.dist.push(1.0);
            inp.receiver_is_outlet.push(false);
        } else {
            inp.receiver.push(cuts.local[rec as usize]);
            inp.dist.push(distance(grid, c, rec as usize));
            inp.receiver_is_outlet.push(outlet[rec as usize]);
        }
    }
    inp
}

/// What a solve measured.
#[derive(Clone, Debug, Default)]
pub struct RoundReport {
    pub rounds: Vec<RoundStats>,
    /// The round-0 breach of the basins (GEN-174), when the solve ran it: the passes, the basins drained and kept, and whether the fixed point was reached (false: the cap was hit, reported in the MARK line).
    pub pre_breach: Option<super::breach::BreachReport>,
}

#[derive(Clone, Debug, Default)]
pub struct RoundStats {
    pub catchments: usize,
    pub largest: usize,
    /// Cells in catchments of at least 1,000 cells.
    pub in_large: usize,
}

/// The taper of the uplift by the height above the sea at the start of the solve (as `carve`), empty when the carve has none.
fn taper_of(z: &[f64], p: &CarveParams, drivers: &Drivers) -> Vec<f32> {
    if p.uplift_taper_m > 0.0 && (p.u_over_k != 0.0 || drivers.uplift.is_some()) {
        z.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect()
    } else {
        Vec::new()
    }
}

fn check(grid: &Grid, z: &[f64], p: &CarveParams, rounds: usize, drivers: &Drivers) {
    assert!(p.reroute_every > 0 && rounds > 0, "a solve has steps per round and rounds");
    assert_eq!(p.steps, rounds * p.reroute_every, "the steps of the carve are the rounds times the steps of a round");
    assert_eq!(z.len(), grid.len(), "one height per cell");
    assert!(p.max_area == 0.0, "the round solver does not cap the area");
    assert_finite("the heights", z);
    if let Some(f) = drivers.k_scale {
        assert!(f.len() == grid.len() && f.iter().all(|v| v.is_finite() && *v >= 0.0), "one finite class factor, not negative, per cell");
    }
    if let Some(u) = drivers.uplift {
        assert!(u.len() == grid.len() && u.iter().all(|v| v.is_finite() && *v >= 0.0), "one finite uplift, not negative, per cell");
    }
    if let Some(w) = drivers.runoff {
        assert!(w.len() == grid.len() && w.iter().all(|v| v.is_finite() && *v > 0.0), "one finite positive runoff weight per cell");
    }
}

/// `rounds` rounds of `p.reroute_every` steps on `z` (`p.steps` is `rounds * p.reroute_every`): each round one region-wide routing under `p.round_rules`, then every catchment's round
/// from its inputs on `par` (the largest first; CON-04). Equal to `carve` with the same `p`, bit for bit. The taper of the uplift is taken from the heights at the start of the call:
/// one call is one carve. Not for a layered rock.
#[allow(clippy::too_many_arguments)]
pub fn solve_rounds(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &CarveParams, rounds: usize, par: &dyn Par, seed: u64) -> RoundReport {
    solve_rounds_with(grid, z, outlet, p, rounds, par, seed, &Drivers::default())
}

/// [`solve_rounds`] with drivers: equal to `carve_rock` with the class factor `k_scale`, a rock with `uplift` and `runoff` and no layers.
#[allow(clippy::too_many_arguments)]
pub fn solve_rounds_with(
    grid: &Grid,
    z: &mut [f64],
    outlet: &[bool],
    p: &CarveParams,
    rounds: usize,
    par: &dyn Par,
    seed: u64,
    drivers: &Drivers,
) -> RoundReport {
    check(grid, z, p, rounds, drivers);
    let mut report = RoundReport::default();
    if let Some(pb) = &p.pre_breach {
        report.pre_breach = Some(super::breach::breach_basins(grid, z, outlet, p.epsilon, pb));
    }
    let taper = taper_of(z, p, drivers);
    for round in 0..rounds {
        let r = route_rules(grid, z, outlet, p.epsilon, seed, p.max_area, drivers.runoff, p.round_rules);
        let cuts = cut_in(Some(grid), &r);
        // A lone outlet keeps its height, so it is not queued.
        let mut queue: Vec<usize> = (0..cuts.members.len()).filter(|&k| cuts.members[k].len() > 1).collect();
        queue.sort_by_key(|&k| (std::cmp::Reverse(cuts.members[k].len()), k));
        let zr: &[f64] = z;
        let done = par::map(par, queue.len(), &|q| {
            let mut inp = gather_with(grid, zr, outlet, &r, &cuts, &taper, drivers, p, queue[q]);
            catchment_round(&mut inp, p, p.reroute_every);
            inp.z
        });
        for (&k, zk) in queue.iter().zip(done) {
            for (&c, v) in cuts.members[k].iter().zip(zk) {
                z[c as usize] = v;
            }
        }
        if p.round_f32 {
            super::carve::narrow_f32_checked(z, p.epsilon);
        }
        report.rounds.push(RoundStats {
            catchments: cuts.members.len(),
            largest: cuts.members.iter().map(Vec::len).max().unwrap_or(0),
            in_large: cuts.members.iter().filter(|m| m.len() >= 1000).map(Vec::len).sum(),
        });
        // The stage probe (BUILD-046, GEN-161): the heights after the round and the area of the routing it ran on; read only.
        let (zz, areas, n_catch, largest): (&[f64], &[f64], usize, usize) =
            (z, &r.area, cuts.members.len(), report.rounds.last().map_or(0, |x| x.largest));
        probe::emit(probe::Stage::Round(round as u32), |s| {
            probe::StageView::new(s, grid)
                .f64("z", probe::FieldKind::Height, zz)
                .f64("area", probe::FieldKind::Area, areas)
                .number("catchments", n_catch as f64)
                .number("largest", largest as f64)
        });
    }
    report
}

/// What a series of memoised solves keeps: the memo of the tile floods and the previous routing (GEN-143) and the memo of the catchment rounds by their key inputs.
#[derive(Default)]
pub struct RoundCache {
    pub part: PartCache,
    forget: bool,
    catchments: Memo,
    /// The budget in bytes of the whole cache, the tile memo and the previous routings included (GEN-153); none is unlimited.
    budget: Option<usize>,
    /// While a solve runs: the tick of the catchment memo from which its entries are pinned.
    pin: Option<u64>,
}

impl RoundCache {
    /// A cache that keeps nothing between solves (GEN-149): the partitioned rounds of a build that is not edited again, at the memory of one solve.
    pub fn without_memory() -> RoundCache {
        RoundCache { part: PartCache::without_memory(), forget: true, ..RoundCache::default() }
    }

    /// A cache with a budget of `bytes` for everything it holds (GEN-153, A3 6.6): after every round of a solve and at its end what is over it is evicted, previous results first, then the tile
    /// floods, then the catchment rounds, each least recently used first, never the data of the running solve. Eviction never changes a result.
    pub fn with_budget(bytes: usize) -> RoundCache {
        RoundCache { budget: Some(bytes), ..RoundCache::default() }
    }

    pub fn set_budget(&mut self, bytes: Option<usize>) {
        self.budget = bytes;
    }

    /// The bytes held: the tile floods, the previous routings and the catchment rounds.
    pub fn bytes(&self) -> usize {
        self.part.bytes() + self.catchments.bytes()
    }

    /// Evicts down to the budget; returns the entries and bytes evicted.
    fn evict(&mut self) -> (usize, usize) {
        let Some(budget) = self.budget else { return (0, 0) };
        // The part cache gets the budget minus what the catchment memo holds: it evicts its own first, then the catchment memo takes what is left over.
        let (mut e, mut b) = self.part.evict_to(budget.saturating_sub(self.catchments.bytes()), None, self.part.tick());
        let rest = budget.saturating_sub(self.part.bytes());
        let (e2, b2) = self.catchments.evict(rest, self.pin.unwrap_or(u64::MAX));
        e += e2;
        b += b2;
        (e, b)
    }

    /// Catchment rounds held.
    pub fn len(&self) -> usize {
        self.catchments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.catchments.is_empty()
    }
}

/// The work of one round of a memoised solve (G-INC).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoundWork {
    /// Tiles, tile floods run and answered by the memo, and the tiles whose filled surface, receivers and area changed against the previous call (GEN-143).
    pub tiles: usize,
    pub tile_floods: usize,
    pub tile_memo_hits: usize,
    pub tiles_filled_changed: usize,
    pub tiles_receiver_changed: usize,
    pub tiles_area_changed: usize,
    /// Catchments of more than one cell, those carved (a key not seen) and those answered by the memo, and the cells of the carved ones.
    pub catchments: usize,
    pub carved: usize,
    pub memo_hits: usize,
    pub cells_carved: usize,
    /// Entries and bytes the budget evicted after the round (GEN-153).
    pub evicted: usize,
    pub evicted_bytes: usize,
    /// Whether the round's routing had a previous result to compare with (see `PartStats::baseline`).
    pub baseline: bool,
}

/// The key of a catchment round: the member cell ids, the key inputs and every parameter of the carve that `catchment_round` reads: `k_dt`, `epsilon`, `breach_slope`, `breach_max_m`, `breach_height_scale_m`,
/// `sea_level`, `keep_pits`, `incise_only`, the sweeps per round, whether the round rules clip locally (`clip_mfd`, GEN-162: it changes the breach) and whether the catchment's outlet is a window cut (`exit_window_cut`, GEN-172: it removes the base of the breach). The memo is in memory only: no stored key.
pub fn catchment_key(grid: &Grid, cells: &[u32], inp: &CatchmentInputs, p: &CarveParams) -> (u64, u64) {
    let mut h = Hash::new();
    for v in [p.k_dt, p.u_over_k, p.epsilon, p.breach_slope, p.breach_max_m, p.breach_height_scale_m, p.sea_level] {
        h.word(v.to_bits());
    }
    h.word(
        u64::from(p.keep_pits)
            | (u64::from(p.incise_only) << 1)
            | ((p.reroute_every as u64) << 2)
            | (u64::from(p.round_rules.clip_mfd == MfdClip::Local) << 63)
            | (u64::from(inp.exit_window_cut) << 62)
            | (u64::from(p.round_f32) << 61),
    );
    h.word(cells.len() as u64);
    for (k, &c) in cells.iter().enumerate() {
        h.word(grid.global_id(c as usize));
        h.word(inp.z[k].to_bits());
        h.word(u64::from(inp.receiver[k]) | (u64::from(inp.receiver_is_outlet[k]) << 32) | (u64::from(inp.taper[k].to_bits()) << 33));
        h.word(inp.area[k].to_bits());
        h.word(inp.dist[k].to_bits());
        h.word(inp.scale[k].to_bits());
        h.word(inp.up_unit[k].to_bits());
    }
    h.key()
}

/// [`solve_rounds_with`] with the memo of A3 section 6.5: each round's routing is the partitioned route (`tile` cells a side) with memoised tile floods, and a catchment whose key (members,
/// receivers, `A_d`, surface, drivers, parameters) was seen is not carved again. The result is the bits of the solve of the same inputs without a cache (G-EQ); the work is reported per round (G-INC).
/// Needs the canonical flood.
#[allow(clippy::too_many_arguments)]
pub fn solve_rounds_memo(
    grid: &Grid,
    z: &mut [f64],
    outlet: &[bool],
    p: &CarveParams,
    rounds: usize,
    par: &dyn Par,
    seed: u64,
    drivers: &Drivers,
    tile: usize,
    cache: &mut RoundCache,
) -> (RoundReport, Vec<RoundWork>) {
    check(grid, z, p, rounds, drivers);
    assert!(p.round_rules.canonical_flood, "the memoised solve routes in tiles: the canonical flood is required");
    let (mut report, mut work) = (RoundReport::default(), Vec::new());
    if let Some(pb) = &p.pre_breach {
        report.pre_breach = Some(super::breach::breach_basins(grid, z, outlet, p.epsilon, pb));
    }
    let taper = taper_of(z, p, drivers);
    // The data this solve uses is pinned until it completes (GEN-153).
    cache.part.begin_solve();
    cache.pin = Some(cache.catchments.tick() + 1);
    for round in 0..rounds {
        cache.part.use_slot(round);
        let route = partitioned_route_weighted(grid, z, outlet, p.epsilon, seed, tile, p.round_rules, drivers.runoff, &mut cache.part);
        let (r, ps) = (route.routing, route.stats);
        let cuts = cut_in(Some(grid), &r);
        let mut queue: Vec<usize> = (0..cuts.members.len()).filter(|&k| cuts.members[k].len() > 1).collect();
        queue.sort_by_key(|&k| (std::cmp::Reverse(cuts.members[k].len()), k));
        let zr: &[f64] = z;
        let held = &cache.catchments;
        let done = par::map(par, queue.len(), &|q| {
            let k = queue[q];
            let mut inp = gather_with(grid, zr, outlet, &r, &cuts, &taper, drivers, p, k);
            let key = catchment_key(grid, &cuts.members[k], &inp, p);
            match held.peek(&key).filter(|v| v.len() == inp.z.len()) {
                Some(v) => (key, Arc::clone(v), true),
                None => {
                    catchment_round(&mut inp, p, p.reroute_every);
                    (key, Arc::new(inp.z), false)
                }
            }
        });
        let mut w = RoundWork {
            tiles: ps.tiles,
            tile_floods: ps.floods,
            tile_memo_hits: ps.memo_hits,
            tiles_filled_changed: ps.tiles_filled_changed,
            tiles_receiver_changed: ps.tiles_receiver_changed,
            tiles_area_changed: ps.tiles_area_changed,
            baseline: ps.baseline,
            catchments: queue.len(),
            ..RoundWork::default()
        };
        for (&k, (key, zk, hit)) in queue.iter().zip(done) {
            for (&c, &v) in cuts.members[k].iter().zip(zk.iter()) {
                z[c as usize] = v;
            }
            if hit {
                w.memo_hits += 1;
                cache.catchments.touch(&key);
            } else {
                w.carved += 1;
                w.cells_carved += cuts.members[k].len();
                if !cache.forget {
                    cache.catchments.insert(key, zk);
                }
            }
        }
        if p.round_f32 {
            super::carve::narrow_f32_checked(z, p.epsilon);
        }
        let (e, b) = cache.evict();
        (w.evicted, w.evicted_bytes) = (w.evicted + e, w.evicted_bytes + b);
        work.push(w);
        report.rounds.push(RoundStats {
            catchments: cuts.members.len(),
            largest: cuts.members.iter().map(Vec::len).max().unwrap_or(0),
            in_large: cuts.members.iter().filter(|m| m.len() >= 1000).map(Vec::len).sum(),
        });
        // The stage probe (BUILD-046, GEN-161): the heights after the round and the area of its routing; read only.
        let (zz, areas, n_catch, largest): (&[f64], &[f64], usize, usize) =
            (z, &r.area, cuts.members.len(), report.rounds.last().map_or(0, |x| x.largest));
        probe::emit(probe::Stage::Round(round as u32), |s| {
            probe::StageView::new(s, grid)
                .f64("z", probe::FieldKind::Height, zz)
                .f64("area", probe::FieldKind::Area, areas)
                .number("catchments", n_catch as f64)
                .number("largest", largest as f64)
        });
    }
    cache.part.use_slot(0);
    // The solve is complete: nothing is pinned, and what is over the budget goes.
    cache.part.end_solve();
    cache.pin = None;
    let (e, b) = cache.evict();
    if let Some(w) = work.last_mut() {
        (w.evicted, w.evicted_bytes) = (w.evicted + e, w.evicted_bytes + b);
    }
    (report, work)
}
