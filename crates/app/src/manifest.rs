//! The full determinism manifest written by `generate --manifest` (GEN-003, ADR 0010, ADR 0023 point 9): the generators'
//! manifest plus the persistent-key known answers of `cache` (ADR 0018). `generators` cannot depend on `cache` (CON-02), so
//! this crate, which sees both, appends the `persistent` lines; the same literals are pinned in `cache`'s own tests.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use planet_cache::global_store::GlobalWorld;
use planet_cache::key::{
    body_params_hash128, cache_key, height_key, mesh_key, node_hash128, query_result_hash128, CacheKey, KeyInputs, Kind, ReadEntry,
    KEY_FORMAT_TAG, PARAM_RESOLUTION, QUERY_CELL,
};
use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, TileId};
use planet_generators::height::Reference;
use planet_world_def::{BodyId, NodeValue, WorldDefinition};

const R: f64 = planet_core::EARTH_RADIUS_M;

fn hex(b: [u8; 16]) -> String {
    CacheKey(b).to_hex()
}

/// The persistent known answers as manifest lines: the same inputs as the pinned literals of `cache`'s tests (`keys.rs`,
/// `global_store.rs`), so a platform whose hashing or encoding differs fails the cross-OS diff, not only its own tests.
pub fn persistent_known_answers() -> String {
    let tile = TileId::new(Face(2), 5, 3, 7).expect("tile");
    let base = KeyInputs {
        kind: Kind::Height,
        hash_version: 1,
        generator_version: 3,
        implementation_id: "impl-a".into(),
        face_mapping_id: "tangent-v1".into(),
        body: BodyId(0),
        body_params: [7; 16],
        tile,
        source: None,
        output_params: BTreeMap::from([(PARAM_RESOLUTION, 32u32.to_le_bytes().to_vec())]),
        reads: vec![
            ReadEntry::Node { path: "world.seed".into(), hash: [1; 16] },
            ReadEntry::Node { path: "body.0.radius_m".into(), hash: [2; 16] },
            ReadEntry::Query { kind: QUERY_CELL, params: vec![9, 9], result: [3; 16] },
        ],
    };
    let planet = Reference(7);
    let world = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 7)).expect("seed").with_block_limit(4);
    let block_tile = TileId::new(Face(2), 2, 1, 3).expect("tile");
    let mut s = String::new();
    let _ = writeln!(s, "version cachekey {KEY_FORMAT_TAG}");
    let _ = writeln!(s, "version cachehash {HASH_VERSION}");
    let mut line = |name: &str, v: String| {
        let _ = writeln!(s, "persistent {name} {v}");
    };
    line("base", cache_key(&base).to_hex());
    line("height", height_key(&planet, &TangentWarp, tile, 32, R).to_hex());
    line("mesh", mesh_key(&planet, &TangentWarp, tile, 32, R).to_hex());
    line("node", hex(node_hash128("world.seed", NodeValue::U64(7))));
    line("body", hex(body_params_hash128(BodyId(1), 9, 1_737_400.0, "sphere")));
    line("emptyquery", hex(query_result_hash128(&[])));
    line("stagekey32", world.stage_key(32).to_hex());
    line("blockkey", world.block_key(block_tile, 8).to_hex());
    line("nodesetid", hex(world.node_set_id()));
    line("oceankey", world.ocean_key(block_tile, 8).to_hex());
    s
}

/// The text `generate --manifest` writes.
pub fn full_manifest() -> String {
    let mut s = planet_generators::manifest::determinism_manifest();
    s.push_str(&persistent_known_answers());
    s.push_str(&region_bake_fixture_lines());
    s
}

/// The region bake fixture of the cross-OS determinism manifest (BUILD-051): the hash of the bake's files at one thread twice and at eight
/// threads (`planet_cache::region_bake::determinism_fixture`). The three lines of one run are equal (a thread count or a repeat that changes a
/// byte breaks that, a tier A test below), and the manifest diff across Windows and Linux compares them with the other platform's.
pub fn region_bake_fixture_lines() -> String {
    let mut s = String::new();
    for (label, threads) in [("t1", 1usize), ("t1again", 1), ("t8", 8)] {
        let h = planet_cache::region_bake::determinism_fixture(threads)
            .unwrap_or_else(|e| panic!("the determinism fixture at {threads} threads: {e}"));
        s.push_str(&format!(
            "regionbake {label} {h:016x}
"
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-051
    #[test]
    fn the_region_bake_fixture_gives_one_hash_at_one_thread_twice_and_at_eight_threads() {
        let lines = region_bake_fixture_lines();
        let hashes: Vec<&str> = lines.lines().map(|l| l.rsplit(' ').next().unwrap()).collect();
        assert_eq!(hashes.len(), 3, "{lines}");
        assert!(hashes.iter().all(|h| h.len() == 16 && *h != "0000000000000000"), "{lines}");
        assert!(hashes[0] == hashes[1], "a second bake at one thread gives other bytes: {lines}");
        assert!(hashes[0] == hashes[2], "eight threads give other bytes than one: {lines}");
        assert!(full_manifest().contains("regionbake t8 "), "the fixture is part of the manifest `generate --manifest` writes");
    }

    // spec: GEN-003
    // spec: STRM-003
    // spec: STRM-012
    #[test]
    fn the_manifest_carries_the_persistent_known_answers_pinned_in_the_cache_tests() {
        let m = full_manifest();
        let get = |k: &str| {
            m.lines()
                .find_map(|l| l.strip_prefix(&format!("persistent {k} ")))
                .unwrap_or_else(|| panic!("no {k} in the manifest"))
                .to_string()
        };
        // The literals of crates/cache/tests/keys.rs and global_store.rs: one pin, two places.
        assert_eq!(get("base"), "b00fa2b4b0f9accc87be74b4dd9d40ca");
        assert_eq!(get("height"), "340480159ae2ca791db08d1cae1232ce");
        // MESH_VERSION 11, channel block, GEN-126 (was 4edba8989effcc04639e468142bd8803 under 10, 4843c471858a5aff0c6741d4cd9a60c0 under 9).
        assert_eq!(get("mesh"), "bd949e7242ab8e5b253406041eb398cc");
        assert_eq!(get("node"), "dd934e9e858a828bb73c45e8a4eb4d45");
        assert_eq!(get("body"), "91f0cebc78907779e2f87196650d468e");
        assert_eq!(get("emptyquery"), "a97668a9d9614a3f4a73f2fceae97219");
        assert_eq!(get("stagekey32"), "ad7f2820858c41fde2c78e8816afa3f6");
        assert_eq!(get("blockkey"), "983f66fa51bcc9195cb7023aff4888f6");
        assert_eq!(get("nodesetid"), "92572f1974620d913454c89a22aca7be");
        assert_eq!(get("oceankey"), "31192efd2f311968e196d7330fee3649");
        assert_eq!(m.lines().filter(|l| l.starts_with("persistent ")).count(), 10);
        assert!(m.starts_with(&planet_generators::manifest::determinism_manifest()), "the generators' manifest comes first, unchanged");
        let mut keys: Vec<&str> = m.lines().map(|l| l.rsplit_once(' ').unwrap().0).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate manifest keys");
    }
}
