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
    // ... and with a carve factor of 1 (U is stored as f32: the claim holds for a `u_over_k` that f32 holds exactly, as 20 does).
    let ones = vec![1.0f64; N * N];
    let (mut plain1, mut with1) = (z0.clone(), z0.clone());
    carve(&g, &mut plain1, &outlet, &p, Some(&ones), 5);
    carve_rock(&g, &mut with1, &outlet, &p, Some(&ones), &Rock { par: &2usize, k: Some(&one), uplift: Some(&u) }, 5);
    assert_eq!(bits(&plain1), bits(&with1), "with a factor of 1 the carves differ");
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
        assert!(with_edge[c] >= 200.0 - 0.015, "cell {c} went from {} to {} past the cap's top", z0[c], with_edge[c]);
        held += usize::from(with_edge[c] < 200.0 + 1e-9);
        // ... while the same carve without the edge lowers some cells straight through the cap.
        skipped += usize::from(without_edge[c] < 160.0);
    }
    assert!(skipped > 0, "the carve without the edge lowered no cell through the cap: the test does not tell the edge from none");
    assert!(held > 0, "no cell stopped at the cap's top");
}

// spec: GEN-127
#[test]
fn the_k_scale_solver_matches_the_incision_of_a_unit_rock_and_the_constant_lies_among_the_default_lithologys_seeds() {
    use planet_generators::lithology::{k_scale_for, ROCK_K_SCALE};
    let g = |f: f64| f / (1.0 + f);
    // A rock of constant K = 2 over a spread of f: the c that removes as much as K = 1 does is the one with mean g(2 c f) = mean g(f), and applying it does.
    let fs: Vec<f64> = (0..400).map(|i| 0.02 * libm::pow(1.03, f64::from(i))).collect();
    let pairs: Vec<(f64, f64)> = fs.iter().map(|f| (2.0, *f)).collect();
    let c = k_scale_for(&pairs);
    let (want, got) = (fs.iter().map(|f| g(*f)).sum::<f64>(), fs.iter().map(|f| g(2.0 * c * f)).sum::<f64>());
    assert!((want - got).abs() < 1e-9 * want, "mean incision {got} against {want} at c = {c}");
    assert!(c > 0.3 && c < 0.5, "a rock twice as erodible needs a factor under 1/2 but not far under, not {c}");
    assert_eq!(k_scale_for(&[]), 1.0);
    // The constant is derived over seeds (`k_calib`): seed 3's own value is 1.04 and the constant lies between the seeds' extremes (0.88 to 1.78).
    let inputs = planet_generators::bake::inputs::BakeInputs::new(3, 8);
    let n = 60_000usize;
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    let mut pairs = Vec::new();
    for i in 0..n {
        let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
        let r = libm::sqrt(1.0 - y * y);
        let (s, c) = libm::sincos(golden * i as f64);
        let d = Vec3::new(r * c, y, r * s);
        let z = inputs.base(d);
        if z > 0.0 {
            let u = (i as f64 * 0.618_033_988_749_895).fract();
            pairs.push((inputs.lithology().k_at(d, z), 0.05 * libm::sqrt(libm::pow(1.0 - u, -1.0 / 0.43).min(1e7))));
        }
    }
    let c3 = k_scale_for(&pairs);
    assert!((c3 - 1.04).abs() < 0.08, "seed 3's factor is {c3}, k_calib gave 1.04");
    assert!((0.88..=1.78).contains(&ROCK_K_SCALE), "the constant {ROCK_K_SCALE} lies outside the seeds' range");
    assert_eq!(inputs.rock_k(Vec3::new(0.0, 0.0, 1.0), 300.0).k, inputs.lithology().k_at(Vec3::new(0.0, 0.0, 1.0), 300.0) * ROCK_K_SCALE);
}

// spec: GEN-127
#[test]
fn a_caprock_holds_a_cell_for_the_following_steps_and_the_edge_path_is_the_same_bits_on_any_thread_count() {
    let n = 24;
    let g = Grid::new(n, n, DX);
    let z0: Vec<f64> = (0..n * n).map(|c| 150.0 + 100.0 * (c % n) as f64).collect();
    let outlet = g.outlets(&z0, 0.0);
    let cap = |_: usize, z: f64| {
        if z > 200.0 {
            Layer { k: 1.8, floor: 200.0 }
        } else if z > 160.0 {
            Layer { k: 0.001, floor: 160.0 }
        } else {
            Layer { k: 1.8, floor: f64::NEG_INFINITY }
        }
    };
    let p = CarveParams { k_dt: 5.0, u_over_k: 0.0, keep_pits: false, uplift_taper_m: 0.0, max_area: 0.0, ..CarveParams::default() };
    let run = |steps: usize, par: &dyn Par| {
        let mut z = z0.clone();
        carve_rock(&g, &mut z, &outlet, &CarveParams { steps, ..p }, None, &Rock { par, k: Some(&cap), uplift: None }, 5);
        z
    };
    let (one, three) = (run(1, &1usize), run(3, &1usize));
    let cell = n + 1; // the first cell above the cap: it starts at 250 m beside a receiver at 150 m
    assert!(one[cell] >= 199.98 && one[cell] <= 200.0, "step 1 ends at {}, not at the cap's top", one[cell]);
    assert!(
        three[cell] <= one[cell] && three[cell] > one[cell] - 5.0,
        "the next steps read the cap (K 0.001): {} after step 1, {} after 3",
        one[cell],
        three[cell]
    );
    assert!(three[cell] > 160.0, "the cell passed the cap's bottom in 3 steps: {}", three[cell]);
    assert_eq!(bits(&run(3, &1usize)), bits(&run(3, &8usize)), "the carve with layer edges differs between 1 and 8 threads");
}

// spec: GEN-127
#[test]
fn the_lower_edge_of_the_rock_band_is_where_the_band_below_begins_and_the_key_names_the_versions_and_the_k_scale() {
    use planet_generators::bake::inputs::BakeInputs;
    use planet_generators::lithology::{LITHOLOGY_VERSION, ROCK_K_SCALE};
    use planet_generators::uplift::UPLIFT_VERSION;
    let inputs = BakeInputs::new(3, 8);
    let li = inputs.lithology();
    let mut checked = 0;
    for i in 0..200 {
        let a = 0.37 * f64::from(i);
        let d = Vec3::new(libm::cos(a) * 0.8, libm::sin(a * 1.3) * 0.5, libm::sin(a) * 0.8);
        let d = d * (1.0 / libm::sqrt(d.x * d.x + d.y * d.y + d.z * d.z));
        let z = 50.0 + 13.0 * f64::from(i);
        let s = li.strata(d, z);
        assert!(s.lower_m <= z + 1e-9, "the lower edge {} lies above the height {z}", s.lower_m);
        assert_eq!(li.strata(d, s.lower_m + 1e-6).band, s.band, "just above the edge the band is another");
        assert_eq!(li.strata(d, s.lower_m - 1e-6).band, s.band - 1, "just below the edge the band is not the one below");
        checked += 1;
    }
    assert_eq!(checked, 200);
    let id = BakeInputs::id();
    assert_eq!(id, BakeInputs::id_with(LITHOLOGY_VERSION, UPLIFT_VERSION, ROCK_K_SCALE));
    assert_ne!(id, BakeInputs::id_with(LITHOLOGY_VERSION + 1, UPLIFT_VERSION, ROCK_K_SCALE), "the lithology version is not in the id");
    assert_ne!(id, BakeInputs::id_with(LITHOLOGY_VERSION, UPLIFT_VERSION + 1, ROCK_K_SCALE), "the uplift version is not in the id");
    assert_ne!(id, BakeInputs::id_with(LITHOLOGY_VERSION, UPLIFT_VERSION, ROCK_K_SCALE + 0.01), "the K scale is not in the id");
}

// spec: GEN-127
#[test]
fn a_layer_edge_above_the_height_never_raises_a_cell_and_a_nan_edge_or_an_unsolvable_k_scale_is_refused() {
    use planet_generators::lithology::k_scale_for;
    let (g, z0, outlet) = setup();
    let p = CarveParams { steps: 3, keep_pits: false, uplift_taper_m: 0.0, u_over_k: 0.0, ..CarveParams::default() };
    let above = |_: usize, z: f64| Layer { k: 1.0, floor: z + 5.0 };
    let mut z = z0.clone();
    carve_rock(&g, &mut z, &outlet, &p, None, &Rock { par: &1usize, k: Some(&above), uplift: None }, 5);
    assert!((0..N * N).all(|c| z[c] <= z0[c] + 1e-9), "a cell was raised above its start by an edge above its height");
    let refused = std::panic::catch_unwind(|| {
        let (g, z0, outlet) = setup();
        let nan = |_: usize, _: f64| Layer { k: 1.0, floor: f64::NAN };
        let mut z = z0;
        carve_rock(&g, &mut z, &outlet, &CarveParams::default(), None, &Rock { par: &1usize, k: Some(&nan), uplift: None }, 1);
    })
    .is_err();
    assert!(refused, "a NaN layer edge was accepted");
    assert!(std::panic::catch_unwind(|| k_scale_for(&[(0.0, 1.0)])).is_err(), "an unsolvable K scale returned a number");
}
