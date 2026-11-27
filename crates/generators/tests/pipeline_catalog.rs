//! Tier-A contracts for the code-generated pipeline catalog (`BUILD-063`) and registry/chain equivalence (`TEST-142`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use planet_core::Vec3;
use planet_generators::bake::probe::Stage;
use planet_generators::chain::{field_chain_erosion, NoBake};
use planet_generators::height_fields::HeightFieldKind;
use planet_generators::pipeline_catalog::{
    metadata_erosion_switch, resolve_runtime_chain, stage_id_for_probe, stages, switches_for_commit, switches_for_definition,
    to_canonical_json, version_pins, walk_height_field,
};
use planet_generators::preview;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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
fn catalog_json_writes_to_target_review_and_is_deterministic() {
    let root = repo_root();
    assert!(
        !root.join("docs/tools/stages/pipeline.json").exists(),
        "docs/tools/stages/pipeline.json should not be committed; generate into target/review/stages/pipeline.json or via --pipeline-json"
    );
    let json = to_canonical_json();
    assert_eq!(json, to_canonical_json(), "catalog JSON must be deterministic");
    for needle in [
        "\"k_label\": \"K5\"",
        "\"k_label\": \"K6\"",
        "\"k_label\": \"K7\"",
        "\"k_label\": \"M9-P1\"",
        "\"k_label\": \"M9-P2\"",
        "\"scope\": \"global\"",
        "\"scope\": \"region\"",
        "\"scope\": \"tile\"",
        "\"probe_stage_kinds\":",
        "\"k5-landmass-and-window-cut\"",
    ] {
        assert!(json.contains(needle), "missing {needle} in catalog JSON");
    }
    let out = root.join("target/review/stages/pipeline.json");
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).expect("create target/review/stages");
    }
    std::fs::write(&out, &json).expect("write target/review/stages/pipeline.json");
}

// spec: BUILD-063
#[test]
fn catalog_pins_and_stage_dependencies_are_coherent() {
    let pins: BTreeSet<&str> = version_pins().into_iter().map(|p| p.key).collect();
    let stage_list = stages();
    let mut seen = BTreeSet::new();
    for s in &stage_list {
        assert!(seen.insert(s.id), "duplicate stage id {}", s.id);
        assert!(!s.k_label.is_empty(), "stage {} missing k_label", s.id);
        assert!(matches!(s.scope.as_str(), "global" | "region" | "tile"));
        for dep in s.reads {
            assert!(seen.contains(dep), "stage {} reads '{dep}' which is missing or ordered after it", s.id);
        }
        for pin in s.version_pins {
            assert!(pins.contains(pin), "stage {} references unknown version pin '{pin}'", s.id);
        }
    }
}

/// Active non-deprecated requirement IDs declared in main OpenSpec specs (`openspec/specs/*/spec.md`).
fn live_openspec_requirement_ids(root: &Path) -> BTreeSet<String> {
    let specs_dir = root.join("openspec/specs");
    let mut active = BTreeSet::new();
    let entries = std::fs::read_dir(&specs_dir).unwrap_or_else(|e| panic!("reading {}: {e}", specs_dir.display()));
    for entry in entries.flatten() {
        let spec_file = entry.path().join("spec.md");
        if !spec_file.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&spec_file).unwrap_or_else(|e| panic!("reading {}: {e}", spec_file.display()));
        let mut current: Option<String> = None;
        let mut deprecated = false;
        let mut removed = false;
        let flush = |cur: &mut Option<String>, dep: &mut bool, set: &mut BTreeSet<String>| {
            if let Some(id) = cur.take() {
                if !*dep {
                    set.insert(id);
                }
            }
            *dep = false;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(h) = trimmed.strip_prefix("## ") {
                flush(&mut current, &mut deprecated, &mut active);
                removed = h.trim_start().starts_with("REMOVED");
            } else if let Some(rest) = trimmed.strip_prefix("### Requirement:") {
                flush(&mut current, &mut deprecated, &mut active);
                if !removed {
                    if let Some(id) = rest.split_whitespace().next() {
                        current = Some(id.to_string());
                    }
                }
            } else if current.is_some() && trimmed.strip_prefix("Status:").is_some_and(|s| s.trim() == "deprecated") {
                deprecated = true;
            }
        }
        flush(&mut current, &mut deprecated, &mut active);
    }
    active
}

// spec: BUILD-063, TEST-142
#[test]
fn every_spec_id_in_catalog_resolves_to_a_live_requirement_in_openspec_specs() {
    let live = live_openspec_requirement_ids(&repo_root());
    assert!(!live.is_empty(), "expected live requirements in openspec/specs/");
    for pin in version_pins() {
        assert!(!pin.spec_ids.is_empty(), "version pin '{}' cites no spec_ids", pin.key);
        for id in pin.spec_ids {
            assert!(live.contains(*id), "version pin '{}' cites '{id}', which is not a live requirement in openspec/specs/", pin.key);
        }
    }
    for stage in stages() {
        assert!(!stage.spec_ids.is_empty(), "stage '{}' cites no spec_ids", stage.id);
        for id in stage.spec_ids {
            assert!(live.contains(*id), "stage '{}' cites '{id}', which is not a live requirement in openspec/specs/", stage.id);
        }
    }
}

// spec: BUILD-063, TEST-142
#[test]
fn bake_stage_hooks_match_probe_registry_and_committed_stage_manifest() {
    let stage_list = stages();

    // 1. Every variant of `probe::Stage::ALL_KINDS` maps through `stage_id_for_probe` to a stage declaring that hook.
    for probe_stage in Stage::ALL_KINDS {
        let stage_id = stage_id_for_probe(probe_stage);
        let entry = stage_list.iter().find(|s| s.id == stage_id).unwrap_or_else(|| {
            panic!("stage_id_for_probe({probe_stage:?}) returned '{stage_id}', missing from pipeline_catalog::stages()");
        });
        assert!(
            entry.probe_hooks.iter().any(|h| h.stage.kind_id() == probe_stage.kind_id()),
            "stage '{stage_id}' missing ProbeHook for kind '{}'",
            probe_stage.kind_id()
        );
    }

    // 2. The ordered `(stage_kind, field)` pairs declared on `stages()` match `docs/tools/stages/manifest.txt`.
    let declared_pairs: Vec<(String, String)> = stage_list
        .iter()
        .flat_map(|s| s.probe_hooks.iter().flat_map(|h| h.fields.iter().map(move |f| (h.stage.kind_id().to_string(), (*f).to_string()))))
        .collect();

    let manifest_path = repo_root().join("docs/tools/stages/manifest.txt");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| panic!("reading {}: {e}", manifest_path.display()));
    let mut manifest_pairs: Vec<(String, String)> = Vec::new();
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let mut cols = line.split(" | ");
        let raw_stage = cols.next().expect("stage column").trim();
        let field = cols.next().expect("field column").trim();
        let kind_id = if raw_stage.starts_with("round-") {
            "round"
        } else if raw_stage.starts_with("k7-") {
            "k7"
        } else {
            raw_stage
        };
        let pair = (kind_id.to_string(), field.to_string());
        if !manifest_pairs.contains(&pair) {
            manifest_pairs.push(pair);
        }
    }
    assert_eq!(declared_pairs, manifest_pairs, "pipeline_catalog probe_hooks drifted from docs/tools/stages/manifest.txt");
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

    let plain_def = || Arc::new(WorldDefinition::single_planet(R, 11));
    let stamped_def = || {
        let mut d = WorldDefinition::single_planet(R, 11);
        d.put_item_spec(BodyId::PLANET, stamp_item(1, "custom-top", 25.0)).unwrap();
        d.put_item_spec(BodyId::PLANET, stamp_item(2, "stamps", 40.0)).unwrap();
        d.put_item_spec(BodyId::PLANET, stamp_item(3, "intent", 15.0)).unwrap();
        Arc::new(d)
    };

    // 1. All HeightFieldKind variants without erosion layer.
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

    // 2. M9 ErosionSwitch combinations over both eroded base kinds.
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

    // 3. K5-K7 commit configuration (`use_commit`: C1 base + layer, no relief channels).
    let def = stamped_def();
    let sw_commit = metadata_erosion_switch(R, 0.0, false, false).unwrap();
    let commit_switches = switches_for_commit(&def, true).unwrap();
    let commit_predicted = resolve_runtime_chain(&commit_switches).unwrap();
    let commit_field = field_chain_erosion(HeightFieldKind::ErodedC1, &def, &NoBake, bake_key, true, Some(&sw_commit)).unwrap();
    assert_eq!(walk_height_field(commit_field.as_ref()), commit_predicted);

    // 4. Invalid erosion flag combinations are refused by both the catalog resolver and the live chain builder.
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
