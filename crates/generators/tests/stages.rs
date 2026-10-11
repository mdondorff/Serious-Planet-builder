//! GEN-007: staged generation. A stage reads only the outputs of earlier stages, and a field's key chains the keys of its
//! inputs.

use std::collections::BTreeMap;

use planet_generators::stage::{FieldGraph, FieldSpec, GraphError, Stage};

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
fn bumping_one_fields_version_re_keys_exactly_its_consumers() {
    let base = graph(&specs()).all_keys(7, 99);
    let mut bumped = specs();
    bumped.iter_mut().find(|s| s.id == "elevation").unwrap().version = 2;
    let after = graph(&bumped).all_keys(7, 99);
    let changed: Vec<&str> = base.keys().filter(|k| base[*k] != after[*k]).copied().collect();
    // Everything downstream of elevation changes, biome included (it reads slope, which reads elevation).
    let mut expected = graph(&specs()).consumers_of("elevation");
    expected.push("elevation");
    expected.sort_unstable();
    let mut got: Vec<&str> = changed;
    got.sort_unstable();
    assert_eq!(got, expected);
    assert_eq!(base["temperature"], after["temperature"], "an unrelated global field keeps its key");
}

// spec: GEN-007
#[test]
fn every_part_of_a_fields_identity_changes_its_key() {
    let g = graph(&specs());
    let up = BTreeMap::from([("elevation", 5u64)]);
    let base = g.output_key("slope", 11, 22, &up);
    assert_eq!(base, g.output_key("slope", 11, 22, &up), "deterministic");
    assert_ne!(base, g.output_key("slope", 12, 22, &up), "scope (tile)");
    assert_ne!(base, g.output_key("slope", 11, 23, &up), "recorded read set");
    assert_ne!(base, g.output_key("slope", 11, 22, &BTreeMap::from([("elevation", 6u64)])), "an input's key");
    assert_ne!(base, g.output_key("flow", 11, 22, &up), "field id");
    let variant = |f: &dyn Fn(&mut FieldSpec)| {
        let mut s = specs();
        f(s.iter_mut().find(|x| x.id == "slope").unwrap());
        graph(&s).output_key("slope", 11, 22, &up)
    };
    assert_ne!(base, variant(&|s| s.version = 2), "version");
    assert_ne!(base, variant(&|s| s.implementation_id = "gpu-test"), "implementation id");
    assert_ne!(base, variant(&|s| s.halo_fraction = 0.5), "halo");
    assert_eq!(base, variant(&|_| {}));
}

// spec: GEN-007
#[test]
#[should_panic(expected = "the key of input 'elevation' of field 'slope' was not supplied")]
fn a_key_that_leaves_an_input_out_is_a_bug_not_a_stale_tile() {
    let g = graph(&specs());
    let _ = g.output_key("slope", 1, 2, &BTreeMap::new());
}
