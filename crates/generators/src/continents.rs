//! The continent stage of the global stage (GEN-104, GEN-105, GEN-106, ADR 0089): the continentalness value that decides land and sea.
//!
//! Hybrid of SPIKE-92 (H2r3): a domain-warped fBm with a gain per octave, biased by a few ranked cratons that are chains of elongated lobes, the
//! fBm damped in the deep ocean and in craton interiors, plus an island term just offshore of the craton margins. A per-seed offset puts
//! [`LAND_FRACTION`] of the sphere above [`crate::global::SEA_LEVEL_C`]. The craton layout is redrawn from derived seeds until the coarse land map
//! has 4 to 7 masses of at least 1 Mkm^2 and the largest holds 47 to 60 % of the land (GEN-105).
//!
//! Every term is value noise on the 3D direction, so nothing reads latitude or longitude. Deterministic and thread-count independent (CON-14): splitmix hashing, `libm`
//! transcendentals, fixed grids and draw orders, and a total order with an index tie-break wherever values are sorted. The per-seed plan (lobes and offset) is
//! computed once per process and seed.

use crate::global::{fbm, SEA_LEVEL_C};
use planet_core::{smoothstep, Vec3};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The share of the sphere that is land.
pub const LAND_FRACTION: f64 = 0.29;

// The layer constants are SPIKE-92's, so that its scores transfer.
const LAYER_SHAPE: u64 = 0x5392_0001;
const LAYER_WARP: u64 = 0x5392_0002;
const LAYER_ISLAND: u64 = 0x5392_0005;

/// The ranked area shares of the land of the cratons, largest first.
const SIZES: [f64; 8] = [0.50, 0.26, 0.10, 0.06, 0.03, 0.02, 0.01, 0.01];
/// Lobes of a craton of at least 20 % of the land (a craton of 4 to 20 % has 1 to this many, a smaller one 1).
const LOBES: u32 = 3;
/// The largest turn of the spine between two lobes, in radians.
const TURN: f64 = 1.2;
/// Lobe centres are this many times the sum of the two major radii apart.
const SPACING: f64 = 0.6;
/// The aspect of a lobe is uniform in this range (major radius r*sqrt(a), minor r/sqrt(a)).
const ASPECT: (f64, f64) = (1.5, 2.5);
/// The shape fBm: base frequency, octaves, gain per octave and domain warp amount.
const SHAPE_FREQ: f64 = 1.6;
const SHAPE_OCTAVES: u32 = 8;
const SHAPE_GAIN: f64 = 0.72;
const SHAPE_WARP: f64 = 0.6;
/// The octaves of the smooth value ([`continent_smooth`]).
const SMOOTH_OCTAVES: u32 = 4;
/// The octaves of the coarse value ([`continent_coarse`]).
const COARSE_OCTAVES: u32 = 3;
/// The weight of the craton bias, and the warp of the direction the cratons are read at.
const BIAS: f64 = 0.8;
const CRATON_WARP: f64 = 0.3;
/// The fBm's floor in the deep ocean and craton interiors (its amplitude is 1 on the margins).
const DEEP: f64 = 0.65;
/// The island term: amplitude, base frequency (octaves 4, gain 0.6) and the domain warp it is read at (GEN-106). The warp is half the shape's: the
/// larger one stretched the high-frequency noise along its gradient into parallel striations.
const ISLAND_AMPLITUDE: f64 = 1.3;
const ISLAND_FREQ: f64 = 3.0;
const ISLAND_WARP: f64 = 0.3;
/// The layout retry (GEN-105): draws, the coarse grid's cell in degrees, and the accepted masses and largest share.
const RETRY_DRAWS: u64 = 12;
const LAYOUT_CELL_DEG: f64 = 2.0;
const MASSES: (f64, f64) = (4.0, 7.0);
const LARGEST: (f64, f64) = (0.47, 0.60);
/// Directions of the offset's quantile.
const OFFSET_DIRECTIONS: usize = 20_000;

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(x: &mut u64) -> f64 {
    (splitmix(x) >> 11) as f64 / (1u64 << 53) as f64
}

fn angle(a: Vec3, b: Vec3) -> f64 {
    libm::atan2(a.cross(b).length(), a.dot(b))
}

/// fBm with a gain per octave (the coast's roughness), each octave its own layer: the first `octaves` of a series of `series` octaves, normalised by the whole series, so that
/// the first octaves of the value are the same terms of it whatever the count (the smooth and coarse values are the full value without its detail).
fn fbm_gain(seed: u64, layer: u64, p: Vec3, freq: f64, octaves: u32, series: u32, gain: f64) -> f64 {
    let (mut sum, mut amp, mut f) = (0.0, 1.0, freq);
    for o in 0..octaves {
        sum += amp * fbm(seed, layer.wrapping_add(u64::from(o) << 8), p, f, 1);
        amp *= gain;
        f *= 2.0;
    }
    let norm: f64 = (0..series).map(|o| libm::pow(gain, f64::from(o))).sum();
    sum / norm
}

/// An elongated lobe: centre, major axis (tangent at the centre), minor axis, major and minor angular radii.
type Lobe = (Vec3, Vec3, Vec3, f64, f64);

/// The lobes of the craton layout drawn from `lseed`. A craton is a chain of lobes along a spine that turns by up to [`TURN`] per lobe; the lobes split the
/// craton's cap area at random. Craton centres are at least the two extents plus 0.1 rad apart (up to 200 draws).
fn lobes(lseed: u64) -> Vec<Lobe> {
    let mut s = lseed ^ 0x5392_e11e;
    let mut out: Vec<Lobe> = Vec::new();
    let mut placed: Vec<(Vec3, f64)> = Vec::new();
    for share in SIZES {
        let n = if share >= 0.2 {
            LOBES
        } else if share >= 0.04 {
            (1 + (unit(&mut s) * f64::from(LOBES)) as u32).min(LOBES)
        } else {
            1
        };
        let weights: Vec<f64> = (0..n).map(|_| 0.5 + unit(&mut s)).collect();
        let total: f64 = weights.iter().sum();
        // The spine in the craton's tangent plane (radians): (x, y, direction, area weight r^2, ra, rb) per lobe.
        let (mut x, mut y, mut dir) = (0.0f64, 0.0f64, core::f64::consts::TAU * unit(&mut s));
        let mut local: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::new();
        for (j, w) in weights.iter().enumerate() {
            // A cap of half-angle r holds 2 pi (1 - cos r) of the sphere's 4 pi.
            let r = libm::acos((1.0 - 2.0 * LAND_FRACTION * share * w / total).clamp(-1.0, 1.0));
            let a = ASPECT.0 + (ASPECT.1 - ASPECT.0) * unit(&mut s);
            let (ra, rb) = (r * libm::sqrt(a), r / libm::sqrt(a));
            if j > 0 {
                dir += (2.0 * unit(&mut s) - 1.0) * TURN;
                let prev = local[j - 1].4;
                x += libm::cos(dir) * SPACING * (prev + ra);
                y += libm::sin(dir) * SPACING * (prev + ra);
            }
            local.push((x, y, dir, r * r, ra, rb));
        }
        let area: f64 = local.iter().map(|l| l.3).sum();
        let cx = local.iter().map(|l| l.0 * l.3).sum::<f64>() / area;
        let cy = local.iter().map(|l| l.1 * l.3).sum::<f64>() / area;
        let extent = local.iter().map(|l| libm::hypot(l.0 - cx, l.1 - cy) + l.4).fold(0.0, f64::max);
        let mut c = Vec3::new(0.0, 0.0, 1.0);
        for _ in 0..200 {
            let z = 2.0 * unit(&mut s) - 1.0;
            let az = core::f64::consts::TAU * unit(&mut s);
            let q = libm::sqrt((1.0 - z * z).max(0.0));
            c = Vec3::new(q * libm::cos(az), q * libm::sin(az), z);
            if placed.iter().all(|(o, eo)| angle(c, *o) >= extent + eo + 0.1) {
                break;
            }
        }
        placed.push((c, extent));
        let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let e1 = c.cross(helper).normalized();
        let e2 = c.cross(e1);
        for (lx, ly, ldir, _, ra, rb) in local {
            // The exponential map from the craton's tangent plane to the sphere.
            let (dx, dy) = (lx - cx, ly - cy);
            let rho = libm::hypot(dx, dy);
            let pc = if rho < 1e-12 { c } else { (c * libm::cos(rho) + (e1 * dx + e2 * dy) * (libm::sin(rho) / rho)).normalized() };
            let d3 = e1 * libm::cos(ldir) + e2 * libm::sin(ldir);
            let t = (d3 - pc * d3.dot(pc)).normalized();
            out.push((pc, t, pc.cross(t), ra, rb));
        }
    }
    out
}

/// The warp of the shape fBm and the islands at `d`.
fn warp_at(seed: u64, d: Vec3) -> Vec3 {
    Vec3::new(
        fbm(seed, LAYER_WARP, d, 1.3, 3),
        fbm(seed, LAYER_WARP, d + Vec3::new(17.0, 0.0, 0.0), 1.3, 3),
        fbm(seed, LAYER_WARP, d + Vec3::new(0.0, 31.0, 0.0), 1.3, 3),
    )
}

/// The raw continent value at `d` for the lobes of a layout (before the per-seed offset).
fn raw(seed: u64, lobes: &[Lobe], d: Vec3, octaves: u32, detail: bool) -> f64 {
    let w = warp_at(seed, d);
    let shape = fbm_gain(seed, LAYER_SHAPE, d + w * SHAPE_WARP, SHAPE_FREQ, octaves, SHAPE_OCTAVES, SHAPE_GAIN) * 1.7;
    let pc = (d + w * CRATON_WARP).normalized();
    // rho is the elliptical distance in the tangent frame of each lobe (front hemisphere only).
    let bias: f64 = lobes
        .iter()
        .map(|(c, t, b, ra, rb)| {
            let z = pc.dot(*c);
            // Only the front hemisphere, fading to 0 over z < 0.25 (the angle from the lobe's centre near 75 to 90 degrees), so that the value is continuous across the great
            // circle 90 degrees from the centre, where the tangent angles below are not defined.
            let front = smoothstep(0.0, 0.25, z);
            if front <= 0.0 {
                return 0.0;
            }
            let (u, v) = (libm::atan2(pc.dot(*t), z) / ra, libm::atan2(pc.dot(*b), z) / rb);
            front * libm::exp(-(u * u + v * v))
        })
        .sum();
    // The fBm's amplitude peaks on the craton margins (bias 0.5) and falls to DEEP in the deep ocean and the interiors.
    let wm = bias.min(1.0);
    // Islands: island-scale fBm just offshore of the craton margins (bias about 0.2), on a smaller warp than the shape's.
    let near = libm::exp(-((wm - 0.2) / 0.15) * ((wm - 0.2) / 0.15));
    let islands =
        if detail { ISLAND_AMPLITUDE * near * fbm_gain(seed, LAYER_ISLAND, d + w * ISLAND_WARP, ISLAND_FREQ, 4, 4, 0.6) } else { 0.0 };
    shape * (DEEP + (1.0 - DEEP) * 4.0 * wm * (1.0 - wm)) + BIAS * bias + islands
}

/// The layout seed of `seed` (GEN-105): draws `seed`, then derived seeds (up to [`RETRY_DRAWS`] draws), until a coarse land map (2 degree cells, [`LAND_FRACTION`] of the area)
/// has 4 to 7 masses of at least 1 Mkm^2 and the largest holds 47 to 60 % of the land; else the draw with the lowest cost, the first on ties.
fn layout_seed(seed: u64) -> u64 {
    let (mut best, mut best_cost) = (seed, f64::INFINITY);
    for t in 0..RETRY_DRAWS {
        let lseed = layout_draw(seed, t);
        let cost = layout_cost(seed, lseed);
        if cost < best_cost {
            (best, best_cost) = (lseed, cost);
        }
        if cost == 0.0 {
            break;
        }
    }
    best
}

/// The layout seed of draw `t` of `seed`: the seed itself for draw 0, then derived seeds.
fn layout_draw(seed: u64, t: u64) -> u64 {
    if t == 0 {
        seed
    } else {
        seed ^ 0x5392_7e7e_u64.wrapping_mul(t + 1)
    }
}

/// The cost of the craton layout of `lseed` for `seed` (GEN-105): 0 when the coarse land map has 4 to 7 masses of at least 1 Mkm^2 and the largest holds 47 to 60 % of the land.
fn layout_cost(seed: u64, lseed: u64) -> f64 {
    let (h, w) = ((180.0 / LAYOUT_CELL_DEG) as usize, (360.0 / LAYOUT_CELL_DEG) as usize);
    let cell_deg = LAYOUT_CELL_DEG.to_radians();
    let r_km = crate::classes::EARTH_RADIUS_M / 1000.0;
    let cell: Vec<f64> = (0..h)
        .map(|i| {
            let lat = core::f64::consts::FRAC_PI_2 - (i as f64 + 0.5) * cell_deg;
            r_km * cell_deg * r_km * cell_deg * libm::cos(lat)
        })
        .collect();
    let total: f64 = cell.iter().sum::<f64>() * w as f64;
    let l = lobes(lseed);
    let v: Vec<f64> = (0..h * w)
        .map(|k| {
            let (i, j) = (k / w, k % w);
            let lat = core::f64::consts::FRAC_PI_2 - (i as f64 + 0.5) * cell_deg;
            let lon = -core::f64::consts::PI + (j as f64 + 0.5) * cell_deg;
            raw(seed, &l, Vec3::from_lat_lon(lat, lon), SHAPE_OCTAVES, true)
        })
        .collect();
    let mut order: Vec<usize> = (0..h * w).collect();
    order.sort_by(|a, b| v[*b].total_cmp(&v[*a]).then(a.cmp(b)));
    let mut land = vec![false; h * w];
    let mut acc = 0.0;
    for k in order {
        if acc >= LAND_FRACTION * total {
            break;
        }
        land[k] = true;
        acc += cell[k / w];
    }
    let (label, n) = crate::shape::components(&land, w, h, true);
    let mut areas = vec![0.0f64; n + 1];
    for k in 0..h * w {
        areas[label[k] as usize] += cell[k / w];
    }
    let masses = areas[1..].iter().filter(|a| **a >= 1e6).count() as f64;
    let largest = areas[1..].iter().cloned().fold(0.0, f64::max) / acc;
    (MASSES.0 - masses).max(0.0) + (masses - MASSES.1).max(0.0) + 10.0 * ((LARGEST.0 - largest).max(0.0) + (largest - LARGEST.1).max(0.0))
}

/// The per-seed plan: the lobes of the chosen layout and the offset that puts [`LAND_FRACTION`] of the sphere above sea level.
struct Plan {
    lobes: Vec<Lobe>,
    offset: f64,
}

fn build(seed: u64) -> Plan {
    let lobes = lobes(layout_seed(seed));
    let n = OFFSET_DIRECTIONS;
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    let mut v: Vec<f64> = (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = libm::sqrt((1.0 - z * z).max(0.0));
            let a = golden * i as f64;
            raw(seed, &lobes, Vec3::new(r * libm::cos(a), r * libm::sin(a), z), SHAPE_OCTAVES, true)
        })
        .collect();
    v.sort_by(f64::total_cmp);
    let offset = v[((1.0 - LAND_FRACTION) * n as f64) as usize];
    Plan { lobes, offset }
}

type Slot = Arc<OnceLock<Arc<Plan>>>;

fn plan(seed: u64) -> Arc<Plan> {
    thread_local! {
        static LAST: RefCell<Option<(u64, Arc<Plan>)>> = const { RefCell::new(None) };
    }
    if let Some(p) = LAST.with(|l| l.borrow().as_ref().filter(|(s, _)| *s == seed).map(|(_, p)| p.clone())) {
        return p;
    }
    static PLANS: OnceLock<Mutex<HashMap<u64, Slot>>> = OnceLock::new();
    let slot: Slot = PLANS.get_or_init(Default::default).lock().expect("continent plans").entry(seed).or_default().clone();
    // Outside the map's lock: a thread that finds the slot empty builds the plan, the others wait for it.
    let p = slot.get_or_init(|| Arc::new(build(seed))).clone();
    LAST.with(|l| *l.borrow_mut() = Some((seed, p.clone())));
    p
}

/// The continentalness at the unit direction `d` (GEN-104): land above [`SEA_LEVEL_C`]. A pure function of `(seed, d)`.
pub fn continent(seed: u64, d: Vec3) -> f64 {
    let p = plan(seed);
    raw(seed, &p.lobes, d, SHAPE_OCTAVES, true) - p.offset + SEA_LEVEL_C
}

/// The continentalness without the detail octaves and the island term (GEN-104): the same plan and offset, features of 500 km and up. Its gradient is
/// well defined at the 25 km step of the class source's coast distance, which the full value's is not (its roughness changes the gradient several times over one lattice cell).
pub fn continent_smooth(seed: u64, d: Vec3) -> f64 {
    let p = plan(seed);
    raw(seed, &p.lobes, d, SMOOTH_OCTAVES, false) - p.offset + SEA_LEVEL_C
}

/// The continentalness with only the first three octaves (features of 1000 km and up, no islands): the distance from the coast of the landform driver, which the lattice
/// of the driver can hold (GEN-104).
pub fn continent_coarse(seed: u64, d: Vec3) -> f64 {
    let p = plan(seed);
    raw(seed, &p.lobes, d, COARSE_OCTAVES, false) - p.offset + SEA_LEVEL_C
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-105
    #[test]
    fn the_layout_retry_keeps_the_first_draw_of_cost_zero_or_else_the_cheapest_and_the_first_on_ties() {
        for seed in 1..=5u64 {
            let kept = layout_seed(seed);
            assert_eq!(kept, layout_seed(seed), "the retry is a pure function of the seed");
            let costs: Vec<f64> = (0..RETRY_DRAWS).map(|t| layout_cost(seed, layout_draw(seed, t))).collect();
            let first_zero = costs.iter().position(|&c| c == 0.0);
            let cheapest = costs.iter().enumerate().fold(0usize, |b, (t, &c)| if c < costs[b] { t } else { b });
            let expected = first_zero.unwrap_or(cheapest);
            assert_eq!(kept, layout_draw(seed, expected as u64), "seed {seed}: costs {costs:?}");
        }
    }

    // spec: GEN-105
    #[test]
    fn the_kept_layout_draw_of_seeds_1_to_5_is_pinned_so_that_a_swapped_layout_needs_a_declared_re_pin() {
        // The island term is part of the layout score (GEN-105): changing it, or any constant of the shape, may swap a seed's layout. Seed 14 is the reference world (the owner's pick from the engine gallery).
        let kept: Vec<u64> = [1u64, 2, 3, 4, 5, 14]
            .into_iter()
            .map(|seed| (0..RETRY_DRAWS).find(|&t| layout_draw(seed, t) == layout_seed(seed)).expect("a kept draw"))
            .collect();
        assert_eq!(kept, vec![2, 1, 1, 2, 3, 1], "a seed's craton layout changed (seed 14 is the reference world): declare the re-pin");
    }
}
