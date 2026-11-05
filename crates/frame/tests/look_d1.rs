//! Tier A, LOOK-008 and LOOK-011 (slice 2b PR 3): the sample ground look on the baked channels (`docs/looks/ground-channels-d1.look.toml`, SPIKE-100 D1) parses, and its curves give the ground
//! materials the owner asked for at characteristic points: rock on a bare steep flank, scree where it carries deposits, wet ground in a hollow with a large drainage area, soil and grass on the
//! gentle ground between. A proposal for the owner's look, never the default.

use planet_frame::look::file::parse_look;
use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIAL_NAMES};
use planet_world_def::BodyId;

const LOOK: &str = include_str!("../../../docs/looks/ground-channels-d1.look.toml");

/// The weights at a ground point: the slope's cosine, the deposited depth in metres, the moisture (the drainage area's ramp) and the annual precipitation in mm.
fn weights(m: &MaterialParams, slope_cos: f64, soil_m: f64, moisture: f64, precipitation_mm: f64) -> Vec<(String, f64)> {
    let mut i = [0.0; INPUTS];
    i[Input::SlopeCos as usize] = slope_cos;
    i[Input::SoilDepth as usize] = soil_m;
    i[Input::Moisture as usize] = moisture;
    i[Input::Precipitation as usize] = precipitation_mm;
    MATERIAL_NAMES.iter().map(|n| n.to_string()).zip(m.weights(&i)).collect()
}

fn of(w: &[(String, f64)], name: &str) -> f64 {
    w.iter().find(|(n, _)| n == name).expect("a material").1
}

// spec: LOOK-008
#[test]
fn the_sample_look_gives_rock_to_a_bare_steep_flank_scree_to_one_with_deposits_wet_ground_to_a_hollow_and_soil_and_grass_between() {
    let file = parse_look(LOOK).unwrap();
    let ground = file.body(BodyId(0)).ground;
    let m = ground.materials;
    assert_eq!((ground.detail, ground.patch), (0.6, 0.12));
    // The Earth default has scree and wet ground off; the sample turns them on and puts curves on the two driver inputs.
    assert!(!MaterialParams::EARTH.is_on(4) && !MaterialParams::EARTH.is_on(5));
    assert!(
        m.is_on(4)
            && m.is_on(5)
            && m.curves[4][Input::SoilDepth as usize] != [0.0, 0.0]
            && m.curves[5][Input::Moisture as usize] != [0.0, 0.0]
    );
    // A bare steep flank (45 degrees, no deposits, a small drainage area) is rock.
    let w = weights(&m, 0.70, 0.0, 0.05, 1200.0);
    assert!(of(&w, "rock") > 0.95, "{w:?}");
    // Steep ground with deposits (38 degrees, 1.5 m) is rock and scree, the scree the larger share.
    let w = weights(&m, 0.78, 1.5, 0.05, 1200.0);
    assert!(of(&w, "scree") > 0.4 && of(&w, "rock") > 0.3, "{w:?}");
    // Moderate ground with deposits is soil and grass with a little scree.
    let w = weights(&m, 0.90, 1.0, 0.05, 1200.0);
    assert!(of(&w, "soil") + of(&w, "grass") > 0.8 && of(&w, "scree") < 0.2 && of(&w, "rock") < 0.01, "{w:?}");
    // A hollow (under 5 degrees, a large drainage area) is wet ground.
    let w = weights(&m, 0.998, 1.5, 0.7, 1200.0);
    assert!(of(&w, "wet") > 0.8, "{w:?}");
    // Gentle ground with a small area, in a wet climate, is grass and soil; in a desert it is sand and soil (the Earth curves of sand and grass stay).
    let w = weights(&m, 0.95, 0.0, 0.05, 1200.0);
    assert!(of(&w, "grass") > 0.3 && of(&w, "soil") > 0.3, "{w:?}");
    let w = weights(&m, 0.95, 0.0, 0.05, 150.0);
    assert!(of(&w, "sand") > 0.4, "{w:?}");
    // A tile without channels reads the neutral drivers (soil 1 m, moisture 0.5): the ground keeps a material and so its detail, and no wet ground or scree appears from nothing.
    let w = weights(&m, 0.95, 1.0, 0.5, 1200.0);
    assert!((w.iter().map(|(_, v)| v).sum::<f64>() - 1.0).abs() < 1e-9 && of(&w, "wet") == 0.0 && of(&w, "scree") < 0.2, "{w:?}");
}

// spec: LOOK-011
#[test]
fn the_sample_look_changes_nothing_but_the_ground_table_and_is_not_the_default() {
    use planet_frame::look::file::LookParams;
    let file = parse_look(LOOK).unwrap();
    let p = file.body(BodyId(0));
    let default = LookParams::default();
    assert_eq!((p.atmosphere, p.water, p.sun, p.albedo), (default.atmosphere, default.water, default.sun, default.albedo));
    assert_ne!(p.ground.materials, default.ground.materials);
    // A body the file does not name has the Earth default, and the look is a look file only: the default ground look is untouched.
    assert_eq!(file.body(BodyId(7)), default);
}
