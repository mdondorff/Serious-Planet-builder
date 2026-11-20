//! Tier-A contracts for the code-generated pipeline catalog (`BUILD-063`) and live `HeightField::upstream` chain equivalence (`TEST-142`).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use planet_core::Vec3;
use planet_generators::chain::{field_chain_erosion, NoBake};
use planet_generators::height_fields::HeightFieldKind;
use planet_generators::pipeline_catalog::{
    metadata_erosion_switch, resolve_runtime_chain, stages, switches_for_definition, to_canonical_json, version_pins, walk_height_field,
};
use planet_generators::preview;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

fn stamp_item(id: u128, layer: &str, height_m: f64) -> ItemSpec {
    ItemSpec {
        id,
        layer: layer.to_string(),
        priority: 0,
        dir: Vec3::new(0.0, 0.0, 1.0),
        content_hash: 0,
        reach_m: 10_000.0,
        max_delta_m: height_m.abs(),
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [height_m, 0.0, 0.0]))
}

// spec: BUILD-063
#[test]
fn the_committed_pipeline_json_matches_the_code() {
    let want = to_canonical_json();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/stages/pipeline.json");
    if std::env::var_os("PLANET_UPDATE_STAGES").is_some() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("create docs/stages");
        }
        std::fs::write(&path, &want).expect("write docs/stages/pipeline.json");
        return;
    }
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("reading {}: {e}; run `cargo xtask stages --update` to generate it", path.display());
    });
    let have = raw.replace("\r\n", "\n");
    if have != want {
        let diff_line = have
            .lines()
            .zip(want.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| format!("first diff at line {}: committed=`{a}` vs code=`{b}`", i + 1))
            .unwrap_or_else(|| format!("line count mismatch: committed {} vs code {}", have.lines().count(), want.lines().count()));
        panic!(
            "docs/stages/pipeline.json is stale against crates/generators/src/pipeline_catalog.rs ({diff_line}); run `cargo xtask stages --update`"
        );
    }
}

// spec: BUILD-063
#[test]
fn catalog_pins_and_stage_dependencies_are_coherent() {
    let pins: BTreeSet<&str> = version_pins().into_iter().map(|p| p.key).collect();
    let stage_list = stages();
    let mut seen = BTreeSet::new();
    for s in &stage_list {
        assert!(seen.insert(s.id), "duplicate stage id {}", s.id);
        for dep in s.reads {
            assert!(seen.contains(dep), "stage {} reads '{dep}' which is missing or ordered after it", s.id);
        }
        for pin in s.version_pins {
            assert!(pins.contains(pin), "stage {} references unknown version pin '{pin}'", s.id);
        }
    }
}

// spec: BUILD-063
#[test]
fn preview_specs_name_the_exact_files_the_preview_renderers_produce() {
    let stage_list = stages();
    let world_stage = stage_list.iter().find(|s| s.id == "continents-and-plates").expect("continents-and-plates stage");
    let spec = world_stage.preview.expect("world preview spec");
    let fields = preview::compute(3, 64, 32, 32, 1);
    let mut actual: Vec<String> = preview::render(&fields).into_iter().map(|(name, _)| name).collect();
    actual.push("report.txt".to_string());
    actual.sort();
    let mut declared: Vec<String> = spec.files.iter().map(|s| (*s).to_string()).collect();
    declared.sort();
    assert_eq!(actual, declared, "continents-and-plates preview files drifted from preview::render");
}

// spec: TEST-142
#[test]
fn the_declared_runtime_chain_matches_the_walked_height_field_upstream_chain() {
    let bake_key = [7u8; 16];

    // 1. Plain WorldDefinition (no stamps) and WorldDefinition with out-of-order ADR 0007 + custom layers.
    let plain_def = || Arc::new(WorldDefinition::single_planet(R, 11));
    let stamped_def = || {
        let mut d = WorldDefinition::single_planet(R, 11);
        d.put_item_spec(BodyId::PLANET, stamp_item(1, "custom-top", 25.0)).unwrap();
        d.put_item_spec(BodyId::PLANET, stamp_item(2, "stamps", 40.0)).unwrap();
        d.put_item_spec(BodyId::PLANET, stamp_item(3, "intent", 15.0)).unwrap();
        Arc::new(d)
    };

    for build_def in [plain_def, stamped_def] {
        for kind in HeightFieldKind::ALL {
            for bound_water in [false, true] {
                let mut def = (*build_def()).clone();
                def.set_height_field(kind.id());
                let def = Arc::new(def);
                let switches = switches_for_definition(&def, bound_water, None).unwrap();
                let predicted = resolve_runtime_chain(&switches).unwrap();
                let field = field_chain_erosion(kind, &def, &NoBake, bake_key, bound_water, None).unwrap();
                let walked = walk_height_field(field.as_ref());
                assert_eq!(walked, predicted, "kind={kind:?} bound_water={bound_water}");
            }
        }
    }

    // 2. ErosionSwitch combinations over both eroded base kinds (with/without relief, with/without channels, with stamps & water bound).
    for kind in [HeightFieldKind::Eroded, HeightFieldKind::ErodedC1] {
        for relief in [false, true] {
            for channels in [false, true] {
                for bound_water in [false, true] {
                    let mut def = (*stamped_def()).clone();
                    def.set_height_field(kind.id());
                    let def = Arc::new(def);
                    let sw = metadata_erosion_switch(R, 0.0, relief, channels).unwrap();
                    let switches = switches_for_definition(&def, bound_water, Some(&sw)).unwrap();
                    let predicted = resolve_runtime_chain(&switches).unwrap();
                    let field = field_chain_erosion(kind, &def, &NoBake, bake_key, bound_water, Some(&sw)).unwrap();
                    let walked = walk_height_field(field.as_ref());
                    assert_eq!(walked, predicted, "kind={kind:?} relief={relief} channels={channels} bound_water={bound_water}");
                }
            }
        }
    }

    // 3. Invalid erosion flag combinations are refused by both the catalog resolver and the live chain builder.
    let sw = metadata_erosion_switch(R, 0.0, true, false).unwrap();
    for kind in [HeightFieldKind::Terrain, HeightFieldKind::Reference] {
        let mut def = (*plain_def()).clone();
        def.set_height_field(kind.id());
        let def = Arc::new(def);
        let switches = switches_for_definition(&def, false, Some(&sw)).unwrap();
        assert!(resolve_runtime_chain(&switches).is_err(), "{kind:?} must reject --erosion");
        assert!(field_chain_erosion(kind, &def, &NoBake, bake_key, false, Some(&sw)).is_err());
    }
}
