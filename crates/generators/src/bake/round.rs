//! The round solver (GEN-142, A3 section 6.2): one region-wide routing under the round rules (GEN-141), then the breach and the implicit sweeps of every catchment from that
//! catchment's key inputs alone, on any thread, in any order. The result is the bits of the whole-region carve (`carve` with the same rules, G-ROUND), on any thread count (G-DET).
//! Behind a switch: nothing in pass 1 or pass 2 calls it.

use super::carve::{distance, CarveParams, BREACH_MIN_M};
use super::route::{catchment_roots, route_rules, Routing, NONE};
use super::{assert_finite, Grid};
use planet_core::par::{self, Par};

/// What a catchment's round reads (A3 section 6.5, the catchment-round key): its members in the region's topological order (outlet first), the receiver of every member as a
/// position in that list, `A_d`, the distance to the receiver, the taper of the uplift, whether the receiver is an outlet of the region, and the surface at the start of the round.
#[derive(Clone, Debug, PartialEq)]
pub struct CatchmentInputs {
    pub z: Vec<f64>,
    pub receiver: Vec<u32>,
    pub area: Vec<f64>,
    pub dist: Vec<f64>,
    pub taper: Vec<f32>,
    pub receiver_is_outlet: Vec<bool>,
}

/// The breach and `steps` sweeps of one catchment of the round (the loops of `carve_rock` without rock, class factor and layers), on the inputs alone.
pub fn catchment_round(inp: &mut CatchmentInputs, p: &CarveParams, steps: usize) {
    let floor = p.sea_level + p.epsilon;
    let uplift = p.u_over_k * p.k_dt;
    let z = &mut inp.z;
    for c in (0..z.len()).rev() {
        let rec = inp.receiver[c];
        if rec == NONE || inp.receiver_is_outlet[c] {
            continue;
        }
        let rec = rec as usize;
        let want = (z[c] - p.breach_slope * inp.dist[c]).max(floor);
        if z[rec] > want {
            let limit = if p.breach_height_scale_m > 0.0 {
                p.breach_max_m.min(BREACH_MIN_M)
                    + (p.breach_max_m - BREACH_MIN_M).max(0.0) * planet_core::smoothstep(0.0, p.breach_height_scale_m, z[rec] - p.sea_level)
            } else {
                p.breach_max_m
            };
            z[rec] = want.max(z[rec] - limit);
        }
    }
    for _ in 0..steps {
        for c in 0..z.len() {
            let rec = inp.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            let f = p.k_dt * libm::sqrt(inp.area[c]) / inp.dist[c];
            let hr = z[rec];
            let t = f64::from(inp.taper[c]);
            let up = uplift * t;
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

/// The inputs of catchment `k` read from the region's state.
#[allow(clippy::too_many_arguments)]
pub fn gather(grid: &Grid, z: &[f64], outlet: &[bool], r: &Routing, cuts: &Catchments, taper: &[f32], k: usize) -> CatchmentInputs {
    let cells = &cuts.members[k];
    let mut inp = CatchmentInputs {
        z: Vec::with_capacity(cells.len()),
        receiver: Vec::with_capacity(cells.len()),
        area: Vec::with_capacity(cells.len()),
        dist: Vec::with_capacity(cells.len()),
        taper: Vec::with_capacity(cells.len()),
        receiver_is_outlet: Vec::with_capacity(cells.len()),
    };
    for &c in cells {
        let c = c as usize;
        let rec = r.receiver[c];
        inp.z.push(z[c]);
        inp.area.push(r.area[c]);
        inp.taper.push(taper.get(c).copied().unwrap_or(1.0));
        if rec == NONE {
            inp.receiver.push(NONE);
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
}

#[derive(Clone, Debug, Default)]
pub struct RoundStats {
    pub catchments: usize,
    pub largest: usize,
    /// Cells in catchments of at least 1,000 cells.
    pub in_large: usize,
}

/// The taper of the uplift by the height above the sea at the start of the solve (as `carve`), empty when the carve has none.
fn taper_of(z: &[f64], p: &CarveParams) -> Vec<f32> {
    if p.uplift_taper_m > 0.0 && p.u_over_k != 0.0 {
        z.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect()
    } else {
        Vec::new()
    }
}

/// `rounds` rounds of `p.reroute_every` steps on `z` (`p.steps` is `rounds * p.reroute_every`): each round one region-wide routing under `p.round_rules`, then every catchment's round
/// from its inputs on `par` (the largest first; CON-04). Equal to `carve` with the same `p`, bit for bit. The taper of the uplift is taken from the heights at the start of the call:
/// one call is one carve. Not for a grid with a rock layer, a class factor or a runoff weight (the prototype's solver has none yet).
#[allow(clippy::too_many_arguments)]
pub fn solve_rounds(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &CarveParams, rounds: usize, par: &dyn Par, seed: u64) -> RoundReport {
    assert!(p.reroute_every > 0 && rounds > 0, "a solve has steps per round and rounds");
    assert_eq!(p.steps, rounds * p.reroute_every, "the steps of the carve are the rounds times the steps of a round");
    assert_eq!(z.len(), grid.len(), "one height per cell");
    assert!(p.max_area == 0.0, "the round solver does not cap the area");
    assert_finite("the heights", z);
    let taper = taper_of(z, p);
    let mut report = RoundReport::default();
    for _ in 0..rounds {
        let r = route_rules(grid, z, outlet, p.epsilon, seed, p.max_area, None, p.round_rules);
        let cuts = cut_in(Some(grid), &r);
        // A lone outlet keeps its height, so it is not queued.
        let mut queue: Vec<usize> = (0..cuts.members.len()).filter(|&k| cuts.members[k].len() > 1).collect();
        queue.sort_by_key(|&k| (std::cmp::Reverse(cuts.members[k].len()), k));
        let zr: &[f64] = z;
        let done = par::map(par, queue.len(), &|q| {
            let mut inp = gather(grid, zr, outlet, &r, &cuts, &taper, queue[q]);
            catchment_round(&mut inp, p, p.reroute_every);
            inp.z
        });
        for (&k, zk) in queue.iter().zip(done) {
            for (&c, v) in cuts.members[k].iter().zip(zk) {
                z[c as usize] = v;
            }
        }
        report.rounds.push(RoundStats {
            catchments: cuts.members.len(),
            largest: cuts.members.iter().map(Vec::len).max().unwrap_or(0),
            in_large: cuts.members.iter().filter(|m| m.len() >= 1000).map(Vec::len).sum(),
        });
    }
    report
}
