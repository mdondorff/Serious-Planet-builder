//! AUTH-002: an item authored against an older stage version loads, re-keyed under the active one and written back; a newer one is refused.

use planet_editor::commands::{Editor, NewItem};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_world_def::item::Anchor;
use planet_world_def::item_store::{ItemStore, LooseFileStore};
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;
use std::path::PathBuf;

const R: f64 = 6_371_000.0;
const GENERATOR: u32 = 4;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("planet-editor-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn editor(dir: &PathBuf, seed: u64, stage: u32) -> Editor<LooseFileStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000 + seed)), Box::new(SeededEntropy(seed)));
    Editor::new(WorldDefinition::single_planet(R, 7), LooseFileStore::new(dir), ids, GENERATOR, stage)
}

fn new_item(i: u64) -> NewItem {
    let f = i as f64;
    NewItem {
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: 0,
        kind: "hill".into(),
        anchor: Anchor { latitude_deg: 10.0 + f, longitude_deg: 20.0 + f, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 900.0,
        max_delta_m: 50.0,
        params: BTreeMap::from([("height_m".to_string(), 20.0)]),
        name: None,
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
    }
}

// spec: AUTH-002
#[test]
fn an_item_of_an_older_stage_loads_re_keyed_and_written_back_and_one_of_a_newer_stage_is_refused() {
    let dir = temp_dir("stage-version");
    // Two items authored against stage version 1, files on disk.
    let mut old = editor(&dir, 31, 1);
    let (a, _) = old.put(new_item(1)).unwrap();
    let (b, _) = old.put(new_item(2)).unwrap();
    let before = old.item(a).unwrap().clone();
    assert_eq!(before.stage_version, 1);
    // Loaded by a build whose stage is version 2: both load, are re-keyed and reported, and the files say version 2 now.
    let mut new = editor(&dir, 1, 2);
    let report = new.load_report(BodyId::PLANET).unwrap();
    assert_eq!(report.loaded, 2);
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(report.restaged, want, "both are named, in id order");
    for id in [a, b] {
        assert_eq!(new.item(id).unwrap().stage_version, 2, "re-keyed under the active stage version");
        assert_eq!(new.store().get(BodyId::PLANET, id).unwrap().unwrap().stage_version, 2, "and written back");
    }
    assert_ne!(new.item(a).unwrap().hash(), before.hash(), "the recorded version is in the item hash");
    assert_eq!(new.item(a).unwrap().spec().content_hash, before.spec().content_hash, "the stamp it makes is the same");
    // A second load of the same files restages nothing.
    let mut again = editor(&dir, 2, 2);
    let report = again.load_report(BodyId::PLANET).unwrap();
    assert!(report.restaged.is_empty() && report.loaded == 2);
    // An item of a stage newer than the build's is refused naming it, and nothing is loaded.
    let mut behind = editor(&dir, 3, 1);
    let e = behind.load(BodyId::PLANET).unwrap_err().to_string();
    assert!(e.contains("stage version 2 (active 1") && e.contains("newer"), "{e}");
    assert!(behind.item_ids().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
