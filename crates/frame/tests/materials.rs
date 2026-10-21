//! GEN-free presentation data (M7 G2, REND-026 family): the material weights of the ground are one function of a fixed input vector with the
//! response curves as look data; the default agrees with the base albedo about what is rock, sand and grass; the curves ride in the lit plan's
//! text only when they are not the default. Tier A, no GPU.

use planet_frame::look::materials::{Input, MaterialInputs, MaterialParams, INPUTS, MATERIALS};
use planet_frame::look::shading::AlbedoParams;
use planet_frame::look::LitPlan;

fn at(slope_cos: f64, precipitation_mm: f64) -> MaterialInputs {
    let mut i = [0.0; INPUTS];
    i[Input::SlopeCos as usize] = slope_cos;
    i[Input::Precipitation as usize] = precipitation_mm;
    i
}

// spec: LOOK-008
#[test]
fn the_default_curves_make_rock_of_steep_ground_sand_of_flat_dry_ground_and_grass_of_flat_wet_ground_and_the_weights_sum_to_one() {
    let m = MaterialParams::EARTH;
    let (rock, sand, grass, soil) = (0, 1, 2, 3);
    let steep = m.weights(&at(0.4, 100.0));
    assert!(steep[rock] > 0.99, "{steep:?}");
    let dry = m.weights(&at(1.0, 50.0));
    assert!(dry[sand] > 0.99, "{dry:?}");
    let wet = m.weights(&at(1.0, 1500.0));
    assert!(wet[grass] > 0.99, "{wet:?}");
    for slope in [0.0, 0.3, 0.62, 0.7, 0.86, 1.0] {
        for mm in [0.0, 100.0, 250.0, 500.0, 900.0, 3000.0] {
            let w = m.weights(&at(slope, mm));
            assert!(w.iter().all(|v| (0.0..=1.0).contains(v)), "{slope} {mm}: {w:?}");
            assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12, "{slope} {mm}: {w:?} do not sum to 1");
            assert_eq!(w[soil], 0.0, "soil is off in the default");
        }
    }
    // The detail agrees with the colour: the weights switch where the base albedo's own thresholds are.
    let a = AlbedoParams::EARTH;
    assert_eq!([m.curves[0][0][0], m.curves[0][0][1]], [a.rock_cos_none, a.rock_cos_full]);
    assert_eq!([m.curves[1][3][0], m.curves[1][3][1]], [a.wet_mm, a.dry_mm]);
}

// spec: LOOK-008
#[test]
fn the_weights_are_monotone_in_the_inputs_an_input_left_out_changes_nothing_and_a_driver_slot_needs_no_new_interface() {
    let m = MaterialParams::EARTH;
    // Rock grows as the ground steepens; grass grows with the rain.
    let mut last = -1.0;
    for k in 0..=40 {
        let w = m.weights(&at(1.0 - f64::from(k) / 40.0, 500.0))[0];
        assert!(w >= last - 1e-12, "rock does not shrink with the slope: {w} after {last}");
        last = w;
    }
    // The reserved inputs read 0 and are left out by the default curves: setting them changes nothing.
    let base = m.weights(&at(0.9, 600.0));
    let mut with_drivers = at(0.9, 600.0);
    for slot in [Input::SoilDepth, Input::Moisture, Input::Insolation] {
        with_drivers[slot as usize] = 123.0;
    }
    assert_eq!(m.weights(&with_drivers), base);
    // A look that gives soil a curve on a reserved driver makes it a material, with no change of the function's interface.
    let mut curves = m.curves;
    curves[3][Input::SoilDepth as usize] = [0.0, 1.0];
    let with_soil = MaterialParams { curves };
    let mut deep = at(1.0, 500.0);
    deep[Input::SoilDepth as usize] = 1.0;
    let w = with_soil.weights(&deep);
    assert!(w[3] > 0.3 && (w.iter().sum::<f64>() - 1.0).abs() < 1e-12, "{w:?}");
    let mut shallow = at(1.0, 500.0);
    shallow[Input::SoilDepth as usize] = 0.0;
    assert_eq!(with_soil.weights(&shallow)[3], 0.0);
}

// spec: LOOK-008
#[test]
fn no_material_on_a_point_means_no_weights_and_the_table_rounds_trips_through_f32_words_and_bits() {
    let none = MaterialParams { curves: [[[0.0, 0.0]; INPUTS]; MATERIALS] };
    assert_eq!(none.weights(&at(0.5, 500.0)), [0.0; MATERIALS]);
    // A point where every material's response is 0: no weights either (the sum is 0, nothing is divided).
    let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
    curves[0][Input::Height as usize] = [10.0, 20.0];
    let w = MaterialParams { curves }.weights(&at(0.5, 500.0));
    assert_eq!(w, [0.0; MATERIALS]);
    let m = MaterialParams::EARTH;
    assert_eq!(MaterialParams::from_bits(&m.bits()), m);
    let words = m.to_words();
    assert_eq!(words.len(), 4 * planet_frame::look::materials::MATERIAL_VEC4S);
    assert_eq!(m.as_f32().to_words(), words, "rounding twice changes nothing");
    assert!(m.validate().is_ok());
    let mut bad = m;
    bad.curves[1][0][1] = f64::NAN;
    let (key, _, _) = bad.validate().unwrap_err();
    assert_eq!(key, "materials.sand.slope_cos");
}

// spec: LOOK-008
#[test]
fn the_curves_enter_the_lit_plans_text_only_when_they_are_not_the_default_and_round_trip_with_the_other_tokens() {
    let plan = LitPlan::fixed(13.0);
    let text = plan.to_text();
    assert!(!text.contains("materials"), "the default is not written: {text}");
    let mut curves = MaterialParams::EARTH.curves;
    curves[2][Input::Precipitation as usize] = [400.0, 1200.0];
    let changed = LitPlan { materials: MaterialParams { curves }, ..plan.with_water_level(2.5) };
    let text = changed.to_text();
    assert!(text.contains(" materials "), "{text}");
    let tokens: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(tokens[0], "lit");
    let parsed = LitPlan::from_tokens(&tokens[1..]).unwrap();
    assert_eq!(parsed, changed);
    assert_eq!(parsed.to_text(), text);
    assert_eq!(
        LitPlan::from_tokens(&plan.to_text().split_whitespace().skip(1).collect::<Vec<_>>()).unwrap().materials,
        MaterialParams::EARTH
    );
    let mut broken = changed;
    broken.materials.curves[0][0][0] = f64::INFINITY;
    assert!(broken.validate().unwrap_err().contains("materials.rock.slope_cos"));
}

// spec: LOOK-009
#[test]
fn the_strength_of_the_ground_detail_enters_the_plans_text_only_when_set_round_trips_and_is_refused_outside_zero_to_one() {
    let plan = LitPlan::fixed(13.0);
    assert_eq!(plan.ground_detail, 0.0, "off by default");
    assert!(!plan.to_text().contains("detail"), "{}", plan.to_text());
    let set = LitPlan { ground_detail: 0.5, ..plan };
    let text = set.to_text();
    assert!(text.contains(" detail "), "{text}");
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let parsed = LitPlan::from_tokens(&tokens[1..]).unwrap();
    assert_eq!(parsed, set);
    assert_eq!(parsed.to_text(), text);
    // Together with the curves, in the written order.
    let mut curves = MaterialParams::EARTH.curves;
    curves[2][Input::Precipitation as usize] = [400.0, 1200.0];
    let both = LitPlan { materials: MaterialParams { curves }, ..set };
    let tokens: Vec<String> = both.to_text().split_whitespace().map(String::from).collect();
    let refs: Vec<&str> = tokens.iter().map(String::as_str).collect();
    assert_eq!(LitPlan::from_tokens(&refs[1..]).unwrap(), both);
    for bad in [1.5, -0.1, f64::NAN] {
        let err = LitPlan { ground_detail: bad, ..plan }.validate().unwrap_err();
        assert!(err.contains("ground_detail"), "{err}");
    }
}

// spec: LOOK-010
#[test]
fn the_gloss_enters_the_plans_text_only_when_not_the_default_round_trips_is_refused_when_unusable_and_the_roughness_follows_the_weights() {
    use planet_frame::look::materials::GlossParams;
    let plan = LitPlan::fixed(13.0);
    assert!(!plan.to_text().contains("gloss"), "{}", plan.to_text());
    let mut gloss = GlossParams::EARTH;
    gloss.roughness[0] = 0.5;
    gloss.f0 = 0.08;
    let set = LitPlan { gloss, ..plan.with_ground_detail(0.6) };
    let text = set.to_text();
    assert!(text.contains(" gloss ") && text.contains(" detail "), "{text}");
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let parsed = LitPlan::from_tokens(&tokens[1..]).unwrap();
    assert_eq!(parsed, set);
    assert_eq!(parsed.to_text(), text);
    // Refused: a roughness of 0 (a mirror the model cannot draw), a modifier on an input that does not exist, a factor of 0, f0 above 1, a
    // curve that differs below f32 only.
    let mut bad = [GlossParams::EARTH; 6];
    bad[0].roughness[1] = 0.0;
    bad[1].modifiers[0][0] = 9.0;
    bad[2].modifiers[1][3] = 0.0;
    bad[3].f0 = 1.5;
    bad[4].modifiers[1][1] = 5.0;
    bad[4].modifiers[1][2] = 5.0 + 1e-12;
    bad[5].modifiers[1][1] = -1e39;
    bad[5].modifiers[1][2] = 1e39;
    for (b, key) in bad.iter().zip([
        "gloss.roughness.sand",
        "gloss.modifier0.input",
        "gloss.modifier1.factor",
        "gloss.f0",
        "gloss.modifier1.curve",
        "gloss.modifier1.value",
    ]) {
        let err = LitPlan { gloss: *b, ..plan }.validate().unwrap_err();
        assert!(err.contains(key), "{key}: {err}");
    }
    // The roughness of a point: the weights' mean of the materials', then the modifiers; no material, none.
    let (m, g) = (MaterialParams::EARTH, GlossParams::EARTH);
    let mut inputs = at(1.0, 50.0);
    inputs[Input::Temperature as usize] = 20.0;
    let w = m.weights(&inputs);
    assert!((g.roughness_at(&w, &inputs).unwrap() - g.roughness[1]).abs() < 1e-9, "dry flat ground is sand: its roughness");
    inputs[Input::Temperature as usize] = -10.0;
    assert!(
        (g.roughness_at(&w, &inputs).unwrap() - g.roughness[1] * g.modifiers[1][3]).abs() < 1e-9,
        "icy ground is glossier by the factor"
    );
    inputs[Input::Moisture as usize] = 5.0;
    assert!(
        (g.roughness_at(&w, &inputs).unwrap() - g.roughness[1] * g.modifiers[1][3]).abs() < 1e-9,
        "the wet modifier is out in the default"
    );
    let mut wet = g;
    wet.modifiers[0] = [Input::Moisture as usize as f64, 0.0, 1.0, 0.25];
    assert!(
        (wet.roughness_at(&w, &inputs).unwrap() - g.roughness[1] * g.modifiers[1][3] * 0.25).abs() < 1e-9,
        "wet ground is glossier still"
    );
    assert_eq!(g.roughness_at(&[0.0; MATERIALS], &inputs), None);
}
