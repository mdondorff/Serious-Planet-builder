//! STRM-011: persistent 128-bit keys of staged field outputs.

use std::collections::BTreeMap;

use planet_cache::key::{body_params_hash128, node_hash128, CacheKey, ReadEntry};
use planet_cache::stage_key::{encode_stage_key, stage_key, StageKeyContext, STAGE_KEY_FORMAT_TAG};
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, TileId};
use planet_generators::stage::{FieldGraph, FieldSpec, Scope, Stage};
use planet_world_def::{BodyId, NodeValue};

fn spec(id: &'static str, stage: Stage, reads: &'static [&'static str], version: u32, halo: f64) -> FieldSpec {
    FieldSpec { id, stage, reads, version, implementation_id: "impl-a", halo_fraction: halo }
}

fn graph(coarse: u32, regional: u32, halo: f64) -> FieldGraph {
    FieldGraph::new(&[
        spec("coarse", Stage::GlobalCoarse, &[], coarse, 0.0),
        spec("terrain", Stage::Regional, &["coarse"], regional, halo),
        spec("detail", Stage::Local, &["terrain", "coarse"], 1, 0.0),
    ])
    .unwrap()
}

fn seed_read() -> Vec<ReadEntry> {
    vec![ReadEntry::Node { path: "world.seed".into(), hash: node_hash128("world.seed", NodeValue::U64(7)) }]
}

fn ctx(reads: &[ReadEntry]) -> StageKeyContext<'_> {
    StageKeyContext {
        body: BodyId(0),
        body_params: body_params_hash128(BodyId(0), 7, 6_371_000.0, "sphere"),
        face_mapping_id: "tangent-v1",
        resolution: 64,
        reads,
    }
}

fn tile() -> TileId {
    TileId::new(Face(2), 5, 3, 7).unwrap()
}

/// Keys of the three fields of `graph` for one tile in the context `c`, each chained to its inputs.
fn keys(graph: &FieldGraph, c: &StageKeyContext) -> BTreeMap<&'static str, CacheKey> {
    let mut k: BTreeMap<&'static str, CacheKey> = BTreeMap::new();
    let coarse = stage_key(graph, "coarse", Scope::Global, c, &BTreeMap::new());
    k.insert("coarse", coarse);
    let terrain = stage_key(graph, "terrain", Scope::Tile(tile()), c, &BTreeMap::from([("coarse", coarse)]));
    k.insert("terrain", terrain);
    let detail = stage_key(graph, "detail", Scope::Tile(tile()), c, &BTreeMap::from([("terrain", terrain), ("coarse", coarse)]));
    k.insert("detail", detail);
    k
}

// spec: STRM-011
#[test]
fn a_key_is_a_pure_function_and_the_input_order_does_not_matter() {
    let reads = seed_read();
    let g = graph(1, 1, 0.25);
    let a = keys(&g, &ctx(&reads));
    assert_eq!(a, keys(&g, &ctx(&reads)));
    let c = a["coarse"];
    let t = a["terrain"];
    let one = stage_key(&g, "detail", Scope::Tile(tile()), &ctx(&reads), &BTreeMap::from([("terrain", t), ("coarse", c)]));
    assert_eq!(one, a["detail"]);
    // Duplicated and reordered reads give the same key.
    let mut more = seed_read();
    more.push(ReadEntry::Node { path: "a.b".into(), hash: node_hash128("a.b", NodeValue::F64(1.0)) });
    let mut reversed = more.clone();
    reversed.reverse();
    reversed.push(reversed[0].clone());
    assert_eq!(keys(&g, &ctx(&more)), keys(&g, &ctx(&reversed)));
}

// spec: STRM-011
#[test]
fn a_change_of_any_input_changes_exactly_the_keys_that_depend_on_it() {
    let reads = seed_read();
    let base = keys(&graph(1, 1, 0.25), &ctx(&reads));

    // A version bump of the global field re-keys all three; of the regional one, itself and the local field only.
    let coarse2 = keys(&graph(2, 1, 0.25), &ctx(&reads));
    assert!(coarse2.iter().all(|(k, v)| base[k] != *v), "{coarse2:?}");
    let regional2 = keys(&graph(1, 2, 0.25), &ctx(&reads));
    assert_eq!(regional2["coarse"], base["coarse"]);
    assert!(regional2["terrain"] != base["terrain"] && regional2["detail"] != base["detail"]);
    // Another halo is another output.
    let halo = keys(&graph(1, 1, 0.5), &ctx(&reads));
    assert_eq!(halo["coarse"], base["coarse"]);
    assert!(halo["terrain"] != base["terrain"] && halo["detail"] != base["detail"]);

    // The context: resolution, face mapping, body parameters, read set. The global key sees them too.
    let g = graph(1, 1, 0.25);
    let mut variants: Vec<StageKeyContext> = vec![
        StageKeyContext { resolution: 128, ..ctx(&reads) },
        StageKeyContext { face_mapping_id: "other", ..ctx(&reads) },
        StageKeyContext { body: BodyId(1), ..ctx(&reads) },
        StageKeyContext { body_params: body_params_hash128(BodyId(0), 8, 6_371_000.0, "sphere"), ..ctx(&reads) },
    ];
    let other_reads = vec![ReadEntry::Node { path: "world.seed".into(), hash: node_hash128("world.seed", NodeValue::U64(8)) }];
    variants.push(ctx(&other_reads));
    for (i, v) in variants.iter().enumerate() {
        let k = keys(&g, v);
        assert!(k.iter().all(|(id, key)| base[id] != *key), "variant {i} must re-key every field");
    }
}

// spec: STRM-011
#[test]
fn the_scope_kind_and_the_tile_are_part_of_the_key_and_the_wrong_scope_panics() {
    let reads = seed_read();
    let g = graph(1, 1, 0.25);
    let c = ctx(&reads);
    let coarse = stage_key(&g, "coarse", Scope::Global, &c, &BTreeMap::new());
    let up = BTreeMap::from([("coarse", coarse)]);
    let a = stage_key(&g, "terrain", Scope::Tile(tile()), &c, &up);
    let b = stage_key(&g, "terrain", Scope::Tile(TileId::new(Face(2), 5, 3, 8).unwrap()), &c, &up);
    assert_ne!(a, b);
    // A tile whose raw id is 0 does not alias the global scope.
    let root = TileId::new(Face(0), 0, 0, 0).unwrap();
    let wrong = std::panic::catch_unwind(|| {
        let g = graph(1, 1, 0.25);
        let reads = seed_read();
        stage_key(&g, "coarse", Scope::Tile(root), &ctx(&reads), &BTreeMap::new())
    });
    assert!(wrong.is_err(), "a global field cannot be keyed per tile");
    let wrong = std::panic::catch_unwind(|| {
        let g = graph(1, 1, 0.25);
        let reads = seed_read();
        stage_key(&g, "terrain", Scope::Global, &ctx(&reads), &BTreeMap::from([("coarse", CacheKey([1; 16]))]))
    });
    assert!(wrong.is_err(), "a regional field cannot be keyed globally");
}

// spec: STRM-011
#[test]
fn a_missing_extra_or_unknown_input_is_a_bug_that_panics() {
    let g = graph(1, 1, 0.25);
    let reads = seed_read();
    let c = ctx(&reads);
    let k = CacheKey([9; 16]);
    for (name, f) in [
        ("missing", Box::new(|| stage_key(&g, "terrain", Scope::Tile(tile()), &c, &BTreeMap::new())) as Box<dyn Fn() -> CacheKey + '_>),
        ("extra", Box::new(|| stage_key(&g, "terrain", Scope::Tile(tile()), &c, &BTreeMap::from([("coarse", k), ("detail", k)])))),
        ("unknown field", Box::new(|| stage_key(&g, "nope", Scope::Global, &c, &BTreeMap::new()))),
    ] {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        assert!(r.is_err(), "{name}");
    }
}

// spec: STRM-011
#[test]
fn the_encoding_layout_and_the_known_answers_are_pinned() {
    assert_eq!(STAGE_KEY_FORMAT_TAG, 1, "a new encoding needs a new tag and new known answers");
    assert_eq!(HASH_VERSION, 1, "known answers belong to this hash version");
    let g = FieldGraph::new(&[spec("coarse", Stage::GlobalCoarse, &[], 3, 0.0)]).unwrap();
    let reads = vec![ReadEntry::Node { path: "w".into(), hash: [0xab; 16] }];
    let c = StageKeyContext { body: BodyId(5), body_params: [0xcd; 16], face_mapping_id: "fm", resolution: 16, reads: &reads };
    // The layout written out: tag, hash version, field version, stage tag, id, implementation id, halo bits, scope kind and
    // value, body id, body parameter hash, mapping id, resolution, read count, the read, input count.
    let mut want: Vec<u8> = vec![1];
    want.extend(1u32.to_le_bytes());
    want.extend(3u32.to_le_bytes());
    want.push(0);
    want.extend(6u32.to_le_bytes());
    want.extend(b"coarse");
    want.extend(6u32.to_le_bytes());
    want.extend(b"impl-a");
    want.extend(0f64.to_bits().to_le_bytes());
    want.push(0);
    want.extend([0u8; 8]);
    want.extend(5u32.to_le_bytes());
    want.extend([0xcd; 16]);
    want.extend(2u32.to_le_bytes());
    want.extend(b"fm");
    want.extend(16u32.to_le_bytes());
    want.extend(1u32.to_le_bytes());
    want.push(1);
    want.extend(1u32.to_le_bytes());
    want.extend(b"w");
    want.extend([0xab; 16]);
    want.extend(0u32.to_le_bytes());
    assert_eq!(encode_stage_key(&g, "coarse", Scope::Global, &c, &BTreeMap::new()), want);
    assert_eq!(stage_key(&g, "coarse", Scope::Global, &c, &BTreeMap::new()).to_hex(), KNOWN_LAYOUT_KEY);

    // A chained example: the three-field graph at a tile.
    let reads = seed_read();
    let k = keys(&graph(1, 1, 0.25), &ctx(&reads));
    assert_eq!(k["coarse"].to_hex(), KNOWN_COARSE);
    assert_eq!(k["terrain"].to_hex(), KNOWN_TERRAIN);
    assert_eq!(k["detail"].to_hex(), KNOWN_DETAIL);
}

const KNOWN_LAYOUT_KEY: &str = "3fa6ad5dff9f501ab224456cabd21384";
const KNOWN_COARSE: &str = "f7eb6786122ed161a75229e125d91653";
const KNOWN_TERRAIN: &str = "222fb7bc04027214cfffd092d0f271f2";
const KNOWN_DETAIL: &str = "f8be7d44cb5d0e89d9eb4b4f58754c5e";
