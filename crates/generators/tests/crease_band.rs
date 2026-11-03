//! Tier A, GEN-109 (ADR 0098): the crease band of the relief field. Off unless `with_crease_band` is called (today's field, bit for bit, whatever the relief switch says); on, the crease
//! octaves above the layer's grid are dropped where the base is at or above the band's sea level; the bounds hold every height and nest; the keys follow the band.
//! Synthetic channels on small lattices; no planet bake.

use std::sync::Arc;

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Vec3};
use planet_generators::erosion::{
    CreaseBand, ErosionField, ErosionGenerator, CREASE_BAND_VERSION, EROSION_BAND_IMPLEMENTATION_ID, EROSION_C1_BAND_IMPLEMENTATION_ID,
    EROSION_C1_RELIEF_IMPLEMENTATION_ID, EROSION_RELIEF_IMPLEMENTATION_ID,
};
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

fn plain<'a>(src: &'a dyn LatticeSource, c1: bool) -> ErosionField<'a> {
    let def = WorldDefinition::single_planet(R, SEED);
    let g = ErosionGenerator;
    if c1 {
        g.field_c1(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
    } else {
        g.field(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
    }
}

fn channels() -> ReliefChannels {
    let (kf, mf): (&dyn Fn(Vec3) -> f64, &dyn Fn(Vec3) -> f64) =
        (&|d| 1.0 + 3.0 * (0.5 + 0.5 * libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y)), &|d| 400.0 * libm::sin(4.0 * d.z) + 150.0);
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

fn relief<'a>(src: &'a dyn LatticeSource, c1: bool) -> ErosionField<'a> {
    plain(src, c1).with_relief(Arc::new(ReliefLattice::new(channels()).unwrap())).unwrap()
}

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized()
}

fn offset(c: Vec3, theta: f64, phi: f64) -> Vec3 {
    let a = if c.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = c.cross(a).normalized();
    let e2 = c.cross(e1);
    (c * libm::cos(theta) + (e1 * libm::cos(phi) + e2 * libm::sin(phi)) * libm::sin(theta)).normalized()
}

// spec: GEN-109
#[test]
fn without_the_call_the_field_is_todays_whatever_the_relief_switch_says_and_with_nothing_dropped_it_is_too() {
    let g = grid();
    let src = GridSource(&g);
    for c1 in [false, true] {
        let off = relief(&src, c1);
        assert!(!off.crease_band_on());
        // A sea level above every ground: the band applies nowhere, so every point keeps today's creases, bit for bit.
        let high = relief(&src, c1).with_crease_band(CreaseBand::of_the_bake(1.0e9)).unwrap();
        // A layer cell so large that every octave is within two cells: the band keeps every octave at weight 1, bit for bit.
        let all_kept = relief(&src, c1)
            .with_crease_band(CreaseBand { layer_cell_m: 1.0e9, near_cells: 2.0, far_cells: 4.0, sea_level_m: -1.0e9 })
            .unwrap();
        let mut rng = SplitMix64(5);
        for _ in 0..400 {
            let d = random_dir(&mut rng);
            let h = off.height(d).to_bits();
            assert_eq!(high.height(d).to_bits(), h, "c1 {c1}: the band below its sea level");
            assert_eq!(all_kept.height(d).to_bits(), h, "c1 {c1}: a band that keeps every octave");
        }
    }
}

// spec: GEN-109
#[test]
fn the_band_changes_the_land_and_leaves_the_sea_floor_alone() {
    let g = grid();
    let src = GridSource(&g);
    let off = relief(&src, true);
    let on = relief(&src, true).with_crease_band(CreaseBand::of_the_bake(0.0)).unwrap();
    let mut rng = SplitMix64(6);
    let (mut land, mut land_changed, mut floor) = (0, 0, 0);
    for _ in 0..3000 {
        let d = random_dir(&mut rng);
        let e = off.base_elevation(d);
        let (a, b) = (off.height(d), on.height(d));
        if e < 0.0 {
            floor += 1;
            assert_eq!(a.to_bits(), b.to_bits(), "the sea floor keeps today's creases (base {e} m)");
        } else {
            land += 1;
            if a.to_bits() != b.to_bits() {
                land_changed += 1;
            }
        }
    }
    assert!(floor > 300 && land > 300, "{floor} floor and {land} land samples");
    assert!(land_changed * 10 >= land * 9, "{land_changed} of {land} land samples change");
}

// spec: GEN-109
#[test]
fn the_band_needs_the_relief_channels_a_valid_band_and_changes_the_keys_and_the_id() {
    let g = grid();
    let src = GridSource(&g);
    // No relief channels: refused, naming the switch.
    let e = plain(&src, false).with_crease_band(CreaseBand::of_the_bake(0.0)).err().unwrap();
    assert!(e.contains("relief channels"), "{e}");
    for bad in [
        CreaseBand { layer_cell_m: 0.0, ..CreaseBand::of_the_bake(0.0) },
        CreaseBand { near_cells: 4.0, far_cells: 4.0, ..CreaseBand::of_the_bake(0.0) },
        CreaseBand { near_cells: 0.0, ..CreaseBand::of_the_bake(0.0) },
        CreaseBand { layer_cell_m: f64::NAN, ..CreaseBand::of_the_bake(0.0) },
        CreaseBand { sea_level_m: f64::INFINITY, ..CreaseBand::of_the_bake(0.0) },
    ] {
        let e = relief(&src, false).with_crease_band(bad).err().unwrap();
        assert!(e.contains("crease band"), "{bad:?}: {e}");
    }
    for (c1, relief_id, band_id) in [
        (false, EROSION_RELIEF_IMPLEMENTATION_ID, EROSION_BAND_IMPLEMENTATION_ID),
        (true, EROSION_C1_RELIEF_IMPLEMENTATION_ID, EROSION_C1_BAND_IMPLEMENTATION_ID),
    ] {
        let off = relief(&src, c1);
        let a = relief(&src, c1).with_crease_band(CreaseBand::of_the_bake(0.0)).unwrap();
        let b = relief(&src, c1).with_crease_band(CreaseBand { near_cells: 3.0, ..CreaseBand::of_the_bake(0.0) }).unwrap();
        let c = relief(&src, c1).with_crease_band(CreaseBand::of_the_bake(5.0)).unwrap();
        assert_eq!((off.implementation_id(), a.implementation_id()), (relief_id, band_id));
        let keys = [off.inputs_hash(), a.inputs_hash(), b.inputs_hash(), c.inputs_hash()];
        for i in 0..4 {
            for j in 0..i {
                assert_ne!(keys[i], keys[j], "c1 {c1}: the keys of {i} and {j} differ");
            }
        }
        assert_eq!(
            a.inputs_hash(),
            relief(&src, c1).with_crease_band(CreaseBand::of_the_bake(0.0)).unwrap().inputs_hash(),
            "deterministic"
        );
    }
    assert_eq!(CREASE_BAND_VERSION, 1);
}

/// Every height of a cap's rings lies within the bounds; returns the extent.
fn check_cap(f: &dyn HeightField, c: Vec3, theta: f64) -> f64 {
    let (lo, hi) = f.height_bounds(c, theta);
    assert!(lo <= hi, "{c:?} {theta}");
    for ring in [0.0, 0.3, 0.7, 1.0] {
        for k in 0..24 {
            let d = offset(c, theta * ring, f64::from(k) * std::f64::consts::TAU / 24.0);
            let h = f.height(d);
            assert!(lo <= h && h <= hi, "height {h} outside [{lo}, {hi}] at {d:?} in the cap {c:?} {theta}");
            let h32 = f64::from(h as f32);
            assert!(lo <= h32 && h32 <= hi, "f32 height {h32} outside [{lo}, {hi}]");
        }
    }
    hi - lo
}

// spec: GEN-109
#[test]
fn the_bounds_hold_every_height_with_the_band_including_caps_that_straddle_its_sea_level() {
    let g = grid();
    let src = GridSource(&g);
    // Levels that put the band's edge through the middle of the land: caps straddle it everywhere.
    for level in [0.0, 150.0, 600.0] {
        for c1 in [false, true] {
            let f = relief(&src, c1).with_crease_band(CreaseBand::of_the_bake(level)).unwrap();
            let mut rng = SplitMix64(11);
            for case in 0..160 {
                let c = random_dir(&mut rng);
                let theta = [1e-5, 1e-4, 1e-3, 0.005, 0.02][case % 5];
                check_cap(&f, c, theta);
            }
            for face in 0..6u8 {
                for &t in &[0.9, 0.999, 1.0] {
                    for &theta in &[0.0005, 0.005, 0.02] {
                        check_cap(&f, face_to_direction(&TangentWarp, Face(face), 0.0, t), theta);
                    }
                }
            }
            assert!(f.error().is_none());
        }
    }
}

// spec: GEN-109
#[test]
fn the_banded_bounds_are_tighter_than_the_plain_ones_on_land_and_nested_caps_give_nested_bounds() {
    let g = grid();
    let src = GridSource(&g);
    let off = relief(&src, true);
    let on = relief(&src, true).with_crease_band(CreaseBand::of_the_bake(-1.0e9)).unwrap();
    let mut rng = SplitMix64(12);
    let (mut tighter, mut total) = (0, 0);
    for _ in 0..200 {
        let c = random_dir(&mut rng);
        let theta = 0.004;
        let (lo0, hi0) = off.height_bounds(c, theta);
        let (lo1, hi1) = on.height_bounds(c, theta);
        total += 1;
        if hi1 - lo1 < hi0 - lo0 {
            tighter += 1;
        }
        // The band never widens the bounds of a cap that lies wholly on land.
        assert!(lo1 >= lo0 - 1e-6 && hi1 <= hi0 + 1e-6, "[{lo1}, {hi1}] not inside [{lo0}, {hi0}]");
    }
    assert!(tighter * 10 >= total * 9, "{tighter} of {total} caps have tighter bounds");
    // LOD-012: an inner cap's bounds lie inside the outer cap's, around the sea level of the band too.
    let band = relief(&src, true).with_crease_band(CreaseBand::of_the_bake(300.0)).unwrap();
    let mut rng = SplitMix64(13);
    for _ in 0..400 {
        let c = random_dir(&mut rng);
        let (outer_r, inner_r) = (0.01, 0.003);
        let inner_c = offset(c, 0.9 * (outer_r - inner_r), rng.next_f64() * std::f64::consts::TAU);
        let (olo, ohi) = band.height_bounds(c, outer_r);
        let (ilo, ihi) = band.height_bounds(inner_c, inner_r);
        assert!(ilo >= olo - 1e-3 && ihi <= ohi + 1e-3, "inner [{ilo}, {ihi}] not within outer [{olo}, {ohi}] at {c:?}");
    }
}

// spec: GEN-109
#[test]
fn a_batch_and_the_threads_give_the_bits_of_the_single_reads_with_the_band() {
    let g = grid();
    let src = GridSource(&g);
    let f = relief(&src, true).with_crease_band(CreaseBand::of_the_bake(0.0)).unwrap();
    let mut rng = SplitMix64(14);
    let dirs: Vec<Vec3> = (0..500).map(|_| random_dir(&mut rng)).collect();
    let one: Vec<u64> = dirs.iter().map(|d| f.height(*d).to_bits()).collect();
    let batch: Vec<u64> = f.heights_until(&dirs, &|| false).iter().map(|h| h.to_bits()).collect();
    assert_eq!(one, batch);
    for threads in [1usize, 3, 6] {
        let t: Vec<u64> = f.heights_threaded(&dirs, &threads).iter().map(|h| h.to_bits()).collect();
        assert_eq!(one, t, "{threads} threads");
    }
}
