//! Tier A, GEN-129 (carve-runoff): the runoff laws, the discharge routing, the carve with weights and the vegetation factor.

use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve_rock, CarveParams, Rock};
use planet_generators::bake::face::{pass1_face_rock, FaceChange, FaceSpec, RockFields};
use planet_generators::bake::route::{route, route_weighted, NONE};
use planet_generators::bake::Grid;
use planet_generators::runoff::{
    discharge_weight, pet_mm, runoff_mm, vegetation_factor, RUNOFF_REF_MM, VEG_FLOOR, VEG_PEAK, VEG_PEAK_MM, WEIGHT_MAX, WEIGHT_MIN,
};

const DX: f64 = 800.0;
const N: usize = 32;
const R: f64 = 6_371_000.0;

fn cone() -> Vec<f64> {
    let c = (N as f64 - 1.0) / 2.0;
    (0..N * N)
        .map(|p| {
            let r = libm::hypot((p % N) as f64 - c, (p / N) as f64 - c);
            (3000.0 * (1.0 - r / c)).max(0.0)
        })
        .collect()
}

// spec: GEN-129
#[test]
fn the_runoff_is_zero_without_rain_rises_with_it_stays_under_it_and_follows_the_budyko_limits() {
    for t in [-10.0, 0.0, 10.0, 20.0, 28.0] {
        assert_eq!(runoff_mm(0.0, t), 0.0);
        let mut last = 0.0;
        for p in (50..6000).step_by(50) {
            let r = runoff_mm(f64::from(p), t);
            assert!(r >= last - 1e-9, "the runoff falls from {last} to {r} at P {p}, T {t}");
            assert!((0.0..=f64::from(p)).contains(&r), "the runoff {r} lies outside [0, P = {p}]");
            last = r;
        }
    }
    // Dry: P far under PET gives almost no runoff; wet: P far over PET gives about P - PET.
    let t = 25.0;
    assert!(runoff_mm(60.0, t) < 0.03 * 60.0, "a desert runs off {} of 60 mm", runoff_mm(60.0, t));
    let t = 10.0;
    let (p, pet) = (3500.0, pet_mm(t));
    assert!((runoff_mm(p, t) - (p - pet)).abs() < 0.1 * p, "a wet climate runs off {} against P - PET {}", runoff_mm(p, t), p - pet);
    assert_eq!(discharge_weight(0.0), WEIGHT_MIN);
    assert_eq!(discharge_weight(1e9), WEIGHT_MAX);
    assert!((discharge_weight(RUNOFF_REF_MM) - 1.0).abs() < 1e-12, "the reference runoff has weight 1");
}

// spec: GEN-129
#[test]
fn the_vegetation_factor_peaks_in_the_semi_arid_climate_and_is_the_floor_for_none_and_for_much_rain() {
    assert_eq!(vegetation_factor(0.0), VEG_FLOOR);
    assert!((vegetation_factor(VEG_PEAK_MM) - VEG_PEAK).abs() < 1e-12);
    assert!(vegetation_factor(6000.0) < VEG_FLOOR + 0.01);
    let best = (0..4000).map(|p| (p, vegetation_factor(f64::from(p)))).fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    assert!((best.0 - VEG_PEAK_MM as i32).abs() <= 1, "the factor peaks at {} mm, not {VEG_PEAK_MM}", best.0);
    assert!(vegetation_factor(150.0) > vegetation_factor(1500.0), "the semi-arid factor is not above the humid one");
}

// spec: GEN-129
#[test]
fn a_routing_with_weights_of_one_is_the_routing_without_and_with_weights_the_outlets_hold_the_weighted_total() {
    let g = Grid::new(N, N, DX);
    let z = cone();
    let outlet = g.outlets(&z, 0.0);
    let plain = route(&g, &z, &outlet, 1e-3, 9, 0.0);
    let ones = vec![1.0f32; N * N];
    let one = route_weighted(&g, &z, &outlet, 1e-3, 9, 0.0, Some(&ones));
    let bits = |a: &[f64]| a.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&plain.area), bits(&one.area), "weights of 1 change the area");
    assert_eq!(plain.receiver, one.receiver);
    let w: Vec<f32> = (0..N * N).map(|c| 0.05 + (c % 9) as f32 * 0.5).collect();
    let q = route_weighted(&g, &z, &outlet, 1e-3, 9, 0.0, Some(&w));
    let total: f64 = (0..N * N).map(|c| f64::from(w[c]) * DX * DX).sum();
    let at_outlets: f64 = (0..N * N).filter(|&c| q.receiver[c] == NONE).map(|c| q.area[c]).sum();
    assert!((at_outlets - total).abs() < 1e-6 * total, "the outlets hold {at_outlets} m2 of discharge, the weighted cells {total}");
    assert_eq!(q.receiver, plain.receiver, "weights moved the receivers");
}

// spec: GEN-129
#[test]
fn a_river_from_a_wet_catchment_keeps_its_discharge_across_a_dry_reach() {
    let n = 40;
    let g = Grid::new(n, n, DX);
    let z: Vec<f64> = (0..n * n).map(|c| 10.0 * (c % n) as f64).collect();
    let outlet = g.outlets(&z, 0.0);
    let dry = vec![0.05f32; n * n];
    let wet_above: Vec<f32> = (0..n * n).map(|c| if c % n >= 20 { 4.0 } else { 0.05 }).collect();
    let all_dry = route_weighted(&g, &z, &outlet, 1e-3, 3, 0.0, Some(&dry));
    let wet = route_weighted(&g, &z, &outlet, 1e-3, 3, 0.0, Some(&wet_above));
    let cell = (n / 2) * n + 5; // in the dry reach, 15 cells below the wet upland
    assert!(
        wet.area[cell] > 5.0 * all_dry.area[cell],
        "the discharge in the dry reach is {} against {} of a dry catchment",
        wet.area[cell],
        all_dry.area[cell]
    );
}

// spec: GEN-129
#[test]
fn a_wet_carve_incises_more_than_a_dry_one_and_the_carve_is_the_same_bits_on_any_thread_count() {
    let g = Grid::new(N, N, DX);
    let z0 = cone();
    let outlet = g.outlets(&z0, 0.0);
    let p = CarveParams { u_over_k: 0.0, keep_pits: false, ..CarveParams::default() };
    let run = |w: f32, threads: usize| {
        let weights = vec![w; N * N];
        let mut z = z0.clone();
        carve_rock(&g, &mut z, &outlet, &p, None, &Rock { par: &threads, k: None, uplift: None, runoff: Some(&weights) }, 5);
        z
    };
    let mean = |z: &[f64]| z.iter().sum::<f64>() / z.len() as f64;
    let (wet, dry) = (run(WEIGHT_MAX as f32, 1), run(WEIGHT_MIN as f32, 1));
    assert!(mean(&wet) < mean(&dry) - 1.0, "mean height wet {} against dry {}", mean(&wet), mean(&dry));
    let bits = |z: &[f64]| z.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&run(2.0, 1)), bits(&run(2.0, 8)), "the carve with weights differs between 1 and 8 threads");
}

fn base(d: Vec3) -> f64 {
    let ridge = 2200.0 * libm::sin(5.0 * d.x + 1.0) * libm::cos(4.0 * d.y) + 1400.0 * libm::sin(9.0 * d.z + 0.3);
    if ridge > 0.0 {
        ridge + 50.0
    } else {
        -2000.0 + 0.5 * ridge
    }
}

// spec: GEN-129
#[test]
fn a_face_carve_reads_the_runoff_at_the_cells_directions_and_the_fields_have_their_key_bits() {
    let spec = FaceSpec { face: Face(0), cells: 64, radius_m: R, sea_level: 0.0, seed: 17, halo: 0 };
    let p = CarveParams::default();
    let wet_west = |d: Vec3| if d.x < 0.0 { 6.0 } else { 0.1 };
    let k = |_: Vec3, _: f64| planet_generators::bake::carve::Layer::of(1.0);
    let rock = RockFields { k: Some(&k), uplift: None, runoff: Some(&wet_west), vegetation: true };
    assert_eq!(rock.mask(), 1 | 4 | 8);
    assert_eq!(RockFields::none().mask(), 0);
    let run = |threads| pass1_face_rock(&spec, &base, None, &rock, &p, threads);
    let (one, eight) = (run(1), run(8));
    let bits_of = |c: &FaceChange| c.change.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits_of(&one.0), bits_of(&eight.0), "the face's change differs between 1 and 8 threads");
    assert_eq!(one.1, eight.1, "the area codes differ between 1 and 8 threads");
    let without = RockFields { runoff: None, vegetation: false, ..rock };
    let plain = pass1_face_rock(&spec, &base, None, &without, &p, 2);
    assert_ne!(bits_of(&one.0), bits_of(&plain.0), "the runoff weights changed nothing");
    assert_ne!(one.1, plain.1, "the area codes do not hold the discharge");
}
