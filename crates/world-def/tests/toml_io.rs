//! WDEF-002 (schema version and migrations) and WDEF-003 (canonical hashing) through the TOML format.

use planet_world_def::toml_io::{parse_definition, parse_with, to_toml, LoadError, Migration, MIGRATIONS, SCHEMA_VERSION};
use planet_world_def::{BodyId, NodeValue, WorldDefinition};

type Check = fn(&LoadError) -> bool;

const FIXTURE: &str = include_str!("fixtures/v1_single_planet.toml");

fn planet() -> WorldDefinition {
    WorldDefinition::single_planet(6_371_000.0, 42)
}

// spec: WDEF-002
#[test]
fn the_committed_fixture_loads_to_the_definition_it_describes() {
    assert_eq!(parse_definition(FIXTURE).unwrap(), planet());
}

// spec: WDEF-002
// spec: WDEF-003
#[test]
fn the_canonical_text_is_pinned_and_round_trips() {
    let text = to_toml(&planet()).unwrap();
    assert_eq!(text, "schema_version = 1\n\n[body.0]\nradius_m = 6371000.0\n\n[world]\nseed = 42\n");
    assert_eq!(SCHEMA_VERSION, 1, "a new schema version needs a migration, a fixture and a new pinned text");
    assert_eq!(parse_definition(&text).unwrap(), planet());
}

// spec: WDEF-002
// spec: WDEF-003
#[test]
fn every_value_round_trips_bit_for_bit() {
    let mut d = WorldDefinition::new(u64::MAX); // beyond TOML's i64 range: a decimal string
    for (i, v) in [0.0, -0.0, 1.0, -1.5e-300, 5e-324, 1.7976931348623157e308, 6_371_000.0, 0.1 + 0.2, 1e21, 123_456_789.123_456_78]
        .into_iter()
        .enumerate()
    {
        d.set(&format!("test.f{i}"), NodeValue::F64(v));
    }
    d.set("test.max_i64", NodeValue::U64(i64::MAX as u64));
    d.set("test.over_i64", NodeValue::U64(i64::MAX as u64 + 1));
    d.set("top", NodeValue::U64(7));
    d.set_body_radius(BodyId(3), 1_737_400.0);
    let text = to_toml(&d).unwrap();
    let back = parse_definition(&text).unwrap();
    assert_eq!(back, d, "{text}");
    assert_eq!(back.hash(), d.hash());
    // Text of equal definitions is equal, whatever order they were built in.
    let mut e = WorldDefinition::empty();
    let mut nodes: Vec<_> = d.nodes().collect();
    nodes.reverse();
    for (p, v) in nodes {
        e.set(p, v);
    }
    assert_eq!(to_toml(&e).unwrap(), text);
}

// spec: WDEF-003
#[test]
fn comments_order_spacing_and_table_style_never_change_the_hash() {
    let reordered = "[body.0]\nradius_m=6371000.0 # metres\n\n[world]\nseed = 42\n\nschema_version = 1\n";
    // `schema_version` after a table header belongs to [world]: it must come first, so spell the same file differently.
    let reordered = reordered.replace("\nschema_version = 1\n", "");
    let reordered = format!("schema_version = 1\n{reordered}");
    let dotted = "schema_version = 1\nworld.seed = 42\nbody.0.radius_m = 6.371e6\n";
    let inline = "schema_version = 1\nworld = { seed = 42 }\nbody = { 0 = { radius_m = 6371000.0 } }\n";
    for text in [reordered.as_str(), dotted, inline, FIXTURE] {
        let d = parse_definition(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(d.hash(), planet().hash(), "{text}");
    }
}

// spec: WDEF-002
// spec: WDEF-003
#[test]
fn a_value_that_cannot_be_stored_exactly_is_an_error_naming_its_path() {
    let cases: [(&str, Check); 8] = [
        ("schema_version = 1\n[a]\nb = -3\n", |e| matches!(e, LoadError::NegativeInteger { path } if path == "a.b")),
        ("schema_version = 1\n[a]\nb = inf\n", |e| matches!(e, LoadError::NonFinite { path } if path == "a.b")),
        ("schema_version = 1\n[a]\nb = nan\n", |e| matches!(e, LoadError::NonFinite { .. })),
        ("schema_version = 1\n[a]\nb = \"text\"\n", |e| matches!(e, LoadError::Unsupported { path, what: "string" } if path == "a.b")),
        ("schema_version = 1\n[a]\nb = true\n", |e| matches!(e, LoadError::Unsupported { what: "boolean", .. })),
        ("schema_version = 1\n[a]\nb = [1, 2]\n", |e| matches!(e, LoadError::Unsupported { what: "array", .. })),
        (
            "schema_version = 1\n[derived.body.0]\nseed = 1\n",
            |e| matches!(e, LoadError::BadPath { path, .. } if path == "derived.body.0.seed"),
        ),
        ("schema_version = 1\n[\"a.b\"]\nc = 1\n", |e| matches!(e, LoadError::BadPath { .. })),
    ];
    for (text, ok) in cases {
        let e = parse_definition(text).unwrap_err();
        assert!(ok(&e), "{text}: {e}");
        assert!(!e.to_string().is_empty());
    }
    // 2^64 does not fit: the string stays a string.
    assert!(matches!(parse_definition("schema_version = 1\n[a]\nb = \"18446744073709551616\"\n"), Err(LoadError::Unsupported { .. })));
    assert_eq!(
        parse_definition("schema_version = 1\n[a]\nb = \"18446744073709551615\"\n").unwrap().get("a.b"),
        Some(NodeValue::U64(u64::MAX))
    );
}

// spec: WDEF-002
// spec: WDEF-003
#[test]
fn a_path_cannot_be_both_a_value_and_a_table() {
    let mut d = WorldDefinition::new(1);
    d.set("a", NodeValue::U64(1));
    d.set("a.b", NodeValue::U64(2));
    assert!(matches!(to_toml(&d), Err(LoadError::Conflict { .. })));
    let mut d = WorldDefinition::new(1);
    d.set("bad key", NodeValue::U64(1));
    assert!(matches!(to_toml(&d), Err(LoadError::BadPath { .. })));
    let mut d = WorldDefinition::new(1);
    d.set("x", NodeValue::F64(f64::NAN));
    assert!(matches!(to_toml(&d), Err(LoadError::NonFinite { .. })));
}

// spec: WDEF-002
#[test]
fn the_version_is_required_and_checked_before_anything_else() {
    assert_eq!(parse_definition("[world]\nseed = 1\n"), Err(LoadError::MissingSchemaVersion));
    assert!(matches!(parse_definition("schema_version = \"1\"\n"), Err(LoadError::BadSchemaVersion(_))));
    assert!(matches!(parse_definition("schema_version = -1\n"), Err(LoadError::BadSchemaVersion(_))));
    assert_eq!(parse_definition("schema_version = 2\n[world]\nseed = 1\n"), Err(LoadError::NewerSchema { found: 2, known: 1 }));
    assert!(matches!(parse_definition("schema_version = 1\n[world\n"), Err(LoadError::Syntax(_))));
    assert!(parse_definition("schema_version = 1\n[world\n").unwrap_err().to_string().starts_with("not valid TOML"));
}

fn rename(table: &mut toml::Table, from: &str, to: &str) -> Result<(), String> {
    let v = table.remove(from).ok_or_else(|| format!("`{from}` is missing"))?;
    table.insert(to.to_string(), v);
    Ok(())
}

/// A synthetic history, newest last: v1 (the fixture) -> v2 renames `world` to `universe` -> v3 moves the seed under
/// `universe.rng`. The engine under test is the production one; only the chain is made up.
fn history() -> [Migration; 2] {
    [
        Migration { from: 1, apply: |t| rename(t, "world", "universe") },
        Migration {
            from: 2,
            apply: |t| {
                let universe = t.get_mut("universe").and_then(|u| u.as_table_mut()).ok_or("`universe` is not a table")?;
                let seed = universe.remove("seed").ok_or("`universe.seed` is missing")?;
                let mut rng = toml::Table::new();
                rng.insert("seed".to_string(), seed);
                universe.insert("rng".to_string(), toml::Value::Table(rng));
                Ok(())
            },
        },
    ]
}

// spec: WDEF-002
#[test]
fn an_old_fixture_migrates_through_every_step_to_the_current_layout() {
    let chain = history();
    // Version 3 of the same content, written the way version 3 would write it.
    let current_v3 = "schema_version = 3\n[body.0]\nradius_m = 6371000.0\n[universe.rng]\nseed = 42\n";
    let from_v1 = parse_with(FIXTURE, 3, &chain).unwrap();
    let from_v3 = parse_with(current_v3, 3, &chain).unwrap();
    assert_eq!(from_v1, from_v3);
    assert_eq!(from_v1.hash(), from_v3.hash());
    assert_eq!(from_v1.get("universe.rng.seed"), Some(NodeValue::U64(42)));
    // A file in the middle of the history takes only the steps after it.
    let v2 = "schema_version = 2\n[universe]\nseed = 42\n[body.0]\nradius_m = 6371000.0\n";
    assert_eq!(parse_with(v2, 3, &chain).unwrap(), from_v3);
}

// spec: WDEF-002
#[test]
fn a_gap_in_the_chain_or_a_failing_step_is_a_named_error_not_a_guess() {
    let chain = history();
    assert_eq!(parse_with(FIXTURE, 3, &chain[1..]), Err(LoadError::NoMigration { from: 1 }));
    let no_world = "schema_version = 1\n[body.0]\nradius_m = 1.0\n";
    let e = parse_with(no_world, 3, &chain).unwrap_err();
    assert!(matches!(&e, LoadError::Migration { from: 1, message } if message.contains("`world` is missing")), "{e}");
    assert!(e.to_string().contains("schema version 1"));
}

// spec: WDEF-002
#[test]
fn a_float_node_accepts_an_exact_integer_literal_and_an_integer_node_rejects_a_float() {
    let hand_written = "schema_version = 1\n[world]\nseed = 42\n[body.0]\nradius_m = 6371000\n";
    let d = parse_definition(hand_written).unwrap();
    assert_eq!(d, planet());
    assert_eq!(d.hash(), planet().hash());
    assert_eq!(d.get("body.0.radius_m"), Some(NodeValue::F64(6_371_000.0)));
    let e = parse_definition("schema_version = 1\n[world]\nseed = 42.0\n").unwrap_err();
    assert_eq!(e, LoadError::WrongType { path: "world.seed".into(), expected: "an integer", found: "a float" });
    assert!(e.to_string().contains("world.seed"));
    // Too large to be exact as a float: name the fix.
    let e = parse_definition("schema_version = 1\n[body.0]\nradius_m = 9007199254740993\n").unwrap_err();
    assert!(matches!(&e, LoadError::NotCanonical { path, .. } if path == "body.0.radius_m"), "{e}");
    // A path the program does not read keeps the type of its literal.
    assert_eq!(parse_definition("schema_version = 1\n[x]\ny = 3\n").unwrap().get("x.y"), Some(NodeValue::U64(3)));
    assert_eq!(parse_definition("schema_version = 1\n[x]\ny = 3.0\n").unwrap().get("x.y"), Some(NodeValue::F64(3.0)));
}

// spec: WDEF-002
#[test]
fn a_quoted_number_is_accepted_only_above_the_integer_range_and_in_canonical_form() {
    let load = |v: &str| parse_definition(&format!("schema_version = 1\n[world]\nseed = {v}\n"));
    assert_eq!(load("\"9223372036854775808\"").unwrap().get("world.seed"), Some(NodeValue::U64(1 << 63)));
    assert_eq!(load("\"18446744073709551615\"").unwrap().get("world.seed"), Some(NodeValue::U64(u64::MAX)));
    for small in ["\"42\"", "\"0042\"", "\"9223372036854775807\""] {
        assert!(matches!(load(small), Err(LoadError::NotCanonical { .. })), "{small}");
    }
    for bad in ["\"+5\"", "\" 5\"", "\"-1\"", "\"1e3\"", "\"\"", "\"18446744073709551616\""] {
        assert!(matches!(load(bad), Err(LoadError::Unsupported { .. })), "{bad}");
    }
}

// spec: WDEF-002
#[test]
fn names_the_file_format_owns_cannot_be_nodes() {
    for path in ["schema_version", "schema_version.x", "derived"] {
        let mut d = WorldDefinition::new(1);
        d.set(path, NodeValue::U64(5));
        assert!(matches!(to_toml(&d), Err(LoadError::BadPath { .. })), "{path}");
    }
    assert!(matches!(parse_definition("schema_version = 1\nderived = 3\n"), Err(LoadError::BadPath { .. })));
}

// spec: WDEF-002
#[test]
fn a_chain_with_two_steps_for_one_version_is_rejected() {
    let twice = [Migration { from: 1, apply: |_| Ok(()) }, Migration { from: 1, apply: |_| Ok(()) }];
    let e = parse_with(FIXTURE, 2, &twice).unwrap_err();
    assert!(matches!(&e, LoadError::Migration { from: 1, message } if message.contains("two migration steps")), "{e}");
    let missing = parse_with(FIXTURE, 3, &[]).unwrap_err();
    assert!(missing.to_string().contains("add the missing migration step"), "{missing}");
}

// spec: WDEF-002
#[test]
fn every_schema_version_has_a_frozen_fixture_and_exactly_one_migration() {
    // The structure behind "files of every older schema version load": a bumped version without its fixture and its
    // migration step fails here, not silently.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in 1..=SCHEMA_VERSION {
        let prefix = format!("v{n}_");
        let fixtures: Vec<&String> = names.iter().filter(|f| f.starts_with(&prefix) && f.ends_with(".toml")).collect();
        assert!(!fixtures.is_empty(), "schema version {n} has no frozen fixture tests/fixtures/{prefix}*.toml");
        for fx in fixtures {
            let text = std::fs::read_to_string(dir.join(fx)).unwrap();
            assert!(text.contains(&format!("schema_version = {n}")), "{fx} must be written in schema version {n}");
            assert_eq!(parse_definition(&text).unwrap(), planet(), "{fx} must load to the current definition of its content");
        }
    }
    let current = std::hint::black_box(SCHEMA_VERSION);
    for from in 1..current {
        assert_eq!(MIGRATIONS.iter().filter(|m| m.from == from).count(), 1, "exactly one migration step from version {from}");
    }
    assert_eq!(MIGRATIONS.len() as u32, SCHEMA_VERSION - 1);
}

// spec: WDEF-002
#[test]
fn a_sweep_of_float_bit_patterns_round_trips_exactly() {
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut bits: Vec<u64> = vec![
        1,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        (1u64 << 53) + 1,
        1e16f64.to_bits(),
        9.999999999999998e15f64.to_bits(),
        1e-5f64.to_bits(),
        9.999999999999999e-6f64.to_bits(),
        f64::MAX.to_bits(),
        f64::MIN_POSITIVE.to_bits(),
        0x8000_0000_0000_0001,
    ];
    for _ in 0..100_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        bits.push(x);
    }
    let mut d = WorldDefinition::new(1);
    let mut kept = 0;
    for (i, b) in bits.iter().enumerate() {
        let v = f64::from_bits(*b);
        if v.is_finite() {
            d.set(&format!("sweep.v{i}"), NodeValue::F64(v));
            kept += 1;
        }
    }
    assert!(kept > 90_000);
    let back = parse_definition(&to_toml(&d).unwrap()).unwrap();
    assert_eq!(back, d, "a float changed its bits through TOML text");
}
