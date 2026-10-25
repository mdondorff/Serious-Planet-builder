//! GEN-016: the terrain field (ADR 0028): a pure function of the direction over the global bake.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Vec3};
use planet_generators::global::{bake, GlobalGrid, GridSource};
use planet_generators::height::HeightField;
use planet_generators::terrain::{TerrainField, TerrainGenerator, MAX_ABS_HEIGHT_M, TERRAIN_IMPLEMENTATION_ID, TERRAIN_VERSION};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const KEY: [u8; 16] = [9; 16];

fn grid(res: u32) -> GlobalGrid {
    let def = WorldDefinition::single_planet(R, SEED);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    bake(seed, &TangentWarp, res, 4).unwrap()
}

fn field<'a>(src: &'a dyn planet_generators::global::LatticeSource, key: [u8; 16]) -> TerrainField<'a> {
    let def = WorldDefinition::single_planet(R, SEED);
    TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).unwrap()
}

/// A direction at angle `theta` from `c` towards a unit angle `phi` around it.
fn offset(c: Vec3, theta: f64, phi: f64) -> Vec3 {
    let a = if c.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = c.cross(a).normalized();
    let e2 = c.cross(e1);
    (c * libm::cos(theta) + (e1 * libm::cos(phi) + e2 * libm::sin(phi)) * libm::sin(theta)).normalized()
}

// spec: GEN-016
#[test]
fn bounds_hold_every_height_of_random_caps_and_of_caps_at_edge_midpoints_and_corners() {
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src, KEY);
    let mut rng = SplitMix64(11);
    let mut unit = move || rng.next_f64();
    let mut centres: Vec<(Vec3, f64)> = Vec::new();
    for _ in 0..150 {
        let c = Vec3::new(unit() * 2.0 - 1.0, unit() * 2.0 - 1.0, unit() * 2.0 - 1.0).normalized();
        centres.push((c, [0.001, 0.01, 0.04][(unit() * 3.0) as usize % 3]));
    }
    // Caps centred near the middle of a face edge (s = 0, t near 1) and near corners: where face coordinates change fastest.
    for face in 0..6u8 {
        for &t in &[0.8, 0.9, 0.95, 0.999, 1.0] {
            for &theta in &[0.02, 0.04, 0.05] {
                centres.push((face_to_direction(&TangentWarp, Face(face), 0.0, t), theta));
                centres.push((face_to_direction(&TangentWarp, Face(face), 0.97, t), theta / 2.0));
            }
        }
    }
    let (mut tight, mut total) = (0, 0);
    for (c, theta) in centres {
        let (lo, hi) = f.height_bounds(c, theta);
        assert!(lo <= hi, "{c:?} {theta}");
        // Densely sampled points of the cap: the centre, rings at fractions of the radius and the boundary.
        for ring in [0.0, 0.3, 0.7, 1.0] {
            for k in 0..24 {
                let d = offset(c, theta * ring, f64::from(k) * std::f64::consts::TAU / 24.0);
                let h = f.height(d);
                assert!(lo <= h && h <= hi, "height {h} outside [{lo}, {hi}] at {d:?} in the cap {c:?} {theta}");
            }
        }
        total += 1;
        if hi - lo < 4000.0 {
            tight += 1;
        }
    }
    assert!(f.error().is_none());
    // A cap with L theta above 0.1 (theta above 0.05) takes the global range.
    assert_eq!(f.height_bounds(Vec3::new(1.0, 0.0, 0.0), 0.06), (-MAX_ABS_HEIGHT_M, MAX_ABS_HEIGHT_M));
    // Bounds that are the whole range everywhere would pass: most caps must hug the terrain.
    assert!(tight * 4 >= total * 3, "{tight} of {total} caps have bounds tighter than 4 km");
    let _ = MAX_ABS_HEIGHT_M;
}

// spec: GEN-016
#[test]
fn the_field_is_pure_and_identifies_itself() {
    let g = grid(32);
    let src = GridSource(&g);
    let f = field(&src, KEY);
    assert_eq!((f.implementation_id(), f.version()), (TERRAIN_IMPLEMENTATION_ID, TERRAIN_VERSION));
    let d = Vec3::new(0.3, -0.5, 0.8).normalized();
    assert_eq!(f.height(d).to_bits(), f.height(d).to_bits());
    assert_eq!(f.source_key(), Some(KEY));
    // A different bake key re-keys the field; the same key and definition give the same inputs.
    assert_eq!(field(&src, KEY).inputs_hash(), f.inputs_hash());
    assert_ne!(field(&src, [8; 16]).inputs_hash(), f.inputs_hash());
}

// spec: GEN-016
#[test]
fn bounds_hug_the_terrain_on_small_caps_and_cover_f32_rounded_heights() {
    let g = grid(256);
    let src = GridSource(&g);
    let f = field(&src, KEY);
    let mut rng = SplitMix64(5);
    let mut worst: f64 = 0.0;
    for _ in 0..1000 {
        let c = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized();
        let (lo, hi) = f.height_bounds(c, 1e-4);
        worst = worst.max(hi - lo);
        for k in 0..8 {
            let d = offset(c, 1e-4, f64::from(k) * 0.8);
            let h = f64::from(f.height(d) as f32);
            assert!(lo <= h && h <= hi, "{h} outside [{lo}, {hi}] at {c:?}");
        }
    }
    // A 640 m cap holds a few tens of metres of relief, not kilometres.
    assert!(worst < 400.0, "worst extent {worst} m");
}

/// A lattice whose adjacent samples differ by one f32 ulp just above a binade.
struct Ulp(f32);

impl planet_generators::global::LatticeSource for Ulp {
    fn res(&self) -> u32 {
        4
    }
    fn at(&self, _: Face, i: u32, _: u32) -> Result<planet_generators::global::GlobalSample, planet_generators::global::ReadError> {
        let e = if i.is_multiple_of(2) { self.0 } else { f32::from_bits(self.0.to_bits() + 1) };
        Ok(planet_generators::global::GlobalSample { continent: 1.0, elevation_m: e, temperature_c: 0.0, precipitation_mm: 0.0 })
    }
}

// spec: GEN-016
#[test]
fn bounds_cover_f32_rounded_heights_where_adjacent_samples_are_one_ulp_apart() {
    for base in [130.0f32, 4100.0] {
        let src = Ulp(base);
        let f = field(&src, KEY);
        // Caps around the rounding midpoint of the blend between two samples of the face's centre cell.
        let mut worst = 0usize;
        for n in 0..4000 {
            // Lattice position u = (s + 1) * 2 = 1.5 is the midpoint between samples 1 and 2 (weight 1/2).
            let s = -0.25 + (f64::from(n) - 2000.0) * 1e-10;
            let c = face_to_direction(&TangentWarp, Face(0), s, 0.1);
            for theta in [1e-9, 1e-8, 1e-7] {
                let (lo, hi) = f.height_bounds(c, theta);
                for k in 0..8 {
                    let h = f64::from(f.height(offset(c, theta, f64::from(k) * 0.8)) as f32);
                    if !(lo <= h && h <= hi) {
                        worst += 1;
                    }
                }
            }
        }
        assert_eq!(worst, 0, "{worst} f32 heights outside the bounds at base {base}");
    }
}

struct Broken;

impl planet_generators::global::LatticeSource for Broken {
    fn res(&self) -> u32 {
        4
    }
    fn at(&self, f: Face, i: u32, j: u32) -> Result<planet_generators::global::GlobalSample, planet_generators::global::ReadError> {
        Err(planet_generators::global::ReadError::Source(format!("block of face {} at ({i}, {j}) is missing", f.0)))
    }
}

// spec: GEN-016
#[test]
fn a_bake_that_cannot_be_read_gives_nan_and_the_global_range_and_names_the_problem() {
    let src = Broken;
    let f = field(&src, KEY);
    assert!(f.error().is_none());
    let c = Vec3::new(0.3, -0.5, 0.8).normalized();
    assert!(f.height(c).is_nan());
    let msg = f.error().expect("the failure is reported");
    assert!(msg.contains("missing"), "{msg}");
    let fresh = field(&src, KEY);
    assert_eq!(fresh.height_bounds(c, 1e-4), (-MAX_ABS_HEIGHT_M, MAX_ABS_HEIGHT_M));
    assert!(fresh.error().is_some());
}

// spec: GEN-016
#[test]
fn one_world_point_has_one_height_whichever_tile_asks() {
    use planet_core::TileId;
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src, KEY);
    let map = TangentWarp;
    let cells = 8u32;
    // Every sample of every level-1 and level-2 tile: equal direction bits must give equal height bits.
    // Per direction: the height bits and the set of faces whose tiles asked.
    let mut by_dir: std::collections::HashMap<[u64; 3], (u64, u8)> = std::collections::HashMap::new();
    let (mut checks, mut shared) = (0, 0);
    let mut visit = |t: TileId| {
        for l in 0..=cells {
            for k in 0..=cells {
                let d = t.sample_direction(&map, cells, k, l);
                let h = f.height(d).to_bits();
                let key = [d.x.to_bits(), d.y.to_bits(), d.z.to_bits()];
                let e = by_dir.entry(key).or_insert((h, 0));
                assert_eq!(e.0, h, "{t:?} ({k}, {l})");
                if e.1 != 0 {
                    shared += 1;
                }
                e.1 |= 1 << t.face().0;
                checks += 1;
            }
        }
    };
    for face in 0..6u8 {
        let root = TileId::new(Face(face), 0, 0, 0).unwrap();
        for a in root.children().unwrap() {
            visit(a);
            for b in a.children().unwrap() {
                visit(b);
            }
        }
    }
    // Shared borders between tiles, between parents and children, across face edges and at the 8 cube corners.
    assert!(shared > 12 * 8 * 2, "{shared} of {checks} samples were shared");
    // Directions asked by tiles of two faces (face edges: 12 * (8 * 4 - 1) + 8 corners... at least every edge point) and by
    // three faces (the 8 cube corners).
    let across = by_dir.values().filter(|v| v.1.count_ones() >= 2).count();
    let corners = by_dir.values().filter(|v| v.1.count_ones() >= 3).count();
    assert_eq!(corners, 8);
    assert!(across >= 12 * 31 + 8, "{across} directions are shared across a face edge");
    assert!(f.error().is_none());
}

// spec: GEN-016
#[test]
fn heights_do_not_depend_on_the_thread_that_asks() {
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src, KEY);
    let dirs: Vec<Vec3> = (0..400).map(|i| offset(Vec3::new(0.2, 0.5, -0.8).normalized(), f64::from(i) * 0.007, f64::from(i))).collect();
    let serial: Vec<u64> = dirs.iter().map(|&d| f.height(d).to_bits()).collect();
    let parallel: Vec<u64> = std::thread::scope(|s| {
        let hs: Vec<_> = dirs.chunks(37).map(|c| s.spawn(|| c.iter().map(|&d| f.height(d).to_bits()).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(serial, parallel);
}

// spec: GEN-016
#[test]
fn known_answers() {
    let g = grid(64);
    let src = GridSource(&g);
    let f = field(&src, KEY);
    assert_eq!((TERRAIN_VERSION, TERRAIN_IMPLEMENTATION_ID), (1, "cpu-terrain-v1"));
    let at = |x: f64, y: f64, z: f64| f.height(Vec3::new(x, y, z).normalized());
    let got = [at(1.0, 0.0, 0.0), at(0.3, -0.5, 0.8), at(-0.6, 0.1, -0.2), at(0.0, 0.0, 1.0)];
    assert_eq!(got.map(f64::to_bits), [4655559553819126269, 13865995584287959468, 13864863918882730402, 4652319206642888986], "{got:?}");
}
