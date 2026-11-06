//! Tier A: the process memo of the class-source inputs (the viewer and the bake commands open the same world again and share one build).

use planet_cache::global_store::{class_inputs_key, Memo, CLASS_INPUTS_CAP};
use planet_world_def::WorldDefinition;

// spec: STRM-073
#[test]
fn the_memo_keeps_the_newest_entries_up_to_its_cap_and_builds_a_key_once() {
    let mut memo: Memo<u64, std::sync::Arc<u64>> = Memo::new(CLASS_INPUTS_CAP);
    assert_eq!(CLASS_INPUTS_CAP, 4);
    let built = std::cell::Cell::new(0);
    let get = |memo: &mut Memo<u64, std::sync::Arc<u64>>, k: u64| {
        memo.get_or_insert_with(k, || {
            built.set(built.get() + 1);
            std::sync::Arc::new(k * 10)
        })
    };
    let first = get(&mut memo, 1);
    assert!(std::sync::Arc::ptr_eq(&first, &get(&mut memo, 1)), "the same key gives another entry");
    assert_eq!(built.get(), 1, "a hit built again");
    for k in 2..=4 {
        get(&mut memo, k);
    }
    assert_eq!((memo.len(), built.get()), (4, 4));
    get(&mut memo, 5); // the memo is full: the oldest (1) goes
    assert_eq!(memo.len(), 4, "the memo grew past its cap");
    assert_eq!(built.get(), 5);
    let again = get(&mut memo, 1);
    assert_eq!(built.get(), 6, "the evicted key was not built again");
    assert!(!std::sync::Arc::ptr_eq(&first, &again), "an evicted entry came back");
    for k in 6..40 {
        get(&mut memo, k);
    }
    assert_eq!(memo.len(), CLASS_INPUTS_CAP, "a long session grew the memo");
}

// spec: STRM-073
#[test]
fn two_definitions_that_differ_in_one_parameter_or_the_seed_get_different_keys_and_equal_ones_the_same() {
    let a = WorldDefinition::single_planet(6_371_000.0, 3);
    let same = WorldDefinition::single_planet(6_371_000.0, 3);
    let radius = WorldDefinition::single_planet(6_371_001.0, 3);
    let seed = WorldDefinition::single_planet(6_371_000.0, 4);
    assert_eq!(class_inputs_key(&a, 3), class_inputs_key(&same, 3));
    assert_ne!(class_inputs_key(&a, 3), class_inputs_key(&radius, 3), "a parameter that differs shares an entry");
    assert_ne!(class_inputs_key(&a, 3), class_inputs_key(&seed, 4), "another seed shares an entry");
    assert_ne!(class_inputs_key(&a, 3), class_inputs_key(&a, 4), "the seed is not in the key");
}
