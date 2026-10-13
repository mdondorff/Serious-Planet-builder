//! GEN-017: the eroded terrain field (ADR 0030): a pure function of the direction over the global bake, with sound bounds.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Vec3};
use planet_generators::erosion::{ErosionField, ErosionGenerator, EROSION_IMPLEMENTATION_ID, EROSION_VERSION, MAX_ABS_HEIGHT_M};
use planet_generators::global::{bake, GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError};
use planet_generators::height::HeightField;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const KEY: [u8; 16] = [9; 16];

fn grid(res: u32) -> GlobalGrid {
    let def = WorldDefinition::single_planet(R, SEED);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    bake(seed, &TangentWarp, res, 4).unwrap()
}

fn field_of<'a>(src: &'a dyn LatticeSource, radius: f64, key: [u8; 16]) -> ErosionField<'a> {
    let def = WorldDefinition::single_planet(radius, SEED);
    ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).unwrap()
}

fn field<'a>(src: &'a dyn LatticeSource) -> ErosionField<'a> {
    field_of(src, R, KEY)
}

/// A direction at angle `theta` from `c` towards a unit angle `phi` around it.
fn offset(c: Vec3, theta: f64, phi: f64) -> Vec3 {
    let a = if c.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = c.cross(a).normalized();
    let e2 = c.cross(e1);
    (c * libm::cos(theta) + (e1 * libm::cos(phi) + e2 * libm::sin(phi)) * libm::sin(theta)).normalized()
}

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized()
}

/// Every height of a cap's rings lies within its bounds; returns the extent `hi - lo`.
fn check_cap(f: &dyn HeightField, c: Vec3, theta: f64, round_f32: bool) -> f64 {
    let (lo, hi) = f.height_bounds(c, theta);
    assert!(lo <= hi, "{c:?} {theta}");
    for ring in [0.0, 0.3, 0.7, 1.0] {
        for k in 0..24 {
            let d = offset(c, theta * ring, f64::from(k) * std::f64::consts::TAU / 24.0);
            let h = if round_f32 { f64::from(f.height(d) as f32) } else { f.height(d) };
            assert!(lo <= h && h <= hi, "height {h} outside [{lo}, {hi}] at {d:?} in the cap {c:?} {theta}");
        }
    }
    hi - lo
}

// spec: GEN-017
#[test]
fn bounds_hold_every_height_of_random_caps_and_of_caps_at_edge_midpoints_and_corners() {
    let g = grid(256);
    let src = GridSource(&g);
    let f = field(&src);
    let mut rng = SplitMix64(11);
    let mut centres: Vec<(Vec3, f64)> = Vec::new();
    for _ in 0..120 {
        let c = random_dir(&mut rng);
        centres.push((c, [1e-4, 1e-3, 0.005, 0.02][(rng.next_f64() * 4.0) as usize % 4]));
    }
    // Caps at the middle of a face edge and near corners, where face coordinates change fastest.
    for face in 0..6u8 {
        for &t in &[0.9, 0.999, 1.0] {
            for &theta in &[0.002, 0.01, 0.02] {
                centres.push((face_to_direction(&TangentWarp, Face(face), 0.0, t), theta));
                centres.push((face_to_direction(&TangentWarp, Face(face), 0.97, t), theta / 2.0));
            }
        }
    }
    let (mut tight, mut total) = (0, 0);
    for (c, theta) in centres {
        if check_cap(&f, c, theta, false) < 4000.0 {
            tight += 1;
        }
        total += 1;
    }
    assert!(f.error().is_none());
    // A cap too wide for the scan (2 theta above 0.1) takes the global range.
    assert_eq!(f.height_bounds(Vec3::new(1.0, 0.0, 0.0), 0.06), (-MAX_ABS_HEIGHT_M, MAX_ABS_HEIGHT_M));
    assert!(tight * 4 >= total * 3, "{tight} of {total} caps have bounds tighter than 4 km");
}

/// A lattice with a steep ramp across `i`: the slope, and with it the carve factor, changes a lot within a few cells.
struct Ramp;

impl LatticeSource for Ramp {
    fn res(&self) -> u32 {
        4096
    }
    fn at(&self, f: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let ramp = (f64::from(i) - 2000.0) / 6.0;
        let e = 8000.0 * ramp.clamp(0.0, 1.0) + 400.0 * libm::sin(f64::from(j) * 0.7 + f64::from(f.0));
        Ok(GlobalSample { continent: 1.0, elevation_m: e as f32, temperature_c: 0.0, precipitation_mm: 0.0 })
    }
}

// spec: GEN-017
#[test]
fn bounds_hold_across_a_ramp_where_the_slope_factor_changes_within_the_cap() {
    let src = Ramp;
    let f = field(&src);
    let mut rng = SplitMix64(3);
    let mut checked = 0;
    for face in 0..6u8 {
        // Directions along the ramp (lattice i = 2000 .. 2006 of the face) and around it, j spread over the face.
        for ij in 0..40 {
            let s = (1996.0 + f64::from(ij) * 0.4) / 2048.0 - 1.0;
            let t = rng.next_f64() * 1.6 - 0.8;
            let c = face_to_direction(&TangentWarp, Face(face), s, t);
            for &theta in &[3e-4, 1e-3, 3e-3, 0.01] {
                check_cap(&f, c, theta, false);
                checked += 1;
            }
        }
    }
    assert!(checked > 900 && f.error().is_none());
}

// spec: GEN-017
#[test]
fn the_field_is_pure_identifies_itself_and_is_keyed_by_seed_radius_and_bake() {
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src);
    assert_eq!((f.implementation_id(), f.version()), (EROSION_IMPLEMENTATION_ID, EROSION_VERSION));
    let d = Vec3::new(0.3, -0.5, 0.8).normalized();
    assert_eq!(f.height(d).to_bits(), f.height(d).to_bits());
    assert_eq!(f.source_key(), Some(KEY));
    assert_eq!(field(&src).inputs_hash(), f.inputs_hash());
    assert_ne!(field_of(&src, R, [8; 16]).inputs_hash(), f.inputs_hash());
    // The slope is rise over run, so the radius is an input of the field.
    assert_ne!(field_of(&src, R * 2.0, KEY).inputs_hash(), f.inputs_hash());
    // So is the world seed (through the body seed the context records).
    let other = WorldDefinition::single_planet(R, SEED + 1);
    let reseeded = ErosionGenerator.field(&GenContext::new(&other), BodyId::PLANET, &src, KEY).unwrap();
    assert_ne!(reseeded.inputs_hash(), f.inputs_hash());
    // The terrain field has its own implementation id and so its own keys.
    assert_ne!(EROSION_IMPLEMENTATION_ID, planet_generators::terrain::TERRAIN_IMPLEMENTATION_ID);
}

// spec: GEN-017
#[test]
fn bounds_hug_the_terrain_on_small_caps_and_cover_f32_rounded_heights() {
    let g = grid(256);
    let src = GridSource(&g);
    let f = field(&src);
    let mut rng = SplitMix64(5);
    let mut extents = Vec::new();
    for _ in 0..600 {
        let c = random_dir(&mut rng);
        extents.push(check_cap(&f, c, 1e-4, true));
    }
    extents.sort_by(f64::total_cmp);
    // A 640 m cap holds tens of metres of relief; the median must show it (a global range would be 20 km).
    let median = extents[extents.len() / 2];
    assert!(median < 40.0, "median extent {median} m");
    assert!(*extents.last().unwrap() < 400.0, "worst extent {} m", extents.last().unwrap());
}

/// A lattice that cannot be read.
struct Broken;

impl LatticeSource for Broken {
    fn res(&self) -> u32 {
        16
    }
    fn at(&self, _: Face, _: u32, _: u32) -> Result<GlobalSample, ReadError> {
        Err(ReadError::Source("the bake is gone".into()))
    }
}

// spec: GEN-017
#[test]
fn a_bake_that_cannot_be_read_gives_nan_and_the_global_range_and_names_the_problem() {
    let src = Broken;
    let f = field(&src);
    assert!(f.error().is_none());
    assert!(f.height(Vec3::new(0.0, 0.0, 1.0)).is_nan());
    assert!(f.error().unwrap().contains("the bake is gone"));
    let f = field(&src);
    assert_eq!(f.height_bounds(Vec3::new(0.0, 0.0, 1.0), 1e-4), (-MAX_ABS_HEIGHT_M, MAX_ABS_HEIGHT_M));
    assert!(f.error().is_some());
}

// spec: GEN-017
#[test]
fn heights_are_continuous_across_face_edges_and_the_cube_corner() {
    let g = grid(256);
    let src = GridSource(&g);
    let f = field(&src);
    let mut worst: f64 = 0.0;
    for face in 0..6u8 {
        for &(s, t) in &[(0.0, 1.0), (0.4, 1.0), (-0.7, 1.0), (1.0, 1.0), (1.0, -1.0)] {
            let edge = face_to_direction(&TangentWarp, Face(face), s, t);
            for k in 0..8 {
                // Points a metre apart across the edge: neighbours in the world, whichever face holds them.
                let phi = f64::from(k) * 0.8;
                let (a, b) = (offset(edge, 1.0 / R, phi), offset(edge, 1.0 / R, phi + std::f64::consts::PI));
                worst = worst.max((f.height(a) - f.height(b)).abs());
            }
        }
    }
    // Two metres apart; the surface here is never steeper than a few metres per metre (the finest octave is 12 m wide).
    assert!(worst < 10.0, "worst height step across an edge {worst} m over 2 m");
}

// spec: GEN-017
#[test]
fn heights_do_not_depend_on_the_thread_that_asks() {
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src);
    let dirs: Vec<Vec3> = {
        let mut rng = SplitMix64(1);
        (0..200).map(|_| random_dir(&mut rng)).collect()
    };
    let serial: Vec<u64> = dirs.iter().map(|&d| f.height(d).to_bits()).collect();
    let parallel: Vec<u64> = std::thread::scope(|s| {
        let hs: Vec<_> = dirs.chunks(50).map(|part| s.spawn(|| part.iter().map(|&d| f.height(d).to_bits()).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(serial, parallel);
}

// spec: GEN-017
#[test]
fn creases_cut_deeper_on_steep_ground_than_on_flat_ground() {
    // On the ramp lattice the same crease noise is scaled by the slope factor: compare the detail (height minus bake)
    // spread on the steep part and on the flat part, over many directions.
    let src = Ramp;
    let f = field(&src);
    let mut rng = SplitMix64(21);
    let mut spread = |s0: f64, s1: f64| {
        let mut v = Vec::new();
        for _ in 0..400 {
            let c = face_to_direction(&TangentWarp, Face(0), s0 + (s1 - s0) * rng.next_f64(), rng.next_f64() * 1.6 - 0.8);
            v.push(f.height(c) - f.base_elevation(c));
        }
        v.iter().map(|x| x.abs()).sum::<f64>() / v.len() as f64
    };
    // Lattice i = 2002..2005 is on the ramp (slope about 0.5); i = 400..800 is flat ground at sea level.
    let steep = spread(2002.0 / 2048.0 - 1.0, 2005.0 / 2048.0 - 1.0);
    let flat = spread(400.0 / 2048.0 - 1.0, 800.0 / 2048.0 - 1.0);
    assert!(steep > 3.0 * flat, "mean incision on the ramp {steep} m, on flat ground {flat} m");
}

// spec: GEN-017
#[test]
fn known_answers() {
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src);
    let probes = [Vec3::new(0.3, -0.5, 0.8).normalized(), Vec3::new(1.0, 0.2, -0.1).normalized(), Vec3::new(-0.6, -0.6, -0.5).normalized()];
    let got: Vec<u64> = probes.iter().map(|&d| f.height(d).to_bits()).collect();
    assert_eq!(got, [13866102441991740545, 4653989013321974524, 4640025291567171986]);
}
