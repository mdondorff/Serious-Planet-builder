//! GEN-003: the determinism manifest that CI diffs between Windows and Linux.

use planet_generators::manifest::{determinism_manifest, manifest_tiles};

// spec: GEN-003
#[test]
fn manifest_is_reproducible_and_covers_heights_meshes_and_geo() {
    let a = determinism_manifest();
    assert_eq!(a, determinism_manifest(), "two runs in one process differ");
    let tiles = manifest_tiles().len();
    let count = |kind: &str| a.lines().filter(|l| l.starts_with(kind)).count();
    assert_eq!(count("height "), tiles);
    assert_eq!(count("mesh "), tiles);
    assert_eq!(count("heightkey "), tiles);
    assert_eq!(count("meshkey "), tiles);
    assert_eq!(count("geo "), 88, "geo round trips");
    assert_eq!(count("version "), 5);
    let mut keys: Vec<&str> = a.lines().map(|l| l.rsplit_once(' ').unwrap().0).collect();
    keys.sort_unstable();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n, "duplicate manifest keys");
    assert!(a.lines().all(|l| l.split(' ').count() >= 3), "every line is <kind> <key> <value>");
}

// spec: GEN-003
#[test]
fn manifest_covers_every_face_cube_corners_and_level_28() {
    let ids = manifest_tiles();
    for f in 0..6 {
        assert!(ids.iter().any(|t| t.face().0 == f && t.level() == 0), "face {f} level 0");
        assert!(ids.iter().any(|t| t.face().0 == f && t.level() == 3 && t.x() == 7 && t.y() == 7), "face {f} corner tile");
    }
    assert!(ids.iter().any(|t| t.level() == 28));
}
