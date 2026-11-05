//! Tier A, GEN-127 (plan PR 4): the carve's rock fields. The erodibility K(cell, z) multiplies the class carve factor in each step, read at the cell's height at the start of the step;
//! the uplift array replaces `u_over_k`, tapered by the height and scaled by neither K nor the factor.

use planet_core::par::Par;
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, carve_rock, CarveParams, Layer, Rock};
use planet_generators::bake::face::{pass1_face_rock, FaceSpec, RockFields};
use planet_generators::bake::Grid;
use std::sync::Mutex;

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

fn bits(z: &[f64]) -> Vec<u64> {
    z.iter().map(|v| v.to_bits()).collect()
}

fn setup() -> (Grid, Vec<f64>, Vec<bool>) {
    let g = Grid::new(N, N, DX);
    let z = cone();
    let outlet = g.outlets(&z, 0.0);
    (g, z, outlet)
}

// spec: GEN-127
#[test]
fn a_unit_erodibility_and_the_uniform_uplift_as_an_array_are_the_carve_without_fields_bit_for_bit() {
    let (g, z0, outlet) = setup();
    let p = CarveParams::default();
    let mut plain = z0.clone();
    carve(&g, &mut plain, &outlet, &p, None, 5);
    let one = |_: usize, _: f64| Layer::of(1.0);
    let u = vec![p.u_over_k as f32; N * N];
    let mut with = z0.clone();
    carve_rock(&g, &mut with, &outlet, &p, None, &Rock { par: &2usize, k: Some(&one), uplift: Some(&u) }, 5);
    assert_eq!(bits(&plain), bits(&with), "K = 1 and U = u_over_k must give today's carve");
}

// spec: GEN-127
#[test]
fn a_harder_rock_settles_higher_at_the_same_uplift_and_a_softer_one_lower() {
    let (g, z0, outlet) = setup();
    let p = CarveParams::default();
    let u = vec![p.u_over_k as f32; N * N];
    let mean = |k: f64| {
        let f = move |_: usize, _: f64| Layer::of(k);
        let mut z = z0.clone();
        carve_rock(&g, &mut z, &outlet, &p, None, &Rock { par: &2usize, k: Some(&f), uplift: Some(&u) }, 5);
        z.iter().sum::<f64>() / z.len() as f64
    };
    let (hard, mid, soft) = (mean(0.12), mean(1.0), mean(1.8));
    assert!(hard > mid && mid > soft, "mean height after the carve: hard {hard}, mid {mid}, soft {soft}");
}

// spec: GEN-127
#[test]
fn the_uplift_is_scaled_by_neither_the_erodibility_nor_the_carve_factor() {
    let (g, z0, outlet) = setup();
    // Erodibility 0: nothing incises, so a cell rises by k_dt * U * taper in each step whatever the factor is.
    let p = CarveParams { steps: 3, reroute_every: 5, keep_pits: false, ..CarveParams::default() };
    let none = |_: usize, _: f64| Layer::of(0.0);
    let u: Vec<f32> = (0..N * N).map(|c| 10.0 + (c % 7) as f32).collect();
    let rise = |factor: f64| {
        let f = vec![factor; N * N];
        let mut z = z0.clone();
        carve_rock(&g, &mut z, &outlet, &p, Some(&f), &Rock { par: &1usize, k: Some(&none), uplift: Some(&u) }, 5);
        z
    };
    let (a, b) = (rise(1.0), rise(5.0));
    assert_eq!(bits(&a), bits(&b), "the carve factor changed the uplift");
    let mut moved = 0;
    for c in 0..N * N {
        if outlet[c] {
            continue;
        }
        let taper = planet_core::smoothstep(0.0, p.uplift_taper_m, z0[c]) as f32;
        let want = p.steps as f64 * p.k_dt * f64::from(u[c]) * f64::from(taper);
        assert!((a[c] - z0[c] - want).abs() < 1e-6, "cell {c} rose by {} m, not {want}", a[c] - z0[c]);
        moved += usize::from(want > 0.0);
    }
    assert!(moved > 500, "{moved} cells were lifted");
}

// spec: GEN-127
#[test]
fn the_erodibility_is_read_at_the_height_the_cell_has_at_the_start_of_the_step() {
    let (g, z0, outlet) = setup();
    let p = CarveParams { steps: 2, reroute_every: 5, ..CarveParams::default() };
    let seen = Mutex::new(Vec::<(usize, f64)>::new());
    let record = |c: usize, z: f64| {
        seen.lock().unwrap().push((c, z));
        Layer::of(1.0)
    };
    let mut z2 = z0.clone();
    carve_rock(&g, &mut z2, &outlet, &p, None, &Rock { par: &1usize, k: Some(&record), uplift: None }, 5);
    let mut z1 = z0.clone();
    carve(&g, &mut z1, &outlet, &CarveParams { steps: 1, ..p }, None, 5);
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.len(), 2 * N * N, "one read of every cell in each step");
    let (first, second) = seen.split_at(N * N);
    let mut changed = 0;
    for c in 0..N * N {
        assert_eq!(first[c], (c, z0[c]), "step 1 reads the start height of cell {c}");
        assert_eq!(second[c], (c, z1[c]), "step 2 reads the height after step 1 of cell {c}");
        changed += usize::from(z1[c] != z0[c]);
    }
    assert!(changed > 300, "{changed} cells changed in the first step: the test would not tell the heights apart");
}

// spec: GEN-127
#[test]
fn the_carve_with_fields_is_the_same_bits_on_any_thread_count_and_refuses_a_negative_uplift_or_erodibility() {
    let (g, z0, outlet) = setup();
    let p = CarveParams::default();
    let k = |c: usize, z: f64| Layer::of(0.2 + 1.5 * (0.5 + 0.5 * libm::sin(0.01 * z + 0.3 * (c % 11) as f64)));
    let u: Vec<f32> = (0..N * N).map(|c| 5.0 + (c % 13) as f32 * 4.0).collect();
    let run = |par: &dyn Par| {
        let mut z = z0.clone();
        carve_rock(&g, &mut z, &outlet, &p, None, &Rock { par, k: Some(&k), uplift: Some(&u) }, 5);
        bits(&z)
    };
    assert_eq!(run(&1usize), run(&8usize), "the carve differs between 1 and 8 threads");
    let refused = |k: &'static (dyn Fn(usize, f64) -> Layer + Sync), u: Vec<f32>| {
        let (g, z0, outlet) = setup();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let mut z = z0;
            carve_rock(&g, &mut z, &outlet, &CarveParams::default(), None, &Rock { par: &1usize, k: Some(k), uplift: Some(&u) }, 1);
        }))
        .is_err()
    };
    assert!(refused(&|_, _| Layer::of(-0.5), vec![1.0; N * N]), "a negative erodibility was accepted");
    assert!(refused(&|_, _| Layer::of(1.0), vec![-1.0; N * N]), "a negative uplift was accepted");
}

fn base(d: Vec3) -> f64 {
    let ridge = 2200.0 * libm::sin(5.0 * d.x + 1.0) * libm::cos(4.0 * d.y) + 1400.0 * libm::sin(9.0 * d.z + 0.3);
    if ridge > 0.0 {
        ridge + 50.0
    } else {
        -2000.0 + 0.5 * ridge
    }
}

// spec: GEN-127
#[test]
fn a_face_carve_reads_the_fields_at_the_cells_directions_and_is_the_same_bytes_on_any_thread_count() {
    let spec = FaceSpec { face: Face(0), cells: 64, radius_m: R, sea_level: 0.0, seed: 17, halo: 0 };
    let p = CarveParams::default();
    let k = |d: Vec3, z: f64| Layer::of(0.3 + 1.2 * (0.5 + 0.5 * libm::sin(3.0 * d.x + 0.004 * z)));
    let u = |d: Vec3| 8.0 + 40.0 * (0.5 + 0.5 * libm::cos(4.0 * d.y));
    let rock = RockFields { k: Some(&k), uplift: Some(&u) };
    let run = |threads| pass1_face_rock(&spec, &base, None, &rock, &p, threads);
    let (one, eight) = (run(1), run(8));
    let bits_of = |c: &planet_generators::bake::face::FaceChange| c.change.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits_of(&one.0), bits_of(&eight.0), "the face's change differs between 1 and 8 threads");
    assert_eq!(one.1, eight.1, "the area codes differ between 1 and 8 threads");
    let plain = pass1_face_rock(&spec, &base, None, &RockFields::none(), &p, 2);
    assert_ne!(bits_of(&one.0), bits_of(&plain.0), "the fields changed nothing");
    let mean = |c: &planet_generators::bake::face::FaceChange| c.change.iter().map(|v| f64::from(*v)).sum::<f64>() / c.change.len() as f64;
    assert!((mean(&one.0) - mean(&plain.0)).abs() > 0.01, "the mean change is the same with and without the fields");
}

// spec: GEN-127
#[test]
fn an_erodibility_without_an_uplift_array_does_not_scale_the_uniform_uplift_and_the_row_blocks_of_a_tall_grid_read_the_right_cells() {
    let (g, z0, outlet) = setup();
    let p = CarveParams { steps: 3, keep_pits: false, ..CarveParams::default() };
    let none = |_: usize, _: f64| Layer::of(0.0);
    let mut z = z0.clone();
    carve_rock(&g, &mut z, &outlet, &p, None, &Rock { par: &1usize, k: Some(&none), uplift: None }, 5);
    let lifted = (0..N * N).filter(|&c| !outlet[c] && z[c] > z0[c]).count();
    assert!(lifted > 500, "K = 0 stopped the uniform uplift: {lifted} cells rose");
    // A grid of 130 rows is three blocks of the erodibility pass, the last one partial: every cell is read once at its own height.
    let n = 130;
    let g = Grid::new(n, n, DX);
    let z0: Vec<f64> = (0..n * n).map(|c| 40.0 + (c % n) as f64 * 3.0 + (c / n) as f64).collect();
    let outlet = g.outlets(&z0, 0.0);
    let seen = Mutex::new(vec![f64::NAN; n * n]);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let record = |c: usize, z: f64| {
        seen.lock().unwrap()[c] = z;
        calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Layer::of(1.0)
    };
    let mut z = z0.clone();
    carve_rock(
        &g,
        &mut z,
        &outlet,
        &CarveParams { steps: 1, ..CarveParams::default() },
        None,
        &Rock { par: &4usize, k: Some(&record), uplift: None },
        5,
    );
    assert_eq!(calls.into_inner(), n * n, "one read of every cell");
    assert_eq!(seen.into_inner().unwrap(), z0, "a cell was read at another cell's height");
}

// spec: GEN-127
#[test]
fn a_caprock_over_soft_rock_forms_a_step_because_a_step_never_lowers_a_cell_past_the_lower_edge_of_its_layer() {
    // A steep ramp of soft rock (K 1.8) with a 40 m caprock (K 0.001) whose top lies 60 m below the start: a trunk's single step lowers by more than the cap's thickness,
    // so without the edge a cell would jump over it.
    let n = 24;
    let g = Grid::new(n, n, DX);
    let z0: Vec<f64> = (0..n * n).map(|c| 150.0 + 100.0 * (c % n) as f64).collect();
    let outlet = g.outlets(&z0, 0.0);
    let cap = |_: usize, z: f64| {
        let top = 200.0; // the cap lies at 160..200 m
        if z > top {
            Layer { k: 1.8, floor: top }
        } else if z > top - 40.0 {
            Layer { k: 0.001, floor: top - 40.0 }
        } else {
            Layer { k: 1.8, floor: f64::NEG_INFINITY }
        }
    };
    let uncapped = |_: usize, z: f64| Layer { k: if (160.0..=200.0).contains(&z) { 0.001 } else { 1.8 }, floor: f64::NEG_INFINITY };
    let p =
        CarveParams { steps: 1, k_dt: 5.0, u_over_k: 0.0, keep_pits: false, uplift_taper_m: 0.0, max_area: 0.0, ..CarveParams::default() };
    let run = |k: &(dyn Fn(usize, f64) -> Layer + Sync), steps: usize| {
        let mut z = z0.clone();
        carve_rock(&g, &mut z, &outlet, &CarveParams { steps, ..p }, None, &Rock { par: &1usize, k: Some(k), uplift: None }, 5);
        z
    };
    let (with_edge, without_edge) = (run(&cap, 1), run(&uncapped, 1));
    let (mut held, mut skipped) = (0, 0);
    for c in 0..n * n {
        if outlet[c] || z0[c] < 200.0 {
            continue;
        }
        // No cell ends below the top of the cap by more than the skip after one step with the edge ...
        assert!(with_edge[c] >= 200.0 - 0.011, "cell {c} went from {} to {} past the cap's top", z0[c], with_edge[c]);
        held += usize::from(with_edge[c] < 200.0 + 1e-9);
        // ... while the same carve without the edge lowers some cells straight through the cap.
        skipped += usize::from(without_edge[c] < 160.0);
    }
    assert!(skipped > 0, "the carve without the edge lowered no cell through the cap: the test does not tell the edge from none");
    assert!(held > 0, "no cell stopped at the cap's top");
}
