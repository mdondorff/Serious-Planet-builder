//! The transient carve of pass 1 (GEN-100, SPIKE-65, SPIKE-66, SPIKE-70): implicit stream power with uplift (Braun and Willett 2013), with a breach before each re-route.
//! Not run to steady state: a fixed number of steps, `u_over_k * k_dt` of uplift in each.

use super::route::{route, Routing, NONE};
use super::{assert_finite, Grid};

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
    /// ... by at most this many metres (500: a ridge's basins are breached, not filled, so the carve keeps its relief; SPIKE-101).
    pub breach_max_m: f64,
    /// Height scaling of the breach limit, off at 0: with a value above 0 the limit is `breach_min_m` at the sea level and rises to `breach_max_m` over this many metres of height
    /// above it (`smoothstep`), so plains keep the small limit. The height of the receiver stands in for the local relief.
    pub breach_height_scale_m: f64,
    /// The sea level: land never falls below it plus `epsilon`, whatever depth the outlets have (a sea outlet is a base level at the sea, not at its floor).
    pub sea_level: f64,
    /// The drainage area is capped at this many square metres (0: no cap): the local carve of pass 2 incises catchments up to 1 km^2 only (GEN-101).
    pub max_area: f64,
    /// The carve only lowers: a cell never ends higher than it began (no fill under a pit, GEN-101). With `u_over_k` 0 and `breach_max_m` 0 it is the local carve.
    pub incise_only: bool,
    /// The uplift is tapered by the height above the sea level at the start of the carve: a cell `h` metres above it gets `smoothstep(0, uplift_taper_m, h)` of the uplift, so a coastal plain
    /// that barely erodes is not lifted by the full uplift (SPIKE-99 and the coast diagnosis: 14 m over the whole plain); 0 switches the taper off (the whole uplift everywhere).
    pub uplift_taper_m: f64,
}

/// The breach limit of a cell at the sea level when the limit scales with height (the former default).
pub const BREACH_MIN_M: f64 = 50.0;

impl Default for CarveParams {
    fn default() -> CarveParams {
        CarveParams {
            u_over_k: 20.0,
            k_dt: 0.05,
            steps: 20,
            reroute_every: 5,
            epsilon: 1e-3,
            breach_slope: 1e-3,
            breach_max_m: 500.0,
            breach_height_scale_m: 0.0,
            sea_level: 0.0,
            max_area: 0.0,
            incise_only: false,
            uplift_taper_m: 60.0,
        }
    }
}

/// Carves `z` in place: `outlet` cells keep their heights; `factor` (per cell, finite and not negative, GEN-097's carve factor) multiplies `K dt` and so the uplift
/// `U dt = (U / K) K dt` of the cell (the ratio `U / K` stays). Returns the last routing. After each route a breach lowers, from the leaves down, the receiver of a cell that
/// stands higher than its donor allows (never an outlet, never below the sea level), by at most `breach_max_m`, so that a ridge across a valley does not hold a lake the flood
/// would fill; the next flood sees the breached surface.
pub fn carve(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &CarveParams, factor: Option<&[f64]>, seed: u64) -> Routing {
    assert!(p.steps > 0 && p.reroute_every > 0, "a carve has steps and a re-route interval");
    assert!(p.max_area >= 0.0 && p.max_area.is_finite(), "the area cap is finite and not negative, not {}", p.max_area);
    assert_finite("the heights", z);
    if let Some(f) = factor {
        assert_eq!(f.len(), grid.len(), "one carve factor per cell");
        assert!(f.iter().all(|v| v.is_finite() && *v >= 0.0), "the carve factors are finite and not negative");
    }
    let floor = p.sea_level + p.epsilon;
    let uplift = p.u_over_k * p.k_dt;
    // The taper of the uplift by the starting height above the sea, kept per cell as an f32 (a face of 1.6e8 cells would not fit another f64 array in its budget).
    let taper: Option<Vec<f32>> = (p.uplift_taper_m > 0.0 && p.u_over_k != 0.0)
        .then(|| z.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect());
    let mut routing: Option<Routing> = None;
    for step in 0..p.steps {
        if step % p.reroute_every == 0 {
            // The previous routing is dropped before the next is built: two live routings of a face would not fit its memory budget.
            drop(routing.take());
            let r = route(grid, z, outlet, p.epsilon, seed, p.max_area);
            for &c in r.order.iter().rev() {
                let c = c as usize;
                let rec = r.receiver[c];
                if rec == NONE || outlet[rec as usize] {
                    continue;
                }
                let rec = rec as usize;
                let want = (z[c] - p.breach_slope * distance(grid, c, rec)).max(floor);
                if z[rec] > want {
                    let limit = if p.breach_height_scale_m > 0.0 {
                        BREACH_MIN_M
                            + (p.breach_max_m - BREACH_MIN_M).max(0.0)
                                * planet_core::smoothstep(0.0, p.breach_height_scale_m, z[rec] - p.sea_level)
                    } else {
                        p.breach_max_m
                    };
                    z[rec] = want.max(z[rec] - limit);
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
            let scale = factor.map_or(1.0, |f| f[c]);
            let f = p.k_dt * scale * libm::sqrt(r.area[c]) / distance(grid, c, rec);
            let hr = z[rec];
            let up = uplift * scale * taper.as_ref().map_or(1.0, |t| f64::from(t[c]));
            let hn = (z[c] + up + f * hr) / (1.0 + f);
            let new = hn.max(hr + p.epsilon).max(floor);
            z[c] = if p.incise_only { new.min(z[c]) } else { new };
        }
    }
    routing.expect("a carve has at least one step")
}

/// The distance between two neighbouring cells.
fn distance(grid: &Grid, a: usize, b: usize) -> f64 {
    let (ai, aj, bi, bj) = (a % grid.w(), a / grid.w(), b % grid.w(), b / grid.w());
    if ai == bi || aj == bj {
        grid.dx()
    } else {
        grid.dx() * core::f64::consts::SQRT_2
    }
}
