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
pub const CLASS_SOURCE_VERSION: u32 = 1;
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
/// Radius of the body in metres used for the margin distances; the default source is defined for Earth's radius (GEN-094's kilometres).
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
/// Land-share sample points per plate set (a Fibonacci sphere), for the back-arc's choice of the plate with more land.
const SHARE_POINTS: usize = 2048;
/// Step of the finite difference that gives the distance to the coastal line (radians, about 25 km).
const GRADIENT_STEP: f64 = 0.004;
/// The land depth of the coastal line the Andean arcs follow (GEN-094).
const COAST_LINE_DEPTH: f64 = 0.10;

const LAYER_PLATE_WARP: u64 = 0x474c_0010;
const LAYER_WIDTH: u64 = 0x474c_0011;
const LAYER_COAST_ARC: u64 = 0x474c_0012;
const LAYER_PLATEAU: u64 = 0x474c_0013;
const LAYER_ROLLING: u64 = 0x474c_0014;

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
    /// The margin's convergence, positive where the nearest plate moves towards the other (not bounded by 1).
    pub convergence: f64,
    pub core: f64,
    pub flank: f64,
    pub old_margin: f64,
    pub back_arc: f64,
    pub plateau_field: f64,
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
}

impl ClassSource {
    pub fn new(seed: u64) -> ClassSource {
        let plates: Vec<Plate> = (0..PLATES as u64)
            .map(|i| {
                let centre = on_sphere(unit(seed, 1, 2 * i), unit(seed, 1, 2 * i + 1));
                let pole = on_sphere(unit(seed, 2, 2 * i), unit(seed, 2, 2 * i + 1));
                let rate = 0.3 + 0.7 * unit(seed, 3, i);
                Plate { centre, rotation: pole * rate }
            })
            .collect();
        let mut source = ClassSource { seed, radius_m: EARTH_RADIUS_M, plates, land_share: vec![0.0; PLATES] };
        source.land_share = source.measure_land_shares();
        source
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
            let (nearest, _, _, _) = self.nearest_two(self.warp(d));
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

    /// The nearest and second-nearest centres to `q` and their angles (ties go to the lower index).
    fn nearest_two(&self, q: Vec3) -> (usize, usize, f64, f64) {
        let (mut i1, mut i2, mut a1, mut a2) = (0usize, 0usize, f64::INFINITY, f64::INFINITY);
        for (i, p) in self.plates.iter().enumerate() {
            let a = q.angle_to(p.centre);
            if a < a1 {
                (i2, a2) = (i1, a1);
                (i1, a1) = (i, a);
            } else if a < a2 {
                (i2, a2) = (i, a);
            }
        }
        (i1, i2, a1, a2)
    }

    /// The land depth (continent value minus the sea level) at `d`, the term the coastal arcs follow.
    fn land_depth(&self, d: Vec3) -> f64 {
        terrain(self.seed, d).continent - SEA_LEVEL_C
    }

    /// The distance in km from `d` to the line of land depth [`COAST_LINE_DEPTH`], by the first-order estimate `|f| / |grad f|` with a central difference.
    fn coast_line_km(&self, d: Vec3, land_depth: f64) -> f64 {
        let (e1, e2) = d.tangent_basis();
        let h = GRADIENT_STEP;
        let at = |t: Vec3, s: f64| self.land_depth((d + t * s).normalized());
        let g1 = (at(e1, h) - at(e1, -h)) / (2.0 * h);
        let g2 = (at(e2, h) - at(e2, -h)) / (2.0 * h);
        let grad = libm::sqrt(g1 * g1 + g2 * g2);
        if grad < 1e-9 {
            return f64::INFINITY;
        }
        libm::fabs(land_depth - COAST_LINE_DEPTH) / grad * self.radius_m / 1000.0
    }

    /// The plate mask at `d` (GEN-094). `land_depth` and `inland` come from the global stage's terrain at `d`.
    pub fn mask_terms(&self, d: Vec3, land_depth: f64, inland: f64) -> MaskTerms {
        let q = self.warp(d);
        let (i1, i2, a1, a2) = self.nearest_two(q);
        let (p1, p2) = (&self.plates[i1], &self.plates[i2]);
        let margin_km = 0.5 * (a2 - a1) * self.radius_m / 1000.0;
        // The margin normal: from the nearest towards the second-nearest centre, in the tangent plane at d.
        let mut n = p2.centre - p1.centre;
        n = n - d * n.dot(d);
        let nl = n.length();
        let n = if nl > 1e-12 { n * (1.0 / nl) } else { Vec3::ZERO };
        let rel = p1.rotation.cross(d) - p2.rotation.cross(d);
        let convergence = rel.dot(n);

        let conv_gate = smoothstep(0.05, 0.35, convergence);
        let width_km = 90.0 * (0.6 + 0.9 * (0.5 + 0.5 * fbm(self.seed, LAYER_WIDTH, d, 2.5, 2)));
        let arc = smoothstep(0.40, 0.52, 0.5 + 0.5 * fbm(self.seed, LAYER_COAST_ARC, d, 0.9, 2));
        let line_km = self.coast_line_km(d, land_depth);
        let term = |w: f64| {
            let belt = conv_gate * libm::exp(-(margin_km / w) * (margin_km / w));
            let coast = arc * libm::exp(-(line_km / (0.8 * w)) * (line_km / (0.8 * w)));
            belt.max(coast)
        };
        let coast_gate = smoothstep(0.0, 0.04, land_depth);
        let core = term(width_km) * coast_gate;
        let flank = term(3.0 * width_km) * coast_gate;
        let old_margin = (1.0 - smoothstep(-0.05, 0.15, convergence)) * libm::exp(-(margin_km / 150.0) * (margin_km / 150.0)) * coast_gate;
        // The back-arc lies on the plate of the pair with the larger land share.
        let on_bigger_plate = self.land_share[i1] > self.land_share[i2];
        let back_arc = if on_bigger_plate {
            smoothstep(0.7, 0.95, convergence) * (1.0 - smoothstep(450.0, 700.0, margin_km)) * smoothstep(0.1, 0.5, inland)
        } else {
            0.0
        };
        let c = 0.5 + 0.5 * fbm(self.seed, LAYER_PLATEAU, d, 1.6, 3);
        let plateau_field = smoothstep(0.50, 0.62, c) * smoothstep(0.2, 0.6, inland) * (1.0 - smoothstep(0.2, 0.6, flank.max(back_arc)));
        MaskTerms { margin_km, convergence, core, flank, old_margin, back_arc, plateau_field, plate: i1 }
    }

    /// The default source at the unit direction `d`: the mask, the base elevation without the hill band (GEN-096), and the class weights (GEN-095).
    pub fn eval(&self, d: Vec3) -> ClassPoint {
        let t = terrain(self.seed, d);
        let terms = self.mask_terms(d, t.land_depth, t.inland);
        let coast =
            if t.land_depth < 0.0 { -4500.0 * smoothstep(0.0, 0.9, -t.land_depth) } else { 1000.0 * smoothstep(0.0, 0.9, t.land_depth) };
        let ridge = ridge_at(self.seed, t.warped);
        let e0 = (coast
            + 5500.0 * terms.core * (0.5 + 0.5 * ridge)
            + 2000.0 * terms.back_arc
            + 200.0 * terms.old_margin.max(0.5 * terms.flank)
            + 1200.0 * terms.plateau_field)
            .clamp(MIN_ELEVATION_M, MAX_ELEVATION_M);
        let rolling = smoothstep(0.45, 0.6, 0.5 + 0.5 * fbm(self.seed, LAYER_ROLLING, d, 3.0, 3));
        ClassPoint { terms, land_depth: t.land_depth, e0, weights: class_weights(&terms, e0, rolling) }
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

/// The default class weights from the mask terms, the base elevation `e0` (GEN-096 without the band) and the rolling-plain membership (GEN-095), summing to 1.
pub fn class_weights(terms: &MaskTerms, e0: f64, rolling: f64) -> [f64; CLASS_COUNT] {
    let mt = smoothstep(0.25, 0.55, terms.core);
    let high = mt * smoothstep(2000.0, 3000.0, e0);
    let hills = (1.0 - mt) * smoothstep(0.2, 0.45, terms.flank.max(terms.old_margin));
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
        let n = self.cells;
        let k = |face: usize, i: usize, j: usize| {
            let p = self.faces[face][j * (n + 1) + i];
            p.iter().zip(CLASS_K).map(|(&w, k)| f64::from(w) / 255.0 * k).sum::<f64>()
        };
        let mut worst = 0.0f64;
        for face in 0..6 {
            for j in 0..=n {
                for i in 0..=n {
                    if i < n {
                        worst = worst.max(libm::fabs(k(face, i + 1, j) - k(face, i, j)));
                    }
                    if j < n {
                        worst = worst.max(libm::fabs(k(face, i, j + 1) - k(face, i, j)));
                    }
                }
            }
        }
        worst
    }
}
