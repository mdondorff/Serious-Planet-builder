//! Tier A: the look file is outside the world definition's hash and every cache key (ADR 0042 point 6, CON-05, CON-09): loading a
//! look file, changing it, or holding any number of bodies' looks changes no definition hash, no read set and no key, and a look
//! file lying beside a definition file is never read as part of it.

use planet_cache::tiles::World;
use planet_core::TileId;
use planet_frame::look::file::*;
use planet_frame::look::{HistogramParams, SceneUnits};
use planet_world_def::toml_io::{parse_definition, to_toml as definition_to_toml};
use planet_world_def::{BodyId, GenContext, NodeValue, WorldDefinition};

/// Everything a definition feeds into caching, as plain comparable data.
#[derive(Debug, PartialEq)]
struct Fingerprint {
    hash: u64,
    base_hash: u64,
    text: String,
    node_set_id: [u8; 16],
    store_params: String,
    tile_keys: Vec<String>,
    read_set: Vec<(String, u64)>,
}

fn fingerprint(def: &WorldDefinition) -> Fingerprint {
    let world = World::from_definition(def).expect("a single planet definition has the nodes the world reads");
    let tiles = [TileId::new(planet_core::Face(0), 0, 0, 0).unwrap(), TileId::new(planet_core::Face(2), 5, 3, 7).unwrap()];
    let ctx = GenContext::new(def);
    ctx.body_seed(BodyId::PLANET).unwrap();
    ctx.body_radius_m(BodyId::PLANET).unwrap();
    Fingerprint {
        hash: def.hash(),
        base_hash: def.base_hash(),
        text: definition_to_toml(def).unwrap(),
        node_set_id: world.node_set_id(),
        store_params: format!("{:?}", world.params()),
        tile_keys: tiles.iter().flat_map(|t| [world.tile_key(*t, 32).to_hex(), world.tile_key(*t, 64).to_hex()]).collect(),
        read_set: ctx.reads(),
    }
}

fn wild_look() -> LookFile {
    let mut f = LookFile::empty();
    for (body, lux) in [(0u32, 3.0e5), (1, 9.0e3), (7, 12.5)] {
        let p = LookParams {
            units: SceneUnits { solar_illuminance_lux: lux },
            exposure: ExposureLook { min_ev100: -9.0, max_ev100: 33.0, speed_up: 50.0, speed_down: 7.5 },
            histogram: HistogramParams { ev_min: -30.0, ev_max: 40.0, band_low: 0.05, band_high: 0.95 },
            ..LookParams::default()
        };
        f.set_body(BodyId(body), p).unwrap();
    }
    f
}

// spec: LOOK-004
#[test]
fn loading_or_changing_a_look_file_changes_no_definition_hash_and_no_cache_key() {
    let def = WorldDefinition::single_planet(6_371_000.0, 1);
    let before = fingerprint(&def);
    assert_eq!(before.tile_keys.len(), 4);
    // The known answer of WDEF-003 (recorded before the look file existed): the definition hashes as it always did.
    assert_eq!(before.hash, 6_345_583_285_605_900_286);

    // Load a look file (the default, a wild one, a third) between the two fingerprints, and keep them alive: nothing changes.
    let defaults = parse_look("schema_version = 1\nkind = \"look\"\n").unwrap();
    let wild = parse_look(&to_toml(&wild_look())).unwrap();
    assert_ne!(defaults, wild, "the looks really differ");
    assert_ne!(to_toml(&defaults), to_toml(&wild));
    for look in [&defaults, &wild] {
        assert_eq!(fingerprint(&def), before, "a loaded look ({} entries) re-keys nothing", look.bodies().count());
    }

    // A definition re-read from its own text, with the look file lying beside it, is the same definition.
    let reread = parse_definition(&before.text).unwrap();
    assert_eq!(fingerprint(&reread), before);

    // Control: the fingerprint does react to what is part of the definition, so the equalities above can fail.
    let mut changed = def.clone();
    changed.set("body.0.radius_m", NodeValue::F64(6_371_001.0));
    let after = fingerprint(&changed);
    assert_ne!(after.hash, before.hash);
    assert_ne!(after.tile_keys, before.tile_keys);
}

// spec: LOOK-004
#[test]
fn a_look_file_cannot_be_read_as_part_of_a_definition() {
    // The look file's text is not a definition (its `kind` is a string), and a definition's text is not a look file, so neither can
    // stand in for the other beside it.
    let text = to_toml(&wild_look());
    assert!(parse_definition(&text).is_err());
    let def_text = definition_to_toml(&WorldDefinition::single_planet(6_371_000.0, 42)).unwrap();
    assert!(parse_look(&def_text).is_err());
    // The definition crate names nothing of the look: its node namespace has no look path to set by accident.
    assert!(WorldDefinition::single_planet(1.0, 1).nodes().all(|(p, _)| !p.contains("look") && !p.contains("exposure")));
}
