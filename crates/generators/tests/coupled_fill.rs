//! GEN-065: the coupled run without erosion keeps the relief of its input (heights are the input plus the priority-flood fill).

use planet_core::cube::TangentWarp;
use planet_generators::coupled::{filled_state, priority_flood, run, CoupledInput, CoupledParams, Lattice, NONE};
use planet_generators::coupledinput::reduce_bake;
use planet_generators::global::{bake, GridSource};

const R: f64 = 6_371_000.0;

/// A real reduced bake: the global-coarse field baked at 256 cells per face edge and reduced to a run of 64 (the box filter of GEN-056), so its
/// pits, flats and coasts are those of the generator and not of a smooth test world.
fn real_input() -> (Lattice, CoupledInput) {
    let grid = bake(1, &TangentWarp, 256, 2).unwrap();
    let z = reduce_bake(&GridSource(&grid), 64, 2).unwrap();
    let lat = Lattice::new(64, R);
    let len = z.len();
    (lat, CoupledInput { z, uplift: vec![0.0; len], erodibility: vec![2.0e-5; len] })
}

fn fill_only() -> CoupledParams {
    CoupledParams { erode: false, ..CoupledParams::default() }
}

// spec: GEN-065
#[test]
fn without_erosion_the_run_is_the_input_outside_the_fill_and_the_fill_inside_it_on_a_real_bake() {
    let (lat, input) = real_input();
    let state = run(&lat, &input, &fill_only(), 80);
    let (filled, _) = priority_flood(&lat, &input.z);
    assert_eq!(state.z, filled, "the heights are the filled surface");
    let (mut raised, mut kept, mut land) = (0usize, 0usize, 0usize);
    for c in 0..lat.len() {
        let (z, f) = (input.z[c], state.z[c]);
        land += usize::from(z > 0.0);
        if f == z {
            kept += 1;
        } else {
            assert!(f > z, "cell {c}: the fill only raises ({z} to {f})");
            raised += 1;
        }
    }
    assert!(raised > 0, "a real bake has depressions, the test must exercise the fill");
    assert!(kept > land / 2, "most cells keep their input height bit for bit: {kept} of {} cells, {raised} raised", lat.len());
    // The sea is untouched.
    assert!((0..lat.len()).filter(|&c| input.z[c] <= 0.0).all(|c| state.z[c] == input.z[c]));
    // The relief of the input survives: erosion at the same input moves the heights by hundreds of metres, the fill by the depth of the pits.
    let constant = CoupledInput { z: input.z.clone(), uplift: vec![1.0e-3; input.z.len()], erodibility: input.erodibility.clone() };
    let eroded = run(&lat, &constant, &CoupledParams::default(), 80);
    let median = |z: &[f32]| {
        let mut d: Vec<f64> =
            (0..lat.len()).filter(|&c| input.z[c] > 1000.0).map(|c| (f64::from(z[c]) - f64::from(input.z[c])).abs()).collect();
        d.sort_by(f64::total_cmp);
        d[d.len() / 2]
    };
    let (m_fill, m_erode) = (median(&state.z), median(&eroded.z));
    eprintln!(
        "median |run - input| over land above 1000 m: fill only {m_fill:.2} m, eroding {m_erode:.1} m ({raised} cells raised of {})",
        lat.len()
    );
    assert_eq!(m_fill, 0.0, "the relief of the high ground is kept exactly");
    assert!(m_erode > 100.0, "the eroding run does not keep it ({m_erode} m)");
}

// spec: GEN-065
#[test]
fn the_network_and_the_areas_are_those_of_the_filled_surface_and_the_run_is_deterministic() {
    let (lat, input) = real_input();
    let p = fill_only();
    let state = filled_state(&lat, &input, &p);
    let again = filled_state(&lat, &input, &p);
    assert!(state.z == again.z && state.area_m2 == again.area_m2 && state.receiver == again.receiver, "deterministic");
    let mut outlets_area = 0.0f64;
    let mut land_area = 0.0f64;
    for c in 0..lat.len() {
        let r = state.receiver[c];
        if r != NONE {
            assert!(state.z[r as usize] < state.z[c], "cell {c} drains uphill on the filled surface");
        }
        if state.z[c] > 0.0 {
            land_area += lat.area_m2(c);
            if r == NONE || state.z[r as usize] <= 0.0 {
                outlets_area += f64::from(state.area_m2[c]);
            }
        }
    }
    assert!((outlets_area / land_area - 1.0).abs() < 1.0e-3, "the outlets drain the land: {outlets_area} against {land_area}");
    assert_eq!(state.steps.len(), 1, "one pass");
}
