//! Tier A, GEN-099 and GEN-102 (m9-bake PR 2): the runtime base `m + k (e - m) + k detail` behind the relief switch. Off (the default) is today's field; on reads `k` and `m` once
//! at each point, a batch equals the heights one by one, the bounds hold every height and nest, and the reader's ranges hold every value of the caps they bound.
//! Synthetic channels on small lattices; no planet bake.

use std::sync::Arc;

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Vec3};
use planet_generators::erosion::{ErosionField, ErosionGenerator, EROSION_IMPLEMENTATION_ID, EROSION_RELIEF_IMPLEMENTATION_ID};
use planet_generators::global::{bake, GlobalGrid, GridSource, LatticeSource};
use planet_generators::height::HeightField;
use planet_generators::relief_channels::ReliefChannels;
use planet_generators::relief_lattice::ReliefLattice;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const KEY: [u8; 16] = [9; 16];
const CELLS: usize = 64;

fn grid() -> GlobalGrid {
    let def = WorldDefinition::single_planet(R, SEED);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    bake(seed, &TangentWarp, 256, 4).unwrap()
}

fn field<'a>(src: &'a dyn LatticeSource) -> ErosionField<'a> {
    let def = WorldDefinition::single_planet(R, SEED);
    ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
}

/// Channels whose `k` and `m` are the functions of the direction `kf` and `mf` at every lattice point.
fn channels(kf: &dyn Fn(Vec3) -> f64, mf: &dyn Fn(Vec3) -> f64) -> ReliefChannels {
    let faces = (0..6u8)
        .map(|f| {
            let (mut k, mut m) = (Vec::new(), Vec::new());
            for j in 0..=CELLS {
                for i in 0..=CELLS {
                    let d =
                        face_to_direction(&TangentWarp, Face(f), 2.0 * i as f64 / CELLS as f64 - 1.0, 2.0 * j as f64 / CELLS as f64 - 1.0);
                    k.push(kf(d) as f32);
                    m.push(mf(d) as f32);
                }
            }
            (k, m)
        })
        .collect();
    ReliefChannels::from_planes(CELLS, faces).expect("channels of a synthetic lattice")
}

fn varying() -> ReliefChannels {
    channels(&|d| 1.0 + 3.0 * (0.5 + 0.5 * libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y)), &|d| 400.0 * libm::sin(4.0 * d.z) + 150.0)
}

fn offset(c: Vec3, theta: f64, phi: f64) -> Vec3 {
    let a = if c.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = c.cross(a).normalized();
    let e2 = c.cross(e1);
    (c * libm::cos(theta) + (e1 * libm::cos(phi) + e2 * libm::sin(phi)) * libm::sin(theta)).normalized()
}

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized()
}

// spec: GEN-099
#[test]
fn with_k_1_the_relief_field_is_todays_field_and_with_k_and_m_it_is_m_plus_k_times_the_rest() {
    let g = grid();
    let src = GridSource(&g);
    let off = field(&src);
    let ones = field(&src).with_relief(Arc::new(ReliefLattice::new(channels(&|_| 1.0, &|_| 300.0), 1)));
    let twice = field(&src).with_relief(Arc::new(ReliefLattice::new(channels(&|_| 2.0, &|_| 100.0), 2)));
    let mut rng = SplitMix64(3);
    for _ in 0..300 {
        let d = random_dir(&mut rng);
        let h = off.height(d);
        let a = ones.height(d);
        assert!((a - h).abs() < 1e-6 * (1.0 + h.abs()), "k 1 changes the height: {a} against {h}");
        let b = twice.height(d);
        let want = 100.0 + 2.0 * (h - 100.0);
        assert!((b - want).abs() < 1e-6 * (1.0 + want.abs()), "m + k (h - m) = {want}, got {b}");
    }
}

// spec: GEN-099
#[test]
fn the_relief_switch_is_off_by_default_and_changes_the_implementation_and_the_key_when_on() {
    let g = grid();
    let src = GridSource(&g);
    let off = field(&src);
    assert!(!off.relief_on());
    assert_eq!(off.implementation_id(), EROSION_IMPLEMENTATION_ID);
    let (a, b) = (Arc::new(ReliefLattice::new(varying(), 11)), Arc::new(ReliefLattice::new(varying(), 12)));
    let (on_a, on_b) = (field(&src).with_relief(a), field(&src).with_relief(b));
    assert!(on_a.relief_on());
    assert_eq!(on_a.implementation_id(), EROSION_RELIEF_IMPLEMENTATION_ID);
    assert_ne!(on_a.inputs_hash(), off.inputs_hash(), "the channels re-key the field");
    assert_ne!(on_a.inputs_hash(), on_b.inputs_hash(), "other channels, another key");
}

// spec: GEN-099
#[test]
fn a_batch_of_heights_with_the_relief_on_equals_the_heights_one_by_one() {
    let g = grid();
    let src = GridSource(&g);
    let f = field(&src).with_relief(Arc::new(ReliefLattice::new(varying(), 1)));
    let mut rng = SplitMix64(5);
    let dirs: Vec<Vec3> = (0..400).map(|_| random_dir(&mut rng)).collect();
    let batch = f.heights_until(&dirs, &|| false);
    for (d, h) in dirs.iter().zip(&batch) {
        assert_eq!(h.to_bits(), f.height(*d).to_bits(), "the batch differs from the single read at {d:?}");
    }
}

// spec: GEN-102
#[test]
fn the_bounds_with_the_relief_on_hold_every_height_and_nested_caps_nest() {
    let g = grid();
    let src = GridSource(&g);
    let f = field(&src).with_relief(Arc::new(ReliefLattice::new(varying(), 1)));
    let mut rng = SplitMix64(8);
    for n in 0..150 {
        let c = if n % 10 == 0 { Vec3::new(1.0, 1.0 - 1e-9, 0.2).normalized() } else { random_dir(&mut rng) };
        let theta = [1e-5, 1e-4, 1e-3, 1e-2, 5e-2][n % 5];
        let (lo, hi) = f.height_bounds(c, theta);
        assert!(lo <= hi, "{c:?} {theta}");
        for ring in [0.0, 0.5, 1.0] {
            for k in 0..16 {
                let d = offset(c, theta * ring, f64::from(k) * std::f64::consts::TAU / 16.0);
                let h = f.height(d);
                assert!(lo <= h && h <= hi, "height {h} outside [{lo}, {hi}] at {d:?} in the cap {c:?} {theta}");
            }
        }
        let (ilo, ihi) = f.height_bounds(c, theta * 0.4);
        assert!(lo <= ilo + 1e-9 && ihi <= hi + 1e-9, "the inner cap's bounds [{ilo}, {ihi}] leave the outer's [{lo}, {hi}]");
    }
}

// spec: GEN-102
#[test]
fn the_ranges_over_a_cap_hold_every_value_read_in_it_include_the_face_edges_and_nest() {
    let lat = ReliefLattice::new(varying(), 1);
    let mut rng = SplitMix64(13);
    for n in 0..300 {
        // A third of the caps sit on a cube edge or corner.
        let c = match n % 3 {
            0 => Vec3::new(1.0, 1.0 - 1e-12, random_dir(&mut rng).z * 0.9).normalized(),
            1 => Vec3::new(1.0 - 1e-12, 1.0, 1.0).normalized(),
            _ => random_dir(&mut rng),
        };
        let theta = [1e-4, 1e-3, 1e-2, 0.1, 0.5][n % 5];
        let r = lat.range_over_cap(c, theta);
        for ring in [0.0, 0.5, 1.0] {
            for k in 0..24 {
                let d = offset(c, theta * ring, f64::from(k) * std::f64::consts::TAU / 24.0);
                let (kv, mv) = lat.at(d);
                assert!(
                    r[0] <= kv && kv <= r[1] && r[2] <= mv && mv <= r[3],
                    "(k, m) = ({kv}, {mv}) outside {r:?} in the cap {c:?} {theta}"
                );
            }
        }
        let inner = lat.range_over_cap(c, theta * 0.3);
        assert!(
            r[0] <= inner[0] && inner[1] <= r[1] && r[2] <= inner[2] && inner[3] <= r[3],
            "nested caps do not nest: {inner:?} in {r:?}"
        );
    }
    let g = lat.global_range();
    let all = lat.range_over_cap(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::PI);
    assert_eq!(all, g, "a cap over the whole sphere takes the global range");
}
