//! The transient carve of pass 1 (GEN-100, SPIKE-65, SPIKE-66, SPIKE-70): implicit stream power with uplift (Braun and Willett 2013), with a breach before each re-route.
//! Not run to steady state: a fixed number of steps, `u_over_k * k_dt` of uplift in each.

use super::route::{catchment_roots, route_rules, RoundRules, Routing, NONE};
use super::{assert_finite, Grid};
use planet_core::par::{self, Par};

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
    /// Height scaling of the breach limit, off at 0: with a value above 0 the limit is `BREACH_MIN_M` at the sea level and rises to `breach_max_m` over this many metres of height
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
    /// Keep the pits (GEN-120): the route runs over the filled copy, but a cell that stands below its receiver (a pit of the unfilled surface) is not raised to it; it only gains its uplift,
    /// so the donors above it carve toward its floor and the basin keeps the relief the fill would have eaten. An incise-only carve never raises, so it ignores the flag.
    pub keep_pits: bool,
    /// The physical cell metric of the face grid (GEN-138, SPIKE-105 F5): true cell areas and neighbour distances instead of the uniform `dx^2`; off (today's metric) by default.
    pub cell_metric: bool,
    /// The round kernels (GEN-141): the clipped MFD area and the canonical flood tie rule; off (today's kernels) by default.
    pub round_rules: RoundRules,
    /// The land outlets on the border of the grid are window cuts (GEN-172): the region goes on beyond them at an unknown, lower level, so the exit height is no base for a breach (`exit_base` skips their chains). An outlet in the interior (a basin's rim) keeps its base. Set by the caller that solves a window; off by default (every land outlet keeps its base).
    pub window_cut: bool,
    /// The round surface is stored f32 (GEN-166): the heights are narrowed to f32 (round to nearest, ties to even) once at the end of every round and read widened by the next round, K6 and K7; the round computes in f64. Off by default (f64 rounds); the callers of the round rules turn it on.
    pub round_f32: bool,
    /// The round-0 breach of the closed basins (GEN-174): one pass before the first round drains each closed basin by the cheapest path to a lower cell within the depth and the length of the profile (`breach::PreBreach`); off (`None`) by default.
    pub pre_breach: Option<super::breach::PreBreach>,
    /// How a round visits the cells (GEN-141): execution only, the result is the same bits (a sweep reads a cell and its receiver, which share a catchment); not part of any key.
    pub schedule: Schedule,
}

/// The order in which a round's breach and sweeps visit the cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Schedule {
    /// The region's topological order, one pass over all catchments.
    #[default]
    Region,
    /// One catchment after the other (each in the region order, catchments by the id of their outlet, ascending or descending): the round solver's per-catchment carve, run in sequence.
    Catchments { descending: bool },
}

/// The breach limit of a cell at the sea level when the limit scales with height (the former default).
pub const BREACH_MIN_M: f64 = 50.0;

/// The most a breach event may lower a cell standing at height `h` (`breach_max_m`, or the height-scaled limit when `breach_height_scale_m` is set).
pub(super) fn breach_limit(p: &CarveParams, h: f64) -> f64 {
    if p.breach_height_scale_m > 0.0 {
        p.breach_max_m.min(BREACH_MIN_M)
            + (p.breach_max_m - BREACH_MIN_M).max(0.0) * planet_core::smoothstep(0.0, p.breach_height_scale_m, h - p.sea_level)
    } else {
        p.breach_max_m
    }
}

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
            keep_pits: true,
            cell_metric: false,
            round_rules: RoundRules { clip_mfd: super::route::MfdClip::Off, canonical_flood: false },
            window_cut: false,
            round_f32: false,
            pre_breach: None,
            schedule: Schedule::Region,
        }
    }
}

/// Carves `z` in place: `outlet` cells keep their heights; `factor` (per cell, finite and not negative, GEN-097's carve factor) multiplies `K dt` and so the uplift
/// `U dt = (U / K) K dt` of the cell (the ratio `U / K` stays). Returns the last routing. After each route a breach lowers, from the leaves down, the receiver of a cell that
/// stands higher than its donor allows (never an outlet, never below the sea level), by at most `breach_max_m`, so that a ridge across a valley does not hold a lake the flood
/// would fill; the next flood sees the breached surface.
pub fn carve(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &CarveParams, factor: Option<&[f64]>, seed: u64) -> Routing {
    carve_rock(grid, z, outlet, p, factor, &Rock::none(), seed)
}

/// What a carve reads besides the class carve factor (GEN-127, ADR 0102 and 0103): the erodibility at a cell and its current height, and the uplift per cell.
#[derive(Clone, Copy)]
pub struct Rock<'a> {
    /// Runs the erodibility evaluation (every cell, once a step).
    pub par: &'a dyn Par,
    /// The rock at the cell's direction and the height `z`: the erodibility K (finite, not negative; it multiplies the class carve factor) and the lower edge of its band, below which a step does not lower the
    /// cell (the rest is carried to the next step, which reads the band below). Absent: K 1 and no edge.
    pub k: Option<&'a (dyn Fn(usize, f64) -> Layer + Sync)>,
    /// The ratio `U / K` at K 1 per cell (GEN-123): when given, a cell's uplift is `k_dt * U * taper`, not scaled by K or by the factor, and `u_over_k` is not read. Finite and not negative.
    pub uplift: Option<&'a [f32]>,
    /// The runoff weight of each cell (GEN-129): the routing starts a cell with the area `w dx^2`, so the drainage area `f` reads is the discharge `Q = sum(w dA)`. Finite and positive.
    pub runoff: Option<&'a [f32]>,
}

/// The rows of the erodibility pass done at once, so that the transient results stay a few rows of the face.
const K_ROWS_AT_ONCE: usize = 64;

/// What a carve reads of the rock at a cell and a height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// The erodibility.
    pub k: f64,
    /// The height of the lower edge of the layer the height lies in (`-inf`: no edge).
    pub floor: f64,
}

impl Layer {
    /// A layer of erodibility `k` without a lower edge.
    pub fn of(k: f64) -> Layer {
        Layer { k, floor: f64::NEG_INFINITY }
    }
}

/// How far below its lower edge a step may lower a cell that reaches the edge: the cell ends inside the layer below (its erodibility blends across the edge), where the next step reads it.
pub const LAYER_SKIP_M: f64 = 0.01;

/// The room above a layer edge is kept as a 16-bit count of `ROOM_STEP_M` (rounded up, so a cell may end up to one step below the skip under the edge): bands are at most 170 m, a room of
/// `ROOM_NONE` steps or more (262 m) means no edge.
const ROOM_STEP_M: f64 = 0.004;
const ROOM_NONE: u16 = u16::MAX;

fn room_code(room_m: f64) -> u16 {
    let steps = libm::ceil(room_m.max(0.0) / ROOM_STEP_M);
    if steps >= f64::from(ROOM_NONE) {
        ROOM_NONE
    } else {
        steps as u16
    }
}

impl Rock<'static> {
    /// No field: the carve is the one of the class factor and the uniform `u_over_k`.
    pub fn none() -> Rock<'static> {
        Rock { par: &1usize, k: None, uplift: None, runoff: None }
    }
}

/// [`carve`] with a [`Rock`]: K at each cell's current height multiplies the carve factor in each step, and the uplift array replaces `u_over_k`.
pub fn carve_rock(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &CarveParams, factor: Option<&[f64]>, rock: &Rock, seed: u64) -> Routing {
    carve_rock_fixed(grid, z, outlet, p, factor, rock, seed, &[])
}

/// [`carve`] with fixed cells (GEN-176): a fixed cell is neither lowered by the breach nor moved by a sweep (a channel cell after the burn of K7: only the final channel pass may lower it); `fixed` has one flag per cell, or none.
pub fn carve_fixed(
    grid: &Grid,
    z: &mut [f64],
    outlet: &[bool],
    p: &CarveParams,
    factor: Option<&[f64]>,
    seed: u64,
    fixed: &[bool],
) -> Routing {
    carve_rock_fixed(grid, z, outlet, p, factor, &Rock::none(), seed, fixed)
}

/// [`carve_rock`] with fixed cells (see [`carve_fixed`]).
#[allow(clippy::too_many_arguments)]
pub fn carve_rock_fixed(
    grid: &Grid,
    z: &mut [f64],
    outlet: &[bool],
    p: &CarveParams,
    factor: Option<&[f64]>,
    rock: &Rock,
    seed: u64,
    fixed: &[bool],
) -> Routing {
    assert!(fixed.is_empty() || fixed.len() == grid.len(), "one fixed flag per cell, or none");
    let is_fixed = |c: usize| !fixed.is_empty() && fixed[c];
    assert!(p.steps > 0 && p.reroute_every > 0, "a carve has steps and a re-route interval");
    assert!(p.max_area >= 0.0 && p.max_area.is_finite(), "the area cap is finite and not negative, not {}", p.max_area);
    assert_finite("the heights", z);
    if let Some(pb) = &p.pre_breach {
        super::breach::breach_basins(grid, z, outlet, p.epsilon, pb);
    }
    if let Some(f) = factor {
        assert_eq!(f.len(), grid.len(), "one carve factor per cell");
        assert!(f.iter().all(|v| v.is_finite() && *v >= 0.0), "the carve factors are finite and not negative");
    }
    assert!(rock.k.is_none() || !grid.is_compact(), "a layered rock reads the erodibility by cell and row: not for a compact grid");
    let floor = p.sea_level + p.epsilon;
    if let Some(u) = rock.uplift {
        assert_eq!(u.len(), grid.len(), "one uplift per cell");
        assert!(u.iter().all(|v| v.is_finite() && *v >= 0.0), "the uplifts are finite and not negative");
    }
    let uplift = p.u_over_k * p.k_dt;
    // GEN-162: under the local clip of the round rules (the round solver's) a breach does not cut below the base level of its chain exit (none when every chain ends at the sea; K7, the sediment pass and the default rules keep today's breach). The array is 8 bytes a cell: the round rules are not used on a face-scale carve (pass 1 of the default bake runs the default rules); a face-scale use would need an f32 or a per-catchment base.
    // The taper of the uplift by the starting height above the sea, kept per cell as an f32 (a face of 1.6e8 cells would not fit another f64 array in its budget).
    let taper: Option<Vec<f32>> = (p.uplift_taper_m > 0.0 && (p.u_over_k != 0.0 || rock.uplift.is_some()))
        .then(|| z.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect());
    let mut k: Vec<f32> = if rock.k.is_some() { vec![1.0; z.len()] } else { Vec::new() };
    let mut room: Vec<u16> = if rock.k.is_some() { vec![ROOM_NONE; z.len()] } else { Vec::new() };
    let mut routing: Option<Routing> = None;
    // The cells grouped by catchment when the schedule asks for it (empty: the region order).
    let mut grouped: Vec<u32> = Vec::new();
    for step in 0..p.steps {
        if step % p.reroute_every == 0 {
            // GEN-166: the heights of the round that ended are narrowed before the next routing reads them.
            if p.round_f32 && step > 0 {
                narrow_f32_checked(z, p.epsilon);
            }
            // The previous routing is dropped before the next is built: two live routings of a face would not fit its memory budget.
            drop(routing.take());
            let r = route_rules(grid, z, outlet, p.epsilon, seed, p.max_area, rock.runoff, p.round_rules);
            grouped.clear();
            if let Schedule::Catchments { descending } = p.schedule {
                let root = catchment_roots(&r.receiver, &r.order);
                grouped = r.order.clone();
                if descending {
                    grouped.sort_by_key(|&c| std::cmp::Reverse(root[c as usize]));
                } else {
                    grouped.sort_by_key(|&c| root[c as usize]);
                }
            }
            // The height each breached cell had at the start of the round (recorded at its first event: nothing else moves a height in the breach loop).
            let mut round_start: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
            let visit: &[u32] = if grouped.is_empty() { &r.order } else { &grouped };
            let base = if p.round_rules.clip_mfd != super::route::MfdClip::Local {
                None
            } else {
                exit_base(p.sea_level, r.order.iter().map(|&c| c as usize), &r.receiver, z, &|c| p.window_cut && grid.is_edge(c))
            };
            for &c in visit.iter().rev() {
                let c = c as usize;
                let rec = r.receiver[c];
                if rec == NONE || outlet[rec as usize] {
                    continue;
                }
                let rec = rec as usize;
                if is_fixed(rec) {
                    continue;
                }
                let mut want = (z[c] - p.breach_slope * distance(grid, c, rec)).max(floor);
                if let Some(b) = &base {
                    want = want.max(b[rec]);
                }
                if z[rec] > want {
                    // GEN-174: the round's total lowering of a cell is at most the limit at its height at the start of the round, however many donors breach it.
                    let start = *round_start.entry(rec).or_insert(z[rec]);
                    z[rec] = want.max(z[rec] - breach_limit(p, z[rec])).max(start - breach_limit(p, start));
                }
            }
            routing = Some(r);
        }
        let r = routing.as_ref().expect("routed at the first step");
        if let Some(kf) = rock.k {
            // The erodibility of every cell at its height at the start of the step: the sweep changes a cell's own height only when it reaches the cell, after its erodibility is read.
            let (w, zr) = (grid.w(), &*z);
            for r0 in (0..grid.h()).step_by(K_ROWS_AT_ONCE) {
                let rows = K_ROWS_AT_ONCE.min(grid.h() - r0);
                let block = par::map(rock.par, rows, &|r| {
                    (0..w)
                        .map(|i| {
                            let (c, h) = ((r0 + r) * w + i, zr[(r0 + r) * w + i]);
                            let l = kf(c, h);
                            assert!(!l.floor.is_nan(), "the layer edge of cell {c} is not a number");
                            (l.k as f32, room_code(h - l.floor + LAYER_SKIP_M))
                        })
                        .collect::<Vec<(f32, u16)>>()
                });
                for (r, row) in block.into_iter().enumerate() {
                    for (i, (kv, rv)) in row.into_iter().enumerate() {
                        (k[(r0 + r) * w + i], room[(r0 + r) * w + i]) = (kv, rv);
                    }
                }
            }
            assert!(k.iter().all(|v| v.is_finite() && *v >= 0.0), "the erodibilities are finite and not negative");
        }
        let visit: &[u32] = if grouped.is_empty() { &r.order } else { &grouped };
        for &c in visit {
            let c = c as usize;
            let rec = r.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            if is_fixed(c) {
                continue;
            }
            let class = factor.map_or(1.0, |f| f[c]);
            let scale = class * if rock.k.is_some() { f64::from(k[c]) } else { 1.0 };
            let f = p.k_dt * scale * libm::sqrt(r.area[c]) / distance(grid, c, rec);
            let hr = z[rec];
            let t = taper.as_ref().map_or(1.0, |t| f64::from(t[c]));
            let up = match rock.uplift {
                Some(u) => p.k_dt * f64::from(u[c]) * t,
                None => uplift * class * t,
            };
            let hn = (z[c] + up + f * hr) / (1.0 + f);
            let new = if p.keep_pits && !p.incise_only && z[c] < hr { (z[c] + up).max(floor) } else { hn.max(hr + p.epsilon).max(floor) };
            // A step does not lower a cell past the lower edge of its layer: a trunk lowered by more than a band is thick would skip a caprock (the rest is carried to the next step).
            let new = if rock.k.is_some() && new < z[c] {
                if room[c] == ROOM_NONE {
                    new
                } else {
                    new.max(z[c] - f64::from(room[c]) * ROOM_STEP_M)
                }
            } else {
                new
            };
            z[c] = if p.incise_only { new.min(z[c]) } else { new };
        }
    }
    if p.round_f32 {
        narrow_f32_checked(z, p.epsilon);
    }
    routing.expect("a carve has at least one step")
}

/// The heights as f32 widened again (GEN-166): `z = f64::from(z as f32)`, round to nearest, ties to even; idempotent on a value that is already f32. A height that does not fit an f32 is refused.
pub fn narrow_f32(z: &mut [f64]) {
    for v in z.iter_mut() {
        let n = *v as f32;
        assert!(n.is_finite(), "the height {v} does not fit an f32");
        *v = f64::from(n);
    }
}

/// [`narrow_f32`] under the epsilon of the round (GEN-175): the epsilon SHALL exceed the f32 spacing at the largest height of the surface, or the carved epsilon chains collapse to equal heights near the summit
/// (the world's epsilon is chosen for its height bounds; a surface above them is refused here, naming the cell, wherever the round is run).
pub fn narrow_f32_checked(z: &mut [f64], epsilon: f64) {
    narrow_f32(z);
    let top = (0..z.len()).fold(0, |m, c| if z[c].abs() > z[m].abs() { c } else { m });
    if let Some(&h) = z.get(top) {
        let spacing = super::precision::f32_spacing(h);
        assert!(
            epsilon > spacing,
            "cell {top} stands at {h} m: the f32 spacing there is {spacing} m, not under the flood epsilon {epsilon} m (GEN-175): the world's height bounds do not hold this surface; choose the epsilon for the bounds (WorldPrecision) or run the rounds in f64"
        );
    }
}

/// The distance between two neighbouring cells.
pub(super) fn distance(grid: &Grid, a: usize, b: usize) -> f64 {
    if grid.has_metric() {
        let d = (0..8).find(|&d| grid.neighbour(a, d) == Some(b)).expect("a receiver is a neighbour");
        return grid.distance_at(a, d);
    }
    let (ai, aj, bi, bj) = (a % grid.w(), a / grid.w(), b % grid.w(), b / grid.w());
    if ai == bi || aj == bj {
        grid.dx()
    } else {
        grid.dx() * core::f64::consts::SQRT_2
    }
}

/// The level a breach may not cut below (GEN-162, A3 sections 5.3 and 6; a window cut gets no base, GEN-172): along each receiver chain that ends in an outlet above the sea level (a fixed exit: a basin's rim), `base[outlet] = z[outlet]` and `base[c] = base[receiver]` (flat: a slope along the chain left a sill within the slope profile standing over a pit behind it, R3b, D4). A chain that ends at or below the sea level has no base (`None` when no chain
/// has one, so that a sea-outlet build allocates nothing and moves no bit). `order` lists the receivers before their donors;
pub(super) fn exit_base(
    sea_level: f64,
    order: impl Iterator<Item = usize> + Clone,
    receiver: &[u32],
    z: &[f64],
    window_cut: &dyn Fn(usize) -> bool,
) -> Option<Vec<f64>> {
    if !order.clone().any(|c| receiver[c] == NONE && z[c] > sea_level && !window_cut(c)) {
        return None;
    }
    let mut base = vec![f64::NEG_INFINITY; z.len()];
    for c in order {
        base[c] = if receiver[c] == NONE {
            if z[c] > sea_level && !window_cut(c) {
                z[c]
            } else {
                f64::NEG_INFINITY
            }
        } else {
            base[receiver[c] as usize]
        };
    }
    Some(base)
}
