//! Tier A: the revisit trigger of the item store (ADR 0066 point 6, AUTH-006): the editor counts the items of a body and times each load, and
//! logs one actionable warning when a body holds more than 20,000 items or a load takes more than 2 s. The stores are synthetic and the load
//! time is injected: nothing here depends on a clock.

use planet_editor::commands::{Editor, NewItem};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::store_watch::{FixedTimer, ITEM_LIMIT, LOAD_LIMIT};
use planet_world_def::item::Anchor;
use planet_world_def::item_store::MemoryStore;
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;
use std::time::Duration;

const R: f64 = 6_371_000.0;

fn editor(store: MemoryStore, seed: u64) -> Editor<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    Editor::new(WorldDefinition::single_planet(R, 7), store, ids, 4, 2)
}

fn hill(i: u64) -> NewItem {
    let f = i as f64;
    NewItem {
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: 0,
        kind: "hill".into(),
        anchor: Anchor {
            latitude_deg: -60.0 + (f * 7.31) % 120.0,
            longitude_deg: -170.0 + (f * 13.7) % 340.0,
            height_m: 0.0,
            heading_deg: 0.0,
        },
        reach_m: 200.0,
        max_delta_m: 20.0,
        params: BTreeMap::from([("radius_m".to_string(), 100.0), ("height_m".to_string(), 10.0)]),
        name: None,
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
        strokes: Vec::new(),
    }
}

/// A store holding `n` items, made by an editor's puts (the store is written through).
fn store_of(n: usize) -> MemoryStore {
    let mut e = editor(MemoryStore::new(), 1);
    for i in 0..n {
        e.put(hill(i as u64)).unwrap();
    }
    e.store().clone()
}

fn loaded(store: MemoryStore, took: Duration) -> Editor<MemoryStore> {
    let mut e = editor(store, 2).with_load_timer(Box::new(FixedTimer(took)));
    e.load(BodyId::PLANET).unwrap();
    e
}

// spec: AUTH-006
#[test]
fn a_store_of_19_999_or_20_000_items_loads_without_a_warning_and_one_of_20_001_warns_once_with_the_count() {
    for n in [ITEM_LIMIT - 1, ITEM_LIMIT] {
        let e = loaded(store_of(n), Duration::from_millis(10));
        assert!(e.store_watch().issued().is_empty(), "{n} items: {:?}", e.store_watch().issued());
        assert_eq!(e.store_watch().max_items(), n);
    }
    let e = loaded(store_of(ITEM_LIMIT + 1), Duration::from_millis(10));
    let issued = e.store_watch().issued();
    assert_eq!(issued.len(), 1, "{issued:?}");
    assert!(
        issued[0].contains("20001 items") && issued[0].contains("20000") && issued[0].contains("ADR 0066 point 6: revisit the backend"),
        "{issued:?}"
    );
    assert!(e.store_watch().summary().contains("store: 20001 items (limit 20000)") && e.store_watch().summary().contains("REVISIT"));
}

// spec: AUTH-006
#[test]
fn adding_the_20_001st_item_warns_once_and_removing_it_clears_the_mark() {
    let mut e = editor(MemoryStore::new(), 3);
    for i in 0..ITEM_LIMIT {
        e.put(hill(i as u64)).unwrap();
    }
    assert!(e.store_watch().issued().is_empty(), "20,000 items are not over the limit");
    e.put(hill(ITEM_LIMIT as u64)).unwrap();
    assert_eq!(e.store_watch().issued().len(), 1, "{:?}", e.store_watch().issued());
    e.put(hill(ITEM_LIMIT as u64 + 1)).unwrap();
    assert_eq!(e.store_watch().issued().len(), 1, "once per crossing");
    e.undo().unwrap();
    e.undo().unwrap();
    assert!(!e.store_watch().any_over(), "back at 20,000");
    e.redo().unwrap();
    assert_eq!(e.store_watch().issued().len(), 2, "crossing again warns again");
}

// spec: AUTH-006
#[test]
fn a_slow_load_warns_with_the_time_and_a_load_at_the_limit_does_not() {
    let at_limit = loaded(store_of(5), LOAD_LIMIT);
    assert!(at_limit.store_watch().issued().is_empty());
    assert_eq!(at_limit.store_watch().last_load(), Some(LOAD_LIMIT));
    let slow = loaded(store_of(5), Duration::from_millis(2_100));
    let issued = slow.store_watch().issued();
    assert_eq!(issued.len(), 1, "{issued:?}");
    assert!(issued[0].contains("took 2100 ms") && issued[0].contains("2000 ms") && issued[0].contains("revisit the backend"), "{issued:?}");
    assert!(slow.store_watch().summary().contains("last load 2100 ms (limit 2000 ms)"));
    // A big and slow store gives both warnings, the count first.
    let both = loaded(store_of(ITEM_LIMIT + 1), Duration::from_secs(3));
    assert_eq!(both.store_watch().issued().len(), 2);
}
