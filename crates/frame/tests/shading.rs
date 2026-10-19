//! Tier A: the base shading's reference (ADR 0039 points 1, 3 and 4; REND-025, LOOK-007): the albedo from the climate, slope and
//! height above the water, Lambert diffuse, the up vector from tile-local numbers, the sun from a time of day, the plan text of the
//! shading tokens and the look tables that feed them. GPU-free (CON-03). Class: presentation data (ADR 0035).

use planet_frame::look::file::*;
use planet_frame::look::shading::*;
use planet_frame::look::{self, LitPlan};
use planet_world_def::BodyId;

const E: AlbedoParams = AlbedoParams::EARTH;

fn close3(a: [f64; 3], b: [f64; 3], tol: f64, what: &str) {
    for c in 0..3 {
        assert!((a[c] - b[c]).abs() <= tol, "{what}: channel {c}: {a:?} against {b:?}");
    }
}

// spec: REND-030
#[test]
fn the_ramp_is_a_function_of_the_fragments_distance_and_the_per_level_rule_it_replaces_gave_a_factor_of_five_between_adjacent_levels() {
    use planet_frame::lod::{shore_ramp_at, LodParams, SHORE_RAMP_K, SHORE_RAMP_MAX_M};
    let lod = LodParams::earth_1080p();
    let per_m = lod.shore_ramp_per_m();
    // The old rule, kept here only to show what the test discriminates: the ramp of the drawn node's level, `min(K s(level - 1), 8)`.
    let by_level = |level: u8| (SHORE_RAMP_K * lod.sample_spacing(level - 1)).min(SHORE_RAMP_MAX_M);
    let luminance = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let at = |ramp: f64, h: f64| luminance(E.albedo_with_ramp(20.0, 50.0, 1.0, h, ramp));
    // The ramp takes the plan's growth and the fragment's distance and nothing else (`shore_ramp_at(per_m, d)` has no level argument; the
    // plan carries one growth for all nodes and the node record no ramp: `node_record_bytes`, a test in the render crate), so a seabed point
    // 2.5 m under the water has the albedo its distance gives, whichever tile draws it.
    // The failure this replaces (SPIKE-16): under the old rule the same point is 4.9 times brighter under L17 than under L19.
    let (old17, old19) = (at(by_level(17), -2.5), at(by_level(19), -2.5));
    assert!(old17 / old19 > 4.0, "the per-level ramp gave {old17} under L17 and {old19} under L19");
    // Adjacent distances give adjacent ramps: no step anywhere along the ground.
    let mut last = shore_ramp_at(per_m, 0.0);
    for k in 1..=2000 {
        let r = shore_ramp_at(per_m, f64::from(k) * 5.0);
        assert!(r >= last && r - last <= per_m * 5.0 + 1e-12, "distance {}: {r} after {last}", f64::from(k) * 5.0);
        last = r;
    }
}

// spec: REND-030
#[test]
fn the_shore_ramp_is_the_looks_blend_where_the_mesh_is_fine_and_follows_the_tolerance_at_the_fragments_distance() {
    use planet_frame::lod::{shore_ramp_at, LodParams, SHORE_RAMP_K, SHORE_RAMP_MAX_M};
    let lod = LodParams::earth_1080p();
    // The width: the blend where the tolerance is small, the tolerance where it is larger, monotone, never below the blend.
    assert_eq!(E.shore_ramp_m(0.0), E.shore_blend_m);
    assert_eq!(E.shore_ramp_m(0.3), E.shore_blend_m);
    assert_eq!(E.shore_ramp_m(7.5), 7.5);
    assert_eq!(E.shore_ramp_m(f64::NAN), E.shore_blend_m, "an unreadable tolerance does not narrow or widen the ramp");
    let mut last = 0.0;
    for x in [0.0, 0.5, 1.0, 1.5, 2.0, 5.0, 40.0] {
        let w = E.shore_ramp_m(x);
        assert!(w >= last && w >= E.shore_blend_m, "{x}: {w}");
        last = w;
    }
    // The growth per metre: `K * tau_px * pixel_angle`, and tau pixels at the far end of a node's drawing range are the parent's spacing, so at
    // that distance the ramp is `K s(level - 1)` up to the cap: the former per-node rule's value at morph 0 (it rose to twice that across the
    // morph, so the new rule is not equal to the old one pointwise; the coastal flights are the evidence, REND-030).
    let pixel_angle = 2.0 * libm::tan(lod.fov_y_rad / 2.0) / lod.viewport_h_px;
    assert!((lod.shore_ramp_per_m() - SHORE_RAMP_K * lod.tau_px * pixel_angle).abs() <= 1e-15 * lod.shore_ramp_per_m());
    for level in 1..=22u8 {
        let (_, end) = lod.morph_interval(level);
        let want = (SHORE_RAMP_K * lod.sample_spacing(level - 1)).min(SHORE_RAMP_MAX_M);
        let got = shore_ramp_at(lod.shore_ramp_per_m(), end);
        assert!((got - want).abs() <= 1e-9 * want, "level {level}: {got} against {want}");
    }
    // The cap and the nearest ground: the coarse view of an orbit asks for kilometres and gets 8 m; at the camera the ramp is nothing.
    assert_eq!(shore_ramp_at(lod.shore_ramp_per_m(), 1.0e7), SHORE_RAMP_MAX_M);
    assert_eq!(shore_ramp_at(lod.shore_ramp_per_m(), 0.0), 0.0);
    assert!(shore_ramp_at(lod.shore_ramp_per_m(), 40.0) < E.shore_blend_m, "near the camera the ramp stays the look's 1 m");
    assert!(shore_ramp_at(lod.shore_ramp_per_m(), 2_000.0) > E.shore_blend_m, "far enough, the tolerance widens it");
    // A growth that is not finite and positive is refused by the parameters' check.
    for bad in [LodParams { viewport_h_px: 0.0, ..lod.clone() }, LodParams { tau_px: 0.0, ..lod.clone() }] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    // It does not depend on the node's level, and follows tau, the viewport and the fov through the pixel angle only.
    let other = LodParams { tau_px: 3.0, viewport_h_px: 1440.0, fov_y_rad: 1.0, ..lod.clone() };
    let other_pixel = 2.0 * libm::tan(other.fov_y_rad / 2.0) / other.viewport_h_px;
    assert!((other.shore_ramp_per_m() - SHORE_RAMP_K * other.tau_px * other_pixel).abs() <= 1e-15 * other.shore_ramp_per_m());
    // The reference albedo: with no least height it is the albedo; a wider ramp spreads the transition, not the ends.
    for h in [-20.0, -1.0, -0.4, 0.0, 2.0] {
        assert_eq!(E.albedo_with_ramp(20.0, 50.0, 1.0, h, 0.0), E.albedo(20.0, 50.0, 1.0, h), "h {h}");
    }
    assert_eq!(E.albedo_with_ramp(20.0, 50.0, 1.0, -9.0, 8.0), E.seabed, "below the ramp: the seabed");
    assert_eq!(E.albedo_with_ramp(20.0, 50.0, 1.0, 0.0, 8.0), E.beach, "at the water level: the beach");
    let mid = E.albedo_with_ramp(20.0, 50.0, 1.0, -4.0, 8.0);
    let narrow = E.albedo(20.0, 50.0, 1.0, -4.0);
    assert!(narrow == E.seabed && mid != E.seabed && mid[0] > E.seabed[0] && mid[0] < E.beach[0], "{mid:?} against {narrow:?}");
}

// spec: REND-025
#[test]
fn the_albedo_is_the_table_colour_where_one_surface_alone_applies() {
    // Weights of exactly 0 and 1: flat, warm, dry, high above the water: desert.
    assert_eq!(E.albedo(25.0, 50.0, 1.0, 100.0), E.desert);
    // Humid but under the forest line: grass; wetter: forest.
    assert_eq!(E.albedo(15.0, 950.0, 1.0, 100.0), E.grass);
    assert_eq!(E.albedo(15.0, 2500.0, 1.0, 100.0), E.forest);
    // Steeper than the full-rock slope: rock (warm, so no snow).
    assert_eq!(E.albedo(20.0, 800.0, 0.3, 100.0), E.rock);
    // Cold and flat: snow; cold but steep: no snow sticks and the rock shows.
    assert_eq!(E.albedo(-20.0, 800.0, 1.0, 100.0), E.snow);
    assert_eq!(E.albedo(-20.0, 800.0, 0.3, 100.0), E.rock);
    // A shore: beach just above the water, the seabed below it, measured from the water level given (no hard-coded sea level).
    assert_eq!(E.albedo(20.0, 50.0, 1.0, 1.0), E.beach);
    assert_eq!(E.albedo(20.0, 50.0, 1.0, -5.0), E.seabed);
    assert_eq!(E.albedo(20.0, 50.0, 1.0, 1.0e6), E.desert, "far above the water the beach is gone");
    // The same point against another water level: a lake 1,500 m up has its beach at 1,501 m.
    let h = 1_501.0;
    assert_eq!(E.albedo(20.0, 50.0, 1.0, h - 1_500.0), E.beach);
    assert_eq!(E.albedo(20.0, 50.0, 1.0, h - 0.0), E.desert);
}

// spec: REND-025
#[test]
fn halfway_through_a_transition_the_colours_are_halfway() {
    let mid = |a: f64, b: f64| 0.5 * (a + b);
    // Precipitation between dry and wet: half desert, half grass.
    let p = mid(E.dry_mm, E.wet_mm);
    let got = E.albedo(25.0, p, 1.0, 100.0);
    close3(got, [0, 1, 2].map(|c| 0.5 * (E.desert[c] + E.grass[c])), 1e-12, "desert to grass");
    // Temperature between the snow limits (no vegetation difference: desert ground, warm tundra weight 1 above +5).
    // At T = -4.5 the ground is tundra-to-desert; use the snow weight alone: compare against the same point without snow.
    let t = mid(E.snow_c_full, E.snow_c_none);
    let snowy = E.albedo(t, 50.0, 1.0, 100.0);
    let bare = AlbedoParams { snow: E.tundra, ..E }.albedo(t, 50.0, 1.0, 100.0);
    close3(snowy, [0, 1, 2].map(|c| 0.5 * (E.snow[c] + bare[c])), 1e-12, "half snow");
    // Slope: halfway between full rock and none.
    let c = mid(E.rock_cos_full, E.rock_cos_none);
    let rocky = E.albedo(25.0, 50.0, c, 100.0);
    close3(rocky, [0, 1, 2].map(|k| 0.5 * (E.rock[k] + E.desert[k])), 1e-12, "half rock");
    // Every channel stays an albedo: in [0, 1] over a sweep of the whole input space.
    let mut worst: f64 = 0.0;
    for ti in 0..40 {
        for pi in 0..30 {
            for ci in 0..11 {
                for hi in 0..14 {
                    let a = E.albedo(-40.0 + 2.0 * f64::from(ti), 150.0 * f64::from(pi), f64::from(ci) / 10.0, -20.0 + 3.0 * f64::from(hi));
                    for v in a {
                        assert!((0.0..=1.0).contains(&v), "{a:?}");
                        worst = worst.max(v);
                    }
                }
            }
        }
    }
    assert!(worst > 0.8, "snow reaches its albedo somewhere in the sweep");
}

// spec: REND-025
#[test]
fn lambert_radiance_has_known_answers() {
    let pi = core::f64::consts::PI;
    let (sun, up) = ([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
    // Facing the sun: albedo / pi (one unit of illuminance).
    let r = terrain_radiance(&E, ShadeView::Lit, [0.0, 0.0, 1.0], up, 25.0, 50.0, 100.0, sun);
    close3(r, E.desert.map(|c| c / pi), 1e-15, "facing the sun");
    // Facing away: no light and no ambient (the atmosphere adds it, ADR 0040).
    assert_eq!(terrain_radiance(&E, ShadeView::Lit, [0.0, 0.0, -1.0], up, 25.0, 50.0, 100.0, sun), [0.0; 3]);
    // The normal is normalised first; a 60 degree incidence halves the light (the slope of 60 degrees is rock, so use the
    // lighting view, which is albedo independent).
    let n60 = [libm::sin(core::f64::consts::FRAC_PI_3) * 7.0, 0.0, libm::cos(core::f64::consts::FRAC_PI_3) * 7.0];
    let l = terrain_radiance(&E, ShadeView::Lighting, n60, up, 25.0, 50.0, 100.0, sun);
    close3(l, [0.5 / pi; 3], 1e-12, "cos 60 degrees");
    // The albedo view is the albedo over pi whatever the light.
    let a = terrain_radiance(&E, ShadeView::Albedo, [0.0, 0.0, 1.0], up, 25.0, 50.0, 100.0, [0.0, 0.0, -1.0]);
    close3(a, E.desert.map(|c| c / pi), 1e-12, "albedo view");
    // A surface tilted 60 degrees from up has cos 0.5 < the full-rock slope: rock.
    let tilted = terrain_radiance(&E, ShadeView::Albedo, n60, up, 25.0, 50.0, 100.0, sun);
    close3(tilted, E.rock.map(|c| c / pi), 1e-12, "steep ground is rock");
    // The climate views are false colours of the channel, independent of the light and the slope.
    let t = terrain_radiance(&E, ShadeView::Temperature, n60, up, 40.0, 50.0, 100.0, [0.0, 0.0, -1.0]);
    close3(t, climate_view_colour(ShadeView::Temperature, 40.0, 0.0).map(|c| c / pi), 1e-12, "temperature view");
    assert_ne!(climate_view_colour(ShadeView::Temperature, -30.0, 0.0), climate_view_colour(ShadeView::Temperature, 40.0, 0.0));
    assert_ne!(climate_view_colour(ShadeView::Precipitation, 0.0, 0.0), climate_view_colour(ShadeView::Precipitation, 0.0, 3000.0));
}

// spec: REND-027, CON-11
#[test]
fn up_is_the_radial_direction_formed_from_tile_local_numbers() {
    // A tile at the finest level far from the origin: R = 6.4e6 m, up0 an arbitrary unit vector, local offsets of a few metres.
    let r = 6_371_000.0 + 1_234.5;
    let up0 = {
        let v = [0.31, -0.62, 0.72];
        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        v.map(|c| c / l)
    };
    let origin = up0.map(|c| c * r);
    for local in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, -2.5, 3.0], [100.0, 40.0, -20.0]] {
        let exact = {
            let p = [origin[0] + local[0], origin[1] + local[1], origin[2] + local[2]];
            let l = libm::sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
            p.map(|c| c / l)
        };
        close3(up_at(up0, 1.0 / r, local), exact, 1e-15, "exact in f64");
        // In f32 (the shader's arithmetic) the up vector is good to a few 1e-7 rad: far under the slope features it shades.
        let f = |v: [f64; 3]| v.map(|c| c as f32);
        let (u, l) = (f(up0), f(local));
        let inv = (1.0 / r) as f32;
        let v = [u[0] + l[0] * inv, u[1] + l[1] * inv, u[2] + l[2] * inv];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let g = v.map(|c| f64::from(c / len));
        close3(g, exact, 3.0e-7, "f32 tile-local form");
    }
}

// spec: REND-024
#[test]
fn the_sun_comes_from_a_time_of_day() {
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|c| (a[c] - b[c]).abs() < 1e-6);
    // COORD's axes (+z north pole, +x at longitude 0, +y at 90 degrees east). Noon at declination 0: over longitude 0, towards +x.
    // Six hours later the sun is over longitude 90 degrees west: -y; at midnight over the antimeridian.
    assert!(near(sun_direction(12.0, 0.0), [1.0, 0.0, 0.0]), "{:?}", sun_direction(12.0, 0.0));
    assert!(near(sun_direction(18.0, 0.0), [0.0, -1.0, 0.0]), "{:?}", sun_direction(18.0, 0.0));
    assert!(near(sun_direction(6.0, 0.0), [0.0, 1.0, 0.0]));
    assert!(near(sun_direction(0.0, 0.0), [-1.0, 0.0, 0.0]));
    // The declination tilts it towards the north pole (+z), and the result is a unit vector.
    assert!(near(sun_direction(12.0, 90.0), [0.0, 0.0, 1.0]));
    for (h, d) in [(3.3, 23.44), (21.0, -23.44), (12.0, 45.0), (7.77, -80.0)] {
        let s = sun_direction(h, d).map(f64::from);
        assert!((libm::sqrt(s[0] * s[0] + s[1] * s[1] + s[2] * s[2]) - 1.0).abs() < 1e-6);
    }
    // 24 hours is one turn: the same direction (within f32 rounding of the angle).
    assert!(near(sun_direction(5.5, 10.0), sun_direction(29.5, 10.0)));
}

// spec: REND-024, LOOK-003
#[test]
fn the_shading_tokens_enter_the_plan_text_only_when_they_differ_from_the_default() {
    let plan = LitPlan::fixed(14.0);
    let text = plan.to_text();
    assert!(!text.contains("albedo") && !text.contains("water") && !text.contains("view"), "{text}");
    // A line from before the base shading (with the sun, no shading tokens) reads as the default shading.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let back = LitPlan::from_tokens(&tokens[1..]).unwrap();
    assert_eq!((back.albedo, back.water_level_m, back.view), (AlbedoParams::EARTH, 0.0, ShadeView::Lit));
    // Each token changes the text and the hash and round-trips, alone and together with the look's units, in the written order.
    let mut custom = AlbedoParams { snow: [0.7, 0.7, 0.7], beach_fade_m: 0.25, ..AlbedoParams::EARTH };
    custom.rock = [0.1, 0.2, 0.3];
    let mut variants = Vec::new();
    for (p, token) in [
        (
            {
                let mut p = LitPlan::fixed(14.0);
                p.albedo = custom;
                p
            },
            " albedo ",
        ),
        (
            {
                let mut p = LitPlan::fixed(14.0);
                p.water_level_m = 1_500.5;
                p
            },
            " water ",
        ),
        (
            {
                let mut p = LitPlan::fixed(14.0);
                p.view = ShadeView::Temperature;
                p
            },
            " view temperature",
        ),
        (
            {
                let mut p = LitPlan::adaptive(11.0, 0.02);
                p.units.solar_illuminance_lux = 50_000.0;
                p.albedo = custom;
                p.water_level_m = -3.0;
                p.view = ShadeView::Albedo;
                p
            },
            " albedo ",
        ),
    ] {
        let t = p.to_text();
        assert!(t.contains(token), "{t}");
        assert_ne!(p.hash(), plan.hash());
        let tk: Vec<&str> = t.split_whitespace().collect();
        assert_eq!(LitPlan::from_tokens(&tk[1..]).unwrap(), p, "{t}");
        assert!(p.validate().is_ok());
        variants.push(p.hash());
    }
    variants.sort();
    variants.dedup();
    assert_eq!(variants.len(), 4, "the four variants hash differently");
    // Out of order, repeated or truncated tokens are refused.
    let base = "fixed 41600000 dt 00000000 sun 3f0066e4 3f19a04d 3f1f1ab2 reset 0 tonemap aces";
    for bad in [
        format!("{base} view albedo water 00000000"),
        format!("{base} water 00000000 water 00000000"),
        format!("{base} view"),
        format!("{base} view nothing"),
        format!("{base} albedo 00"),
    ] {
        let tk: Vec<&str> = bad.split_whitespace().collect();
        assert!(LitPlan::from_tokens(&tk).is_err(), "{bad}");
    }
}

// spec: LOOK-003
#[test]
fn a_plan_with_an_unusable_shading_is_refused_naming_the_value() {
    let mut p = LitPlan::fixed(14.0);
    p.albedo.snow = [1.5, 0.5, 0.5];
    let e = p.validate().unwrap_err();
    assert!(e.contains("albedo.snow") && e.contains("1.5"), "{e}");
    let mut p = LitPlan::fixed(14.0);
    p.albedo.wet_mm = p.albedo.dry_mm;
    assert!(p.validate().unwrap_err().contains("albedo.dry_mm"));
    // A pair ordered in f64 that collapses in f32 is refused (the shader's smoothstep would be undefined).
    let mut p = LitPlan::fixed(14.0);
    p.albedo.rock_cos_none = p.albedo.rock_cos_full + 1.0e-12;
    assert!(p.validate().unwrap_err().contains("albedo.rock_cos_full"));
    let mut p = LitPlan::fixed(14.0);
    p.water_level_m = f32::NAN;
    assert!(p.validate().unwrap_err().contains("water level"));
}

// spec: LOOK-001, LOOK-007
#[test]
fn the_sun_and_albedo_tables_read_write_and_refuse_by_key() {
    let head = "schema_version = 2\nkind = \"look\"\n";
    let err = |text: &str| parse_look(text).unwrap_err().to_string();
    // Earth defaults: the look's sun is the sun of the first lit views, bit for bit, so the default plan keeps its text.
    let d = LookParams::default();
    assert_eq!(LitPlan::fixed_with(&d, 10.0).sun_dir, look::DEFAULT_SUN);
    assert_eq!(d.albedo, AlbedoParams::EARTH);
    // A table with some keys: the rest are the default; the canonical text writes every key and reads back equal.
    let f =
        parse_look(&format!("{head}[body.2.albedo]\nsnow = [0.9, 0.9, 1]\nwet_mm = 1000\n[body.2.sun]\ndirection = [0, 0, 1]\n")).unwrap();
    let p = f.body(BodyId(2));
    assert_eq!((p.albedo.snow, p.albedo.wet_mm, p.albedo.dry_mm, p.sun.direction), ([0.9, 0.9, 1.0], 1000.0, 250.0, [0.0, 0.0, 1.0]));
    assert_eq!(parse_look(&to_toml(&f)).unwrap(), f);
    assert_eq!(LitPlan::fixed_with(&p, 10.0).sun_dir, [0.0, 0.0, 1.0]);
    assert_eq!(LitPlan::fixed_with(&p, 10.0).albedo, p.albedo);
    // Refusals name the full key and the value.
    for (body, key, value) in [
        ("[body.2.albedo]\nsnow = [1.5, 0.5, 0.5]", "body.2.albedo.snow", "1.5"),
        ("[body.2.albedo]\nrock = [0.1, 0.2]", "body.2.albedo.rock", "2 values"),
        ("[body.2.albedo]\nrock = 0.3", "body.2.albedo.rock", "float"),
        ("[body.2.albedo]\nrock = [0.1, \"x\", 0.2]", "body.2.albedo.rock[1].v", "string"),
        ("[body.2.albedo]\ndry_mm = 900.0", "body.2.albedo.dry_mm", "900.0"),
        ("[body.2.albedo]\nrock_cos_none = 0.2", "body.2.albedo.rock_cos_full", "0.62"),
        ("[body.2.albedo]\nshore_blend_m = 0.0", "body.2.albedo.shore_blend_m", "0.0"),
        ("[body.2.albedo]\nbeach_fade_m = 0.0", "body.2.albedo.beach_fade_m", "0.0"),
        ("[body.2.albedo]\nsnow_cos = 0.5", "body.2.albedo.snow_cos", ""),
        ("[body.2.sun]\ndirection = [1, 1, 1]", "body.2.sun.direction", "1.0"),
        ("[body.2.sun]\ndirection = [nan, 0, 1]", "body.2.sun.direction", "NaN"),
        ("[body.2.sun]\ndir = [0, 0, 1]", "body.2.sun.dir", ""),
    ] {
        let e = err(&format!("{head}{body}\n"));
        assert!(e.contains(&format!("`{key}`")) && e.contains(value), "{body}: {e}");
    }
}

// spec: LOOK-002
#[test]
fn a_file_with_a_newer_schema_is_refused_as_newer_not_as_an_unknown_key() {
    // The rule (CON-10): a build that does not know a table must say "newer schema", which is why every added table or key raises
    // the schema version. A file of the next version that uses a table this build lacks is refused as newer.
    let next = SCHEMA_VERSION + 1;
    let text = format!("schema_version = {next}\nkind = \"look\"\n[body.0.atmosphere]\nscale_height_m = 8500.0\n");
    let e = parse_look(&text).unwrap_err();
    assert_eq!(e, LookError::NewerSchema { found: next, known: SCHEMA_VERSION });
    assert!(e.to_string().contains("newer") && e.to_string().contains("update the program"), "{e}");
    // The same table in a file that claims the current version is an unknown key (a typo, not a release).
    let e = parse_look(&format!("schema_version = {SCHEMA_VERSION}\nkind = \"look\"\n[body.0.atmosphere]\nscale_height_m = 8500.0\n"))
        .unwrap_err();
    assert!(e.to_string().contains("body.0.atmosphere"), "{e}");
    // Version 1 files, written before the tables existed, load through the identity step and write the current layout.
    let v1 = parse_look("schema_version = 1\nkind = \"look\"\n[body.0.units]\nsolar_illuminance_lux = 126600.0\n").unwrap();
    assert!(to_toml(&v1).starts_with(&format!("schema_version = {SCHEMA_VERSION}\n")));
}
