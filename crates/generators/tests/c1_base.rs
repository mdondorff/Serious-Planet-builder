//! GEN-107: the C1 read of the bake lattice (`read_elevation_c1`) and the field `cpu-erosion-v2` that reads with it.

use planet_core::cube::{face_to_direction, FaceMapping, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Vec3};
use planet_generators::erosion::{
    ErosionField, ErosionGenerator, EROSION_C1_IMPLEMENTATION_ID, EROSION_C1_VERSION, EROSION_IMPLEMENTATION_ID, EROSION_VERSION,
};
use planet_generators::global::{
    bake, locate_direction, read_direction, read_elevation_c1, GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError,
    C1_OVERSHOOT, READER_C1_VERSION,
};
use planet_generators::height::HeightField;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const KEY: [u8; 16] = [9; 16];
const RES: u32 = 64;

fn grid(res: u32) -> GlobalGrid {
    let def = WorldDefinition::single_planet(R, SEED);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    bake(seed, &TangentWarp, res, 4).unwrap()
}

fn c1_field<'a>(src: &'a dyn LatticeSource) -> ErosionField<'a> {
    let def = WorldDefinition::single_planet(R, SEED);
    ErosionGenerator.field_c1(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
}

fn v1_field<'a>(src: &'a dyn LatticeSource) -> ErosionField<'a> {
    let def = WorldDefinition::single_planet(R, SEED);
    ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
}

fn c1(src: &dyn LatticeSource, d: Vec3) -> f64 {
    f64::from(read_elevation_c1(src, &TangentWarp, d).unwrap())
}

fn bilinear(src: &dyn LatticeSource, d: Vec3) -> f64 {
    f64::from(read_direction(src, &TangentWarp, d).unwrap().elevation_m)
}

/// The direction at lattice position `(u, v)` (cells, from the face's lower corner) of `face`.
fn at_lattice(face: u8, u: f64, v: f64, res: u32) -> Vec3 {
    let half = f64::from(res) * 0.5;
    face_to_direction(&TangentWarp, Face(face), u / half - 1.0, v / half - 1.0)
}

// spec: GEN-107
#[test]
fn a_lattice_point_reads_as_its_own_sample_on_faces_edges_and_corners() {
    let g = grid(RES);
    let src = GridSource(&g);
    let mut rng = SplitMix64(1);
    let mut checked = 0;
    for face in 0..6u8 {
        for k in 0..60 {
            // Interior points, and the four edges and corners of the face.
            let (i, j) = match k {
                0 => (0, 0),
                1 => (RES, 0),
                2 => (0, RES),
                3 => (RES, RES),
                4..=19 => (0, (rng.next_f64() * f64::from(RES)) as u32),
                20..=35 => (RES, (rng.next_f64() * f64::from(RES)) as u32),
                36..=47 => ((rng.next_f64() * f64::from(RES)) as u32, 0),
                48..=55 => ((rng.next_f64() * f64::from(RES)) as u32, RES),
                _ => ((rng.next_f64() * f64::from(RES)) as u32, (rng.next_f64() * f64::from(RES)) as u32),
            };
            let d = at_lattice(face, f64::from(i), f64::from(j), RES);
            let at = locate_direction(RES, &TangentWarp, d).unwrap();
            if at.wu != 0.0 || at.wv != 0.0 {
                continue;
            }
            let want = src.at(at.face, at.i, at.j).unwrap().elevation_m;
            assert_eq!(read_elevation_c1(&src, &TangentWarp, d).unwrap().to_bits(), want.to_bits(), "face {face} vertex ({i}, {j})");
            checked += 1;
        }
    }
    assert!(checked > 250, "{checked} lattice points checked");
}

/// The jump of the slope (metres per cell) across the lattice line `u = i` at height `v` of `face`, measured `eps` cells either side.
fn slope_jump(src: &dyn LatticeSource, read: &dyn Fn(&dyn LatticeSource, Vec3) -> f64, face: u8, i: f64, v: f64, eps: f64) -> f64 {
    let (a, b, c) = (
        read(src, at_lattice(face, i - eps, v, RES)),
        read(src, at_lattice(face, i, v, RES)),
        read(src, at_lattice(face, i + eps, v, RES)),
    );
    ((c - b) - (b - a)) / eps
}

// spec: GEN-107
#[test]
fn across_a_cell_edge_inside_a_face_the_slope_breaks_far_less_than_the_bilinears() {
    let g = grid(RES);
    let src = GridSource(&g);
    let mut rng = SplitMix64(2);
    let (mut sum_c1, mut sum_bi, mut worst_c1, mut n) = (0.0f64, 0.0f64, 0.0f64, 0);
    for _ in 0..400 {
        let face = (rng.next_f64() * 6.0) as u8 % 6;
        let i = 3.0 + (rng.next_f64() * f64::from(RES - 7)).floor();
        let v = 4.0 + rng.next_f64() * f64::from(RES - 8);
        let (jc, jb) = (slope_jump(&src, &c1, face, i, v, 0.01).abs(), slope_jump(&src, &bilinear, face, i, v, 0.01).abs());
        sum_c1 += jc;
        sum_bi += jb;
        worst_c1 = worst_c1.max(jc);
        n += 1;
    }
    eprintln!(
        "slope jump across a cell edge: C1 mean {:.3} worst {:.3}, bilinear mean {:.3} (m per cell, {n} edges)",
        sum_c1 / n as f64,
        worst_c1,
        sum_bi / n as f64
    );
    assert!(sum_bi / n as f64 > 5.0, "the bilinear read breaks the slope visibly ({:.3})", sum_bi / n as f64);
    assert!(sum_c1 < 0.05 * sum_bi, "C1 mean {sum_c1} against bilinear {sum_bi}");
}

/// The jump of the slope across a face edge, along the great circle through `d0` on the edge towards `towards`, `eps` radians either side.
fn edge_jump(src: &dyn LatticeSource, read: &dyn Fn(&dyn LatticeSource, Vec3) -> f64, d0: Vec3, towards: Vec3, eps: f64) -> f64 {
    let at = |a: f64| (d0 * libm::cos(a) + towards * libm::sin(a)).normalized();
    let (x, y, z) = (read(src, at(-eps)), read(src, d0), read(src, at(eps)));
    ((z - y) - (y - x)) / eps
}

// spec: GEN-107
#[test]
fn across_a_face_edge_the_slope_breaks_far_less_than_the_bilinears() {
    let g = grid(RES);
    let src = GridSource(&g);
    let mut rng = SplitMix64(3);
    // A direction on an edge, and the unit vector across it that lies in the tangent plane.
    let (mut sum_c1, mut sum_bi, mut n) = (0.0f64, 0.0f64, 0);
    for face in 0..6u8 {
        for _ in 0..40 {
            let t = rng.next_f64() * 1.6 - 0.8;
            for s_edge in [-1.0, 1.0] {
                let d0 = face_to_direction(&TangentWarp, Face(face), s_edge, t);
                // Across the edge: along the face's own u axis, projected into the tangent plane.
                let ax = Face(face).u_axis();
                let towards = (ax - d0 * ax.dot(d0)).normalized();
                // `eps` is one hundredth of a cell in radians.
                let eps = 0.01 * std::f64::consts::FRAC_PI_2 / f64::from(RES);
                sum_c1 += edge_jump(&src, &c1, d0, towards, eps).abs();
                sum_bi += edge_jump(&src, &bilinear, d0, towards, eps).abs();
                n += 1;
            }
        }
    }
    // The jump is in metres per radian here; one cell is pi / 2 / RES radians.
    let cell = std::f64::consts::FRAC_PI_2 / f64::from(RES);
    eprintln!(
        "slope jump across a face edge: C1 mean {:.3}, bilinear mean {:.3} (m per cell, {n} points)",
        sum_c1 / n as f64 * cell,
        sum_bi / n as f64 * cell
    );
    assert!(sum_bi / n as f64 * cell > 5.0, "the bilinear read breaks the slope at face edges");
    assert!(sum_c1 < 0.5 * sum_bi, "C1 {sum_c1} against bilinear {sum_bi}");
}

/// The three faces' coordinates of a direction: where it lies on each face's plane, clamped nowhere.
fn on_face(face: u8, d: Vec3) -> (f64, f64) {
    let f = Face(face);
    let w = d.dot(f.normal());
    let (u, v) = (d.dot(f.u_axis()) / w, d.dot(f.v_axis()) / w);
    // `plane_to_face` clamps beyond the face, which would make a direction far outside look like it sits on the boundary: return an outside marker.
    if u.abs() > 1.0 + 1e-12 || v.abs() > 1.0 + 1e-12 {
        return (f64::INFINITY, f64::INFINITY);
    }
    (TangentWarp.plane_to_face(u), TangentWarp.plane_to_face(v))
}

// spec: GEN-107
#[test]
fn the_same_direction_reads_the_same_bits_from_either_face_at_every_edge_and_corner() {
    let g = grid(RES);
    let src = GridSource(&g);
    let mut rng = SplitMix64(4);
    let (mut same_dir, mut equal, mut worst) = (0, 0, 0.0f64);
    // The twelve edges: a point on face A's edge, taken from face A's coordinates and from the neighbour's.
    for a in 0..6u8 {
        for s_edge in [-1.0, 1.0] {
            for k in 0..=32 {
                // The midpoint and the corners, then points within about two cells of a corner (the stencil's ghosts differ most there), then random ones.
                let t = match k {
                    0..=2 => [-1.0, 0.0, 1.0][k],
                    3..=14 => (1.0 - 0.0125 * (k - 2) as f64 * rng.next_f64()) * if k % 2 == 0 { 1.0 } else { -1.0 },
                    _ => rng.next_f64() * 2.0 - 1.0,
                };
                let da = face_to_direction(&TangentWarp, Face(a), s_edge, t);
                let (b, _, _) = planet_core::cube::direction_to_face(&TangentWarp, da);
                for other in 0..6u8 {
                    if other == a || Face(other).normal().dot(Face(a).normal()).abs() > 0.5 {
                        continue;
                    }
                    let (s, t2) = on_face(other, da);
                    // The direction must lie on the other face's boundary (a shared edge or corner), on its positive side.
                    if da.dot(Face(other).normal()) <= 0.0
                        || s.abs() > 1.0 + 1e-12
                        || t2.abs() > 1.0 + 1e-12
                        || (s.abs() < 1.0 - 1e-9 && t2.abs() < 1.0 - 1e-9)
                    {
                        continue;
                    }
                    let db = face_to_direction(&TangentWarp, Face(other), s.clamp(-1.0, 1.0), t2.clamp(-1.0, 1.0));
                    let (ea, eb) = (read_elevation_c1(&src, &TangentWarp, da).unwrap(), read_elevation_c1(&src, &TangentWarp, db).unwrap());
                    if (da.x.to_bits(), da.y.to_bits(), da.z.to_bits()) == (db.x.to_bits(), db.y.to_bits(), db.z.to_bits()) {
                        same_dir += 1;
                        assert_eq!(ea.to_bits(), eb.to_bits(), "faces {a} and {other} at t = {t}: {ea} and {eb} (the same direction bits)");
                        equal += 1;
                    } else {
                        // The direction bits differ by an ulp or so (the two faces' coordinate arithmetic): the heights differ by a similar amount.
                        worst = worst.max(f64::from((ea - eb).abs()));
                    }
                    let _ = b;
                }
            }
        }
    }
    eprintln!(
        "edges: {same_dir} directions with equal bits all give equal heights ({equal}); worst difference of the others {worst:.2e} m"
    );
    assert!(same_dir >= 12, "the edge midpoints and corners are shared exactly");
    assert!(worst < 1e-3, "neighbouring-bit directions give heights within a millimetre ({worst})");
}

// spec: GEN-107
#[test]
fn near_a_cube_corner_the_read_is_finite_and_continuous() {
    let g = grid(RES);
    let src = GridSource(&g);
    let corner = Vec3::new(1.0, 1.0, 1.0).normalized();
    let mut rng = SplitMix64(5);
    for _ in 0..200 {
        let mut p = corner;
        for _ in 0..3 {
            let a = Vec3::new(rng.next_f64() - 0.5, rng.next_f64() - 0.5, rng.next_f64() - 0.5);
            p = (p + a * (0.02 * rng.next_f64())).normalized();
        }
        let q = (p + Vec3::new(1.0, -0.5, 0.2) * 1e-7).normalized();
        let (a, b) = (c1(&src, p), c1(&src, q));
        assert!(a.is_finite() && b.is_finite());
        assert!((a - b).abs() < 0.5, "{a} against {b} 0.6 m apart near a corner");
    }
    assert!(c1(&src, corner).is_finite());
}

/// Every height of a cap's rings lies within the field's bounds.
fn check_cap(f: &dyn HeightField, c: Vec3, theta: f64) {
    let (lo, hi) = f.height_bounds(c, theta);
    assert!(lo <= hi);
    let a = if c.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = c.cross(a).normalized();
    let e2 = c.cross(e1);
    for ring in [0.0, 0.3, 0.7, 1.0] {
        for k in 0..24 {
            let phi = f64::from(k) * std::f64::consts::TAU / 24.0;
            let th = theta * ring;
            let d = (c * libm::cos(th) + (e1 * libm::cos(phi) + e2 * libm::sin(phi)) * libm::sin(th)).normalized();
            let h = f.height(d);
            assert!(lo <= h && h <= hi, "height {h} outside [{lo}, {hi}] at {d:?} in the cap {c:?} {theta}");
            let h32 = f64::from(h as f32);
            assert!(lo <= h32 && h32 <= hi, "f32 height {h32} outside [{lo}, {hi}]");
        }
    }
}

fn centres(rng: &mut SplitMix64) -> Vec<(Vec3, f64)> {
    let mut v = Vec::new();
    for _ in 0..120 {
        let c = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized();
        v.push((c, [1e-4, 1e-3, 0.005, 0.02][(rng.next_f64() * 4.0) as usize % 4]));
    }
    // Caps on the face edges, near them and at the corners, where the stencil reaches the neighbour face.
    for face in 0..6u8 {
        for &t in &[0.9, 0.99, 0.999, 1.0] {
            for &theta in &[0.0005, 0.002, 0.01, 0.02] {
                v.push((face_to_direction(&TangentWarp, Face(face), 0.0, t), theta));
                v.push((face_to_direction(&TangentWarp, Face(face), 0.97, t), theta / 2.0));
                v.push((face_to_direction(&TangentWarp, Face(face), t, t), theta));
            }
        }
    }
    v
}

// spec: GEN-107
#[test]
fn bounds_hold_every_height_of_caps_everywhere_including_edges_and_corners() {
    let g = grid(256);
    let src = GridSource(&g);
    let f = c1_field(&src);
    let mut rng = SplitMix64(11);
    for (c, theta) in centres(&mut rng) {
        check_cap(&f, c, theta);
    }
    assert!(f.error().is_none());
    // A cap too wide for the scan takes the global range.
    let (lo, hi) = f.height_bounds(Vec3::new(1.0, 0.0, 0.0), 0.06);
    assert!(lo < -8000.0 && hi > 8000.0);
}

/// A lattice with a steep ramp and a spike: the C1 read overshoots next to them, which the bounds must hold.
struct Spiky;

impl LatticeSource for Spiky {
    fn res(&self) -> u32 {
        4096
    }
    fn at(&self, f: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let spike = if i % 7 == 3 && j % 5 == 2 { 3000.0 } else { 0.0 };
        let ramp = (f64::from(i) - 2000.0) / 6.0;
        let e = spike
            + 8000.0 * ramp.clamp(0.0, 1.0) * 0.0
            + 400.0 * libm::sin(f64::from(j) * 0.7 + f64::from(f.0))
            + if i > 2040 { 2500.0 } else { 0.0 };
        Ok(GlobalSample { continent: 1.0, elevation_m: e as f32, temperature_c: 0.0, precipitation_mm: 0.0 })
    }
}

// spec: GEN-107
#[test]
fn bounds_hold_over_isolated_spikes_and_steps_where_the_blend_overshoots() {
    let src = Spiky;
    let f = c1_field(&src);
    let mut rng = SplitMix64(12);
    let mut checked = 0;
    for face in 0..6u8 {
        for _ in 0..80 {
            let c = face_to_direction(&TangentWarp, Face(face), rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0);
            for &theta in &[3e-4, 1e-3, 3e-3, 0.01] {
                check_cap(&f, c, theta);
                checked += 1;
            }
        }
    }
    assert!(checked > 1500 && f.error().is_none());
    // The overshoot is real beside the step and the spikes, and never more than C1_OVERSHOOT of the range of the sixteen samples around.
    let (mut outside, mut worst_share) = (0, 0.0f64);
    for k in 0..6000 {
        let (u, v) = (2036.0 + f64::from(k % 400) * 0.0257, 1000.0 + f64::from(k / 400) * 3.37 + f64::from(k % 7) * 0.31);
        let d = at_lattice(0, u, v, 4096);
        let at = locate_direction(4096, &TangentWarp, d).unwrap();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for dj in -1i64..=2 {
            for di in -1i64..=2 {
                let e = f64::from(src.at(at.face, (i64::from(at.i) + di) as u32, (i64::from(at.j) + dj) as u32).unwrap().elevation_m);
                lo = lo.min(e);
                hi = hi.max(e);
            }
        }
        let e = f64::from(read_elevation_c1(&src, &TangentWarp, d).unwrap());
        let share = ((e - hi).max(lo - e)).max(0.0) / (hi - lo).max(1e-9);
        if share > 0.0 {
            outside += 1;
        }
        worst_share = worst_share.max(share);
        assert!(share <= C1_OVERSHOOT + 1e-9, "overshoot {share} of the range at ({u}, {v})");
    }
    eprintln!("overshoot: {outside} of 6000 reads outside the sample range, worst {worst_share:.3} of the range (bound {C1_OVERSHOOT})");
    assert!(outside > 0, "the interpolant overshoots next to a step");
}

// spec: GEN-107
#[test]
fn heights_do_not_depend_on_the_batch_or_the_thread_count() {
    let g = grid(RES);
    let src = GridSource(&g);
    let f = c1_field(&src);
    let mut rng = SplitMix64(13);
    let mut dirs: Vec<Vec3> = (0..600)
        .map(|_| Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized())
        .collect();
    for face in 0..6u8 {
        for k in 0..20 {
            dirs.push(face_to_direction(&TangentWarp, Face(face), 1.0 - f64::from(k) * 0.001, f64::from(k) * 0.05 - 0.5));
            dirs.push(face_to_direction(&TangentWarp, Face(face), -1.0, f64::from(k) * 0.1 - 1.0));
        }
    }
    let one: Vec<u64> = dirs.iter().map(|d| f.height(*d).to_bits()).collect();
    let batch: Vec<u64> = f.heights_until(&dirs, &|| false).iter().map(|h| h.to_bits()).collect();
    assert_eq!(one, batch, "the gathered batch equals the single reads");
    for threads in [1usize, 2, 4, 7] {
        let t: Vec<u64> = f.heights_threaded(&dirs, &threads).iter().map(|h| h.to_bits()).collect();
        assert_eq!(one, t, "{threads} threads");
    }
    assert!(f.error().is_none());
}

// spec: GEN-107
#[test]
fn the_c1_field_identifies_itself_and_is_keyed_apart_from_the_bilinear_one() {
    let g = grid(RES);
    let src = GridSource(&g);
    let (a, b) = (v1_field(&src), c1_field(&src));
    assert_eq!((a.implementation_id(), a.version()), (EROSION_IMPLEMENTATION_ID, EROSION_VERSION));
    assert_eq!((b.implementation_id(), b.version()), (EROSION_C1_IMPLEMENTATION_ID, EROSION_C1_VERSION));
    assert_eq!(EROSION_C1_IMPLEMENTATION_ID, "cpu-erosion-v2");
    assert_ne!(a.inputs_hash(), b.inputs_hash(), "the two readers never share a key");
    assert_eq!(b.inputs_hash(), c1_field(&src).inputs_hash());
    assert_eq!(b.source_key(), Some(KEY));
    // The pinned versions: a change of the interpolant, the stencil or the ghost rule bumps READER_C1_VERSION (known answers below).
    assert_eq!((READER_C1_VERSION, EROSION_C1_VERSION), (1, 1));
}

// spec: GEN-107
#[test]
fn known_answers_of_the_c1_read_and_the_field_are_pinned() {
    let g = grid(RES);
    let src = GridSource(&g);
    let f = c1_field(&src);
    let dirs = [
        Vec3::new(0.3, -0.5, 0.8).normalized(),
        face_to_direction(&TangentWarp, Face(2), 0.37, -0.61),
        face_to_direction(&TangentWarp, Face(5), 1.0, 0.25),
        Vec3::new(1.0, 1.0, 1.0).normalized(),
    ];
    let got: Vec<(u32, u64)> =
        dirs.iter().map(|d| (read_elevation_c1(&src, &TangentWarp, *d).unwrap().to_bits(), f.height(*d).to_bits())).collect();
    eprintln!("known answers: {got:#x?}");
    assert_eq!(got, PINNED);
}

/// `(read_elevation_c1 bits, field height bits)` of the four directions above on the 64-cell bake of seed 7.
const PINNED: [(u32, u64); 4] = [
    (1_118_074_014, 4_633_626_389_980_356_180),
    (3_299_332_605, 13_876_993_057_341_092_339),
    (3_311_221_168, 13_883_385_936_620_562_706),
    (3_308_198_617, 13_881_764_335_007_643_907),
];
