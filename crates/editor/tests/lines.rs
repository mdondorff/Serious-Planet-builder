//! Tier A: lines and regions in the command layer (VIEW-028, ADR 0069): a line longer than one item reaches is put as linked parts in one
//! command, re-split when its points change, deleted whole, and each of these undoes and redoes as one step; a region is refused beyond one
//! reach; a group that fails halfway leaves nothing behind.

use planet_editor::commands::{EditError, Editor, GroupStep, NewItem};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::polyline::{GeoPoint, PolylineError};
use planet_world_def::cells::CELL_EDGE_M;
use planet_world_def::item::{Anchor, Item};
use planet_world_def::item_store::{ItemStore, MemoryStore, StoreError};
use planet_world_def::{BodyId, ItemId, WorldDefinition};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

const R: f64 = 6_371_000.0;

fn editor<S: ItemStore>(store: S) -> Editor<S> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(5)));
    Editor::new(WorldDefinition::single_planet(R, 7), store, ids, 4, 2)
}

fn east(east_m: f64) -> GeoPoint {
    GeoPoint::new(0.0, east_m / R * 180.0 / std::f64::consts::PI)
}

fn ridge() -> NewItem {
    NewItem {
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "stamps".into(),
        priority: 0,
        kind: "ridge".into(),
        anchor: Anchor { latitude_deg: 0.0, longitude_deg: 0.0, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 1.0,
        max_delta_m: 60.0,
        params: BTreeMap::from([("amount_m".to_string(), 30.0), ("width_m".to_string(), 200.0)]),
        name: Some("ridge".into()),
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
        strokes: Vec::new(),
    }
}

fn region() -> NewItem {
    NewItem {
        kind: "region".into(),
        params: BTreeMap::from([("surface_type".to_string(), 2.0), ("mask_mode".to_string(), 0.0)]),
        max_delta_m: 0.0,
        name: Some("meadow".into()),
        ..ridge()
    }
}

fn long_line() -> Vec<GeoPoint> {
    (0..=15).map(|k| east(k as f64 * 10_000.0)).collect()
}

fn assert_store_matches<S: ItemStore>(ed: &Editor<S>, what: &str) {
    let in_store = ed.store().load_all(BodyId::PLANET).unwrap();
    let in_editor: Vec<Item> = ed.item_ids().iter().map(|id| ed.item(*id).unwrap().clone()).collect();
    assert_eq!(in_store, in_editor, "{what}: the files are the items");
    assert_eq!(ed.definition().item_ids(BodyId::PLANET), ed.item_ids(), "{what}: the definition holds the same items");
}

// spec: VIEW-028
#[test]
fn a_line_longer_than_one_reach_is_put_as_linked_parts_in_one_command_and_undone_whole() {
    let mut ed = editor(MemoryStore::new());
    let start = ed.hash();
    let (ids, affected) = ed.put_line(ridge(), &long_line()).unwrap();
    assert!(ids.len() >= 4, "{} parts", ids.len());
    assert_eq!(ed.undo_depth(), 1, "one command for all the parts");
    assert_eq!(affected.len(), ids.len(), "a place to refresh for every part");
    let group = ed.item(ids[0]).unwrap().group.expect("a split line has a group");
    for (k, id) in ids.iter().enumerate() {
        let item = ed.item(*id).unwrap();
        assert_eq!((item.group, item.part), (Some(group), Some(k as u32)));
        assert_eq!(item.anchor.latitude_deg, item.points[0].0);
        assert!(item.reach_m <= CELL_EDGE_M, "reach {}", item.reach_m);
        assert_eq!(item.params["width_m"], 200.0);
    }
    for w in ids.windows(2) {
        assert_eq!(ed.item(w[0]).unwrap().points.last(), ed.item(w[1]).unwrap().points.first(), "parts share their boundary point");
    }
    assert_eq!(ed.line_items(ids[2]).unwrap(), ids, "any part names the whole line");
    let joined = ed.line_points(ids[1]).unwrap();
    assert_eq!(joined.first(), Some(&GeoPoint::new(0.0, 0.0)));
    assert!(joined.len() >= long_line().len(), "the joined points hold the original ones");
    assert_store_matches(&ed, "after the put");
    let end = ed.hash();
    assert_ne!(end, start);

    ed.undo().unwrap().expect("one undo");
    assert_eq!((ed.hash(), ed.item_ids().len()), (start, 0), "one undo takes every part back");
    assert!(!ed.can_undo());
    assert_store_matches(&ed, "after the undo");
    ed.redo().unwrap().expect("one redo");
    assert_eq!(ed.hash(), end, "one redo does them all again, to the same hash");
    assert_store_matches(&ed, "after the redo");
}

// spec: VIEW-028
#[test]
fn a_short_line_is_one_item_without_a_group() {
    let mut ed = editor(MemoryStore::new());
    let (ids, _) = ed.put_line(ridge(), &[east(0.0), east(5_000.0), east(9_000.0)]).unwrap();
    assert_eq!(ids.len(), 1);
    let item = ed.item(ids[0]).unwrap();
    assert_eq!((item.group, item.part, item.points.len()), (None, None, 3));
    assert!(item.reach_m >= 9_000.0 + 200.0 && item.reach_m <= 9_000.0 + 200.0 + 10.0, "{}", item.reach_m);
    // The width is needed.
    let mut no_width = ridge();
    no_width.params.remove("width_m");
    let e = ed.put_line(no_width, &[east(0.0), east(5_000.0)]).unwrap_err();
    assert!(e.to_string().contains("width_m"), "{e}");
    assert_eq!(ed.item_ids().len(), 1);
}

// spec: VIEW-028
#[test]
fn moving_a_point_re_splits_the_line_changing_putting_and_deleting_parts_as_one_command() {
    let mut ed = editor(MemoryStore::new());
    let (ids, _) = ed.put_line(ridge(), &long_line()).unwrap();
    let before = ed.hash();
    let depth = ed.undo_depth();

    // The same points moved a little: the same parts, changed in place.
    let mut moved = ed.line_points(ids[0]).unwrap();
    moved[3] = GeoPoint::new(0.01, moved[3].lon_deg);
    let (after_ids, _) = ed.set_line_points(ids[1], &moved).unwrap();
    assert_eq!(ed.undo_depth(), depth + 1);
    assert_eq!(ed.item_ids().len(), ids.len(), "as many parts as before");
    assert!(ids.iter().all(|id| ed.item(*id).is_some()), "the parts keep their ids");
    assert!(after_ids.is_empty(), "nothing was put");
    ed.undo().unwrap();
    assert_eq!(ed.hash(), before, "undo restores the parts exactly");

    // Shortened to one item: the first part stays (without a group), the others go, in one command.
    let (kept, _) = ed.set_line_points(ids[2], &[east(0.0), east(4_000.0)]).unwrap();
    assert!(kept.is_empty());
    assert_eq!(ed.item_ids(), vec![ids[0]], "only the first part is left");
    let only = ed.item(ids[0]).unwrap();
    assert_eq!((only.group, only.part, only.points.len()), (None, None, 2));
    assert_store_matches(&ed, "shortened");
    ed.undo().unwrap();
    assert_eq!(ed.hash(), before, "one undo brings every part back");
    assert_store_matches(&ed, "after undoing the shortening");

    // Lengthened: parts are put.
    let mut longer = ed.line_points(ids[0]).unwrap();
    longer.push(east(190_000.0));
    longer.push(east(230_000.0));
    let (put, _) = ed.set_line_points(ids[0], &longer).unwrap();
    assert!(!put.is_empty(), "more parts");
    assert_eq!(ed.item_ids().len(), ids.len() + put.len());
    let group = ed.item(ids[0]).unwrap().group;
    assert!(group.is_some() && ed.line_items(ids[0]).unwrap().iter().all(|id| ed.item(*id).unwrap().group == group));
    ed.undo().unwrap();
    assert_eq!(ed.hash(), before);

    // A refused point list changes nothing.
    let (single, _) = ed.put_line(ridge(), &[east(0.0), east(1_000.0)].map(|p| GeoPoint::new(p.lat_deg + 10.0, p.lon_deg))).unwrap();
    let h = ed.hash();
    let e = ed.set_line_points(single[0], &[east(0.0), east(0.0)]).unwrap_err();
    assert!(matches!(e, EditError::Geometry(PolylineError::Repeated { .. })), "{e}");
    assert_eq!(ed.hash(), h);
}

// spec: VIEW-028
#[test]
fn deleting_a_line_removes_all_its_parts_in_one_command() {
    let mut ed = editor(MemoryStore::new());
    let (ids, _) = ed.put_line(ridge(), &long_line()).unwrap();
    let (other, _) = ed.put_line(ridge(), &[GeoPoint::new(20.0, 0.0), GeoPoint::new(20.0, 0.05)]).unwrap();
    let before = ed.hash();
    let depth = ed.undo_depth();
    let affected = ed.delete_line(ids[1]).unwrap();
    assert_eq!(affected.len(), ids.len());
    assert_eq!(ed.undo_depth(), depth + 1);
    assert_eq!(ed.item_ids(), other, "only the other line is left");
    assert_store_matches(&ed, "after the delete");
    ed.undo().unwrap();
    assert_eq!(ed.hash(), before);
    assert_store_matches(&ed, "after the undo");
}

// spec: VIEW-028
#[test]
fn a_group_that_fails_halfway_leaves_the_definition_the_files_and_the_stack_as_they_were() {
    struct Failing {
        inner: MemoryStore,
        puts_left: Rc<Cell<u32>>,
    }
    impl ItemStore for Failing {
        fn put(&mut self, item: &Item) -> Result<(), StoreError> {
            if self.puts_left.get() == 0 {
                return Err(StoreError::Io { what: "writing", path: PathBuf::from("items/x.toml"), message: "disk full".into() });
            }
            self.puts_left.set(self.puts_left.get() - 1);
            self.inner.put(item)
        }
        fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
            self.inner.delete(body, id)
        }
        fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
            self.inner.get(body, id)
        }
        fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
            self.inner.load_all(body)
        }
    }
    let left = Rc::new(Cell::new(1_000));
    let mut ed = editor(Failing { inner: MemoryStore::new(), puts_left: left.clone() });
    ed.put_line(ridge(), &[east(0.0), east(3_000.0)]).unwrap();
    let (hash, depth, items) = (ed.hash(), ed.undo_depth(), ed.item_ids());
    left.set(2);
    let e = ed.put_line(ridge(), &long_line()).unwrap_err();
    assert!(matches!(e, EditError::Store(_)), "{e}");
    assert_eq!((ed.hash(), ed.undo_depth(), ed.item_ids()), (hash, depth, items), "nothing of the group is left");
    assert_eq!(ed.definition().item_ids(BodyId::PLANET), ed.item_ids());
    assert_eq!(ed.store().load_all(BodyId::PLANET).unwrap().len(), 1, "no file of the group is left");

    // A bad step (an invalid item) refuses the whole group before anything is written; two steps on one item are refused.
    left.set(1_000);
    let mut bad = ridge();
    bad.params.insert("amount_m".into(), -1.0);
    let e = ed.group(vec![GroupStep::Put(ridge_with_points()), GroupStep::Put(bad)]).unwrap_err();
    assert!(matches!(e, EditError::Invalid(_)), "{e}");
    assert_eq!(ed.hash(), hash);
    let id = ed.item_ids()[0];
    let e = ed.group(vec![GroupStep::Delete(id), GroupStep::Delete(id)]).unwrap_err();
    assert!(e.to_string().contains("two steps"), "{e}");
    assert_eq!(ed.hash(), hash);
    // A group that changes nothing is not a command.
    let (none, affected) = ed.group(Vec::new()).unwrap();
    assert!((none.len(), affected.len(), ed.undo_depth()) == (0, 0, depth));
}

fn ridge_with_points() -> NewItem {
    NewItem { points: vec![(0.0, 0.0), (0.0, 0.01)], reach_m: 1_500.0, ..ridge() }
}

// spec: VIEW-028
#[test]
fn a_region_is_put_over_its_points_refused_beyond_one_reach_and_re_pointed_in_one_change() {
    let mut ed = editor(MemoryStore::new());
    let d = |m: f64| m / R * 180.0 / std::f64::consts::PI;
    let square = |side: f64| {
        vec![GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, d(side)), GeoPoint::new(d(side), d(side)), GeoPoint::new(d(side), 0.0)]
    };
    let (id, affected) = ed.put_region(region(), &square(10_000.0)).unwrap();
    assert_eq!(affected.len(), 1);
    let item = ed.item(id).unwrap().clone();
    assert_eq!((item.kind.as_str(), item.points.len(), item.group, item.part), ("region", 4, None, None));
    assert!(item.reach_m >= 14_100.0 && item.reach_m <= 14_200.0, "the far corner is 14.1 km away: {}", item.reach_m);

    let h = ed.hash();
    let e = ed.put_region(region(), &square(50_000.0)).unwrap_err();
    assert!(matches!(e, EditError::Geometry(PolylineError::RegionTooLarge { limit_m, .. }) if limit_m == CELL_EDGE_M), "{e}");
    assert!(e.to_string().contains("40 km"), "{e}");
    let mut bow = square(10_000.0);
    bow.swap(2, 3);
    assert!(matches!(ed.put_region(region(), &bow).unwrap_err(), EditError::Geometry(PolylineError::SelfCrossing { .. })));
    assert_eq!((ed.hash(), ed.item_ids().len()), (h, 1), "a refused region changes nothing");

    let depth = ed.undo_depth();
    ed.set_region_points(id, &square(8_000.0)).unwrap();
    assert_eq!(ed.undo_depth(), depth + 1, "one change");
    let moved = ed.item(id).unwrap().clone();
    assert_eq!((moved.name.as_deref(), moved.params["surface_type"]), (Some("meadow"), 2.0), "type and name stay");
    assert!(moved.reach_m < item.reach_m);
    ed.undo().unwrap();
    assert_eq!(ed.hash(), h);
    assert_store_matches(&ed, "after the undo");
}

// spec: VIEW-028
#[test]
fn points_are_written_in_the_item_file_spelling_and_a_line_command_refuses_what_is_not_a_line() {
    let mut ed = editor(MemoryStore::new());
    // Longitude -180 and a pole at any longitude are spelled as an item file takes them.
    let (ids, _) = ed.put_line(ridge(), &[GeoPoint::new(0.0, -180.0), GeoPoint::new(0.0, -179.9)]).unwrap();
    assert_eq!(ed.item(ids[0]).unwrap().points[0], (0.0, 180.0));
    assert_eq!(ed.item(ids[0]).unwrap().anchor.longitude_deg, 180.0);
    let (pole, _) = ed.put_line(ridge(), &[GeoPoint::new(90.0, 40.0), GeoPoint::new(89.99, 40.0)]).unwrap();
    assert_eq!(ed.item(pole[0]).unwrap().points[0], (90.0, 0.0));

    // An unsplit line takes no group id: two puts in a row get ids two apart only by the ids of the items themselves.
    let mut other = editor(MemoryStore::new());
    let (a, _) = other.put_line(ridge(), &[east(0.0), east(1_000.0)]).unwrap();
    let (b, _) = other.put_line(ridge(), &[east(5_000.0), east(6_000.0)]).unwrap();
    let mut third = editor(MemoryStore::new());
    let (c, _) = third.put(NewItem { points: vec![(0.0, 0.0), (0.0, 0.0089)], reach_m: 1_500.0, ..ridge() }).unwrap();
    let at = Anchor { latitude_deg: 0.0, longitude_deg: 0.05, height_m: 0.0, heading_deg: 0.0 };
    let (d, _) = third.put(NewItem { anchor: at, points: vec![(0.0, 0.05), (0.0, 0.06)], reach_m: 1_500.0, ..ridge() }).unwrap();
    assert_eq!((a[0], b[0]), (c, d), "put_line made no id for a group an unsplit line does not have");

    // What is not a line is refused by the line commands, and a hill by the region ones.
    let (hill, _) = ed
        .put(NewItem {
            kind: "hill".into(),
            params: BTreeMap::from([("height_m".to_string(), 10.0)]),
            max_delta_m: 10.0,
            points: Vec::new(),
            reach_m: 300.0,
            ..ridge()
        })
        .unwrap();
    let h = ed.hash();
    assert!(ed.delete_line(hill).unwrap_err().to_string().contains("not a line"));
    assert!(ed.set_line_points(hill, &[east(0.0), east(10.0)]).unwrap_err().to_string().contains("not a line"));
    assert!(ed
        .put_line(NewItem { kind: "hill".into(), ..ridge() }, &[east(0.0), east(10.0)])
        .unwrap_err()
        .to_string()
        .contains("not a line"));
    assert!(ed.put_region(ridge(), &[east(0.0), east(10.0), GeoPoint::new(0.001, 0.0)]).unwrap_err().to_string().contains("not a region"));
    assert_eq!(ed.hash(), h);
}

// spec: VIEW-028
#[test]
fn a_group_of_changes_that_fails_once_halfway_is_taken_back_step_by_step_to_the_exact_state() {
    /// Fails the write number `fail_at` (counting from 1) once; every other write works, the rollback's too.
    struct FailsOnce {
        inner: MemoryStore,
        writes: Rc<Cell<u32>>,
        fail_at: Rc<Cell<u32>>,
    }
    impl ItemStore for FailsOnce {
        fn put(&mut self, item: &Item) -> Result<(), StoreError> {
            self.writes.set(self.writes.get() + 1);
            if self.writes.get() == self.fail_at.get() {
                return Err(StoreError::Io { what: "writing", path: PathBuf::from("items/x.toml"), message: "disk full".into() });
            }
            self.inner.put(item)
        }
        fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
            self.inner.delete(body, id)
        }
        fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
            self.inner.get(body, id)
        }
        fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
            self.inner.load_all(body)
        }
    }
    let (writes, fail_at) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let mut ed = editor(FailsOnce { inner: MemoryStore::new(), writes: writes.clone(), fail_at: fail_at.clone() });
    let (ids, _) = ed.put_line(ridge(), &long_line()).unwrap();
    let (hash, depth, files) = (ed.hash(), ed.undo_depth(), ed.store().load_all(BodyId::PLANET).unwrap());
    let mut moved = ed.line_points(ids[0]).unwrap();
    moved[1] = GeoPoint::new(0.02, moved[1].lon_deg);
    moved[9] = GeoPoint::new(0.02, moved[9].lon_deg);
    // The parts that change are written in order; the third write fails, the two before it are written back.
    fail_at.set(writes.get() + 3);
    let e = ed.set_line_points(ids[0], &moved).unwrap_err();
    assert!(matches!(e, EditError::Store(_)), "a rollback that worked says only what failed: {e:?}");
    assert_eq!((ed.hash(), ed.undo_depth()), (hash, depth), "the definition and the stack are as they were");
    assert_eq!(ed.store().load_all(BodyId::PLANET).unwrap(), files, "so are the files");
    assert_store_matches(&ed, "after the rollback");
    // The editor takes commands as before.
    ed.set_line_points(ids[0], &moved).unwrap();
    ed.undo().unwrap();
    assert_eq!(ed.hash(), hash);
}

// spec: VIEW-028
#[test]
fn a_rollback_that_fails_is_named_and_the_editor_refuses_commands_until_the_items_are_reloaded() {
    struct Failing {
        inner: MemoryStore,
        puts_left: Rc<Cell<u32>>,
        load_fails: Rc<Cell<bool>>,
    }
    impl ItemStore for Failing {
        fn put(&mut self, item: &Item) -> Result<(), StoreError> {
            if self.puts_left.get() == 0 {
                return Err(StoreError::Io { what: "writing", path: PathBuf::from("items/x.toml"), message: "disk full".into() });
            }
            self.puts_left.set(self.puts_left.get() - 1);
            self.inner.put(item)
        }
        fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
            self.inner.delete(body, id)
        }
        fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
            self.inner.get(body, id)
        }
        fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
            if self.load_fails.get() {
                return Err(StoreError::Io { what: "reading", path: PathBuf::from("items"), message: "unreadable".into() });
            }
            self.inner.load_all(body)
        }
    }
    let left = Rc::new(Cell::new(1_000));
    let load_fails = Rc::new(Cell::new(false));
    let mut ed = editor(Failing { inner: MemoryStore::new(), puts_left: left.clone(), load_fails: load_fails.clone() });
    let (ids, _) = ed.put_line(ridge(), &long_line()).unwrap();
    let mut moved = ed.line_points(ids[0]).unwrap();
    moved[1] = GeoPoint::new(0.02, moved[1].lon_deg);
    moved[9] = GeoPoint::new(0.02, moved[9].lon_deg);
    let depth = ed.undo_depth();

    // Two parts are written, the third write fails, and the disk stays full: the two writes cannot be taken back.
    left.set(2);
    let e = ed.set_line_points(ids[0], &moved).unwrap_err();
    let EditError::RollbackFailed { ids: left_ids, .. } = &e else { panic!("{e:?}") };
    assert!(!left_ids.is_empty() && left_ids.iter().all(|id| ids.contains(id)), "the items left on the after side are named: {left_ids:?}");
    let text = e.to_string();
    assert!(text.contains("disk full") && text.contains("reload the items from the store"), "{text}");
    for id in left_ids {
        assert!(text.contains(&planet_world_def::item::ulid_text(*id)), "{text}");
    }

    // Every command is refused, naming the same items, and nothing changes.
    let hash = ed.hash();
    left.set(1_000);
    for e in [
        ed.put(NewItem { points: Vec::new(), ..region() }).map(|_| ()).unwrap_err(),
        ed.delete(ids[0]).map(|_| ()).unwrap_err(),
        ed.undo().map(|_| ()).unwrap_err(),
        ed.put_line(ridge(), &[east(0.0), east(10.0)]).map(|_| ()).unwrap_err(),
    ] {
        assert!(matches!(&e, EditError::Inconsistent { ids } if ids == left_ids), "{e:?}");
    }
    assert_eq!(ed.hash(), hash);

    // A reload the store refuses leaves the editor as it was: still refusing commands, the same items named.
    load_fails.set(true);
    assert!(matches!(ed.reload(BodyId::PLANET), Err(EditError::Store(_))));
    assert!(matches!(ed.put_line(ridge(), &[east(0.0), east(10.0)]), Err(EditError::Inconsistent { .. })));
    assert_eq!(ed.hash(), hash);
    load_fails.set(false);

    // Reloading from the store makes the editor what the files are, and it takes commands again; the undo stack is gone.
    let n = ed.reload(BodyId::PLANET).unwrap();
    assert_eq!(n, ids.len());
    assert_store_matches(&ed, "after the reload");
    assert_eq!(ed.undo_depth(), 0, "the stack described a state that is gone (was {depth})");
    ed.put_line(ridge(), &[GeoPoint::new(30.0, 0.0), GeoPoint::new(30.0, 0.01)]).unwrap();
    assert_eq!(ed.undo_depth(), 1);
}

// spec: VIEW-028
#[test]
fn an_undo_whose_rollback_fails_is_named_and_a_reload_recovers() {
    struct Failing {
        inner: MemoryStore,
        puts_left: Rc<Cell<u32>>,
    }
    impl ItemStore for Failing {
        fn put(&mut self, item: &Item) -> Result<(), StoreError> {
            if self.puts_left.get() == 0 {
                return Err(StoreError::Io { what: "writing", path: PathBuf::from("items/x.toml"), message: "disk full".into() });
            }
            self.puts_left.set(self.puts_left.get() - 1);
            self.inner.put(item)
        }
        fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
            self.inner.delete(body, id)
        }
        fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
            self.inner.get(body, id)
        }
        fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
            self.inner.load_all(body)
        }
    }
    let left = Rc::new(Cell::new(1_000));
    let mut ed = editor(Failing { inner: MemoryStore::new(), puts_left: left.clone() });
    let (ids, _) = ed.put_line(ridge(), &long_line()).unwrap();
    let mut moved = ed.line_points(ids[0]).unwrap();
    moved[1] = GeoPoint::new(0.02, moved[1].lon_deg);
    moved[9] = GeoPoint::new(0.02, moved[9].lon_deg);
    ed.set_line_points(ids[0], &moved).unwrap();
    // The undo writes the old parts back: one write works, the next fails, and the one written cannot be written back.
    left.set(1);
    let e = ed.undo().unwrap_err();
    assert!(matches!(&e, EditError::RollbackFailed { ids, .. } if !ids.is_empty()), "{e:?}");
    assert!(matches!(ed.redo(), Err(EditError::Inconsistent { .. })), "redo is refused too");
    left.set(1_000);
    assert_eq!(ed.reload(BodyId::PLANET).unwrap(), ids.len());
    assert_store_matches(&ed, "after the reload");
    assert_eq!((ed.can_undo(), ed.can_redo()), (false, false));
}
