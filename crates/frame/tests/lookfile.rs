//! Tier A: the look file (ADR 0042 point 6, ADR 0048 point 2): schema, parse, validation, canonical text, migrations (CON-10), the
//! Earth defaults and the plan they feed. GPU-free (CON-03).

use planet_frame::look::atmosphere::AtmosphereParams;
use planet_frame::look::file::*;
use planet_frame::look::materials::{Input, MaterialParams};
use planet_frame::look::shading::AlbedoParams;
use planet_frame::look::water::WaterLook;
use planet_frame::look::{self, ExposureMode, HistogramParams, LitPlan, SceneUnits, ToneMapMode};
use planet_world_def::BodyId;

const FIXTURE_V1: &str = include_str!("data/look_v1_moon_and_earth.toml");
const FIXTURE_V2: &str = include_str!("data/look_v2_albedo_and_sun.toml");
const FIXTURE_V3: &str = include_str!("data/look_v3_atmosphere.toml");
const FIXTURE_V4: &str = include_str!("data/look_v4_water.toml");
const FIXTURE_V5: &str = include_str!("data/look_v5_ground.toml");

fn err(text: &str) -> String {
    parse_look(text).unwrap_err().to_string()
}

fn custom() -> LookParams {
    LookParams {
        units: SceneUnits { solar_illuminance_lux: 55_000.0 },
        exposure: ExposureLook { min_ev100: -4.5, max_ev100: 12.25, speed_up: 2.0, speed_down: 0.25 },
        histogram: HistogramParams { ev_min: -8.0, ev_max: 24.0, band_low: 0.25, band_high: 0.75 },
        tone_curve: ToneCurve::Aces,
        albedo: AlbedoParams { snow: [0.7, 0.72, 0.75], wet_mm: 1200.0, beach_height_m: 2.0, ..AlbedoParams::EARTH },
        sun: SunLook { direction: [0.0, 1.0, 0.0] },
        atmosphere: AtmosphereParams { mie_g: 0.55, top_height_m: 80_000.0, ..AtmosphereParams::EARTH },
        water: WaterLook { absorption_per_m: 0.35, foam_width_m: 0.6, wave_scale: 1.5, ..WaterLook::EARTH },
        ground: GroundLook { detail: 0.8, materials: ground_curves() },
    }
}

/// The Earth curves with one soil curve (moisture rising 0.55 to 0.85) and the rock curve on the slope moved.
fn ground_curves() -> MaterialParams {
    let mut m = MaterialParams::EARTH;
    m.curves[3][Input::Moisture as usize] = [0.55, 0.85];
    m.curves[0][Input::SlopeCos as usize] = [0.9, 0.7];
    m
}

// spec: LOOK-001
#[test]
fn a_complete_and_a_partial_entry_read_as_the_defaults_they_leave_out() {
    let f = parse_look(FIXTURE_V1).unwrap();
    assert_eq!(f.body(BodyId(0)), LookParams::default(), "body 0 is the Earth written out in full (min_ev100 as an exact integer literal)");
    let moon = f.body(BodyId(3));
    assert_eq!(moon.units.solar_illuminance_lux, 25_000.0);
    assert_eq!((moon.exposure.min_ev100, moon.exposure.max_ev100), (-2.0, 10.5), "min stays the default, max is overridden");
    assert_eq!((moon.exposure.speed_up, moon.exposure.speed_down), (3.0, 0.5));
    assert_eq!(moon.histogram, HistogramParams::EARTH);
    assert!(f.has_entry(BodyId(3)) && !f.has_entry(BodyId(1)));
    assert_eq!(f.bodies().map(|(b, _)| b.0).collect::<Vec<_>>(), [0, 3]);
    // Only a version and a kind: every body is the Earth.
    assert_eq!(parse_look("schema_version = 1\nkind = \"look\"\n").unwrap(), LookFile::empty());
}

// spec: LOOK-001
#[test]
fn canonical_text_is_deterministic_and_round_trips_bit_for_bit() {
    let mut f = LookFile::empty();
    f.set_body(BodyId(7), custom()).unwrap();
    f.set_body(BodyId(0), LookParams::default()).unwrap();
    // Awkward floats: shortest round-trip forms must give back the same bits.
    let mut odd = custom();
    odd.units.solar_illuminance_lux = 1361.0 / 3.0 * 93.0;
    odd.exposure.min_ev100 = -0.1;
    odd.histogram.band_low = 0.1 + 0.2;
    f.set_body(BodyId(12), odd).unwrap();
    let text = to_toml(&f);
    assert_eq!(parse_look(&text).unwrap(), f);
    assert_eq!(to_toml(&parse_look(&text).unwrap()), text, "writing what was read gives the same text");
    assert!(text.starts_with("schema_version = 5\nkind = \"look\"\n"));
    let order: Vec<usize> = ["[body.0.units]", "[body.7.units]", "[body.12.units]"].iter().map(|k| text.find(k).unwrap()).collect();
    assert!(order[0] < order[1] && order[1] < order[2], "bodies are written in id order");
    // The same content from a differently formatted file writes the same text.
    let reformatted = parse_look(FIXTURE_V1).unwrap();
    assert_eq!(to_toml(&reformatted), to_toml(&parse_look(&to_toml(&reformatted)).unwrap()));
}

// spec: LOOK-001
#[test]
fn a_value_outside_its_range_is_refused_naming_the_key_and_the_value() {
    let with = |table: &str, line: &str| format!("schema_version = 1\nkind = \"look\"\n[body.2.{table}]\n{line}\n");
    let cases = [
        ("units", "solar_illuminance_lux = 0.0", "body.2.units.solar_illuminance_lux", "0.0"),
        ("units", "solar_illuminance_lux = 1e9", "body.2.units.solar_illuminance_lux", "1000000000.0"),
        ("units", "solar_illuminance_lux = nan", "body.2.units.solar_illuminance_lux", "NaN"),
        ("exposure", "min_ev100 = 20.0", "body.2.exposure.min_ev100", "20.0"),
        ("exposure", "max_ev100 = -9.0", "body.2.exposure.min_ev100", "-2.0"),
        ("exposure", "speed_up = -1.0", "body.2.exposure.speed_up", "-1.0"),
        ("exposure", "speed_down = inf", "body.2.exposure.speed_down", "inf"),
        ("histogram", "ev_max = -9.5", "body.2.histogram.ev_max", "-9.5"),
        ("histogram", "band_low = 0.95", "body.2.histogram.band_low", "0.95"),
        ("histogram", "band_high = 1.5", "body.2.histogram.band_high", "1.5"),
        ("tone_map", "curve = \"reinhard\"", "body.2.tone_map.curve", "\"reinhard\""),
    ];
    for (table, line, key, value) in cases {
        let e = err(&with(table, line));
        assert!(e.contains(&format!("`{key}`")) && e.contains(value), "{line}: {e}");
    }
}

// spec: LOOK-001
#[test]
fn unknown_keys_wrong_types_and_bad_bodies_are_named() {
    let head = "schema_version = 1\nkind = \"look\"\n";
    assert!(err(&format!("{head}[body.0.exposur]\nmin_ev100 = 1.0")).contains("body.0.exposur"));
    assert!(err(&format!("{head}[body.0.exposure]\nmin_ev = 1.0")).contains("body.0.exposure.min_ev"));
    assert!(err(&format!("{head}[body.0]\nalbedo = 1.0")).contains("body.0.albedo"));
    assert!(err(&format!("{head}sun = 1.0")).contains("`sun`"));
    let e = err(&format!("{head}[body.0.units]\nsolar_illuminance_lux = \"bright\""));
    assert!(e.contains("body.0.units.solar_illuminance_lux") && e.contains("a number") && e.contains("string"), "{e}");
    let e = err(&format!("{head}[body.0.tone_map]\ncurve = 3"));
    assert!(e.contains("body.0.tone_map.curve") && e.contains("a string"), "{e}");
    for id in ["earth", "-1", "01", "4294967296"] {
        let e = err(&format!("{head}[body.\"{id}\"]\n"));
        assert!(e.contains(&format!("body.{id}")) && e.contains("id"), "{id}: {e}");
    }
    assert!(err(&format!("{head}body = 3")).contains("table of bodies"));
    assert!(err("schema_version = 1\nkind = \"look\"\n[body.0.units]\nsolar_illuminance_lux = 9007199254740993").contains("not exact"));
    assert!(err("this is not toml").contains("not valid TOML"));
}

// spec: LOOK-001
#[test]
fn set_body_refuses_what_a_file_would_refuse() {
    let mut f = LookFile::empty();
    let mut p = LookParams::default();
    p.exposure.min_ev100 = 17.0;
    p.exposure.max_ev100 = 16.0;
    let e = f.set_body(BodyId(1), p).unwrap_err();
    assert_eq!(e.key, "exposure.min_ev100");
    assert!(e.to_string().contains("17.0") && e.to_string().contains("max_ev100"), "{e}");
    assert_eq!(f, LookFile::empty(), "a refused entry leaves the file as it was");
    assert!(custom().validate().is_ok() && LookParams::default().validate().is_ok());
}

// spec: LOOK-002
#[test]
fn a_look_file_is_not_a_definition_and_the_other_way_round() {
    let e = err("schema_version = 1\n[world]\nseed = 1\n[body.0]\nradius_m = 6371000.0\n");
    assert!(e.contains("not a look file") && e.contains("kind"), "{e}");
    assert!(err("schema_version = 1\nkind = \"world\"\n").contains("`kind = \"world\"`"));
    // And a look file fed to the definition loader fails (the `kind` string is not a definition value), so it cannot be loaded as
    // a definition by mistake.
    let small = "schema_version = 1
kind = \"look\"
[body.0.units]
solar_illuminance_lux = 126600.0
";
    assert!(parse_look(small).is_ok());
    let as_definition = planet_world_def::toml_io::parse_definition(small).unwrap_err().to_string();
    assert!(as_definition.contains("kind"), "{as_definition}");
    assert!(planet_world_def::toml_io::parse_definition(FIXTURE_V1).is_err());
}

// spec: LOOK-002
#[test]
fn version_checks_refuse_with_actionable_messages() {
    assert_eq!(parse_look("kind = \"look\"\n").unwrap_err(), LookError::MissingSchemaVersion);
    assert!(err("kind = \"look\"\n").contains(&format!("schema_version = {SCHEMA_VERSION}")));
    let future = format!("schema_version = {}\nkind = \"look\"\n[body.0.units]\nsolar_illuminance_lux = 126600.0\n", SCHEMA_VERSION + 1);
    assert_eq!(parse_look(&future).unwrap_err(), LookError::NewerSchema { found: SCHEMA_VERSION + 1, known: SCHEMA_VERSION });
    let e = err(&future);
    assert!(e.contains("newer") && e.contains(&format!("{}", SCHEMA_VERSION + 1)) && e.contains("update the program"), "{e}");
    assert!(err("schema_version = -1\nkind = \"look\"").contains("non-negative"));
    assert!(err("schema_version = \"1\"\nkind = \"look\"").contains("string"));
    // Version 0 never existed and has no step.
    assert_eq!(parse_look("schema_version = 0\nkind = \"look\"").unwrap_err(), LookError::NoMigration { from: 0 });
    assert!(err("schema_version = 0\nkind = \"look\"").contains("no migration from look schema version 0 to 1"));
}

/// Rename `from` to `to` in the table `section` of every body.
fn rename(table: &mut toml::Table, section: &str, from: &str, to: &str) -> Result<(), String> {
    let Some(toml::Value::Table(bodies)) = table.get_mut("body") else { return Ok(()) };
    for (_, b) in bodies.iter_mut() {
        let Some(toml::Value::Table(t)) = b.get_mut(section) else { continue };
        if let Some(v) = t.remove(from) {
            if t.contains_key(to) {
                return Err(format!("both `{from}` and `{to}` are present"));
            }
            t.insert(to.to_string(), v);
        }
    }
    Ok(())
}

fn v1_to_v2(t: &mut toml::Table) -> Result<(), String> {
    rename(t, "exposure", "lo", "min")?;
    rename(t, "exposure", "hi", "max")
}

fn v2_to_v3(t: &mut toml::Table) -> Result<(), String> {
    rename(t, "exposure", "min", "min_ev100")?;
    rename(t, "exposure", "max", "max_ev100")
}

fn failing(_: &mut toml::Table) -> Result<(), String> {
    Err("the old key `lo` is missing".into())
}

// spec: LOOK-002
#[test]
fn the_engine_migrates_an_older_layout_step_by_step_to_the_current_one() {
    // Until a second layout exists the engine is exercised with a synthetic chain, like WDEF-002: a "version 1" that spelled the
    // exposure range `lo`/`hi`, a "version 2" that spelled it `min`/`max`, and the layout of this build as "version 3".
    let chain = [Migration { from: 1, apply: v1_to_v2 }, Migration { from: 2, apply: v2_to_v3 }];
    let old = "schema_version = 1\nkind = \"look\"\n[body.5.exposure]\nlo = -3.5\nhi = 11.0\nspeed_up = 2.0\n[body.5.units]\nsolar_illuminance_lux = 5e4\n";
    let mid = "schema_version = 2\nkind = \"look\"\n[body.5.exposure]\nmin = -3.5\nmax = 11.0\nspeed_up = 2.0\n[body.5.units]\nsolar_illuminance_lux = 5e4\n";
    let new = "schema_version = 3\nkind = \"look\"\n[body.5.exposure]\nmin_ev100 = -3.5\nmax_ev100 = 11.0\nspeed_up = 2.0\n[body.5.units]\nsolar_illuminance_lux = 5e4\n";
    let from_old = parse_with(old, 3, &chain).unwrap();
    assert_eq!(parse_with(mid, 3, &chain).unwrap(), from_old);
    assert_eq!(parse_with(new, 3, &chain).unwrap(), from_old, "every older layout reads as the current layout of the same content");
    let p = from_old.body(BodyId(5));
    assert_eq!((p.exposure.min_ev100, p.exposure.max_ev100, p.exposure.speed_up), (-3.5, 11.0, 2.0));
    assert_eq!(p.units.solar_illuminance_lux, 50_000.0);
    // Lossless and deterministic: what the chain produced writes the canonical text of the current layout, twice the same.
    assert_eq!(to_toml(&from_old), to_toml(&parse_with(new, 3, &chain).unwrap()));
    assert_eq!(to_toml(&parse_with(old, 3, &chain).unwrap()), to_toml(&from_old));
    assert_eq!(parse_look(&to_toml(&from_old)).unwrap(), from_old);
}

// spec: LOOK-002
#[test]
fn migration_failures_name_the_step_and_the_gap() {
    let old = "schema_version = 1\nkind = \"look\"\n[body.5.exposure]\nlo = -3.5\n";
    let broken = [Migration { from: 1, apply: failing }, Migration { from: 2, apply: v2_to_v3 }];
    let e = parse_with(old, 3, &broken).unwrap_err();
    assert_eq!(e, LookError::Migration { from: 1, message: "the old key `lo` is missing".into() });
    assert!(e.to_string().contains("from schema version 1") && e.to_string().contains("`lo`"));
    // A missing step is a gap, named.
    let gap = [Migration { from: 2, apply: v2_to_v3 }];
    let e = parse_with(old, 3, &gap).unwrap_err();
    assert_eq!(e, LookError::NoMigration { from: 1 });
    assert!(e.to_string().contains("version 1 to 2"));
    // Two steps from one version is a defect of the chain, refused before anything runs.
    let twice = [Migration { from: 1, apply: v1_to_v2 }, Migration { from: 1, apply: v1_to_v2 }];
    assert!(matches!(parse_with(old, 3, &twice).unwrap_err(), LookError::Migration { from: 1, .. }));
    // A file newer than the chain's end is refused whatever the chain.
    assert_eq!(parse_with("schema_version = 4\nkind = \"look\"", 3, &[]).unwrap_err(), LookError::NewerSchema { found: 4, known: 3 });
    // A step that leaves an unreadable layout is caught by the checks of the current layout, naming the key.
    let e = parse_with("schema_version = 1\nkind = \"look\"\n[body.5.exposure]\nlo = 30.0\n", 2, &[Migration { from: 1, apply: v1_to_v2 }])
        .unwrap_err();
    assert!(e.to_string().contains("body.5.exposure.min"), "{e}");
}

// spec: LOOK-002
#[test]
fn the_production_chain_reads_the_version_1_fixture_and_writes_the_current_layout() {
    assert_eq!(
        SCHEMA_VERSION, 5,
        "a new table or key needs a new version, an identity step in MIGRATIONS and a fixture of this version kept"
    );
    assert_eq!(MIGRATIONS.len() as u32, SCHEMA_VERSION - 1, "one step per older version");
    assert_eq!(MIGRATIONS.iter().map(|m| m.from).collect::<Vec<_>>(), [1, 2, 3, 4], "the steps run from version 1 up without a gap");
    // The version 1 fixture goes through the real chain (the identity step 1 to 2) and reads as written.
    let f = parse_look(FIXTURE_V1).unwrap();
    assert_eq!(f.body(BodyId(3)).units.solar_illuminance_lux, 25_000.0);
    assert_eq!(f.body(BodyId(3)).albedo, AlbedoParams::EARTH, "the tables version 2 added are the Earth default in a version 1 file");
    assert_eq!(f.body(BodyId(3)).sun, SunLook::default());
    assert_eq!(parse_look(&to_toml(&f)).unwrap(), f);
    assert_eq!(f.body(BodyId(3)).atmosphere, AtmosphereParams::EARTH, "the table version 3 added is the Earth default in a version 1 file");
    // The same content written as a version 3 file (the current layout) reads equal and writes the same text.
    let v3 = FIXTURE_V1.replacen("schema_version = 1", "schema_version = 3", 1);
    assert_eq!(parse_look(&v3).unwrap(), f);
    assert_eq!(to_toml(&parse_look(&v3).unwrap()), to_toml(&f));
    // The fixture of version 2 carries the tables version 2 added.
    let g = parse_look(FIXTURE_V2).unwrap();
    assert_eq!(g.body(BodyId(1)).sun.direction, [0.0, 0.0, 1.0]);
    assert_eq!(g.body(BodyId(1)).albedo.snow, [0.9, 0.9, 0.95]);
    assert_eq!(g.body(BodyId(1)).albedo.dry_mm, 100.0);
    assert_eq!(g.body(BodyId(1)).albedo.wet_mm, AlbedoParams::EARTH.wet_mm, "a key left out is the default");
    assert_eq!(parse_look(&to_toml(&g)).unwrap(), g);
    assert_eq!(g.body(BodyId(1)).atmosphere, AtmosphereParams::EARTH, "a version 2 file has no atmosphere table: the Earth default");
    // The same content as a version 3 file reads equal (the identity step 2 to 3).
    let g3 = FIXTURE_V2.replacen("schema_version = 2", "schema_version = 3", 1);
    assert_eq!(parse_look(&g3).unwrap(), g);
    // The fixture of version 3 carries the atmosphere table; keys it leaves out are the Earth default.
    let h = parse_look(FIXTURE_V3).unwrap();
    let a = h.body(BodyId(2)).atmosphere;
    assert_eq!(a.rayleigh_scattering, [20.0e-6, 8.0e-6, 3.0e-6]);
    assert_eq!(
        (a.mie_scale_height_m, a.mie_g, a.top_height_m),
        (15_000.0, 0.6, 60_000.0),
        "an integer literal exact as a float is accepted"
    );
    assert_eq!(a.ground_albedo, [0.45, 0.25, 0.15]);
    assert_eq!(a.rayleigh_scale_height_m, AtmosphereParams::EARTH.rayleigh_scale_height_m, "a key left out is the default");
    assert_eq!(a.ozone_absorption, AtmosphereParams::EARTH.ozone_absorption);
    assert_eq!(h.body(BodyId(2)).units.solar_illuminance_lux, 55_000.0);
    assert_eq!(parse_look(&to_toml(&h)).unwrap(), h);
    assert_eq!(to_toml(&parse_look(&to_toml(&h)).unwrap()), to_toml(&h));
    assert_eq!(h.body(BodyId(2)).water, WaterLook::EARTH, "a version 3 file has no water table: the Earth default");
    // The same content as a version 4 file reads equal (the identity step 3 to 4).
    let h4 = FIXTURE_V3.replacen("schema_version = 3", "schema_version = 4", 1);
    assert_eq!(parse_look(&h4).unwrap(), h);
    // The fixture of version 4 carries the water table; keys it leaves out are the Earth default.
    let w = parse_look(FIXTURE_V4).unwrap();
    let water = w.body(BodyId(1)).water;
    assert_eq!((water.absorption_per_m, water.fresnel_f0, water.foam_width_m, water.choppy, water.wave_scale), (0.25, 0.04, 2.0, 0.0, 0.0));
    assert_eq!(water.glint_roughness, WaterLook::EARTH.glint_roughness, "a key left out is the default");
    assert_eq!(water.shore_fade_m, WaterLook::EARTH.shore_fade_m);
    assert_eq!(w.body(BodyId(1)).units.solar_illuminance_lux, 90_000.0);
    assert_eq!(parse_look(&to_toml(&w)).unwrap(), w);
    assert_eq!(to_toml(&parse_look(&to_toml(&w)).unwrap()), to_toml(&w));
    assert_eq!(
        w.body(BodyId(1)).ground,
        GroundLook::default(),
        "a version 4 file has no ground table: the window's natural detail and the Earth curves"
    );
    // The same content as a version 5 file reads equal (the identity step 4 to 5).
    let w5 = FIXTURE_V4.replacen("schema_version = 4", "schema_version = 5", 1);
    assert_eq!(parse_look(&w5).unwrap(), w);
    // The fixture of version 5 carries the ground table; the keys it leaves out are the Earth default.
    let g = parse_look(FIXTURE_V5).unwrap().body(BodyId(1)).ground;
    assert_eq!(g.detail, 0.6);
    assert_eq!(g.materials.curves[3][Input::Moisture as usize], [0.55, 0.85]);
    assert_eq!(g.materials.curves[3][Input::SoilDepth as usize], [0.5, 1.2]);
    assert_eq!(g.materials.curves[0][Input::SlopeCos as usize], [0.86, 0.62], "a curve left out is the Earth default");
    assert_eq!(g.materials.curves[2], MaterialParams::EARTH.curves[2], "a material left out is the Earth default");
    let f = parse_look(FIXTURE_V5).unwrap();
    assert_eq!(parse_look(&to_toml(&f)).unwrap(), f);
    assert_eq!(to_toml(&parse_look(&to_toml(&f)).unwrap()), to_toml(&f));
}

// spec: LOOK-002, LOOK-011
#[test]
fn an_older_build_refuses_the_ground_table_as_a_newer_schema_and_ground_values_outside_their_range_are_refused_naming_the_key() {
    let e = parse_with(FIXTURE_V5, 4, &MIGRATIONS[..3]).unwrap_err().to_string();
    assert!(e.contains("newer") && e.contains('5') && e.contains('4') && e.contains("update the program"), "{e}");
    let head = "schema_version = 5\nkind = \"look\"\n";
    for (text, key, value) in [
        ("[body.0.ground]\ndetail = 1.5\n", "body.0.ground.detail", "1.5"),
        ("[body.0.ground]\ndetail = 1e-9\n", "body.0.ground.detail", "1e-9"),
        ("[body.0.ground.rock]\nslope_cos = [0.9]\n", "body.0.ground.rock.slope_cos", "1 values"),
        ("[body.0.ground.rock]\nslope_cos = [0.9, \"x\"]\n", "body.0.ground.rock.slope_cos[1]", "string"),
        ("[body.0.ground.rock]\nslope_cos = [0.9, 0.9000000000000001]\n", "ground.rock.slope_cos", "0.9000000000000001"),
        ("[body.0.ground.rock]\nslop_cos = [0.9, 0.7]\n", "body.0.ground.rock.slop_cos", ""),
        ("[body.0.ground.gravel]\nslope_cos = [0.9, 0.7]\n", "body.0.ground.gravel", ""),
        ("[body.0.ground]\nrock = 1.0\n", "body.0.ground.rock", "table"),
    ] {
        let e = err(&format!("{head}{text}"));
        assert!(e.contains(key) && e.contains(value), "{text}: {e}");
    }
}

// spec: LOOK-011
#[test]
fn a_look_without_a_ground_table_has_the_windows_natural_detail_and_the_earth_curves_and_a_written_default_reads_back() {
    let f = parse_look("schema_version = 5\nkind = \"look\"\n[body.0.units]\nsolar_illuminance_lux = 126600.0\n").unwrap();
    let g = f.body(BodyId(0)).ground;
    assert_eq!((g.detail, g.materials), (planet_frame::look::materials::NATURAL_STRENGTH, MaterialParams::EARTH));
    assert_eq!(parse_look(&to_toml(&f)).unwrap(), f);
    assert!(to_toml(&f).contains("[body.0.ground.soil]\n"), "every material is written");
}

// spec: LOOK-001, LOOK-002
#[test]
fn an_older_build_refuses_the_water_table_as_a_newer_schema_and_water_values_outside_their_range_are_refused_naming_the_key() {
    // A build that knows versions up to 3 refuses a version 4 file with "newer", not "unknown key".
    let e = parse_with(FIXTURE_V4, 3, &MIGRATIONS[..2]).unwrap_err();
    assert_eq!(e, LookError::NewerSchema { found: 4, known: 3 });
    assert!(e.to_string().contains("update the program"), "{e}");
    let bad = FIXTURE_V4.replace("[body.1.water]", "[body.1.waters]");
    assert!(err(&bad).contains("body.1.waters"), "{}", err(&bad));
    let bad = FIXTURE_V4.replace("fresnel_f0", "fresnel_f1");
    assert!(err(&bad).contains("body.1.water.fresnel_f1"), "{}", err(&bad));
    let with = |line: &str| format!("schema_version = 4\nkind = \"look\"\n[body.4.water]\n{line}\n");
    for (line, key, value) in [
        ("absorption_per_m = 0.0", "body.4.water.absorption_per_m", "0.0"),
        ("fresnel_f0 = 0.5", "body.4.water.fresnel_f0", "0.5"),
        ("glint_roughness = 0.0", "body.4.water.glint_roughness", "0.0"),
        ("shore_fade_m = 0.0", "body.4.water.shore_fade_m", "0.0"),
        ("foam_width_m = 0.0", "body.4.water.foam_width_m", "0.0"),
        ("foam_strength = 1.5", "body.4.water.foam_strength", "1.5"),
        ("choppy = -0.1", "body.4.water.choppy", "-0.1"),
        ("gravity_m_s2 = 0.0", "body.4.water.gravity_m_s2", "0.0"),
        ("wave_scale = 11.0", "body.4.water.wave_scale", "11.0"),
        ("wave_scale = \"big\"", "body.4.water.wave_scale", "string"),
    ] {
        let e = err(&with(line));
        assert!(e.contains(key) && e.contains(value), "{line}: {e}");
    }
    assert!(parse_look(&with("wave_scale = 0.0\nchoppy = 2.0")).is_ok(), "the ends of the ranges are valid");
}

// spec: LOOK-001, LOOK-002
#[test]
fn an_older_build_refuses_the_atmosphere_table_as_a_newer_schema_and_this_build_names_a_typo() {
    // A build that knows versions up to 2 refuses a version 3 file (which may carry the table) with "newer", not "unknown key".
    let e = parse_with(FIXTURE_V3, 2, &MIGRATIONS[..1]).unwrap_err();
    assert_eq!(e, LookError::NewerSchema { found: 3, known: 2 });
    assert!(e.to_string().contains("update the program"), "{e}");
    // In a file of this version the same table with a typo is an unknown key naming it.
    let bad = FIXTURE_V3.replace("[body.2.atmosphere]", "[body.2.atmospher]");
    assert!(err(&bad).contains("body.2.atmospher"), "{}", err(&bad));
    let bad = FIXTURE_V3.replace("mie_g", "mie_gee");
    assert!(err(&bad).contains("body.2.atmosphere.mie_gee"), "{}", err(&bad));
}

// spec: LOOK-001
#[test]
fn atmosphere_values_outside_their_range_are_refused_naming_the_key_and_the_value() {
    let with = |line: &str| format!("schema_version = 3\nkind = \"look\"\n[body.4.atmosphere]\n{line}\n");
    for (line, key, value) in [
        ("rayleigh_scattering = [1e-5, -1e-6, 1e-5]", "body.4.atmosphere.rayleigh_scattering", "-1e-6"),
        ("rayleigh_scattering = [1e-5, 1e-5, 0.5]", "body.4.atmosphere.rayleigh_scattering", "0.5"),
        ("mie_g = 1.0", "body.4.atmosphere.mie_g", "1.0"),
        ("mie_scale_height_m = 0.0", "body.4.atmosphere.mie_scale_height_m", "0.0"),
        ("top_height_m = 10.0", "body.4.atmosphere.top_height_m", "10.0"),
        ("ground_albedo = [0.3, 1.5, 0.3]", "body.4.atmosphere.ground_albedo", "1.5"),
        ("sun_angular_radius_rad = 0.0", "body.4.atmosphere.sun_angular_radius_rad", "0.0"),
        ("ozone_half_width_m = 0.0", "body.4.atmosphere.ozone_half_width_m", "0.0"),
        ("mie_absorption = [1e-6, 1e-6]", "body.4.atmosphere.mie_absorption", "2 values"),
        ("mie_g = \"high\"", "body.4.atmosphere.mie_g", "string"),
    ] {
        let e = err(&with(line));
        assert!(e.contains(key) && e.contains(value), "{line}: {e}");
    }
    assert!(parse_look(&with("mie_g = -0.9\ntop_height_m = 1000.0")).is_ok(), "the ends of the ranges are valid");
}

// spec: LOOK-003
#[test]
fn the_earth_defaults_are_the_values_that_were_code_constants() {
    let d = LookParams::default();
    assert_eq!(d.units.solar_illuminance_lux, 126_600.0);
    assert_eq!(d.units.luminance_per_unit(), look::LUMINANCE_PER_UNIT);
    assert_eq!(d.exposure_params(), look::ExposureParams { min_ev100: -2.0, max_ev100: 16.0, speed_up: 3.0, speed_down: 1.0 });
    assert_eq!(d.histogram, HistogramParams { ev_min: -10.0, ev_max: 20.0, band_low: 0.4, band_high: 0.9 });
    assert_eq!(d.tone_curve, ToneCurve::Aces);
    assert_eq!(d.tone_curve.mode(), ToneMapMode::Aces);
    // A body with no entry, and a file with no bodies at all, give the same thing.
    let f = parse_look(FIXTURE_V1).unwrap();
    assert_eq!(f.body(BodyId(99)), d);
    assert_eq!(LookFile::empty().body(BodyId::PLANET), d);
    // The pre-exposure factor under the defaults is the function of old.
    assert_eq!(look::pre_exposure_factor_with(&d.units, 15.0), look::pre_exposure_factor(15.0));
    assert_eq!(look::pre_exposure_factor(15.0), 126_600.0 / (1.2 * 32_768.0));
}

// spec: LOOK-003
#[test]
fn the_default_lit_plans_are_what_they_were_and_keep_their_text() {
    assert_eq!(LitPlan::fixed(14.5), LitPlan::fixed_with(&LookParams::default(), 14.5));
    assert_eq!(LitPlan::adaptive(11.0, 0.016), LitPlan::adaptive_with(&LookParams::default(), 11.0, 0.016));
    let plan = LitPlan::adaptive(11.0, 0.016);
    assert_eq!((plan.units, plan.histogram), (SceneUnits::EARTH, HistogramParams::EARTH));
    match plan.exposure {
        ExposureMode::Adaptive { params, .. } => assert_eq!(params, look::ExposureParams::default()),
        ExposureMode::Fixed { .. } => unreachable!(),
    }
    // The text carries nothing of the look for the defaults, so plan hashes, snapshots and repro bundles made before the look file
    // read and write as they did.
    let text = plan.to_text();
    assert!(text.ends_with("tonemap aces") && !text.contains("units") && !text.contains("hist"), "{text}");
    let old_line =
        "lit adaptive 41300000 c0000000 41800000 40400000 3f800000 dt 3c83126f sun 3f0066e4 3f19a04d 3f1f1ab2 reset 0 tonemap aces";
    let tokens: Vec<&str> = old_line.split_whitespace().collect();
    let parsed = LitPlan::from_tokens(&tokens[1..]).unwrap();
    assert_eq!((parsed.units, parsed.histogram), (SceneUnits::EARTH, HistogramParams::EARTH), "a line from before the look file");
    assert_eq!(parsed.exposure.pre_ev100(), 11.0);
}

// spec: LOOK-003
#[test]
fn a_bodys_look_reaches_the_plan_and_changes_the_text_and_the_factor() {
    let f = parse_look(FIXTURE_V1).unwrap();
    let moon = f.body(BodyId(3));
    let plan = LitPlan::adaptive_with(&moon, 9.0, 0.02);
    assert_eq!(plan.units.solar_illuminance_lux, 25_000.0);
    match plan.exposure {
        ExposureMode::Adaptive { params, pre_ev100 } => {
            assert_eq!((params.max_ev100, params.speed_down, pre_ev100), (10.5, 0.5, 9.0));
        }
        ExposureMode::Fixed { .. } => unreachable!(),
    }
    assert_ne!(plan.hash(), LitPlan::adaptive(9.0, 0.02).hash());
    // The look-dependent part of the text is exact (f64 bits) and round-trips, for a fixed and an adaptive plan.
    for p in [plan, LitPlan::fixed_with(&custom(), 13.0)] {
        let text = p.to_text();
        assert!(text.contains(" units ") && text.contains(" hist "), "{text}");
        let tokens: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(LitPlan::from_tokens(&tokens[1..]).unwrap(), p, "{text}");
        assert!(p.validate().is_ok());
    }
    // The factor scales with the unit: a sun half as bright writes half the value.
    let half = SceneUnits { solar_illuminance_lux: 63_300.0 };
    assert_eq!(look::pre_exposure_factor_with(&half, 12.0) * 2.0, look::pre_exposure_factor(12.0));
    assert_eq!(look::written_value_with(&half, [1.0, 0.5, 0.0], 12.0)[0] * 2.0, look::written_value([1.0, 0.5, 0.0], 12.0)[0]);
}

// spec: LOOK-003
#[test]
fn a_plan_with_an_unusable_look_is_refused_naming_the_value() {
    let mut p = LitPlan::fixed(10.0);
    p.units.solar_illuminance_lux = -5.0;
    let e = p.validate().unwrap_err();
    assert!(e.contains("units.solar_illuminance_lux") && e.contains("-5.0"), "{e}");
    let mut p = LitPlan::fixed(10.0);
    p.histogram.band_low = 0.95;
    let e = p.validate().unwrap_err();
    assert!(e.contains("histogram.band_low") && e.contains("0.95"), "{e}");
    // A band that rounds to nothing in f32 is refused, and so is a factor that overflows f32.
    let mut p = LitPlan::fixed(10.0);
    p.histogram.band_low = 0.5;
    p.histogram.band_high = 0.500_000_01;
    assert!(p.validate().unwrap_err().contains("histogram.band_low"));
    let mut p = LitPlan::fixed(-120.0);
    p.units.solar_illuminance_lux = 1.0e7;
    assert!(p.validate().unwrap_err().contains("pre-exposure factor"));
    let mut dark = LookParams::default();
    dark.units.solar_illuminance_lux = 1.0;
    dark.exposure.max_ev100 = 30.0;
    let e = dark.validate().unwrap_err();
    assert_eq!(e.key, "exposure.max_ev100", "{e}");
    let tokens =
        ["fixed", "41200000", "dt", "00000000", "sun", "3f800000", "00000000", "00000000", "reset", "0", "tonemap", "aces", "units", "zz"];
    assert!(LitPlan::from_tokens(&tokens).is_err());
}

// spec: LOOK-003
#[test]
fn histogram_params_generalise_the_axis_and_the_band() {
    let g = HistogramParams { ev_min: -8.0, ev_max: 24.0, band_low: 0.25, band_high: 0.75 };
    assert_eq!(g.bin_width(), 0.5);
    assert_eq!(g.bin_center_ev(0), -7.75);
    assert_eq!(g.bin_of_ev(-8.0), Some(0));
    assert_eq!(g.bin_of_ev(0.0), Some(16));
    assert_eq!(g.bin_of_ev(23.99), Some(63));
    assert_eq!(g.bin_of_ev(99.0), Some(63));
    assert_eq!(g.bin_of_ev(f64::NAN), None);
    // Four equal counts in bins 10, 20, 30 and 40: the band 25 % to 75 % covers the middle two entirely.
    let mut h = [0u32; look::HIST_BINS];
    for b in [10, 20, 30, 40] {
        h[b] = 100;
    }
    let want = (g.bin_center_ev(20) + g.bin_center_ev(30)) / 2.0;
    assert!((g.band_mean_ev(&h).unwrap() - want).abs() < 1e-12);
    // The default band over the same counts differs, and the earth functions are the Earth parameters.
    assert_ne!(look::band_mean_ev(&h), g.band_mean_ev(&h));
    assert_eq!(look::band_mean_ev(&h), HistogramParams::EARTH.band_mean_ev(&h));
    let params = look::ExposureParams::default();
    assert_eq!(g.target_ev(&[0; look::HIST_BINS], &params, 7.0), 7.0, "nothing counted holds the current EV");
    assert_eq!(g.target_ev(&h, &params, 0.0), want.clamp(-2.0, 16.0));
    let px = [[0.5, 0.5, 0.5], [0.0; 3]];
    assert_eq!(g.histogram_of(&px, 12.0).iter().sum::<u32>(), 1, "black is not counted");
}
