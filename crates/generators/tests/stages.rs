//! GEN-007: staged generation. A stage reads only the outputs of earlier stages, and a field's key chains the keys of its
//! inputs.

use std::collections::BTreeMap;

use planet_core::{Face, TileId};
use planet_generators::stage::{FieldGraph, FieldSpec, GraphError, OutputContext, Scope, Stage, STAGE_KEY_VERSION};
use planet_world_def::BodyId;

const fn spec(id: &'static str, stage: Stage, reads: &'static [&'static str], halo_fraction: f64) -> FieldSpec {
    FieldSpec { id, stage, reads, version: 1, implementation_id: "cpu-test", halo_fraction }
}

fn specs() -> Vec<FieldSpec> {
    vec![
        spec("elevation", Stage::GlobalCoarse, &[], 0.0),
        spec("temperature", Stage::GlobalCoarse, &[], 0.0),
        spec("slope", Stage::Regional, &["elevation"], 0.25),
        spec("flow", Stage::Regional, &["elevation"], 0.5),
        spec("height", Stage::Local, &["elevation", "slope"], 0.0),
        spec("biome", Stage::Local, &["temperature", "slope"], 0.0),
    ]
}

fn graph(specs: &[FieldSpec]) -> FieldGraph {
    FieldGraph::new(specs).unwrap()
}

fn tile() -> TileId {
    TileId::new(Face(2), 5, 3, 7).unwrap()
}

fn ctx() -> OutputContext<'static> {
    OutputContext { body: BodyId::PLANET, surface_id: "sphere", inputs_hash: 99, face_mapping_id: "tangent-v1", resolution: 32 }
}

fn changed(a: &BTreeMap<&'static str, u64>, b: &BTreeMap<&'static str, u64>) -> Vec<&'static str> {
    a.keys().filter(|k| a[*k] != b[*k]).copied().collect()
}

fn with(id: &str, f: impl Fn(&mut FieldSpec)) -> Vec<FieldSpec> {
    let mut s = specs();
    f(s.iter_mut().find(|x| x.id == id).unwrap());
    s
}

// spec: GEN-007
#[test]
fn a_graph_that_only_reads_earlier_stages_is_ordered_by_stage() {
    let g = graph(&specs());
    let order = g.order();
    assert_eq!(order, ["elevation", "temperature", "flow", "slope", "biome", "height"]);
    let pos = |id: &str| order.iter().position(|x| *x == id).unwrap();
    for s in specs() {
        for r in s.reads {
            assert!(pos(r) < pos(s.id), "{} must come after {r}", s.id);
        }
    }
    assert_eq!(g.consumers_of("elevation"), ["biome", "flow", "height", "slope"]);
    assert_eq!(g.consumers_of("temperature"), ["biome"]);
    assert_eq!(g.consumers_of("slope"), ["biome", "height"]);
    assert!(g.consumers_of("height").is_empty());
    assert_eq!(g.field("slope").unwrap().stage.name(), "regional");
}

// spec: GEN-007
#[test]
fn a_read_of_the_same_or_a_later_stage_is_rejected_naming_both_stages() {
    let mut same = specs();
    same.push(spec("rivers", Stage::Regional, &["flow"], 0.5));
    let e = FieldGraph::new(&same).unwrap_err();
    assert_eq!(e, GraphError::IllegalRead { field: "rivers", field_stage: Stage::Regional, reads: "flow", reads_stage: Stage::Regional });
    let text = e.to_string();
    assert!(text.contains("rivers") && text.contains("regional stage") && text.contains("flow"), "{text}");

    let mut later = specs();
    later.push(spec("uplift", Stage::GlobalCoarse, &["height"], 0.0));
    let e = FieldGraph::new(&later).unwrap_err();
    assert_eq!(
        e,
        GraphError::IllegalRead { field: "uplift", field_stage: Stage::GlobalCoarse, reads: "height", reads_stage: Stage::Local }
    );
    assert!(e.to_string().contains("global-coarse stage") && e.to_string().contains("local stage"), "{e}");

    let mut cyclic = specs();
    cyclic.push(spec("a", Stage::Local, &["b"], 0.0));
    cyclic.push(spec("b", Stage::Local, &["a"], 0.0));
    assert!(matches!(FieldGraph::new(&cyclic), Err(GraphError::IllegalRead { .. })), "a cycle needs a same-stage read");
}

// spec: GEN-007
#[test]
fn unknown_duplicate_and_badly_haloed_fields_are_rejected() {
    let mut unknown = specs();
    unknown.push(spec("x", Stage::Local, &["nowhere"], 0.0));
    assert_eq!(FieldGraph::new(&unknown).unwrap_err(), GraphError::UnknownInput { field: "x", reads: "nowhere" });
    let mut dup = specs();
    dup.push(spec("slope", Stage::Regional, &[], 0.0));
    assert_eq!(FieldGraph::new(&dup).unwrap_err(), GraphError::DuplicateField { id: "slope" });
    let mut twice = specs();
    twice.push(spec("d", Stage::Local, &["elevation", "slope", "elevation"], 0.0));
    let e = FieldGraph::new(&twice).unwrap_err();
    assert_eq!(e, GraphError::DuplicateRead { field: "d", reads: "elevation" });
    assert!(e.to_string().contains("'d'") && e.to_string().contains("'elevation'"));
    for halo in [f64::NAN, -0.1, 1.5, f64::INFINITY] {
        let mut s = specs();
        s.push(spec("h", Stage::Regional, &["elevation"], halo));
        assert_eq!(FieldGraph::new(&s).unwrap_err(), GraphError::BadHalo { field: "h" }, "{halo}");
    }
    // Only regional fields have a halo.
    let mut local_halo = specs();
    local_halo.push(spec("h", Stage::Local, &["elevation"], 0.25));
    assert_eq!(FieldGraph::new(&local_halo).unwrap_err(), GraphError::BadHalo { field: "h" });
}

// spec: GEN-007
#[test]
fn bumping_one_fields_version_re_keys_exactly_the_fields_downstream_of_it() {
    let base = graph(&specs()).all_keys(tile(), &ctx());
    // The root of a chain: everything below it changes, the unrelated global field does not.
    let root = graph(&with("elevation", |s| s.version = 2)).all_keys(tile(), &ctx());
    assert_eq!(changed(&base, &root), ["biome", "elevation", "flow", "height", "slope"]);
    // A field in the middle: only it and what reads it; its inputs and siblings keep their keys.
    let middle = graph(&with("slope", |s| s.version = 2)).all_keys(tile(), &ctx());
    assert_eq!(changed(&base, &middle), ["biome", "height", "slope"]);
    // A leaf changes only itself.
    let leaf = graph(&with("height", |s| s.version = 2)).all_keys(tile(), &ctx());
    assert_eq!(changed(&base, &leaf), ["height"]);
    // The same holds for the implementation id and the halo.
    let implementation = graph(&with("elevation", |s| s.implementation_id = "gpu-test")).all_keys(tile(), &ctx());
    assert_eq!(changed(&base, &implementation), ["biome", "elevation", "flow", "height", "slope"]);
    let halo = graph(&with("slope", |s| s.halo_fraction = 0.5)).all_keys(tile(), &ctx());
    assert_eq!(changed(&base, &halo), ["biome", "height", "slope"]);
}

// spec: GEN-007
#[test]
fn every_part_of_a_fields_identity_changes_its_key_on_its_own() {
    let up = BTreeMap::from([("elevation", 5u64)]);
    let g = graph(&specs());
    let key = |g: &FieldGraph, id: &str, scope: Scope, c: &OutputContext, up: &BTreeMap<&str, u64>| g.output_key(id, scope, c, up);
    let at = Scope::Tile(tile());
    let base = key(&g, "slope", at, &ctx(), &up);
    assert_eq!(base, key(&g, "slope", at, &ctx(), &up), "deterministic");

    // The scope, the context and the inputs.
    let other_tile = Scope::Tile(TileId::new(Face(2), 5, 3, 8).unwrap());
    assert_ne!(base, key(&g, "slope", other_tile, &ctx(), &up), "tile");
    for (what, c) in [
        ("recorded read set", OutputContext { inputs_hash: 100, ..ctx() }),
        ("body", OutputContext { body: BodyId(1), ..ctx() }),
        ("surface", OutputContext { surface_id: "ellipsoid", ..ctx() }),
        ("face mapping", OutputContext { face_mapping_id: "other-warp", ..ctx() }),
        ("resolution", OutputContext { resolution: 64, ..ctx() }),
    ] {
        assert_ne!(base, key(&g, "slope", at, &c, &up), "{what}");
    }
    assert_ne!(base, key(&g, "slope", at, &ctx(), &BTreeMap::from([("elevation", 6u64)])), "an input's key");

    // The field itself, each part alone.
    let changed_spec = |f: &dyn Fn(&mut FieldSpec)| key(&graph(&with("slope", f)), "slope", at, &ctx(), &up);
    assert_ne!(base, changed_spec(&|s| s.version = 2), "version");
    assert_ne!(base, changed_spec(&|s| s.implementation_id = "gpu-test"), "implementation id");
    assert_ne!(base, changed_spec(&|s| s.halo_fraction = 0.5), "halo");
    assert_eq!(base, changed_spec(&|_| {}));

    // The id alone: the same field under another name (nothing else reads either).
    let mut renamed = specs();
    renamed.retain(|s| s.id != "height" && s.id != "biome");
    renamed.push(spec("slope2", Stage::Regional, &["elevation"], 0.25));
    assert_ne!(base, key(&graph(&renamed), "slope2", at, &ctx(), &up), "id");

    // The stage alone: a local and a regional field with no halo and the same input.
    let probe = |stage: Stage| {
        let mut s = specs();
        s.push(spec("probe", stage, &["elevation"], 0.0));
        key(&graph(&s), "probe", at, &ctx(), &up)
    };
    assert_ne!(probe(Stage::Regional), probe(Stage::Local), "stage");

    // The name of an input, with the same upstream key: swapping which field is read changes the key.
    let reads = |name: &'static str| {
        let mut s = specs();
        s.push(spec("probe", Stage::Regional, if name == "elevation" { &["elevation"] } else { &["temperature"] }, 0.0));
        key(&graph(&s), "probe", at, &ctx(), &BTreeMap::from([(name, 5u64)]))
    };
    assert_ne!(reads("elevation"), reads("temperature"), "the name of an input");

    // The number of inputs.
    let mut two = specs();
    two.push(spec("probe", Stage::Regional, &["elevation", "temperature"], 0.0));
    let mut one = specs();
    one.push(spec("probe", Stage::Regional, &["elevation"], 0.0));
    let both = BTreeMap::from([("elevation", 5u64), ("temperature", 5u64)]);
    assert_ne!(key(&graph(&two), "probe", at, &ctx(), &both), key(&graph(&one), "probe", at, &ctx(), &both), "the number of inputs");
}

// spec: GEN-007
#[test]
fn the_order_of_the_inputs_and_a_negative_zero_halo_do_not_change_a_key() {
    let a =
        vec![spec("e", Stage::GlobalCoarse, &[], 0.0), spec("t", Stage::GlobalCoarse, &[], 0.0), spec("x", Stage::Local, &["e", "t"], 0.0)];
    let b =
        vec![spec("e", Stage::GlobalCoarse, &[], 0.0), spec("t", Stage::GlobalCoarse, &[], 0.0), spec("x", Stage::Local, &["t", "e"], 0.0)];
    assert_eq!(graph(&a).all_keys(tile(), &ctx()), graph(&b).all_keys(tile(), &ctx()));
    let zero = vec![spec("e", Stage::GlobalCoarse, &[], 0.0), spec("r", Stage::Regional, &["e"], 0.0)];
    let negative = vec![spec("e", Stage::GlobalCoarse, &[], 0.0), spec("r", Stage::Regional, &["e"], -0.0)];
    assert_eq!(graph(&zero).all_keys(tile(), &ctx()), graph(&negative).all_keys(tile(), &ctx()), "-0.0 is 0.0");
}

// spec: GEN-007
#[test]
fn a_scope_must_fit_the_stage_and_global_and_tile_scopes_never_alias() {
    let g = graph(&specs());
    let up = BTreeMap::from([("elevation", 5u64)]);
    // The tile `raw() == 0` is a real tile: it must not alias the global scope.
    let root = TileId::from_raw(0).expect("raw id 0 is face 0, level 0");
    let probe = graph(&[spec("p", Stage::GlobalCoarse, &[], 0.0), spec("r", Stage::Regional, &["p"], 0.0)]);
    let kp = BTreeMap::from([("p", 1u64)]);
    let global = probe.output_key("p", Scope::Global, &ctx(), &BTreeMap::new());
    let regional_root = probe.output_key("r", Scope::Tile(root), &ctx(), &kp);
    assert_ne!(global, regional_root);
    let panics = |f: &dyn Fn()| std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err();
    assert!(
        panics(&|| {
            let _ = g.output_key("elevation", Scope::Tile(tile()), &ctx(), &BTreeMap::new());
        }),
        "a global field keyed per tile"
    );
    assert!(
        panics(&|| {
            let _ = g.output_key("slope", Scope::Global, &ctx(), &up);
        }),
        "a regional field keyed globally"
    );
    assert!(
        panics(&|| {
            let _ = g.output_key("height", Scope::Global, &ctx(), &BTreeMap::new());
        }),
        "a local field keyed globally"
    );
}

// spec: GEN-007
#[test]
#[should_panic(expected = "the key of input 'elevation' of field 'slope' was not supplied")]
fn a_key_that_leaves_an_input_out_is_a_bug_not_a_stale_tile() {
    let g = graph(&specs());
    let _ = g.output_key("slope", Scope::Tile(tile()), &ctx(), &BTreeMap::new());
}

// spec: GEN-007
#[test]
fn the_stage_key_layout_has_a_version_and_pinned_known_answers() {
    assert_eq!(STAGE_KEY_VERSION, 1, "a new stage key layout needs a new version and new known answers");
    let keys = graph(&specs()).all_keys(tile(), &ctx());
    let got: Vec<(&str, u64)> = keys.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(got, KNOWN);
}

const KNOWN: [(&str, u64); 6] = [
    ("biome", 4264704039266636079),
    ("elevation", 16113924286495544269),
    ("flow", 12132567731774507209),
    ("height", 5167291103920336170),
    ("slope", 11751902504623432559),
    ("temperature", 865822066384076764),
];
