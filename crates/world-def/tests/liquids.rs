//! WDEF-006: liquid kinds as a parameter table in the definition, and the sea of a body (ADR 0041 points 1 to 3).

use planet_world_def::toml_io::{parse_definition, parse_with, to_toml, LoadError, MIGRATIONS, SCHEMA_VERSION};
use planet_world_def::{
    BodyId, GenContext, LiquidKind, LiquidKindId, LiquidTable, NodeValue, SeaParams, WorldDefinition, EARTH_OCEAN_MIN_AREA_FRACTION,
};

const LAVA_LAKE: &str = include_str!("fixtures/liquids_lava_lake.toml");

fn lava() -> LiquidKind {
    LiquidKind { colour: [0.8, 0.12, 0.02], emission: [40.0, 9.0, 0.5], viscosity_pa_s: 1.0e3, wave_amplitude_m: 0.0, wave_length_m: 0.0 }
}

fn planet() -> WorldDefinition {
    WorldDefinition::single_planet(6_371_000.0, 42)
}

// spec: WDEF-006
#[test]
fn a_world_without_a_liquid_table_has_the_earth_table_of_water_and_hashes_as_before() {
    let d = planet();
    let t = d.liquids().unwrap();
    assert_eq!(t, LiquidTable::earth());
    assert_eq!(t.len(), 1);
    assert_eq!(t.get(LiquidKindId::WATER), Some(&LiquidKind::WATER));
    assert!(t.get(LiquidKindId::WATER).unwrap().has_waves());
    assert_eq!(t.get(LiquidKindId::WATER).unwrap().emission, [0.0; 3]);
    // The pinned hash of the version 1 definition of the same content: the table adds no node.
    assert_eq!(WorldDefinition::single_planet(6_371_000.0, 1).hash(), 6_345_583_285_605_900_286);
    assert_eq!(d.sea(BodyId::PLANET).unwrap(), SeaParams::EARTH);
    assert_eq!(SeaParams::EARTH.level_m, 0.0);
    assert_eq!(SeaParams::EARTH.liquid, LiquidKindId::WATER);
    assert!((SeaParams::EARTH.min_area_fraction * 4.0 * core::f64::consts::PI * 6371.0 * 6371.0 - 1.0e6).abs() < 1.0);
}

// spec: WDEF-006
#[test]
fn defining_a_table_replaces_the_earth_table_and_changes_the_hash() {
    let mut d = planet();
    let before = d.hash();
    d.set_liquid(LiquidKindId(0), &LiquidKind::WATER).unwrap();
    assert_ne!(d.hash(), before, "a table, even the Earth one written out, is a different definition");
    d.set_liquid(LiquidKindId(7), &lava()).unwrap();
    let t = d.liquids().unwrap();
    assert_eq!(t.ids().collect::<Vec<_>>(), [LiquidKindId(0), LiquidKindId(7)]);
    assert_eq!(t.get(LiquidKindId(7)), Some(&lava()));
    assert!(!t.get(LiquidKindId(7)).unwrap().has_waves());
    // Not merged with the Earth table: a world that defines only kind 3 has only kind 3.
    let mut only = planet();
    only.set_liquid(LiquidKindId(3), &lava()).unwrap();
    assert_eq!(only.liquids().unwrap().ids().collect::<Vec<_>>(), [LiquidKindId(3)]);
    // Every field of every row is in the hash.
    let h = d.hash();
    for tweak in 0..9 {
        let mut v = lava().to_values();
        v[tweak] = if tweak < 3 { 0.5 } else { v[tweak] + 0.125 };
        let mut e = d.clone();
        e.set(&planet_world_def::liquid_path(LiquidKindId(7), planet_world_def::LIQUID_FIELDS[tweak]), NodeValue::F64(v[tweak]));
        assert_ne!(e.hash(), h, "field {tweak}");
    }
}

// spec: WDEF-006
#[test]
fn a_row_with_meaningless_numbers_is_refused_naming_the_field() {
    let mut d = planet();
    type Tweak = fn(&mut LiquidKind);
    let cases: [(&str, Tweak); 7] = [
        ("colour_g", |k| k.colour[1] = 1.5),
        ("emission_b", |k| k.emission[2] = -1.0),
        ("viscosity_pa_s", |k| k.viscosity_pa_s = 0.0),
        ("wave_amplitude_m", |k| k.wave_amplitude_m = -0.1),
        ("wave_length_m", |k| {
            k.wave_amplitude_m = 1.0;
            k.wave_length_m = 0.0
        }),
        ("colour_r", |k| k.colour[0] = f64::NAN),
        ("viscosity_pa_s", |k| k.viscosity_pa_s = f64::INFINITY),
    ];
    for (field, make) in cases {
        let mut k = lava();
        make(&mut k);
        let e = d.set_liquid(LiquidKindId(1), &k).unwrap_err();
        assert!(e.contains(field) && e.contains("liquid 1"), "{field}: {e}");
    }
    // Nothing was written by a refused row.
    assert_eq!(d, planet());
    // A row that reached the definition by other means (a hand-written file) is refused when the table is read.
    d.set("liquid.2.colour_r", NodeValue::F64(0.1));
    let e = d.liquids().unwrap_err().to_string();
    assert!(e.contains("liquid.2.colour_g"), "a missing field is named: {e}");
    for (name, v) in [
        ("colour_g", 0.1),
        ("colour_b", 0.1),
        ("emission_r", 0.0),
        ("emission_g", 0.0),
        ("emission_b", 0.0),
        ("wave_amplitude_m", 0.0),
        ("wave_length_m", 0.0),
    ] {
        d.set(&format!("liquid.2.{name}"), NodeValue::F64(v));
    }
    assert!(d.liquids().unwrap_err().to_string().contains("liquid.2.viscosity_pa_s"));
    d.set("liquid.2.viscosity_pa_s", NodeValue::U64(3));
    assert!(d.liquids().unwrap_err().to_string().contains("not a number"));
    d.set("liquid.2.viscosity_pa_s", NodeValue::F64(-3.0));
    assert!(d.liquids().unwrap_err().to_string().contains("viscosity"));
}

// spec: WDEF-006
#[test]
fn the_sea_reads_its_nodes_over_the_earth_defaults_and_refuses_what_means_nothing() {
    let mut d = planet();
    d.set(&planet_world_def::body_sea_level_path(BodyId::PLANET), NodeValue::F64(-120.5));
    assert_eq!(d.sea(BodyId::PLANET).unwrap(), SeaParams { level_m: -120.5, ..SeaParams::EARTH });
    d.set_sea(BodyId::PLANET, &SeaParams { level_m: 3.0, liquid: LiquidKindId(7), min_area_fraction: 0.25 });
    assert_eq!(d.sea(BodyId::PLANET).unwrap(), SeaParams { level_m: 3.0, liquid: LiquidKindId(7), min_area_fraction: 0.25 });
    // Another body has its own sea, by default Earth's.
    assert_eq!(d.sea(BodyId(1)).unwrap(), SeaParams::EARTH);
    for (path, v, what) in [
        ("body.0.sea.min_area_fraction", NodeValue::F64(1.5), "fraction"),
        ("body.0.sea.min_area_fraction", NodeValue::F64(-0.1), "fraction"),
        ("body.0.sea.min_area_fraction", NodeValue::U64(1), "not a number"),
        ("body.0.sea.level_m", NodeValue::F64(f64::INFINITY), "not finite"),
        ("body.0.sea.liquid", NodeValue::F64(1.0), "not an integer"),
        ("body.0.sea.liquid", NodeValue::U64(u64::from(u32::MAX) + 1), "kind id"),
    ] {
        let mut e = planet();
        e.set(path, v);
        let m = e.sea(BodyId::PLANET).unwrap_err().to_string();
        assert!(m.contains(path) && m.contains(what), "{path}: {m}");
    }
}

// spec: WDEF-006
// spec: WDEF-004
#[test]
fn a_generator_that_reads_the_table_or_the_sea_records_what_the_definition_sets() {
    let read = |d: &WorldDefinition| {
        let ctx = GenContext::new(d);
        ctx.liquids().unwrap();
        ctx.sea(BodyId::PLANET).unwrap();
        (ctx.read_set_hash(), ctx.reads().into_iter().map(|(p, _)| p).collect::<Vec<_>>())
    };
    let (earth, paths) = read(&planet());
    assert!(paths.is_empty(), "nothing set, nothing read: {paths:?}");
    let mut d = planet();
    d.set_liquid(LiquidKindId(0), &LiquidKind::WATER).unwrap();
    let (with_table, paths) = read(&d);
    assert_ne!(with_table, earth);
    assert_eq!(paths.len(), 9);
    assert!(paths.iter().all(|p| p.starts_with("liquid.0.")));
    d.set_sea(BodyId::PLANET, &SeaParams::EARTH);
    let (with_sea, paths) = read(&d);
    assert_ne!(with_sea, with_table);
    assert_eq!(paths.len(), 12);
    // A node nobody read changes nothing: another body's sea.
    let mut e = d.clone();
    e.set_sea(BodyId(5), &SeaParams { level_m: 9.0, ..SeaParams::EARTH });
    assert_eq!(read(&e).0, with_sea);
    // A value change changes the read set.
    let mut f = d.clone();
    f.set_sea(BodyId::PLANET, &SeaParams { level_m: 1.0, ..SeaParams::EARTH });
    assert_ne!(read(&f).0, with_sea);
}

// spec: WDEF-006
// spec: WDEF-002
#[test]
fn the_table_and_the_sea_round_trip_through_the_file_and_the_fixture_describes_a_lava_lake_world() {
    let mut d = planet();
    d.set_liquid(LiquidKindId(0), &LiquidKind::WATER).unwrap();
    d.set_liquid(LiquidKindId(1), &lava()).unwrap();
    d.set_sea(BodyId::PLANET, &SeaParams { level_m: 0.0, liquid: LiquidKindId(1), min_area_fraction: 0.01 });
    let text = to_toml(&d).unwrap();
    assert!(text.starts_with("schema_version = 2\n"));
    assert!(text.contains("[liquid.1]\n") && text.contains("emission_r = 40.0\n") && text.contains("[body.0]\n"));
    assert!(text.contains("liquid = 1\n"), "the kind id is an integer: {text}");
    let back = parse_definition(&text).unwrap();
    assert_eq!(back, d);
    assert_eq!(back.hash(), d.hash());
    assert_eq!(back.liquids().unwrap(), d.liquids().unwrap());
    // The committed fixture: the same lava-lake world written by hand (integer literals for float nodes, comments).
    let fx = parse_definition(LAVA_LAKE).unwrap();
    assert_eq!(fx.liquids().unwrap().len(), 2);
    assert_eq!(fx.liquids().unwrap().get(LiquidKindId(1)).unwrap(), &lava());
    assert_eq!(fx.sea(BodyId::PLANET).unwrap(), SeaParams { level_m: 0.0, liquid: LiquidKindId(1), min_area_fraction: 0.01 });
    assert_eq!(fx, d);
    assert_eq!(to_toml(&fx).unwrap(), text, "canonical");
}

// spec: WDEF-002
// spec: WDEF-006
#[test]
fn the_types_of_the_new_nodes_are_fixed_where_the_program_reads_them() {
    let ok = |extra: &str| parse_definition(&format!("schema_version = 2\n[world]\nseed = 1\n[body.0]\nradius_m = 6371000\n{extra}"));
    // An integer literal is accepted for a float node, a float for the kind id is refused.
    assert!(ok("[body.0.sea]\nlevel_m = 0\nmin_area_fraction = 1\nliquid = 2\n").is_ok());
    assert!(matches!(ok("[body.0.sea]\nliquid = 2.0\n"), Err(LoadError::WrongType { path, .. }) if path == "body.0.sea.liquid"));
    let d = ok("[liquid.0]\ncolour_r = 0\ncolour_g = 0\ncolour_b = 0\nemission_r = 0\nemission_g = 0\nemission_b = 0\nviscosity_pa_s = 1\nwave_amplitude_m = 0\nwave_length_m = 0\n").unwrap();
    assert_eq!(d.get("liquid.0.viscosity_pa_s"), Some(NodeValue::F64(1.0)), "float nodes even when written as integers");
}

// spec: WDEF-002
#[test]
fn version_one_files_load_through_the_identity_step_and_a_build_that_knows_version_two_names_newer_ones() {
    assert_eq!(SCHEMA_VERSION, 2);
    assert_eq!(MIGRATIONS.len(), 1);
    assert_eq!(MIGRATIONS[0].from, 1);
    let v1 = "schema_version = 1\n[world]\nseed = 42\n[body.0]\nradius_m = 6371000.0\n";
    let v2 = "schema_version = 2\n[world]\nseed = 42\n[body.0]\nradius_m = 6371000.0\n";
    let (a, b) = (parse_definition(v1).unwrap(), parse_definition(v2).unwrap());
    assert_eq!(a, b);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a, planet());
    assert_eq!(a.liquids().unwrap(), LiquidTable::earth(), "a migrated world has the Earth table");
    // A build of version 1 (simulated: the engine with current = 1) says the file is newer, not that a key is unknown.
    let older = parse_with("schema_version = 2\n[liquid.0]\nviscosity_pa_s = 1\n", 1, &[]).unwrap_err();
    assert_eq!(older, LoadError::NewerSchema { found: 2, known: 1 });
}

// spec: WDEF-006
#[test]
fn the_earth_default_area_fraction_is_a_million_square_kilometres() {
    // 1e6 km^2 over the sphere of radius 6371 km.
    let surface_km2 = 4.0 * core::f64::consts::PI * 6371.0 * 6371.0;
    assert!((EARTH_OCEAN_MIN_AREA_FRACTION * surface_km2 - 1.0e6).abs() < 1e-6 * 1.0e6);
    // For a body half as wide the same fraction is a quarter of the area.
    assert!((EARTH_OCEAN_MIN_AREA_FRACTION * surface_km2 / 4.0 - 2.5e5).abs() < 1.0);
}

// spec: WDEF-006
#[test]
fn ids_with_leading_zeros_and_seas_far_from_the_reference_surface_are_refused() {
    let text = "schema_version = 2\n[world]\nseed = 1\n[liquid.01]\ncolour_r = 0.1\n";
    let d = parse_definition(text).unwrap();
    // `liquid.01` is not a liquid node: it is an ordinary node nobody reads, and no table is built from it.
    assert_eq!(d.liquids().unwrap(), LiquidTable::earth());
    let mut e = planet();
    e.set("body.0.sea.level_m", NodeValue::F64(6.371e6));
    assert!(e.sea(BodyId::PLANET).unwrap_err().to_string().contains("within"));
    e.set("body.0.sea.level_m", NodeValue::F64(-1.0e5));
    assert_eq!(e.sea(BodyId::PLANET).unwrap().level_m, -1.0e5);
}
