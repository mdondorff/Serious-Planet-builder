//! The transient carve of pass 1 (GEN-100, SPIKE-65, SPIKE-66, SPIKE-70): implicit stream power with uplift (Braun and Willett 2013), with a breach before each re-route.
//! Not run to steady state: a fixed number of steps, `u_over_k * k_dt` of uplift in each.

use super::route::{route, Routing, NONE};
use super::Grid;

/// The parameters of the carve (start values of SPIKE-65 and SPIKE-70; the spec holds them, R7 tunes them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarveParams {
    /// `U / K`, with `K dt` the uplift of a step is `u_over_k * k_dt` metres.
    pub u_over_k: f64,
    /// `K dt` per step, in the units of `A^0.5 / dx` with the area in square metres and the distance in metres.
    pub k_dt: f64,
    pub steps: usize,
    /// The surface is re-routed every this many steps (and before the first).
    pub reroute_every: usize,
    /// The epsilon of the flood in metres (1 mm over flats).
    pub epsilon: f64,
    /// A breach lowers a receiver to this fraction of the distance below its donor (0.1 %) ...
    pub breach_slope: f64,
    /// ... by at most this many metres.
    pub breach_max_m: f64,
}

impl Default for CarveParams {
    fn default() -> CarveParams {
        CarveParams { u_over_k: 20.0, k_dt: 0.05, steps: 20, reroute_every: 5, epsilon: 1e-3, breach_slope: 1e-3, breach_max_m: 50.0 }
    }
}

/// Carves `z` in place: `outlet` cells keep their heights; `factor` (per cell, GEN-097's carve factor) multiplies `k_dt` when given.
/// Returns the last routing. Before each re-route a breach lowers, from the leaves down, the receiver of a cell that stands higher than its donor allows
/// (never an outlet), by at most `breach_max_m`, so that a ridge across a valley does not hold a lake the flood would fill.
pub fn carve(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &CarveParams, factor: Option<&[f64]>, seed: u64) -> Routing {
    assert!(p.steps > 0 && p.reroute_every > 0, "a carve has steps and a re-route interval");
    if let Some(f) = factor {
        assert_eq!(f.len(), grid.len(), "one carve factor per cell");
    }
    let uplift = p.u_over_k * p.k_dt;
    let mut routing: Option<Routing> = None;
    for step in 0..p.steps {
        if step % p.reroute_every == 0 {
            let r = route(grid, z, outlet, p.epsilon, seed, 0.0);
            for &c in r.order.iter().rev() {
                let c = c as usize;
                let rec = r.receiver[c];
                if rec == NONE || outlet[rec as usize] {
                    continue;
                }
                let rec = rec as usize;
                let want = z[c] - p.breach_slope * distance(grid, c, rec);
                if z[rec] > want {
                    z[rec] = want.max(z[rec] - p.breach_max_m);
                }
            }
            routing = Some(r);
        }
        let r = routing.as_ref().expect("routed at the first step");
        for &c in &r.order {
            let c = c as usize;
            let rec = r.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            let k_dt = p.k_dt * factor.map_or(1.0, |f| f[c]);
            let f = k_dt * libm::sqrt(r.area[c]) / distance(grid, c, rec);
            let hr = z[rec];
            let hn = (z[c] + uplift * factor.map_or(1.0, |f| f[c]) + f * hr) / (1.0 + f);
            z[c] = hn.max(hr + p.epsilon);
        }
    }
    routing.expect("a carve has at least one step")
}

/// The distance between two neighbouring cells.
fn distance(grid: &Grid, a: usize, b: usize) -> f64 {
    let (ai, aj, bi, bj) = (a % grid.w, a / grid.w, b % grid.w, b / grid.w);
    if ai == bi || aj == bj {
        grid.dx
    } else {
        grid.dx * core::f64::consts::SQRT_2
    }
}
