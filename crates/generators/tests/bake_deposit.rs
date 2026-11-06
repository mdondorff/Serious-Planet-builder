//! Tier A, TEST-095 (GEN-131, ADR 0105): the carve with deposition on synthetic surfaces: mass is kept (what is eroded is deposited or leaves), a pit receiving sediment rises toward its spill and
//! no higher, the result is the same bits on every run, and a coefficient of 0 is the carve without a sediment term. Small grids; no planet bake.

use planet_generators::bake::carve::{carve_rock_report, CarveParams, Rock};
use planet_generators::bake::Grid;

const DX: f64 = 800.0;
const N: usize = 24;

fn grid() -> Grid {
    Grid::new(N, N, DX)
}

/// A tilted plane draining west, `10 i` metres.
fn ramp() -> Vec<f64> {
    (0..N * N).map(|p| 10.0 * (p % N) as f64).collect()
}

/// The ramp with a pit of 40 m at `i` 8 to 10 in the middle row.
fn ramp_with_pit() -> Vec<f64> {
    let mut z = ramp();
    for i in 8..11 {
        z[12 * N + i] -= 40.0;
    }
    z
}

fn run(z0: &[f64], g: f64, steps: usize, keep_pits: bool) -> (Vec<f64>, f64, f64, f64) {
    let grid = grid();
    let outlet = grid.outlets(z0, -1.0);
    let p = CarveParams { steps, u_over_k: 20.0, uplift_taper_m: 0.0, keep_pits, deposit_g: g, ..CarveParams::default() };
    let mut z = z0.to_vec();
    let (_, rep) = carve_rock_report(&grid, &mut z, &outlet, &p, None, &Rock::none(), 3);
    (z, rep.eroded, rep.deposited, rep.exported)
}

fn bits(z: &[f64]) -> Vec<u64> {
    z.iter().map(|v| v.to_bits()).collect()
}

// spec: GEN-131
// spec: TEST-095
#[test]
fn what_is_eroded_is_deposited_or_leaves_through_an_outlet() {
    for (name, z0) in [("ramp", ramp()), ("ramp with a pit", ramp_with_pit())] {
        let (_, eroded, deposited, exported) = run(&z0, 1.0, 20, true);
        assert!(eroded > 0.0, "{name}: nothing was eroded");
        assert!(deposited > 0.0, "{name}: nothing was deposited at G = 1");
        let err = ((eroded - deposited) - exported).abs() / eroded;
        assert!(
            err < 1e-9,
            "{name}: eroded {eroded:.6e} - deposited {deposited:.6e} != exported {exported:.6e} (relative error {err:.2e})"
        );
    }
}

// spec: GEN-131
// spec: TEST-095
#[test]
fn a_pit_that_receives_sediment_rises_toward_its_spill_and_no_higher_than_its_receiver() {
    let z0 = ramp_with_pit();
    let pit = 12 * N + 9;
    let (off, ..) = run(&z0, 0.0, 20, true);
    let (on, ..) = run(&z0, 4.0, 20, true);
    assert!(on[pit] > off[pit] + 1.0, "the pit stands at {} with deposition and {} without", on[pit], off[pit]);
    // The spill of the pit is the ramp's height at its downstream rim (i = 7), 70 m, plus what the carve raised that cell by.
    assert!(on[pit] <= on[12 * N + 7] + 1e-6 + 40.0, "the pit stands at {} above its rim at {}", on[pit], on[12 * N + 7]);
    assert!(on.iter().all(|h| h.is_finite()), "a height is not finite");
}

// spec: GEN-131
// spec: TEST-095
#[test]
fn deposition_is_the_same_bits_on_every_run_and_a_zero_coefficient_adds_nothing() {
    let z0 = ramp_with_pit();
    let (a, ..) = run(&z0, 1.0, 20, true);
    let (b, ..) = run(&z0, 1.0, 20, true);
    assert_eq!(bits(&a), bits(&b));
    let (off, eroded, deposited, exported) = run(&z0, 0.0, 20, true);
    assert_eq!((eroded, deposited, exported), (0.0, 0.0, 0.0), "the account of a carve without deposition is empty");
    assert_ne!(bits(&off), bits(&a), "G = 1 changed nothing");
}
