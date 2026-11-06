//! The transient carve of pass 1 (GEN-100, SPIKE-65, SPIKE-66, SPIKE-70): implicit stream power with uplift (Braun and Willett 2013), with a breach before each re-route.
//! Not run to steady state: a fixed number of steps, `u_over_k * k_dt` of uplift in each.

use super::deposit::DepositReport;
use super::route::{route_weighted, Routing, NONE};
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
    /// The deposition coefficient `G` of Yuan et al. 2019 (GEN-131, ADR 0105): a cell gains `G Qs / Q` metres a step from the sediment flux `Qs` it carries (cubic metres a step) over its discharge `Q`.
    /// 0 (the default) switches deposition off and the carve is the one without a sediment term; an incise-only carve ignores it.
    pub deposit_g: f64,
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
            keep_pits: true,
            deposit_g: 0.0,
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
    carve_rock_report(grid, z, outlet, p, factor, rock, seed).0
}

/// [`carve_rock`] and the sediment account of its deposition steps in cubic metres over all steps (GEN-131): `eroded` is the net lowering of the cells that were lowered relative to their
/// uplift, `deposited` the net raising, `exported` what reached an outlet. `eroded - deposited = exported` up to rounding. All zero when `deposit_g` is 0.
pub fn carve_rock_report(
    grid: &Grid,
    z: &mut [f64],
    outlet: &[bool],
    p: &CarveParams,
    factor: Option<&[f64]>,
    rock: &Rock,
    seed: u64,
) -> (Routing, DepositReport) {
    let mut report = DepositReport { eroded: 0.0, deposited: 0.0, exported: 0.0 };
    assert!(p.deposit_g >= 0.0 && p.deposit_g.is_finite(), "the deposition coefficient is finite and not negative, not {}", p.deposit_g);
    let deposit = p.deposit_g > 0.0 && !p.incise_only;
    assert!(p.steps > 0 && p.reroute_every > 0, "a carve has steps and a re-route interval");
    assert!(p.max_area >= 0.0 && p.max_area.is_finite(), "the area cap is finite and not negative, not {}", p.max_area);
    assert_finite("the heights", z);
    if let Some(f) = factor {
        assert_eq!(f.len(), grid.len(), "one carve factor per cell");
        assert!(f.iter().all(|v| v.is_finite() && *v >= 0.0), "the carve factors are finite and not negative");
    }
    let floor = p.sea_level + p.epsilon;
    if let Some(u) = rock.uplift {
        assert_eq!(u.len(), grid.len(), "one uplift per cell");
        assert!(u.iter().all(|v| v.is_finite() && *v >= 0.0), "the uplifts are finite and not negative");
    }
    let uplift = p.u_over_k * p.k_dt;
    // The taper of the uplift by the starting height above the sea, kept per cell as an f32 (a face of 1.6e8 cells would not fit another f64 array in its budget).
    let taper: Option<Vec<f32>> = (p.uplift_taper_m > 0.0 && (p.u_over_k != 0.0 || rock.uplift.is_some()))
        .then(|| z.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect());
    let mut k: Vec<f32> = if rock.k.is_some() { vec![1.0; z.len()] } else { Vec::new() };
    let mut room: Vec<u16> = if rock.k.is_some() { vec![ROOM_NONE; z.len()] } else { Vec::new() };
    let mut routing: Option<Routing> = None;
    for step in 0..p.steps {
        if step % p.reroute_every == 0 {
            // The previous routing is dropped before the next is built: two live routings of a face would not fit its memory budget.
            drop(routing.take());
            let r = route_weighted(grid, z, outlet, p.epsilon, seed, p.max_area, rock.runoff);
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
                        p.breach_max_m.min(BREACH_MIN_M)
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
        if deposit {
            let sweep = Sweep { grid, r, p, factor, rock, k: &k, room: &room, taper: taper.as_deref(), uplift, floor };
            sweep.run(z, &mut report);
            continue;
        }
        for &c in &r.order {
            let c = c as usize;
            let rec = r.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
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
    (routing.expect("a carve has at least one step"), report)
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

/// One step of the carve with deposition (GEN-131, Yuan et al. 2019), over a routing: the same implicit stream power as the sweep without deposition, plus the sediment term.
///
/// With `d` the change of a cell in the step, `f` the stream-power number, `up` the uplift, `Qs` the flux through the cell and `g = G / Q`:
/// `d = up - f (h - h_r) + f d_r + g Qs`, `Qs = sum of the donors' Qs + dx^2 (up - d)`. The flux of a donor is affine in the change of its receiver, so a sweep from the leaves gives every cell
/// `d = a + b d_r` with `b = f / (1 + f + g (dx^2 - D))`, `D` the sum of the donors' slopes `d Qs / d d_r` (not positive, so the denominator is at least `1 + f`: the step is as stable as the one
/// without a sediment term); a sweep from the outlets solves the changes; a last sweep from the leaves adds the flux of the final heights, so that what the clamps hold back moves on and mass is kept.
struct Sweep<'a, 'b> {
    grid: &'a Grid,
    r: &'a Routing,
    p: &'a CarveParams,
    factor: Option<&'a [f64]>,
    rock: &'a Rock<'b>,
    k: &'a [f32],
    room: &'a [u16],
    taper: Option<&'a [f32]>,
    uplift: f64,
    floor: f64,
}

impl Sweep<'_, '_> {
    /// The stream-power number and the uplift of a cell with a receiver (both of the heights at the start of the step, and not of the class when the uplift field is given).
    fn cell(&self, c: usize, rec: usize) -> (f64, f64) {
        let class = self.factor.map_or(1.0, |f| f[c]);
        let scale = class * if self.rock.k.is_some() { f64::from(self.k[c]) } else { 1.0 };
        let f = self.p.k_dt * scale * libm::sqrt(self.r.area[c]) / distance(self.grid, c, rec);
        let t = self.taper.map_or(1.0, |t| f64::from(t[c]));
        let up = match self.rock.uplift {
            Some(u) => self.p.k_dt * f64::from(u[c]) * t,
            None => self.uplift * class * t,
        };
        (f, up)
    }

    fn run(&self, z: &mut [f64], report: &mut DepositReport) {
        let (n, r, p) = (z.len(), self.r, self.p);
        let dx2 = self.grid.dx() * self.grid.dx();
        // A pit of the unfilled surface keeps its height: no stream power, only uplift and the sediment that reaches it.
        let pit: Vec<bool> = (0..n)
            .map(|c| {
                let rec = r.receiver[c];
                p.keep_pits && rec != NONE && z[c] < z[rec as usize]
            })
            .collect();
        // The land that takes sediment: not the sea's cells.
        let land = |c: usize, z: &[f64]| z[c] > self.floor;
        let g_of = |c: usize, z: &[f64]| if land(c, z) && r.area[c] > 0.0 { p.deposit_g / r.area[c] } else { 0.0 };
        let mut a = vec![0.0f64; n];
        let mut flux_c = vec![0.0f64; n];
        let mut slope_d = vec![0.0f64; n];
        // Leaves to outlets: the affine relation of every cell to its receiver's change, and the flux it hands on.
        for &c in r.order.iter().rev() {
            let c = c as usize;
            let rec = r.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            let (f, up) = self.cell(c, rec);
            let f = if pit[c] { 0.0 } else { f };
            let g = g_of(c, z);
            let (cc, dd) = (flux_c[c], slope_d[c]);
            let denom = 1.0 + f + g * (dx2 - dd);
            let drop = if pit[c] { 0.0 } else { z[c] - z[rec] };
            a[c] = (up * (1.0 + g * dx2) - f * drop + g * cc) / denom;
            let b = f / denom;
            flux_c[rec] += cc + dx2 * up + (dd - dx2) * a[c];
            slope_d[rec] += (dd - dx2) * b;
        }
        // Outlets to leaves: solve the changes, then the clamps of the sweep without deposition. A cell deposits no higher than its receiver's new height plus the drop it had above it.
        let mut delta = a;
        for &c in &r.order {
            let c = c as usize;
            let rec = r.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            let (f, up) = self.cell(c, rec);
            let dr = delta[rec];
            let b = if pit[c] {
                0.0
            } else {
                let g = g_of(c, z);
                f / (1.0 + f + g * (dx2 - slope_d[c]))
            };
            let hr_new = z[rec];
            let hr_old = hr_new - dr;
            let start = z[c];
            let mut new = start + delta[c] + b * dr;
            let ceiling = (hr_new + (start - hr_old).max(0.0) + p.epsilon).max(start + up);
            new = new.min(ceiling);
            if pit[c] {
                new = new.max(start + up).max(self.floor);
            } else {
                new = new.max(hr_new + p.epsilon).max(self.floor);
            }
            if self.rock.k.is_some() && new < start && self.room[c] != ROOM_NONE {
                new = new.max(start - f64::from(self.room[c]) * ROOM_STEP_M);
            }
            delta[c] = new - start;
            z[c] = new;
        }
        // Leaves to outlets with the final changes: the flux every cell hands on, and the account.
        flux_c.iter_mut().for_each(|v| *v = 0.0);
        for &c in r.order.iter().rev() {
            let c = c as usize;
            let rec = r.receiver[c];
            if rec == NONE {
                continue;
            }
            let rec = rec as usize;
            let (_, up) = self.cell(c, rec);
            let net = dx2 * (up - delta[c]);
            if net > 0.0 {
                report.eroded += net;
            } else {
                report.deposited -= net;
            }
            let out = flux_c[c] + net;
            flux_c[rec] += out;
            if r.receiver[rec] == NONE {
                report.exported += out;
            }
        }
    }
}
