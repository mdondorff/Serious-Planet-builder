//! The default relief-class source: the plate mask and the class weights on a lattice (ADR 0089 decisions 1 to 3 and 10; GEN-093 to GEN-095, GEN-103).
//!
//! CPU fields only: `libm` for every transcendental, integer hashing for every random number, results the same bits for any thread count (CON-14).
//! The source reads the global stage's continent value ([`crate::global`]) and nothing else; it is not wired into the bake yet (`GLOBAL_VERSION`
//! is unchanged). [`ClassSource::eval`] is a pure function of `(seed, direction)`; [`ClassLattice`] stores its weights as `u8` on the landform
//! lattice's geometry and is the one interface consumers read (ADR 0089 decision 1).

use crate::global::{fbm, hill_octaves, ridge_at, terrain, MAX_ELEVATION_M, MIN_ELEVATION_M, SEA_LEVEL_C};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{smoothstep, Face, Vec3};

/// The version of the class list (GEN-093): plain, rolling plain, hills, mountains, high mountains, plateau, badlands, sand sea, in that order.
pub const CLASS_LIST_VERSION: u32 = 1;
/// Bump whenever the default source's output changes for a seed (the rules of GEN-094 and GEN-095, their constants, or the plate construction).
pub const CLASS_SOURCE_VERSION: u32 = 5;
pub const CLASS_COUNT: usize = 8;
pub const CLASS_NAMES: [&str; CLASS_COUNT] =
    ["plain", "rolling plain", "hills", "mountains", "high mountains", "plateau", "badlands", "sand sea"];
pub const PLAIN: usize = 0;
pub const ROLLING: usize = 1;
pub const HILLS: usize = 2;
pub const MOUNTAINS: usize = 3;
pub const HIGH_MOUNTAINS: usize = 4;
pub const PLATEAU: usize = 5;

/// The relief driver `k` of each class (GEN-097, start values).
pub const CLASS_K: [f64; CLASS_COUNT] = [1.0, 1.0, 5.0, 6.0, 7.5, 1.0, 2.0, 0.35];

/// Cells per face edge of the class lattice: the landform lattice's geometry (ADR 0080 point 3, GEN-093).
pub const CLASS_LATTICE_N: usize = 256;

/// Plates of the default source (GEN-094).
const PLATES: usize = 14;
/// Plate draws and the points that score them (GEN-105).
const PLATE_DRAWS: u64 = 64;
const PLATE_SCORE_POINTS: usize = 8192;
/// Radius of the body in metres used for the margin distances; the default source is defined for Earth's radius (GEN-094's kilometres).
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
/// Land-share sample points per plate set (a Fibonacci sphere), for the back-arc's choice of the plate with more land.
const SHARE_POINTS: usize = 2048;
/// Step of the finite difference that gives the distance to the coastal line (radians, about 25 km).
const GRADIENT_STEP: f64 = 0.004;
/// The land depth of the coastal line the Andean arcs follow (GEN-094).
const COAST_LINE_DEPTH: f64 = 0.10;
/// The belt term's amplitude in metres (GEN-096; 5500 m in the first start values, raised because the belts are now cut into segments of lower height).
const BELT_AMPLITUDE_M: f64 = 7800.0;
/// The plateau field's uplift in metres (GEN-096).
const PLATEAU_UPLIFT_M: f64 = 600.0;
/// The base elevation of the land far from the coast and the land depth over which it is reached (GEN-096).
const COAST_TOP_M: f64 = 620.0;
const COAST_RAMP_DEPTH: f64 = 1.2;
/// The narrowest half-width a belt has, in km (a belt tapers in width and height towards the ends of its segment, never below this).
const MIN_BELT_KM: f64 = 125.0;
/// The core value at which the mountains' membership is 1 (it rises smoothly from 0 at the belt's edge, so the class weights change over the belt's width and not in one lattice step).
const MOUNTAIN_FULL_CORE: f64 = 0.4;
/// The share of the flank term (a tent three times as wide as the core's) that counts towards the mountains' membership, so that the mountains cover the flanks of a belt and not only its core.
const FLANK_MEMBERSHIP: f64 = 1.0;
/// The class elevation over which a mountain becomes a high mountain, in metres.
const HIGH_FROM_M: f64 = 1000.0;
const HIGH_TO_M: f64 = 5500.0;
/// The largest sideways shift of a belt's centreline, in km (low-frequency noise bends the trace).
const BEND_KM: f64 = 120.0;
/// The mask's coast gate is 0 within this distance of the smooth coast and 1 from the second distance inland, in km.
const COAST_BUFFER_KM: f64 = 50.0;
const COAST_RAMP_KM: f64 = 300.0;
/// The land depth over which the gate rises from the true coast.
const COAST_DEPTH_RAMP: f64 = 0.3;
/// The land depth over which the gate rises from the true coast, on top of the smooth ramp (the full value is rough at the lattice scale).
const TRUE_COAST_RAMP: f64 = 0.2;

const LAYER_PLATE_WARP: u64 = 0x474c_0010;
const LAYER_WIDTH: u64 = 0x474c_0011;
const LAYER_COAST_ARC: u64 = 0x474c_0012;
const LAYER_PLATEAU: u64 = 0x474c_0013;
const LAYER_ROLLING: u64 = 0x474c_0014;
const LAYER_SEGMENT: u64 = 0x474c_0015;
const LAYER_STRIKE: u64 = 0x474c_0016;
const LAYER_BEND: u64 = 0x474c_0017;
const LAYER_HILLY: u64 = 0x474c_0018;

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A uniform number in `[0, 1)` from the seed, a stream tag and an index.
fn unit(seed: u64, tag: u64, index: u64) -> f64 {
    (splitmix(splitmix(seed ^ (tag << 32)) ^ index) >> 11) as f64 / (1u64 << 53) as f64
}

/// A uniformly distributed unit vector from two uniform numbers.
fn on_sphere(u: f64, v: f64) -> Vec3 {
    let z = 2.0 * u - 1.0;
    let r = libm::sqrt((1.0 - z * z).max(0.0));
    let phi = 2.0 * core::f64::consts::PI * v;
    Vec3::new(r * libm::cos(phi), r * libm::sin(phi), z)
}

/// The plates of a draw seed: a centre, an Euler pole and a rate each.
fn draw_plates(seed: u64) -> Vec<Plate> {
    (0..PLATES as u64)
        .map(|i| {
            let centre = on_sphere(unit(seed, 1, 2 * i), unit(seed, 1, 2 * i + 1));
            let pole = on_sphere(unit(seed, 2, 2 * i), unit(seed, 2, 2 * i + 1));
            let rate = 0.3 + 0.7 * unit(seed, 3, i);
            Plate { centre, rotation: pole * rate }
        })
        .collect()
}

/// `n` directions of a Fibonacci sphere.
fn fibonacci_points(n: usize) -> Vec<Vec3> {
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = libm::sqrt((1.0 - z * z).max(0.0));
            let phi = golden * i as f64;
            Vec3::new(r * libm::cos(phi), r * libm::sin(phi), z)
        })
        .collect()
}

struct Plate {
    centre: Vec3,
    /// The Euler-pole rotation vector: the pole times the rate (0.3 to 1.0, dimensionless).
    rotation: Vec3,
}

/// The terms of the plate mask at one direction (GEN-094), each in `[0, 1]` unless noted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaskTerms {
    /// Distance to the nearest plate margin in km (half the angular difference of the two nearest centres, times the radius).
    pub margin_km: f64,
    /// The convergence of the nearest and second-nearest plate (a diagnostic: it changes sign where the second plate changes), positive where they move towards each other (not bounded by 1).
    pub convergence: f64,
    pub core: f64,
    pub flank: f64,
    pub old_margin: f64,
    pub back_arc: f64,
    pub plateau_field: f64,
    /// Hilly country away from the belts, in `[0, 1]`: a low-frequency field gated to the interior (round 3: hills are 15 to 30 % of the land on Earth, and belts alone give under 10 %).
    pub hilly: f64,
    /// The along-strike factor of the belts in `[0, 1]`: 0 in a gap between segments, below 1 where a segment is lower.
    pub strike: f64,
    /// Index of the plate the direction lies on.
    pub plate: usize,
}

/// Everything the default source says about one direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassPoint {
    pub terms: MaskTerms,
    /// The global stage's continent value minus the sea level.
    pub land_depth: f64,
    /// The base elevation of GEN-096 without the hill band, in metres.
    pub e0: f64,
    /// The default class weights, f64, summing to 1 (before the lattice's `u8` rounding).
    pub weights: [f64; CLASS_COUNT],
}

/// The default class source of one seed (GEN-094, GEN-095): the plates, and the land share of each for the back-arc.
pub struct ClassSource {
    seed: u64,
    radius_m: f64,
    plates: Vec<Plate>,
    land_share: Vec<f64>,
    /// The index of the plate draw that was kept (GEN-105): 0 is the seed's own draw.
    plate_draw: u64,
}

impl ClassSource {
    pub fn new(seed: u64) -> ClassSource {
        // The plates are drawn up to PLATE_DRAWS times (GEN-105); the draw with the most land next to a convergent margin is kept (the first on ties), so that the belts stand on the continents.
        let points = fibonacci_points(PLATE_SCORE_POINTS);
        let land: Vec<bool> = points.iter().map(|&d| terrain(seed, d).land_depth > 0.0).collect();
        let land_count = land.iter().filter(|&&l| l).count().max(1) as f64;
        let mut best: Option<(f64, ClassSource)> = None;
        for t in 0..PLATE_DRAWS {
            let draw = if t == 0 { seed } else { splitmix(seed ^ (t << 40) ^ 0x71a7_e500) };
            let plates = draw_plates(draw);
            let source = ClassSource { seed, radius_m: EARTH_RADIUS_M, plates, land_share: vec![0.0; PLATES], plate_draw: t };
            let score =
                points.iter().zip(&land).filter(|(_, &l)| l).map(|(&d, _)| source.convergent_margin_near(d)).sum::<f64>() / land_count;
            if best.as_ref().is_none_or(|(b, _)| score > *b) {
                best = Some((score, source));
            }
        }
        let mut source = best.expect("at least one plate draw").1;
        source.land_share = source.measure_land_shares();
        source
    }

    /// In `[0, 1]`: how much of a convergent margin of the two nearest plates lies within a few hundred km of `d` (the score of a plate draw).
    fn convergent_margin_near(&self, d: Vec3) -> f64 {
        let (idx, ang) = self.nearest_three(self.warp(d));
        let dist_km = 0.5 * (ang[1] - ang[0]) * self.radius_m / 1000.0;
        let (px, py) = (&self.plates[idx[0]], &self.plates[idx[1]]);
        let nrm = {
            let n = py.centre - px.centre;
            let n = n - d * n.dot(d);
            let l = n.length();
            if l > 1e-12 {
                n * (1.0 / l)
            } else {
                Vec3::ZERO
            }
        };
        let conv = (px.rotation.cross(d) - py.rotation.cross(d)).dot(nrm);
        (1.0 - smoothstep(150.0, 450.0, dist_km)) * smoothstep(0.05, 0.35, conv)
    }

    /// The index of the plate draw kept by the retry (GEN-105): 0 is the seed's own draw.
    pub fn plate_draw(&self) -> u64 {
        self.plate_draw
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The share of each plate's area that is land, from a Fibonacci sphere of points assigned by the plate rule (the warped direction's nearest centre).
    fn measure_land_shares(&self) -> Vec<f64> {
        let (mut land, mut total) = (vec![0u32; PLATES], vec![0u32; PLATES]);
        let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
        for i in 0..SHARE_POINTS {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / SHARE_POINTS as f64;
            let r = libm::sqrt((1.0 - z * z).max(0.0));
            let phi = golden * i as f64;
            let d = Vec3::new(r * libm::cos(phi), r * libm::sin(phi), z);
            let nearest = self.nearest_three(self.warp(d)).0[0];
            total[nearest] += 1;
            if terrain(self.seed, d).land_depth > 0.0 {
                land[nearest] += 1;
            }
        }
        (0..PLATES).map(|p| if total[p] == 0 { 0.0 } else { f64::from(land[p]) / f64::from(total[p]) }).collect()
    }

    /// The direction warped by 0.35 times a two-octave fBm of frequency 1.3 (GEN-094), re-normalised.
    fn warp(&self, d: Vec3) -> Vec3 {
        let w = Vec3::new(
            fbm(self.seed, LAYER_PLATE_WARP, d, 1.3, 2),
            fbm(self.seed, LAYER_PLATE_WARP, d + Vec3::new(11.0, 0.0, 0.0), 1.3, 2),
            fbm(self.seed, LAYER_PLATE_WARP, d + Vec3::new(0.0, 23.0, 0.0), 1.3, 2),
        );
        (d + w * 0.35).normalized()
    }

    /// The three nearest centres to `q` and the angles of the four nearest, nearest first (ties go to the lower index).
    fn nearest_three(&self, q: Vec3) -> ([usize; 3], [f64; 4]) {
        let (mut idx, mut ang) = ([0usize; 3], [f64::INFINITY; 4]);
        for (i, p) in self.plates.iter().enumerate() {
            let a = q.angle_to(p.centre);
            if a < ang[0] {
                (idx, ang) = ([i, idx[0], idx[1]], [a, ang[0], ang[1], ang[2]]);
            } else if a < ang[1] {
                (idx[1], idx[2], ang[1], ang[2], ang[3]) = (i, idx[1], a, ang[1], ang[2]);
            } else if a < ang[2] {
                (idx[2], ang[2], ang[3]) = (i, a, ang[2]);
            } else if a < ang[3] {
                ang[3] = a;
            }
        }
        (idx, ang)
    }

    /// The smooth land depth (the continent stage without its detail, GEN-104) at `d`: what the coast distance and the coastal arcs follow, because the full value's gradient is not defined at the lattice's scale.
    fn land_depth(&self, d: Vec3) -> f64 {
        crate::continents::continent_smooth(self.seed, d) - SEA_LEVEL_C
    }

    /// The gradient magnitude of the land depth at `d` per radian: central differences of [`GRADIENT_STEP`] along the fixed axes x, y and z, with the radial part removed, so
    /// that it does not depend on a tangent basis (a basis that switches its helper axis near the poles would step the estimate).
    fn land_gradient(&self, d: Vec3) -> f64 {
        let h = GRADIENT_STEP;
        let at = |axis: Vec3| (self.land_depth((d + axis * h).normalized()) - self.land_depth((d - axis * h).normalized())) / (2.0 * h);
        let g = Vec3::new(at(Vec3::new(1.0, 0.0, 0.0)), at(Vec3::new(0.0, 1.0, 0.0)), at(Vec3::new(0.0, 0.0, 1.0)));
        (g - d * g.dot(d)).length()
    }

    /// The first-order estimate in km of the distance over which the land depth changes by `delta` at `d`: `|delta| / |gradient|` (valid near `d`), the gradient
    /// floored at 1e-9 so that the estimate has no step where it vanishes.
    fn distance_km(&self, delta: f64, gradient: f64) -> f64 {
        libm::fabs(delta) / gradient.max(1e-9) * self.radius_m / 1000.0
    }

    /// Like [`distance_km`](Self::distance_km) but signed: `delta / gradient` in km, positive where `delta` is.
    fn signed_km(&self, delta: f64, gradient: f64) -> f64 {
        delta / gradient.max(1e-9) * self.radius_m / 1000.0
    }

    /// The continental weight of a plate in `[0, 1]`: 0 for a plate that is mostly ocean (land share under 0.15), 1 for one with 0.40 of its area land.
    fn continental(&self, plate: usize) -> f64 {
        smoothstep(0.15, 0.40, self.land_share[plate])
    }

    /// The plate mask at `d` (GEN-094, round 1). `land_depth` and `inland` come from the global stage's terrain at `d`.
    ///
    /// Every term is a maximum over the pairs of the three nearest plates, each pair's term symmetric in its two plates (the margin distance, the convergence and
    /// the continental difference do not change when the plates swap), so the mask is continuous where the nearest plate changes. Gates ramp over 100 to 300 km.
    pub fn mask_terms(&self, d: Vec3, land_depth: f64, inland: f64) -> MaskTerms {
        let q = self.warp(d);
        let (idx, ang) = self.nearest_three(q);
        // The pair of the second and third plate is a real margin only next to the margin of the first two (where the nearest plate changes, it takes the place of the pair (0, 2)).
        // Each plate's weight depends on its own angle, so a pair of plates that trade places has the same weight before and after (the margin factor of (second, third) too).
        let rank = |r: usize| smoothstep(0.0, 0.05, ang[3] - ang[r]);
        let pair_weight = [rank(0) * rank(1), rank(0) * rank(2), rank(1) * rank(2) * (1.0 - smoothstep(0.0, 0.04, ang[1] - ang[0]))];
        // The width of a belt: a tent profile of 260 to 560 km half-width before the taper (never under 125 km after it); the belt is cut into segments and its height varies along it.
        let width_km = 200.0 + 220.0 * (0.5 + 0.5 * fbm(self.seed, LAYER_WIDTH, d, 2.5, 2));
        let segment = smoothstep(0.25, 0.55, 0.5 + 0.5 * fbm(self.seed, LAYER_SEGMENT, d, 2.8, 2));
        let along = 0.5 + 0.5 * fbm(self.seed, LAYER_STRIKE, d, 5.0, 2);
        let strike = segment * (0.45 + 0.55 * along);
        // A segment tapers in width as well as in height towards its ends, and its width varies along the strike; the trace of a belt bends sideways.
        let belt_km = (width_km * (0.55 + 0.45 * segment) * (0.9 + 0.2 * along)).max(MIN_BELT_KM);
        let bend_km = BEND_KM * fbm(self.seed, LAYER_BEND, d, 3.0, 2);
        let arc = smoothstep(0.38, 0.56, 0.5 + 0.5 * fbm(self.seed, LAYER_COAST_ARC, d, 0.9, 2));
        let smooth = self.land_depth(d);
        // The gate below is 0 unless both depths are positive, and every term that reads the gradient is multiplied by it: elsewhere the gradient is not computed (7 of 10 points).
        let on_land = land_depth > 0.0 && smooth > 0.0;
        let gradient = if on_land { self.land_gradient(d) } else { 0.0 };
        let line_km = self.signed_km(smooth - COAST_LINE_DEPTH, gradient);
        // The gate of the land: 0 within COAST_BUFFER_KM of the smooth coast and 1 from COAST_RAMP_KM inland, and 0 at the true coast (a depth ramp), so that no belt stops at the
        // coast in a step (GEN-097's limit on the step of `k`) and none stands in the sea or on a peninsula that the smooth coast does not hold.
        let coast_gate = if !on_land {
            0.0
        } else {
            smoothstep(COAST_BUFFER_KM, COAST_RAMP_KM, self.distance_km(smooth, gradient))
                * smoothstep(0.0, COAST_DEPTH_RAMP, smooth)
                * smoothstep(0.0, TRUE_COAST_RAMP, land_depth)
        };
        let (mut core, mut flank, mut old_margin, mut back_arc) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let (mut margin_km, mut convergence) = (0.0, 0.0);
        for (n, &(x, y)) in [(0usize, 1usize), (0, 2), (1, 2)].iter().enumerate() {
            let (px, py) = (&self.plates[idx[x]], &self.plates[idx[y]]);
            let dist_km = 0.5 * (ang[y] - ang[x]) * self.radius_m / 1000.0;
            // The signed distance to the margin, positive on the side of the plate with the lower index: continuous through the margin where the nearest plate changes.
            let across_km = if idx[x] < idx[y] { dist_km } else { -dist_km };
            // The margin normal: from plate x towards plate y, in the tangent plane at d (symmetric: swapping the plates flips it and the velocity difference).
            let mut nrm = py.centre - px.centre;
            nrm = nrm - d * nrm.dot(d);
            let nl = nrm.length();
            let nrm = if nl > 1e-12 { nrm * (1.0 / nl) } else { Vec3::ZERO };
            let conv = (px.rotation.cross(d) - py.rotation.cross(d)).dot(nrm);
            if n == 0 {
                (margin_km, convergence) = (dist_km, conv);
            }
            let conv_gate = smoothstep(0.05, 0.35, conv) * pair_weight[n];
            // A coastal belt only where a convergent margin with an oceanic side lies within a few hundred km: one plate continental, the other oceanic.
            let (cx, cy) = (self.continental(idx[x]), self.continental(idx[y]));
            let one_oceanic = cx * (1.0 - cy) + cy * (1.0 - cx);
            let near = 1.0 - smoothstep(250.0, 500.0, dist_km);
            let term = |w: f64| {
                let belt = conv_gate * (1.0 - libm::fabs(across_km - bend_km) / w).max(0.0);
                let coast = arc * one_oceanic * near * conv_gate * (1.0 - libm::fabs(line_km - bend_km) / (0.8 * w)).max(0.0);
                belt.max(coast)
            };
            core = core.max(term(belt_km));
            flank = flank.max(term(3.0 * belt_km));
            old_margin = old_margin.max(pair_weight[n] * (1.0 - smoothstep(-0.05, 0.15, conv)) * (1.0 - dist_km / 300.0).max(0.0));
            if x == 0 {
                // The back-arc lies behind the margin, on the plate of the pair with the larger land share (a smooth preference), and starts 0 at the margin.
                let prefer = smoothstep(-0.08, 0.08, self.land_share[idx[0]] - self.land_share[idx[y]]);
                let b = smoothstep(0.7, 0.95, conv)
                    * (1.0 - smoothstep(450.0, 700.0, dist_km))
                    * smoothstep(0.0, 150.0, dist_km)
                    * smoothstep(0.0, 150.0, margin_km)
                    * pair_weight[n]
                    * smoothstep(0.1, 0.5, inland)
                    * prefer;
                back_arc = back_arc.max(b);
            }
        }
        let (core, flank) = (core * strike * coast_gate, flank * strike * coast_gate);
        let old_margin = old_margin * coast_gate;
        let c = 0.5 + 0.5 * fbm(self.seed, LAYER_PLATEAU, d, 1.6, 3);
        let plateau_field = smoothstep(0.45, 0.70, c) * smoothstep(0.2, 0.6, inland) * (1.0 - smoothstep(0.2, 0.6, flank.max(back_arc)));
        let hilly = smoothstep(0.50, 0.76, 0.5 + 0.5 * fbm(self.seed, LAYER_HILLY, d, 2.2, 3)) * smoothstep(0.1, 0.4, smooth);
        MaskTerms { margin_km, convergence, core, flank, old_margin, back_arc, plateau_field, hilly, strike, plate: idx[0] }
    }

    /// The default source at the unit direction `d`: the mask, the base elevation without the hill band (GEN-096), and the class weights (GEN-095).
    pub fn eval(&self, d: Vec3) -> ClassPoint {
        let t = terrain(self.seed, d);
        let terms = self.mask_terms(d, t.land_depth, t.inland);
        let coast = if t.land_depth < 0.0 {
            -4500.0 * smoothstep(0.0, 0.9, -t.land_depth)
        } else {
            COAST_TOP_M * smoothstep(0.0, COAST_RAMP_DEPTH, t.land_depth)
        };
        let ridge = ridge_at(self.seed, t.warped);
        let e0 = (coast
            + BELT_AMPLITUDE_M * terms.core * (0.5 + 0.5 * ridge)
            + 2000.0 * terms.back_arc
            + 200.0 * terms.old_margin.max(0.5 * terms.flank)
            + PLATEAU_UPLIFT_M * terms.plateau_field)
            .clamp(MIN_ELEVATION_M, MAX_ELEVATION_M);
        let rolling = smoothstep(0.45, 0.6, 0.5 + 0.5 * fbm(self.seed, LAYER_ROLLING, d, 3.0, 3));
        // The class decision reads the base elevation without the ridge term's fine structure, so that the weights change gently (GEN-097's step limit).
        let e_smooth = (coast
            + BELT_AMPLITUDE_M * terms.core * 0.75
            + 2000.0 * terms.back_arc
            + 200.0 * terms.old_margin.max(0.5 * terms.flank)
            + PLATEAU_UPLIFT_M * terms.plateau_field)
            .clamp(MIN_ELEVATION_M, MAX_ELEVATION_M);
        ClassPoint { terms, land_depth: t.land_depth, e0, weights: class_weights(&terms, e_smooth, rolling) }
    }

    /// The elevation of GEN-096 at `d` given the untapered blended `k` there: `e0` plus the class hill band (GEN-098), clamped. `point` is `eval(d)`.
    pub fn elevation_m(&self, d: Vec3, point: &ClassPoint, k: f64) -> f64 {
        let t = terrain(self.seed, d);
        let band = if t.land_depth <= 0.0 {
            0.0
        } else {
            hill_amplitude_for_k(k) * hill_octaves(self.seed, &t) * smoothstep(0.0, 0.1, t.land_depth)
        };
        (point.e0 + band).clamp(MIN_ELEVATION_M, MAX_ELEVATION_M)
    }
}

/// The hill band's amplitude in metres for the untapered blended `k` (GEN-098).
pub fn hill_amplitude_for_k(k: f64) -> f64 {
    30.0 + 145.0 * smoothstep(1.0, 5.0, k) + 650.0 * smoothstep(5.0, 7.5, k)
}

/// The default class weights from the mask terms, the class elevation `e_class` (GEN-096 without the band and the ridge factor) and the rolling-plain membership (GEN-095), summing to 1.
pub fn class_weights(terms: &MaskTerms, e0: f64, rolling: f64) -> [f64; CLASS_COUNT] {
    // Mountains follow the core with a gain, so the flanks of a belt that is high enough to be above 2 km are mountains, not hills.
    let mt = smoothstep(0.0, MOUNTAIN_FULL_CORE, terms.core.max(FLANK_MEMBERSHIP * terms.flank));
    let high = mt * smoothstep(HIGH_FROM_M, HIGH_TO_M, e0);
    let hills = (1.0 - mt) * smoothstep(0.1, 0.35, terms.flank.max(terms.old_margin).max(terms.hilly));
    // What mountains and hills leave is shared by the plateau and the rest, so the memberships partition 1 before the normalisation.
    let left = 1.0 - mt - hills;
    let plateau = left * smoothstep(0.3, 0.6, terms.plateau_field.max(terms.back_arc));
    let rest = left - plateau;
    let mut w = [0.0; CLASS_COUNT];
    w[PLAIN] = rest * (1.0 - rolling);
    w[ROLLING] = rest * rolling;
    w[HILLS] = hills;
    w[MOUNTAINS] = mt - high;
    w[HIGH_MOUNTAINS] = high;
    w[PLATEAU] = plateau;
    let sum: f64 = w.iter().sum();
    for x in &mut w {
        *x /= sum;
    }
    w
}

/// The relief driver `k` blended by the weights (GEN-097, untapered).
pub fn blended_k(weights: &[f64; CLASS_COUNT]) -> f64 {
    weights.iter().zip(CLASS_K).map(|(w, k)| w * k).sum()
}

/// The `u8` weights of GEN-093: the f64 weights scaled by 255 and rounded down, the remainder given one unit at a time to the classes with the largest
/// fractional parts, ties to the earlier class.
pub fn quantize_weights(weights: &[f64; CLASS_COUNT]) -> [u8; CLASS_COUNT] {
    let scaled: Vec<f64> = weights.iter().map(|w| w * 255.0).collect();
    let mut out = [0u8; CLASS_COUNT];
    for (o, s) in out.iter_mut().zip(&scaled) {
        *o = libm::floor(*s).clamp(0.0, 255.0) as u8;
    }
    let mut remainder = 255i32 - out.iter().map(|&v| i32::from(v)).sum::<i32>();
    let mut order: Vec<usize> = (0..CLASS_COUNT).collect();
    order.sort_by(|&a, &b| {
        let (fa, fb) = (scaled[a] - libm::floor(scaled[a]), scaled[b] - libm::floor(scaled[b]));
        fb.partial_cmp(&fa).expect("finite weights").then(a.cmp(&b))
    });
    for &c in order.iter().cycle() {
        if remainder <= 0 {
            break;
        }
        out[c] += 1;
        remainder -= 1;
    }
    out
}

/// The class weights on a lattice over the cube faces, `u8` per class, read bilinearly (GEN-093). The one interface the consumers read.
pub struct ClassLattice {
    cells: usize,
    faces: Vec<Vec<[u8; CLASS_COUNT]>>,
}

impl ClassLattice {
    /// The lattice of the production geometry ([`CLASS_LATTICE_N`] cells per face edge), built on `threads` threads: the bytes do not depend on the count.
    pub fn build(source: &ClassSource, threads: usize) -> ClassLattice {
        Self::build_with(source, CLASS_LATTICE_N, threads)
    }

    /// A lattice of `cells` cells per face edge (tests use a coarse one).
    pub fn build_with(source: &ClassSource, cells: usize, threads: usize) -> ClassLattice {
        // A power of two, so the face coordinates `2 i / n - 1` are exact and a shared edge point is the same direction on both faces (GEN-093).
        assert!(cells.is_power_of_two(), "the class lattice has a power of two cells per face edge, not {cells}");
        let n = cells;
        let map = TangentWarp;
        let par: &dyn Par = &threads;
        // One row of one face per piece of work: every value is independent of the others.
        let rows = par::map(par, 6 * (n + 1), &|idx| {
            let (f, j) = (idx / (n + 1), idx % (n + 1));
            let t = 2.0 * j as f64 / n as f64 - 1.0;
            (0..=n)
                .map(|i| {
                    let s = 2.0 * i as f64 / n as f64 - 1.0;
                    quantize_weights(&source.eval(face_to_direction(&map, Face(f as u8), s, t)).weights)
                })
                .collect::<Vec<_>>()
        });
        let mut faces: Vec<Vec<[u8; CLASS_COUNT]>> = (0..6).map(|_| Vec::with_capacity((n + 1) * (n + 1))).collect();
        for (idx, row) in rows.into_iter().enumerate() {
            faces[idx / (n + 1)].extend(row);
        }
        ClassLattice { cells: n, faces }
    }

    pub fn cells(&self) -> usize {
        self.cells
    }

    /// The `u8` weights at lattice point `(i, j)` of `face`.
    pub fn point(&self, face: Face, i: usize, j: usize) -> [u8; CLASS_COUNT] {
        self.faces[face.0 as usize][j * (self.cells + 1) + i]
    }

    fn locate(&self, s: f64) -> (usize, f64) {
        let x = ((s + 1.0) * 0.5 * self.cells as f64).clamp(0.0, self.cells as f64);
        let i = (x as usize).min(self.cells - 1);
        (i, x - i as f64)
    }

    /// The weights at the unit direction `d`: the bilinear interpolant of the `u8` weights over 255, continuous across face edges, summing to 1.
    pub fn weights_at(&self, d: Vec3) -> [f64; CLASS_COUNT] {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let ((i, fx), (j, fy)) = (self.locate(s), self.locate(t));
        let (p00, p10, p01, p11) =
            (self.point(face, i, j), self.point(face, i + 1, j), self.point(face, i, j + 1), self.point(face, i + 1, j + 1));
        let mut w = [0.0; CLASS_COUNT];
        for c in 0..CLASS_COUNT {
            let (a, b) = (
                f64::from(p00[c]) + (f64::from(p10[c]) - f64::from(p00[c])) * fx,
                f64::from(p01[c]) + (f64::from(p11[c]) - f64::from(p01[c])) * fx,
            );
            w[c] = (a + (b - a) * fy) / 255.0;
        }
        w
    }

    /// The largest step of the untapered `k` between neighbouring lattice points (GEN-097's limit of 2.0).
    pub fn largest_k_step(&self) -> f64 {
        self.largest_k_step_at().0
    }

    /// The largest step of the untapered `k` and where it is: the lattice point `(face, i, j)` of the pair's first point.
    pub fn largest_k_step_at(&self) -> (f64, Face, usize, usize) {
        let n = self.cells;
        let k = |face: usize, i: usize, j: usize| {
            let p = self.faces[face][j * (n + 1) + i];
            p.iter().zip(CLASS_K).map(|(&w, k)| f64::from(w) / 255.0 * k).sum::<f64>()
        };
        let mut worst = (0.0f64, Face(0), 0usize, 0usize);
        for face in 0..6 {
            for j in 0..=n {
                for i in 0..=n {
                    for (di, dj) in [(1, 0), (0, 1)] {
                        if i + di <= n && j + dj <= n {
                            let step = libm::fabs(k(face, i + di, j + dj) - k(face, i, j));
                            if step > worst.0 {
                                worst = (step, Face(face as u8), i, j);
                            }
                        }
                    }
                }
            }
        }
        worst
    }

    /// The share of neighbouring lattice point pairs whose untapered `k` differs by more than `limit` (the report beside the largest step).
    pub fn k_steps_over(&self, limit: f64) -> f64 {
        let n = self.cells;
        let k = |face: usize, i: usize, j: usize| {
            let p = self.faces[face][j * (n + 1) + i];
            p.iter().zip(CLASS_K).map(|(&w, k)| f64::from(w) / 255.0 * k).sum::<f64>()
        };
        let (mut over, mut total) = (0u64, 0u64);
        for face in 0..6 {
            for j in 0..=n {
                for i in 0..=n {
                    for (di, dj) in [(1, 0), (0, 1)] {
                        if i + di <= n && j + dj <= n {
                            total += 1;
                            if libm::fabs(k(face, i + di, j + dj) - k(face, i, j)) > limit {
                                over += 1;
                            }
                        }
                    }
                }
            }
        }
        over as f64 / total as f64
    }

    /// The unit direction of lattice point `(i, j)` of `face`.
    pub fn direction(&self, face: Face, i: usize, j: usize) -> Vec3 {
        let n = self.cells as f64;
        face_to_direction(&TangentWarp, face, 2.0 * i as f64 / n - 1.0, 2.0 * j as f64 / n - 1.0)
    }
}
