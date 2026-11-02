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

/// Channels of `cells` cells per face edge whose `k` and `m` are the functions of the direction `kf` and `mf` at every lattice point.
fn channels_n(cells: usize, kf: &dyn Fn(Vec3) -> f64, mf: &dyn Fn(Vec3) -> f64) -> ReliefChannels {
    let faces = (0..6u8)
        .map(|f| {
            let (mut k, mut m) = (Vec::new(), Vec::new());
            for j in 0..=cells {
                for i in 0..=cells {
                    let d =
                        face_to_direction(&TangentWarp, Face(f), 2.0 * i as f64 / cells as f64 - 1.0, 2.0 * j as f64 / cells as f64 - 1.0);
                    k.push(kf(d) as f32);
                    m.push(mf(d) as f32);
                }
            }
            (k, m)
        })
        .collect();
    ReliefChannels::from_planes(cells, faces).expect("channels of a synthetic lattice")
}

fn channels(kf: &dyn Fn(Vec3) -> f64, mf: &dyn Fn(Vec3) -> f64) -> ReliefChannels {
    channels_n(CELLS, kf, mf)
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
    let ones = field(&src).with_relief(Arc::new(ReliefLattice::new(channels(&|_| 1.0, &|_| 300.0)).unwrap())).unwrap();
    let twice = field(&src).with_relief(Arc::new(ReliefLattice::new(channels(&|_| 2.0, &|_| 100.0)).unwrap())).unwrap();
    let mut rng = SplitMix64(3);
    for _ in 0..300 {
        let d = random_dir(&mut rng);
        let h = off.height(d);
        let a = ones.height(d);
        assert!((a - h).abs() < 1e-9 * (1.0 + h.abs() + 300.0), "k 1 changes the height: {a} against {h}");
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
    let on = |c: ReliefChannels| field(&src).with_relief(Arc::new(ReliefLattice::new(c).unwrap())).unwrap();
    let (a, a2, b) = (on(varying()), on(varying()), on(channels(&|_| 2.0, &|_| 100.0)));
    assert!(a.relief_on());
    assert_eq!(a.implementation_id(), EROSION_RELIEF_IMPLEMENTATION_ID);
    assert_ne!(a.inputs_hash(), off.inputs_hash(), "the channels re-key the field");
    assert_eq!(a.inputs_hash(), a2.inputs_hash(), "the same channels give the same key");
    assert_ne!(a.inputs_hash(), b.inputs_hash(), "other channels, another key (the identity is a hash of their content)");
}

// spec: GEN-099
#[test]
fn channels_of_another_resolution_key_the_field_differently_and_a_scaled_body_or_a_bad_lattice_is_an_error() {
    let g = grid();
    let src = GridSource(&g);
    let (coarse, fine) = (channels_n(CELLS, &|_| 2.0, &|_| 100.0), channels_n(2 * CELLS, &|_| 2.0, &|_| 100.0));
    let on = |c: ReliefChannels| field(&src).with_relief(Arc::new(ReliefLattice::new(c).unwrap())).unwrap();
    assert_ne!(on(coarse).inputs_hash(), on(fine).inputs_hash(), "the same values at another resolution are another key");
    // A body with a relief scale other than 1: the channels are of the bake as it is.
    let mut def = WorldDefinition::single_planet(R, SEED);
    def.set(&planet_world_def::body_relief_scale_path(BodyId::PLANET), planet_world_def::NodeValue::F64(2.0));
    let scaled = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, KEY).unwrap();
    let e = scaled
        .with_relief(Arc::new(ReliefLattice::new(channels(&|_| 1.0, &|_| 0.0)).unwrap()))
        .err()
        .expect("a scaled body refuses the channels");
    assert!(e.contains("relief scale 1") && e.contains('2'), "{e}");
    // A lattice with fewer cells than a block cannot be built; planes with a value that is not finite are refused when the channels are made.
    assert!(ReliefChannels::from_planes(16, vec![(vec![1.0; 17 * 17], vec![0.0; 17 * 17]); 6]).is_none(), "fewer cells than a block");
    let mut planes: Vec<(Vec<f32>, Vec<f32>)> = vec![(vec![1.0; (CELLS + 1) * (CELLS + 1)], vec![0.0; (CELLS + 1) * (CELLS + 1)]); 6];
    planes[2].1[5] = f32::NAN;
    assert!(ReliefChannels::from_planes(CELLS, planes).is_none(), "a NaN plane is refused");
}

// spec: GEN-099
#[test]
fn a_batch_of_heights_with_the_relief_on_equals_the_heights_one_by_one() {
    let g = grid();
    let src = GridSource(&g);
    let f = field(&src).with_relief(Arc::new(ReliefLattice::new(varying()).unwrap())).unwrap();
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
    let f = field(&src).with_relief(Arc::new(ReliefLattice::new(varying()).unwrap())).unwrap();
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
    let lat = ReliefLattice::new(varying()).unwrap();
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

// spec: GEN-102
#[test]
fn the_range_over_a_cap_below_a_block_comes_from_the_lattice_points_around_it_not_from_the_whole_block() {
    use planet_core::cube::direction_to_face;
    let lat = ReliefLattice::new(varying()).unwrap();
    let mut rng = SplitMix64(21);
    let (mut tight, mut block_width, mut checked) = (0.0f64, 0.0f64, 0);
    for _ in 0..400 {
        let c = random_dir(&mut rng);
        let (face, s, t) = direction_to_face(&TangentWarp, c);
        let (x, y) = ((s + 1.0) * 0.5 * CELLS as f64, (t + 1.0) * 0.5 * CELLS as f64);
        let (i, j) = (x as usize, y as usize);
        // Away from the face edges, so that the neighbourhood lies on this face.
        if i < 4 || j < 4 || i + 5 > CELLS || j + 5 > CELLS {
            continue;
        }
        let r = lat.range_over_cap(c, 1e-7);
        let mut hood = [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY];
        for jj in j - 3..=j + 4 {
            for ii in i - 3..=i + 4 {
                let (k, m) = lat.channels().at(face, ii, jj);
                hood = [hood[0].min(f64::from(k)), hood[1].max(f64::from(k)), hood[2].min(f64::from(m)), hood[3].max(f64::from(m))];
            }
        }
        assert!(
            hood[0] <= r[0] && r[1] <= hood[1] && hood[2] <= r[2] && r[3] <= hood[3],
            "the range {r:?} of a tiny cap leaves the lattice points around it {hood:?}"
        );
        let block = lat.channels().block_range(face, i / 32, j / 32).map(f64::from);
        tight += (r[1] - r[0]) + (r[3] - r[2]);
        block_width += (block[1] - block[0]) + (block[3] - block[2]);
        checked += 1;
    }
    assert!(checked > 100, "{checked} caps away from the face edges");
    assert!(tight < 0.5 * block_width, "the ranges of tiny caps ({tight}) are not much tighter than their blocks' ({block_width})");
}

// spec: GEN-102
#[test]
fn the_global_height_bound_with_the_relief_on_holds_every_height_and_is_not_wildly_loose() {
    let g = grid();
    let src = GridSource(&g);
    let steep = channels(&|d| 1.0 + 3.0 * (0.5 + 0.5 * libm::sin(5.0 * d.x + 2.0) * libm::cos(3.0 * d.z)), &|d| {
        600.0 * libm::sin(5.0 * d.y) + 100.0
    });
    let f = field(&src).with_relief(Arc::new(ReliefLattice::new(steep).unwrap())).unwrap();
    let mut rng = SplitMix64(33);
    let (bound, mut seen) = (f.max_abs_height_m(), 0.0f64);
    for _ in 0..4_000 {
        let h = f.height(random_dir(&mut rng)).abs();
        assert!(h <= bound, "height {h} over max_abs_height_m {bound}");
        seen = seen.max(h);
    }
    assert!(bound < 5.0 * (seen + 1.0e4), "the bound {bound} against the largest height seen {seen}");
}

// spec: GEN-102
#[test]
fn ranges_hold_for_caps_over_a_face_or_the_hemisphere_and_non_concentric_caps_nest() {
    let lat = ReliefLattice::new(varying()).unwrap();
    let mut rng = SplitMix64(44);
    for theta in [0.8, 1.0, std::f64::consts::FRAC_PI_2, 2.0, 3.0] {
        let c = random_dir(&mut rng);
        let r = lat.range_over_cap(c, theta);
        for _ in 0..400 {
            let d = random_dir(&mut rng);
            if c.angle_to(d) <= theta {
                let (k, m) = lat.at(d);
                assert!(r[0] <= k && k <= r[1] && r[2] <= m && m <= r[3], "({k}, {m}) outside {r:?} in the cap {c:?} {theta}");
            }
        }
    }
    for _ in 0..300 {
        let outer_c = random_dir(&mut rng);
        let outer_r = 0.05 + rng.next_f64() * 0.6;
        let inner_r = rng.next_f64() * outer_r * 0.4;
        let inner_c = offset(outer_c, rng.next_f64() * (outer_r - inner_r) * 0.99, rng.next_f64() * std::f64::consts::TAU);
        let (o, i) = (lat.range_over_cap(outer_c, outer_r), lat.range_over_cap(inner_c, inner_r));
        assert!(
            o[0] <= i[0] && i[1] <= o[1] && o[2] <= i[2] && i[3] <= o[3],
            "an inner cap at another centre {i:?} leaves its outer cap {o:?}"
        );
    }
}

// spec: GEN-102
#[test]
fn the_pyramid_holds_at_depth_with_512_cells_per_face_edge_and_the_planet_wide_range_is_the_top() {
    use planet_core::cube::direction_to_face;
    let n = 512;
    let lat = ReliefLattice::new(channels_n(n, &|d| 1.0 + 3.0 * (0.5 + 0.5 * libm::sin(9.0 * d.x + 1.0) * libm::cos(7.0 * d.y)), &|d| {
        400.0 * libm::sin(11.0 * d.z) + 150.0
    }))
    .unwrap();
    let mut rng = SplitMix64(55);
    for theta in [1e-4, 1e-3, 1e-2, 0.05, 0.2, 0.7, 1.2] {
        for _ in 0..30 {
            let c = random_dir(&mut rng);
            let r = lat.range_over_cap(c, theta);
            let inner = lat.range_over_cap(c, theta * 0.4);
            assert!(r[0] <= inner[0] && inner[1] <= r[1] && r[2] <= inner[2] && inner[3] <= r[3], "nesting at depth, cap {theta}");
            for ring in [0.0, 0.5, 1.0] {
                for k in 0..12 {
                    let d = offset(c, theta * ring, f64::from(k) * std::f64::consts::TAU / 12.0);
                    let (kv, mv) = lat.at(d);
                    assert!(
                        r[0] <= kv && kv <= r[1] && r[2] <= mv && mv <= r[3],
                        "(k, m) = ({kv}, {mv}) outside {r:?} at depth, cap {theta}"
                    );
                }
            }
        }
    }
    let g = lat.global_range();
    assert_eq!(lat.range_over_cap(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::PI), g);
    // The top of the pyramid is the whole planet: a point range is inside it.
    let (face, _, _) = direction_to_face(&TangentWarp, Vec3::new(0.3, -0.5, 0.8).normalized());
    let (k, m) = lat.channels().at(face, 17, 99);
    assert!(g[0] <= f64::from(k) && f64::from(k) <= g[1] && g[2] <= f64::from(m) && f64::from(m) <= g[3]);
}

// spec: GEN-102
#[test]
fn field_bounds_with_the_relief_on_nest_for_caps_at_other_centres_and_every_batch_path_agrees() {
    let g = grid();
    let src = GridSource(&g);
    let f = field(&src).with_relief(Arc::new(ReliefLattice::new(varying()).unwrap())).unwrap();
    let mut rng = SplitMix64(66);
    for _ in 0..120 {
        let outer_c = random_dir(&mut rng);
        let outer_r = 1e-3 + rng.next_f64() * 0.02;
        let inner_r = rng.next_f64() * outer_r * 0.4;
        let inner_c = offset(outer_c, rng.next_f64() * (outer_r - inner_r) * 0.99, rng.next_f64() * std::f64::consts::TAU);
        let (olo, ohi) = f.height_bounds(outer_c, outer_r);
        let (ilo, ihi) = f.height_bounds(inner_c, inner_r);
        assert!(
            olo <= ilo + 1e-9 && ihi <= ohi + 1e-9,
            "the bounds of a cap at another centre [{ilo}, {ihi}] leave its outer cap's [{olo}, {ohi}]"
        );
    }
    // The batch paths agree with the single read: the threaded batch and the batch with the climate.
    let dirs: Vec<Vec3> = (0..200).map(|_| random_dir(&mut rng)).collect();
    let single: Vec<u64> = dirs.iter().map(|d| f.height(*d).to_bits()).collect();
    let threaded = f.heights_threaded_until(&dirs, &|| false, &4usize);
    assert_eq!(threaded.iter().map(|h| h.to_bits()).collect::<Vec<_>>(), single, "the threaded batch differs from the single reads");
    let id = planet_core::TileId::new(Face(2), 5, 9, 14).unwrap();
    let (with_climate, _) = f.heights_and_climate(&TangentWarp, id, &dirs, &|| false);
    assert_eq!(
        with_climate.iter().map(|h| h.to_bits()).collect::<Vec<_>>(),
        single,
        "the batch with the climate differs from the single reads"
    );
}
