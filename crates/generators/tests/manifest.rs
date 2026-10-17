//! GEN-003: the determinism manifest that CI diffs between Windows and Linux.

use planet_generators::manifest::{determinism_manifest, determinism_manifest_perturbed, manifest_tiles, GlobalFieldValues};

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
    assert_eq!(count("moonheightkey "), tiles);
    assert_eq!(count("moonmeshkey "), tiles);
    assert_eq!(count("geo "), 88, "geo round trips");
    assert_eq!(count("version "), 8);
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

fn kind_of(l: &str) -> &str {
    l.split(' ').next().unwrap()
}

// spec: GEN-003
#[test]
fn manifest_lists_the_global_bake_and_both_terrain_fields_at_edges_and_corners() {
    let a = determinism_manifest();
    let count = |kind: &str| a.lines().filter(|l| kind_of(l) == kind).count();
    // 64 cells per face edge as 16-cell blocks of level 2: 16 blocks per face.
    assert_eq!(count("globalblock"), 6 * 16, "every block of the small bake");
    assert_eq!(count("globalgrid"), 1, "the whole-grid hash");
    assert_eq!((count("oceantable"), count("oceangrid")), (1, 1), "the ocean channel: its component table and its bits");
    assert_eq!(count("terrain"), 6 * 12, "the terrain field, 12 directions per face");
    assert_eq!(count("erosion"), 6 * 12, "the eroded field, 12 directions per face");
    assert!(
        a.contains(
            "version global cpu-global-coarse-v1 4
"
        ),
        "the bake's implementation id and version"
    );
    assert_eq!(count("version"), 8, "bake, terrain and erosion versions join the header");
    // The directions include the four corners and the four edge midpoints of every face.
    for f in 0..6 {
        for k in ["-1.0/-1.0", "1.0/-1.0", "-1.0/1.0", "1.0/1.0", "0.0/-1.0", "0.0/1.0", "-1.0/0.0", "1.0/0.0"] {
            assert!(a.contains(&format!("terrain {f}/{k} ")), "face {f} direction {k}");
            assert!(a.contains(&format!("erosion {f}/{k} ")), "face {f} direction {k}");
        }
    }
}

// spec: GEN-003
#[test]
fn one_ulp_in_any_new_entry_changes_the_manifest_in_that_entry_only() {
    let values = GlobalFieldValues::compute();
    let base = values.entries(&|_, _| None);
    assert!(determinism_manifest().contains(&base), "the new entries are the manifest's own section");
    let new: Vec<&str> = base.lines().collect();
    assert_eq!(new.len(), 96 + 1 + 2 + 72 + 72);
    for line in new {
        let (head, value) = line.rsplit_once(' ').unwrap();
        let (kind, key) = head.split_once(' ').unwrap();
        // Bake entries: every channel at the first and at the last sample; fields: one variant.
        let variants = if kind == "terrain" || kind == "erosion" || kind.starts_with("ocean") { 1 } else { 8 };
        for v in 0..variants {
            let m = values.entries(&|k, kk| (k == kind && kk == key).then_some(v));
            let diff: Vec<(&str, &str)> = base.lines().zip(m.lines()).filter(|(x, y)| x != y).collect();
            assert_eq!(diff.len(), 1, "{head} variant {v}: exactly one line changes, got {diff:?}");
            assert!(diff[0].0.starts_with(head) && !diff[0].1.ends_with(value), "{head} variant {v}: the value changes");
        }
    }
}

// spec: GEN-003
#[test]
fn no_perturbation_means_the_plain_manifest() {
    assert_eq!(determinism_manifest(), determinism_manifest_perturbed(&|_, _| None));
}

// spec: GEN-003
// spec: GEN-017
#[test]
fn field_lines_agree_where_faces_share_a_direction_and_are_finite() {
    use planet_core::cube::{face_to_direction, TangentWarp};
    use planet_core::Face;
    use planet_generators::manifest::MANIFEST_FIELD_POINTS;
    let a = determinism_manifest();
    for kind in ["terrain", "erosion"] {
        let mut by_dir: std::collections::BTreeMap<[u64; 3], Vec<String>> = Default::default();
        for f in 0..6u8 {
            for (s, t) in MANIFEST_FIELD_POINTS {
                let d = face_to_direction(&TangentWarp, Face(f), s, t);
                let line = a.lines().find(|l| l.starts_with(&format!("{kind} {f}/{s:?}/{t:?} "))).unwrap();
                let bits = line.rsplit_once(' ').unwrap().1;
                assert!(f64::from_bits(u64::from_str_radix(bits, 16).unwrap()).is_finite(), "{line}");
                by_dir.entry([d.x.to_bits(), d.y.to_bits(), d.z.to_bits()]).or_default().push(bits.to_string());
            }
        }
        let shared: Vec<_> = by_dir.values().filter(|v| v.len() > 1).collect();
        assert_eq!(shared.len(), 8 + 12, "{kind}: 8 cube corners and 12 edge midpoints are shared by faces");
        for v in shared {
            assert!(v.iter().all(|x| x == &v[0]), "{kind}: one direction, one height: {v:?}");
        }
    }
}
