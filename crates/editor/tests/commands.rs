//! Tier A: the command layer (AUTH-004, ADR 0066 point 7): put, change, delete, a drag as one command, undo and redo over 1,000 operations
//! restoring the exact previous definition hash, the item files written through, and two people's additions merging in git.

use planet_editor::commands::{Affected, EditError, Editor, NewItem, UNDO_LIMIT};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_world_def::item::{ulid_text, Anchor, Item};
use planet_world_def::item_store::{ItemStore, LooseFileStore, MemoryStore, StoreError};
use planet_world_def::{BodyId, ItemId, WorldDefinition};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

const R: f64 = 6_371_000.0;
const GENERATOR: u32 = 4;
const STAGE: u32 = 2;

fn editor<S: ItemStore>(store: S, seed: u64) -> Editor<S> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    Editor::new(WorldDefinition::single_planet(R, 7), store, ids, GENERATOR, STAGE)
}

fn new_item(i: u64) -> NewItem {
    let f = i as f64;
    NewItem {
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: (i % 5) as i32,
        kind: if i.is_multiple_of(2) { "crater" } else { "hill" }.into(),
        anchor: Anchor {
            latitude_deg: -60.0 + (f * 7.31) % 120.0,
            longitude_deg: -170.0 + (f * 13.7) % 340.0,
            height_m: f * 0.5,
            heading_deg: (f * 11.0) % 360.0,
        },
        reach_m: 200.0 + (f * 37.0) % 4_000.0,
        max_delta_m: 5.0 + f % 40.0,
        params: BTreeMap::from([("radius_m".to_string(), 100.0 + f), ("depth_m".to_string(), 10.0 + (f % 7.0))]),
        name: (i.is_multiple_of(3)).then(|| format!("item {i}")),
        notes: None,
    }
}

/// A small deterministic generator for the random sessions.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The store holds exactly the items the editor holds, each as it is.
fn assert_store_matches<S: ItemStore>(ed: &Editor<S>, what: &str) {
    let in_store = ed.store().load_all(BodyId::PLANET).unwrap();
    let in_editor: Vec<Item> = ed.item_ids().iter().map(|id| ed.item(*id).unwrap().clone()).collect();
    assert_eq!(in_store, in_editor, "{what}: the files are the items");
    let in_def: Vec<ItemId> = ed.definition().item_ids(BodyId::PLANET);
    assert_eq!(in_def, ed.item_ids(), "{what}: the definition holds the same items");
}

/// One random command: returns true when it was recorded as a command.
fn random_command<S: ItemStore>(ed: &mut Editor<S>, rng: &mut Rng, n: u64) -> bool {
    let depth = ed.undo_depth();
    let ids = ed.item_ids();
    let pick = |rng: &mut Rng| ids[rng.below(ids.len() as u64) as usize];
    match if ids.is_empty() { 0 } else { rng.below(100) } {
        0..=39 => {
            ed.put(new_item(n)).unwrap();
        }
        40..=69 => {
            let id = pick(rng);
            let (lat, h) = (rng.unit() * 100.0 - 50.0, rng.unit() * 500.0);
            ed.change(id, |it| {
                it.anchor.latitude_deg = lat;
                it.anchor.height_m = h;
                it.params.insert("depth_m".into(), 1.0 + h);
                it.priority += 1;
            })
            .unwrap();
        }
        70..=84 => {
            ed.delete(pick(rng)).unwrap();
        }
        _ => {
            // A drag: many steps, one command.
            let id = pick(rng);
            ed.begin_edit(id).unwrap();
            for _ in 0..(2 + rng.below(8)) {
                let (dlat, dlon) = (rng.unit() - 0.5, rng.unit() - 0.5);
                ed.edit(|it| {
                    it.anchor.latitude_deg = (it.anchor.latitude_deg + dlat).clamp(-80.0, 80.0);
                    it.anchor.longitude_deg = (it.anchor.longitude_deg + dlon).clamp(-170.0, 170.0);
                })
                .unwrap();
            }
            ed.end_edit().unwrap();
        }
    }
    ed.undo_depth() > depth
}

fn thousand_operations<S: ItemStore>(mut ed: Editor<S>, seed: u64) {
    let start = ed.hash();
    assert_eq!(start, ed.definition().base_hash(), "no items: the definition hash is the node hash");
    let mut rng = Rng(seed);
    let mut states = vec![start];
    let mut n = 0u64;
    while ed.undo_depth() < 1_000 {
        n += 1;
        if random_command(&mut ed, &mut rng, n) {
            states.push(ed.hash());
        }
    }
    let end = ed.hash();
    assert_eq!(states.len(), 1_001);
    assert_store_matches(&ed, "after 1,000 commands");
    assert_ne!(end, start);
    // Undo them all: after each one the hash is exactly the one before that command.
    for k in (0..1_000).rev() {
        let affected = ed.undo().unwrap().expect("something to undo");
        assert!(!affected.is_empty());
        assert_eq!(ed.hash(), states[k], "undo to state {k}");
        if k % 97 == 0 {
            assert_store_matches(&ed, &format!("undone to {k}"));
        }
    }
    assert_eq!(ed.hash(), start, "undoing everything restores the starting hash");
    assert!(ed.item_ids().is_empty() && ed.store().load_all(BodyId::PLANET).unwrap().is_empty(), "no item and no file is left");
    assert!(ed.undo().unwrap().is_none());
    assert_eq!(ed.undo_depth(), 0);
    // And redo them all: the same hashes in the same order.
    for (k, state) in states.iter().enumerate().skip(1) {
        assert!(ed.redo().unwrap().is_some());
        assert_eq!(ed.hash(), *state, "redo to state {k}");
    }
    assert_eq!(ed.hash(), end);
    assert_store_matches(&ed, "after redoing everything");
    assert!(ed.redo().unwrap().is_none());
}

// spec: AUTH-004
#[test]
fn a_thousand_commands_undo_to_the_exact_starting_hash_and_redo_to_the_exact_end_one_in_memory() {
    thousand_operations(editor(MemoryStore::new(), 11), 1);
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("planet-editor-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// spec: AUTH-004
#[test]
fn a_thousand_commands_undo_and_redo_through_files_that_are_written_through_each_time() {
    let dir = temp_dir("thousand");
    thousand_operations(editor(LooseFileStore::new(&dir), 12), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: AUTH-004
#[test]
fn a_drag_is_one_command_whose_file_is_written_once_and_that_undoes_and_redoes_whole() {
    #[derive(Default)]
    struct Counting {
        inner: MemoryStore,
        puts: Rc<Cell<u32>>,
    }
    impl ItemStore for Counting {
        fn put(&mut self, item: &Item) -> Result<(), StoreError> {
            self.puts.set(self.puts.get() + 1);
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
    let puts = Rc::new(Cell::new(0));
    let mut ed = editor(Counting { puts: puts.clone(), ..Counting::default() }, 3);
    let (id, _) = ed.put(new_item(1)).unwrap();
    assert_eq!(puts.get(), 1);
    let (before_hash, before_item) = (ed.hash(), ed.item(id).unwrap().clone());
    ed.begin_edit(id).unwrap();
    let mut last = before_hash;
    for step in 1..=50 {
        let affected = ed.edit(|it| it.anchor.longitude_deg += 0.01 * f64::from(step)).unwrap();
        assert_eq!(affected.len(), 2, "the anchor before and after the step");
        assert_ne!(ed.hash(), last, "the definition follows every step");
        last = ed.hash();
    }
    assert_eq!(puts.get(), 1, "no file is written during the drag");
    assert_eq!(ed.undo_depth(), 1, "the drag is no command yet");
    assert!(
        matches!(ed.undo(), Err(EditError::EditOpen)) && matches!(ed.redo(), Err(EditError::EditOpen)) && !ed.can_undo(),
        "undo waits for the end of the drag"
    );
    assert!(matches!(ed.load(BodyId::PLANET), Err(EditError::EditOpen)), "so does a load");
    ed.end_edit().unwrap();
    assert_eq!(puts.get(), 2, "one write at the end");
    assert_eq!(ed.undo_depth(), 2, "one command for 50 steps");
    let after = (ed.hash(), ed.item(id).unwrap().clone());
    assert_ne!(after.0, before_hash);
    assert_eq!(ed.store().get(BodyId::PLANET, id).unwrap().unwrap(), after.1, "the file holds the end of the drag");
    ed.undo().unwrap().unwrap();
    assert_eq!((ed.hash(), ed.item(id).unwrap()), (before_hash, &before_item), "one undo takes back the whole drag");
    assert_eq!(ed.store().get(BodyId::PLANET, id).unwrap().unwrap(), before_item);
    ed.redo().unwrap().unwrap();
    assert_eq!((ed.hash(), ed.item(id).unwrap()), (after.0, &after.1));
    // Cancelling a drag puts the item back and records nothing.
    let depth = ed.undo_depth();
    ed.begin_edit(id).unwrap();
    ed.edit(|it| it.anchor.latitude_deg = 1.0).unwrap();
    assert_eq!(ed.begin_edit(id).unwrap_err().to_string(), EditError::EditOpen.to_string());
    assert!(matches!(ed.put(new_item(2)), Err(EditError::EditOpen)), "no other command during a drag");
    assert!(matches!(ed.delete(id), Err(EditError::EditOpen)));
    assert!(matches!(ed.change(id, |_| {}), Err(EditError::EditOpen)));
    let affected = ed.cancel_edit().unwrap();
    assert_eq!(affected.len(), 2);
    assert_eq!((ed.hash(), ed.item(id).unwrap()), (after.0, &after.1));
    assert_eq!(ed.undo_depth(), depth);
    // A drag that ends where it began is not a command, and the file is not touched.
    let puts_before = puts.get();
    ed.begin_edit(id).unwrap();
    ed.edit(|it| it.anchor.latitude_deg += 1.0).unwrap();
    ed.edit(|it| it.anchor.latitude_deg -= 1.0).unwrap();
    assert!(ed.end_edit().unwrap().is_empty());
    assert_eq!((ed.undo_depth(), puts.get()), (depth, puts_before));
    // No drag open: the ends are errors.
    assert!(matches!(ed.end_edit(), Err(EditError::NoEditOpen)));
    assert!(matches!(ed.cancel_edit(), Err(EditError::NoEditOpen)));
    assert!(matches!(ed.edit(|_| {}), Err(EditError::NoEditOpen)));
    // A step the item cannot take is refused and leaves the drag as it was.
    ed.begin_edit(id).unwrap();
    let held = ed.hash();
    assert!(matches!(ed.edit(|it| it.anchor.latitude_deg = f64::NAN), Err(EditError::Invalid(_))));
    assert!(matches!(ed.edit(|it| it.reach_m = 1.0e9), Err(EditError::Invalid(_))));
    assert_eq!(ed.hash(), held);
    ed.cancel_edit().unwrap();
}

// spec: AUTH-004
#[test]
fn a_new_command_clears_redo_and_undo_and_redo_with_nothing_to_do_say_so() {
    let mut ed = editor(MemoryStore::new(), 4);
    assert!(ed.undo().unwrap().is_none() && ed.redo().unwrap().is_none());
    let (a, _) = ed.put(new_item(1)).unwrap();
    let (b, _) = ed.put(new_item(2)).unwrap();
    assert!(a < b, "ids made in a row increase");
    ed.undo().unwrap().unwrap();
    assert!(ed.can_redo());
    ed.put(new_item(3)).unwrap();
    assert!(!ed.can_redo(), "a new command clears what could be redone");
    assert!(ed.redo().unwrap().is_none());
    assert_eq!(ed.item_ids().len(), 2);
}

// spec: AUTH-004
#[test]
fn a_command_that_fails_changes_nothing_not_the_definition_the_files_or_the_stack() {
    let mut ed = editor(MemoryStore::new(), 5);
    let (id, _) = ed.put(new_item(1)).unwrap();
    let snapshot = |ed: &Editor<MemoryStore>| (ed.hash(), ed.item_ids(), ed.undo_depth(), ed.store().load_all(BodyId::PLANET).unwrap());
    let before = snapshot(&ed);
    // An item the file cannot carry.
    let mut bad = new_item(2);
    bad.reach_m = 1.0e9;
    assert!(matches!(ed.put(bad), Err(EditError::Invalid(e)) if e.key == "reach_m"));
    let mut bad = new_item(2);
    bad.anchor.latitude_deg = f64::INFINITY;
    assert!(matches!(ed.put(bad), Err(EditError::Invalid(_))));
    // A body the definition has no radius for: the index refuses.
    let mut elsewhere = new_item(2);
    elsewhere.body = BodyId(9);
    assert!(matches!(ed.put(elsewhere), Err(EditError::Index(_))));
    // A change to something invalid, to an unknown item; a delete of an unknown item.
    assert!(matches!(ed.change(id, |it| it.max_delta_m = -1.0), Err(EditError::Invalid(_))));
    assert!(matches!(ed.change(id, |it| it.anchor.longitude_deg = 500.0), Err(EditError::Invalid(_))));
    assert!(matches!(ed.change(12345, |_| {}), Err(EditError::NoSuchItem(12345))));
    assert!(matches!(ed.delete(12345), Err(EditError::NoSuchItem(12345))));
    assert_eq!(snapshot(&ed), before);
    // A change that changes nothing is no command.
    assert!(ed.change(id, |_| {}).unwrap().is_empty());
    assert!(ed.change(id, |it| it.name = it.name.clone()).unwrap().is_empty());
    assert_eq!(ed.undo_depth(), before.2);
    // The id and the body cannot be changed by a change.
    ed.change(id, |it| {
        it.id += 1;
        it.body = BodyId(3);
        it.priority = 77;
    })
    .unwrap();
    assert_eq!(ed.item(id).unwrap().priority, 77);
    assert_eq!((ed.item(id).unwrap().id, ed.item(id).unwrap().body), (id, BodyId::PLANET));
}

// spec: AUTH-004
#[test]
fn a_store_that_fails_leaves_the_definition_and_the_stack_as_they_were() {
    struct Failing {
        inner: MemoryStore,
        fail: Rc<Cell<bool>>,
    }
    impl ItemStore for Failing {
        fn put(&mut self, item: &Item) -> Result<(), StoreError> {
            if self.fail.get() {
                return Err(StoreError::Io { what: "writing", path: PathBuf::from("items/x.toml"), message: "disk full".into() });
            }
            self.inner.put(item)
        }
        fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
            if self.fail.get() {
                return Err(StoreError::Io { what: "removing", path: PathBuf::from("items/x.toml"), message: "locked".into() });
            }
            self.inner.delete(body, id)
        }
        fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
            self.inner.get(body, id)
        }
        fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
            self.inner.load_all(body)
        }
    }
    let fail = Rc::new(Cell::new(false));
    let mut ed = editor(Failing { inner: MemoryStore::new(), fail: fail.clone() }, 6);
    let (id, _) = ed.put(new_item(1)).unwrap();
    let (hash, depth) = (ed.hash(), ed.undo_depth());
    fail.set(true);
    assert!(matches!(ed.put(new_item(2)), Err(EditError::Store(_))));
    assert!(matches!(ed.change(id, |it| it.priority += 5), Err(EditError::Store(_))));
    assert!(matches!(ed.delete(id), Err(EditError::Store(_))));
    assert!(matches!(ed.undo(), Err(EditError::Store(_))), "undo writes through too");
    assert_eq!((ed.hash(), ed.undo_depth(), ed.item_ids()), (hash, depth, vec![id]));
    // The failed undo is still there to try again.
    fail.set(false);
    ed.undo().unwrap().unwrap();
    assert!(ed.item_ids().is_empty());
    // A failing drag end keeps the drag open.
    fail.set(false);
    let (id, _) = ed.put(new_item(3)).unwrap();
    ed.begin_edit(id).unwrap();
    ed.edit(|it| it.priority = 9).unwrap();
    fail.set(true);
    assert!(matches!(ed.end_edit(), Err(EditError::Store(_))));
    fail.set(false);
    ed.end_edit().unwrap();
    assert_eq!(ed.store().get(BodyId::PLANET, id).unwrap().unwrap().priority, 9);
}

// spec: AUTH-004, AUTH-001
#[test]
fn files_are_written_through_for_every_command_and_a_new_session_loads_the_same_definition() {
    let dir = temp_dir("session");
    let mut ed = editor(LooseFileStore::new(&dir), 7);
    let (a, _) = ed.put(new_item(1)).unwrap();
    let (b, _) = ed.put(new_item(2)).unwrap();
    let (c, _) = ed.put(new_item(3)).unwrap();
    ed.change(b, |it| it.anchor.height_m = 123.5).unwrap();
    ed.delete(a).unwrap();
    let path = |id: ItemId| dir.join("0").join(format!("{}.toml", ulid_text(id)));
    assert!(!path(a).exists(), "a deleted item's file is gone");
    assert_eq!(std::fs::read_to_string(path(b)).unwrap(), ed.item(b).unwrap().to_toml().unwrap(), "the file is the item");
    assert!(path(c).exists());
    ed.undo().unwrap().unwrap();
    assert!(path(a).exists(), "undoing a delete writes the file again");
    ed.undo().unwrap().unwrap();
    assert_eq!(Item::from_toml(&std::fs::read_to_string(path(b)).unwrap()).unwrap().anchor.height_m, new_item(2).anchor.height_m);
    ed.redo().unwrap().unwrap();
    let hash = ed.hash();
    // A new session over the same files: the same items, the same definition hash.
    let mut again = editor(LooseFileStore::new(&dir), 99);
    assert_eq!(again.load(BodyId::PLANET).unwrap(), 3);
    assert_eq!(again.hash(), hash);
    assert_eq!(again.item_ids(), ed.item_ids());
    // Loading an item authored against other versions is refused naming them (AUTH-002), and nothing is loaded.
    let ids = IdMaker::new(Box::new(FixedClock(1)), Box::new(SeededEntropy(1)));
    let mut other_versions = Editor::new(WorldDefinition::single_planet(R, 7), LooseFileStore::new(&dir), ids, GENERATOR + 1, STAGE);
    let e = other_versions.load(BodyId::PLANET).unwrap_err().to_string();
    assert!(e.contains("generator version 4 (active 5)") && e.contains("pin") && e.contains("re-author"), "{e}");
    assert!(other_versions.item_ids().is_empty() && !other_versions.definition().has_items());
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: AUTH-004
#[test]
fn the_places_to_refresh_are_the_anchors_before_and_after_with_their_reach() {
    let mut ed = editor(MemoryStore::new(), 8);
    let (id, put) = ed.put(new_item(1)).unwrap();
    assert_eq!(put.len(), 1, "a put has an after only");
    let item = ed.item(id).unwrap().clone();
    assert_eq!(put[0], Affected { body: BodyId::PLANET, dir: item.direction(), reach_m: item.reach_m });
    let moved = ed.change(id, |it| it.anchor.longitude_deg += 10.0).unwrap();
    assert_eq!(moved.len(), 2, "a change has the old place and the new one");
    assert_eq!(moved[0].dir, item.direction());
    assert_ne!(moved[1].dir, item.direction());
    let deleted = ed.delete(id).unwrap();
    assert_eq!(deleted.len(), 1, "a delete has a before only");
    // Undo and redo name the same places.
    let undone = ed.undo().unwrap().unwrap();
    assert_eq!(undone.len(), 1);
    assert_eq!(undone[0].body, BodyId::PLANET);
}

// spec: AUTH-004
#[test]
fn the_undo_stack_keeps_the_last_ten_thousand_commands() {
    let mut ed = editor(MemoryStore::new(), 9);
    for i in 0..(UNDO_LIMIT + 5) as u64 {
        ed.put(NewItem { name: None, ..new_item(i) }).unwrap();
    }
    assert_eq!(ed.undo_depth(), UNDO_LIMIT);
    assert_eq!(ed.item_ids().len(), UNDO_LIMIT + 5);
    for _ in 0..UNDO_LIMIT {
        ed.undo().unwrap().unwrap();
    }
    assert!(ed.undo().unwrap().is_none());
    assert_eq!(ed.item_ids().len(), 5, "the five oldest commands can no longer be undone");
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    let out = Command::new("git").args(args).current_dir(dir).output().expect("git must be installed to run this test");
    assert!(
        out.status.success() || args[0] == "merge",
        "git {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["-c", "user.name=test", "-c", "user.email=test@example.com", "commit", "-q", "-m", message]);
}

fn merge(dir: &Path, branch: &str) -> std::process::Output {
    git(dir, &["-c", "user.name=test", "-c", "user.email=test@example.com", "merge", "--no-edit", branch])
}

// spec: AUTH-001, AUTH-004
#[test]
fn two_additions_made_on_two_branches_merge_in_git_without_a_conflict_and_load_as_one_definition() {
    let repo = temp_dir("git");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    let items = repo.join("items");
    std::fs::create_dir_all(items.join("0")).unwrap();
    std::fs::write(items.join("0").join(".gitkeep"), "").unwrap();
    // A shared starting item, then two people add one each (different entropy: different ids) and change a different item each.
    let mut base = editor(LooseFileStore::new(&items), 100);
    let (shared1, _) = base.put(new_item(1)).unwrap();
    let (shared2, _) = base.put(new_item(2)).unwrap();
    let (doomed, _) = base.put(new_item(3)).unwrap();
    commit_all(&repo, "the shared items");
    git(&repo, &["branch", "alice"]);
    git(&repo, &["branch", "bob"]);
    git(&repo, &["checkout", "-q", "alice"]);
    let mut alice = editor(LooseFileStore::new(&items), 200);
    alice.load(BodyId::PLANET).unwrap();
    let (a_new, _) = alice.put(new_item(10)).unwrap();
    alice.change(shared1, |it| it.priority = 40).unwrap();
    alice.delete(doomed).unwrap();
    commit_all(&repo, "alice");
    git(&repo, &["checkout", "-q", "bob"]);
    let mut bob = editor(LooseFileStore::new(&items), 300);
    bob.load(BodyId::PLANET).unwrap();
    let (b_new, _) = bob.put(new_item(20)).unwrap();
    bob.change(shared2, |it| it.anchor.height_m = 999.0).unwrap();
    commit_all(&repo, "bob");
    assert_ne!(a_new, b_new);
    // Merge bob into alice: no conflict.
    git(&repo, &["checkout", "-q", "alice"]);
    let out = merge(&repo, "bob");
    assert!(out.status.success(), "the merge conflicts: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    // The merged files load as one definition with everything both people did.
    let mut merged = editor(LooseFileStore::new(&items), 400);
    assert_eq!(merged.load(BodyId::PLANET).unwrap(), 4);
    let want: Vec<ItemId> = {
        let mut v = vec![shared1, shared2, a_new, b_new];
        v.sort_unstable();
        v
    };
    assert_eq!(merged.item_ids(), want, "both additions, alice's deletion, nothing lost");
    assert_eq!(merged.item(shared1).unwrap().priority, 40, "alice's change");
    assert_eq!(merged.item(shared2).unwrap().anchor.height_m, 999.0, "bob's change");
    // The same definition whichever way it is merged: bob merged alice gives the same hash.
    let hash = merged.hash();
    git(&repo, &["checkout", "-q", "bob"]);
    assert!(merge(&repo, "alice").status.success());
    let mut other_way = editor(LooseFileStore::new(&items), 500);
    other_way.load(BodyId::PLANET).unwrap();
    assert_eq!(other_way.hash(), hash, "the merge order does not matter");
    let _ = std::fs::remove_dir_all(&repo);
}

// spec: AUTH-004
#[test]
fn loading_the_same_items_twice_is_refused_as_a_duplicate() {
    let mut store = MemoryStore::new();
    let mut maker = editor(MemoryStore::new(), 55);
    let (id, _) = maker.put(new_item(1)).unwrap();
    store.put(maker.item(id).unwrap()).unwrap();
    // The items were made against this base (the hash of the definition without items), the generator 4 and the stage 2.
    let mut ed = editor(store, 56);
    assert_eq!(ed.load(BodyId::PLANET).unwrap(), 1);
    let hash = ed.hash();
    assert!(matches!(ed.load(BodyId::PLANET), Err(EditError::Duplicate(d)) if d == id));
    assert_eq!(ed.hash(), hash, "a refused load changes nothing");
}
