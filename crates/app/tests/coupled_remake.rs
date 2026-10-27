//! The directory a re-made coupled run is written to (GEN-042): never the store the bake is read from, always under a `target` directory.

use planet::coupledworld::remake_target;
use std::path::Path;

// spec: GEN-042
#[test]
fn a_re_made_run_is_written_under_target_and_never_into_the_store_it_reads() {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("coupled-remake");
    let store = tmp.join("store");
    std::fs::create_dir_all(&store).unwrap();

    // The store itself is refused, spelled directly or through a `..`.
    let err = remake_target(&store, &store).unwrap_err();
    assert!(err.contains("into the store it reads"), "{err}");
    let err = remake_target(&store, &store.join("..").join("store")).unwrap_err();
    assert!(err.contains("into the store it reads"), "{err}");

    // A `target` directory inside the store is still the store.
    let err = remake_target(&store, &store.join("target").join("x")).unwrap_err();
    assert!(err.contains("into the store it reads"), "{err}");

    // A directory outside any `target` is refused, and a directory under target/ is made and returned.
    let outside = std::env::temp_dir().join("coupled-remake-outside-target-check");
    let err = remake_target(&store, &outside).unwrap_err();
    assert!(err.contains("under a `target` directory"), "{err}");
    let _ = std::fs::remove_dir_all(&outside);
    let ok = remake_target(&store, &tmp.join("out")).unwrap();
    assert!(ok.is_dir() && ok.ends_with("out"), "{ok:?}");
}
