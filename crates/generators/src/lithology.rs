//! The lithology field: an analytic erodibility K(x, z) for the erosion carve (ADR 0102, GEN-116; SPIKE-102 F1, SPIKE-81's hardness).
//!
//! CPU-canonical: `libm` for every transcendental, integer hashing for every random number, results the same bits for any thread count (CON-14). The field is **never stored**: it is a pure function of
//! `(seed, direction, height)` and [`LITHOLOGY_VERSION`], which a consumer folds into its keys. All numbers here are start values; the owner's preview round decides them.
//!
//! Three layers, from the plate history to the rock a cell of the carve meets at its current height:
//! 1. **Provinces**: the plate-history terms of the default class source (the craton, the orogen core, the foreland, the basin, the rift, the old margin) on a lattice of the class lattice's
//!    geometry ([`ProvinceLattice`], built in memory, read bilinearly). The class source's mask is far too dear to evaluate for every cell of a bake; the provinces are hundreds of kilometres across.
//! 2. **Hardness** `H(d)` in `[0, 1]`: SPIKE-81's formula of the province terms plus a noise ([`Lithology::hardness`]).
//! 3. **Strata** `K(d, z)`: an unbounded stack of bands, 30 to 170 m thick, each with a hardness offset from the province's `H`; the stack is tilted and folded by a smooth offset of the height
//!    (strongly in orogens, gently on cratons), so a hard band outcrops as a ridge or a cuesta and the rock the carve meets depends on the height it has cut to. A band above
//!    [`CAPROCK_HARDNESS`] is a caprock; a band of carbonate is soluble where a province-scale gate allows it ([`Column::at`]).
//!
//! The values of a column at several heights share the province read and the noise: [`Lithology::column`] once, [`Column::at`] per height.

use crate::classes::{ClassSource, CLASS_LATTICE_N};
use crate::global::{fbm, terrain};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::hash::hash_u64s;
use planet_core::par::{self, Par};
use planet_core::{smoothstep, Face, Vec3};

/// Bump whenever the field's output changes for a seed (the formulas, the constants, the province terms or their lattice); a consumer folds it into its keys.
pub const LITHOLOGY_VERSION: u32 = 1;

/// The nominal thickness of a stratigraphic band in metres; the bands' edges jitter by up to [`BAND_JITTER`] of it, so a band is 30 to 170 m thick.
pub const BAND_NOMINAL_M: f64 = 100.0;
/// The jitter of a band edge in nominal thicknesses (a band is `1 +- 2 * 0.35` nominal thicknesses).
pub const BAND_JITTER: f64 = 0.35;
/// The thinnest and the thickest band in metres.
pub const BAND_MIN_M: f64 = BAND_NOMINAL_M * (1.0 - 2.0 * BAND_JITTER);
pub const BAND_MAX_M: f64 = BAND_NOMINAL_M * (1.0 + 2.0 * BAND_JITTER);
/// The hardness above which a band is a caprock.
pub const CAPROCK_HARDNESS: f64 = 0.92;
/// The share of the band stack that is carbonate (soluble where the province allows it).
pub const CARBONATE_SHARE: f64 = 0.25;
/// The erodibility of a band of hardness 0 and of hardness 1 (see [`erodibility`]).
pub const K_SOFT: f64 = 1.8;
pub const K_HARD: f64 = 0.12;
/// The offset field's amplitude in metres on a craton and at the heart of an orogen (the stack is tilted and folded by it), and its two frequencies.
pub const OFFSET_CRATON_M: f64 = 150.0;
pub const OFFSET_OROGEN_M: f64 = 1200.0;
const TILT_FREQUENCY: f64 = 60.0;
const FOLD_FREQUENCY: f64 = 120.0;
/// The share of a band's thickness over which its erodibility blends into its neighbour's (each side of an edge).
pub const BAND_BLEND: f64 = 0.08;
const _: () = assert!(BAND_JITTER < 0.5, "the band lookup assumes an edge jitters by under half a nominal thickness");

const LAYER_HARDNESS: u64 = 0x4c49_0001;
const LAYER_TILT: u64 = 0x4c49_0002;
const LAYER_FOLD: u64 = 0x4c49_0003;
const LAYER_BAND: u64 = 0x4c49_0004;
const LAYER_SOLUBLE: u64 = 0x4c49_0005;
const TAG_BAND_EDGE: u64 = 0x4c49_0101;
const TAG_BAND_HARD: u64 = 0x4c49_0102;
const TAG_BAND_CARBONATE: u64 = 0x4c49_0103;

/// The province terms at a direction, each in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Provinces {
    /// Far from every plate margin (500 to 1,100 km): old, stable, hard.
    pub craton: f64,
    /// The orogen core: hard.
    pub core: f64,
    /// The foreland: the flank of an orogen outside its core: soft.
    pub foreland: f64,
    /// A lowland basin of sediment: soft.
    pub basin: f64,
    /// A rift: a divergent margin.
    pub rift: f64,
    /// The old margin.
    pub old_margin: f64,
}

const TERMS: usize = 6;

impl Provinces {
    fn to_array(self) -> [f64; TERMS] {
        [self.craton, self.core, self.foreland, self.basin, self.rift, self.old_margin]
    }

    fn from_array(a: [f64; TERMS]) -> Provinces {
        Provinces { craton: a[0], core: a[1], foreland: a[2], basin: a[3], rift: a[4], old_margin: a[5] }
    }

    /// The province terms of the class source at `d` (the ground truth the lattice samples).
    pub fn of_source(source: &ClassSource, d: Vec3) -> Provinces {
        let p = source.eval(d);
        let t = terrain(source.seed(), d);
        let m = p.terms;
        let craton = smoothstep(500.0, 1100.0, m.margin_km);
        let foreland = m.flank * (1.0 - smoothstep(0.2, 0.5, m.core));
        let basin = (1.0 - smoothstep(200.0, 700.0, p.e0)) * smoothstep(0.3, 0.7, t.inland);
        // A divergent margin (the plates move apart) near the margin line.
        let rift = smoothstep(-0.15, -0.35, m.convergence) * (1.0 - smoothstep(100.0, 200.0, m.margin_km));
        Provinces { craton, core: m.core, foreland, basin, rift, old_margin: m.old_margin }
    }
}

/// The province terms on a lattice over the cube faces, `u8` per term, read bilinearly (the class lattice's geometry, never stored).
#[derive(Debug)]
pub struct ProvinceLattice {
    cells: usize,
    faces: Vec<Vec<[u8; TERMS]>>,
}

impl ProvinceLattice {
    /// The lattice of the production geometry ([`CLASS_LATTICE_N`] cells per face edge) built on `threads` threads: the bytes do not depend on the count.
    pub fn build(source: &ClassSource, threads: usize) -> ProvinceLattice {
        Self::build_with(source, CLASS_LATTICE_N, threads)
    }

    /// A lattice of `cells` cells per face edge (a power of two, so a shared edge point is the same direction on both faces; tests use a coarse one).
    pub fn build_with(source: &ClassSource, cells: usize, threads: usize) -> ProvinceLattice {
        assert!(cells.is_power_of_two(), "the province lattice has a power of two cells per face edge, not {cells}");
        let n = cells;
        let par: &dyn Par = &threads;
        let rows = par::map(par, 6 * (n + 1), &|idx| {
            let (f, j) = (idx / (n + 1), idx % (n + 1));
            let t = 2.0 * j as f64 / n as f64 - 1.0;
            (0..=n)
                .map(|i| {
                    let s = 2.0 * i as f64 / n as f64 - 1.0;
                    Provinces::of_source(source, face_to_direction(&TangentWarp, Face(f as u8), s, t))
                        .to_array()
                        .map(|v| libm::round(255.0 * v.clamp(0.0, 1.0)) as u8)
                })
                .collect::<Vec<_>>()
        });
        let mut faces: Vec<Vec<[u8; TERMS]>> = (0..6).map(|_| Vec::with_capacity((n + 1) * (n + 1))).collect();
        for (idx, row) in rows.into_iter().enumerate() {
            faces[idx / (n + 1)].extend(row);
        }
        ProvinceLattice { cells: n, faces }
    }

    pub fn cells(&self) -> usize {
        self.cells
    }

    /// The `u8` terms at lattice point `(i, j)` of `face`.
    pub fn point(&self, face: Face, i: usize, j: usize) -> [u8; TERMS] {
        self.faces[face.0 as usize][j * (self.cells + 1) + i]
    }

    fn locate(&self, s: f64) -> (usize, f64) {
        let x = ((s + 1.0) * 0.5 * self.cells as f64).clamp(0.0, self.cells as f64);
        let i = (x as usize).min(self.cells - 1);
        (i, x - i as f64)
    }

    /// The terms at the unit direction `d`: the bilinear interpolant of the `u8` terms, continuous across face edges.
    pub fn at(&self, d: Vec3) -> Provinces {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let ((i, fx), (j, fy)) = (self.locate(s), self.locate(t));
        let (p00, p10, p01, p11) =
            (self.point(face, i, j), self.point(face, i + 1, j), self.point(face, i, j + 1), self.point(face, i + 1, j + 1));
        let mut out = [0.0; TERMS];
        for c in 0..TERMS {
            let (a, b) = (
                f64::from(p00[c]) + (f64::from(p10[c]) - f64::from(p00[c])) * fx,
                f64::from(p01[c]) + (f64::from(p11[c]) - f64::from(p01[c])) * fx,
            );
            out[c] = (a + (b - a) * fy) / 255.0;
        }
        Provinces::from_array(out)
    }
}

/// The erodibility of a band of hardness `h`: `K_HARD + (K_SOFT - K_HARD) (1 - h)^1.5`, so a soft band is eroded about fifteen times faster than a hard one.
pub fn erodibility(h: f64) -> f64 {
    K_HARD + (K_SOFT - K_HARD) * libm::pow((1.0 - h).clamp(0.0, 1.0), 1.5)
}

/// A number in `[0, 1)` from the hash of `(seed, tag, n)`.
fn hash01(seed: u64, tag: u64, n: i64) -> f64 {
    (hash_u64s(tag, &[seed, n as u64]) >> 11) as f64 / (1u64 << 53) as f64
}

/// What the rock at one point and height is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stratum {
    /// The erodibility the carve reads there (blended across a band's edge, so continuous in the height).
    pub k: f64,
    /// The hardness of the band the point lies in.
    pub hardness: f64,
    /// Whether the band is a caprock (hardness above [`CAPROCK_HARDNESS`]).
    pub caprock: bool,
    /// The solubility in `[0, 1]`: 1 in a carbonate band where the province allows it, 0 elsewhere.
    pub soluble: f64,
    /// The index of the band in the stack (the stratigraphic coordinate's cell).
    pub band: i64,
}

/// The lithology of one seed over a province lattice.
pub struct Lithology {
    seed: u64,
    provinces: ProvinceLattice,
}

/// The lithology at one direction, with everything that does not depend on the height computed once.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    seed: u64,
    /// The province hardness `H(d)`.
    pub hardness: f64,
    /// The offset of the stack in metres (tilt and folds): a point at height `z` lies in the stack at `z - offset`.
    pub offset_m: f64,
    /// The province-scale gate of the carbonate bands in `[0, 1]`.
    pub soluble_gate: f64,
    d: Vec3,
}

impl Lithology {
    /// The lithology of `seed` over the province lattice of `source`, built on `threads` threads.
    pub fn new(source: &ClassSource, threads: usize) -> Lithology {
        Lithology { seed: source.seed(), provinces: ProvinceLattice::build(source, threads) }
    }

    /// The lithology over a province lattice built elsewhere (tests use a coarse one).
    pub fn with_provinces(seed: u64, provinces: ProvinceLattice) -> Lithology {
        Lithology { seed, provinces }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn provinces(&self) -> &ProvinceLattice {
        &self.provinces
    }

    /// SPIKE-81's hardness of the province terms `p` and the noise `noise` in `[-1, 1]`: `0.45 + 0.4 craton (1 - basin) + 0.35 core - 0.4 foreland - 0.35 basin + 0.25 noise`, clamped.
    pub fn hardness_of(p: &Provinces, noise: f64) -> f64 {
        (0.45 + 0.4 * p.craton * (1.0 - p.basin) + 0.35 * p.core - 0.4 * p.foreland - 0.35 * p.basin + 0.25 * noise).clamp(0.0, 1.0)
    }

    /// The hardness `H` at the unit direction `d` in `[0, 1]`.
    pub fn hardness(&self, d: Vec3) -> f64 {
        Self::hardness_of(&self.provinces.at(d), fbm(self.seed, LAYER_HARDNESS, d, 6.0, 4))
    }

    /// The column at `d`: the province read and the noises shared by every height.
    pub fn column(&self, d: Vec3) -> Column {
        let p = self.provinces.at(d);
        let hardness = Self::hardness_of(&p, fbm(self.seed, LAYER_HARDNESS, d, 6.0, 4));
        // The stack is tilted (a low frequency, gentle dips: cuestas) and folded (a higher one, strong in an orogen); an orogen's folds are the tilt's amplitude times its belt.
        let belt = p.core.max(p.foreland);
        let amp = OFFSET_CRATON_M + (OFFSET_OROGEN_M - OFFSET_CRATON_M) * belt;
        let offset_m =
            amp * (0.4 * fbm(self.seed, LAYER_TILT, d, TILT_FREQUENCY, 2) + 0.6 * fbm(self.seed, LAYER_FOLD, d, FOLD_FREQUENCY, 2));
        // Carbonate is laid down on stable, old ground and on the old margins, and is not there in a basin of sediment; a low-frequency noise breaks the provinces into regions.
        let base = 0.6 * p.craton + 0.8 * p.old_margin + 0.3 * (1.0 - p.basin);
        let soluble_gate = smoothstep(0.35, 0.7, base) * smoothstep(-0.15, 0.3, fbm(self.seed, LAYER_SOLUBLE, d, 1.8, 2));
        Column { seed: self.seed, hardness, offset_m, soluble_gate, d }
    }

    /// The rock at `d` and height `z` in metres.
    pub fn strata(&self, d: Vec3, z: f64) -> Stratum {
        self.column(d).at(z)
    }

    /// The erodibility at `d` and height `z`. It rebuilds the [`Column`] on every call: for many heights at one direction (a carve's steps), take [`Lithology::column`] once and call [`Column::at`].
    pub fn k_at(&self, d: Vec3, z: f64) -> f64 {
        self.strata(d, z).k
    }
}

impl Column {
    /// The edge `n` of the band stack in nominal thicknesses: band `n` spans `[edge(n), edge(n + 1))`.
    fn edge(&self, n: i64) -> f64 {
        n as f64 + BAND_JITTER * (2.0 * hash01(self.seed, TAG_BAND_EDGE, n) - 1.0)
    }

    /// The hardness of band `n`: the column's hardness, a per-band offset of up to 0.45 and a smooth variation across the region.
    fn band_hardness(&self, n: i64) -> f64 {
        let off = 0.45 * (2.0 * hash01(self.seed, TAG_BAND_HARD, n) - 1.0);
        let region = 0.15 * fbm(self.seed, LAYER_BAND, self.d + Vec3::new(0.0, 0.0, 0.37 * (n.rem_euclid(97)) as f64), 1.5, 1);
        (self.hardness + off + region).clamp(0.0, 1.0)
    }

    fn band_carbonate(&self, n: i64) -> bool {
        hash01(self.seed, TAG_BAND_CARBONATE, n) < CARBONATE_SHARE
    }

    /// The rock at height `z` in metres.
    pub fn at(&self, z: f64) -> Stratum {
        let u = (z - self.offset_m) / BAND_NOMINAL_M;
        let n0 = libm::floor(u) as i64;
        // The band that holds `u`: its edges jitter by less than half a nominal thickness, so it is one of the three around `n0`; the edges already read are not read again.
        let (e0, e1) = (self.edge(n0), self.edge(n0 + 1));
        let (n, lo, hi) = if u < e0 {
            (n0 - 1, self.edge(n0 - 1), e0)
        } else if u >= e1 {
            (n0 + 1, e1, self.edge(n0 + 2))
        } else {
            (n0, e0, e1)
        };
        let thickness = hi - lo;
        let h = self.band_hardness(n);
        let mut k = erodibility(h);
        // Near an edge the erodibility blends into the neighbour's, so that it is continuous in the height (a half-and-half mix at the edge itself).
        let e = BAND_BLEND * thickness;
        if u - lo < e {
            let w = 0.5 + 0.5 * (u - lo) / e;
            k = erodibility(self.band_hardness(n - 1)) * (1.0 - w) + k * w;
        } else if hi - u < e {
            let w = 0.5 + 0.5 * (hi - u) / e;
            k = erodibility(self.band_hardness(n + 1)) * (1.0 - w) + k * w;
        }
        let soluble = if self.band_carbonate(n) { self.soluble_gate } else { 0.0 };
        Stratum { k, hardness: h, caprock: h > CAPROCK_HARDNESS, soluble, band: n }
    }
}
