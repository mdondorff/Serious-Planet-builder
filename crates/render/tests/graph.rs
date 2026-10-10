//! REND-012: the render graph declaration and its compile-time checks (pure, no GPU).

use planet_render::graph::{GraphError, RenderGraph};

fn terrain_frame() -> RenderGraph {
    let mut g = RenderGraph::new(&["swapchain"]);
    g.describe("depth", "depth32-1x").describe("color", "rgba8-1x").describe("hdr", "rgba8-1x").describe("shadow", "depth32-1x");
    g.add_pass("depth-prepass", &[], &["depth"], false)
        .add_pass("terrain", &["depth"], &["color"], false)
        .add_pass("unused-ssao", &["depth"], &["ao"], false)
        .add_pass("tonemap", &["color"], &["hdr"], false)
        .add_pass("present", &["hdr"], &["swapchain"], true);
    g
}

// spec: REND-012
#[test]
fn passes_are_ordered_unused_ones_are_culled_and_lifetimes_are_computed() {
    let c = terrain_frame().compile().unwrap();
    assert_eq!(c.culled, ["unused-ssao"], "a pass whose output nobody reads is dropped");
    assert_eq!(c.order, [0, 1, 3, 4]);
    let life = |name: &str| c.lifetimes.iter().find(|l| l.resource == name).unwrap().clone();
    assert_eq!((life("depth").first, life("depth").last), (0, 1));
    assert_eq!((life("color").first, life("color").last), (1, 2));
    assert_eq!((life("hdr").first, life("hdr").last), (2, 3));
    assert!(c.lifetimes.iter().all(|l| l.resource != "swapchain"), "external resources are not transient");
}

// spec: REND-012
#[test]
fn transient_resources_with_disjoint_lifetimes_and_equal_class_share_memory() {
    let c = terrain_frame().compile().unwrap();
    // color (1..2) and hdr (2..3) overlap at pass 2, so they must not alias; depth is a different class.
    let together = |a: &str, b: &str| c.alias_groups.iter().any(|g| g.contains(&a) && g.contains(&b));
    assert!(!together("color", "hdr"));
    assert!(!together("depth", "color"));
    // A later, same-class resource that starts after `color` ended can reuse it.
    // a (0..1), b (1..2), c (2..3): a is last read in the pass that writes b, so they must not alias; a and c can.
    let mut g = RenderGraph::new(&["swapchain"]);
    g.describe("a", "rgba8-1x").describe("b", "rgba8-1x").describe("c", "rgba8-1x");
    g.add_pass("p0", &[], &["a"], false).add_pass("p1", &["a"], &["b"], false).add_pass("p2", &["b"], &["c"], false).add_pass(
        "p3",
        &["c"],
        &["swapchain"],
        true,
    );
    let c = g.compile().unwrap();
    let together = |x: &str, y: &str| c.alias_groups.iter().any(|grp| grp.contains(&x) && grp.contains(&y));
    assert!(together("a", "c"), "{:?}", c.alias_groups);
    assert!(!together("a", "b") && !together("b", "c"), "{:?}", c.alias_groups);
}

// spec: REND-012
#[test]
fn a_read_without_a_producer_and_duplicate_passes_are_errors_with_names() {
    let mut g = RenderGraph::new(&[]);
    g.add_pass("terrain", &["depth"], &["color"], true);
    let e = g.compile().unwrap_err();
    assert_eq!(e, GraphError::MissingProducer { pass: "terrain", resource: "depth" });
    assert!(e.to_string().contains("terrain") && e.to_string().contains("depth"));
    let mut g = RenderGraph::new(&[]);
    g.add_pass("x", &[], &["a"], true).add_pass("x", &[], &["b"], true);
    assert_eq!(g.compile().unwrap_err(), GraphError::DuplicatePass("x"));
    // A read that is produced only by a LATER pass is also an error (declaration order is execution order).
    let mut g = RenderGraph::new(&[]);
    g.add_pass("consumer", &["late"], &[], true).add_pass("producer", &[], &["late"], false);
    assert!(matches!(g.compile().unwrap_err(), GraphError::MissingProducer { .. }));
}

// spec: REND-012
#[test]
fn the_latest_earlier_writer_is_the_producer_and_side_effects_anywhere_are_roots() {
    // `late` rewrites `x`; `use` reads the second version, so the first writer is NOT needed, but both writers are kept
    // only if something reads the first version. Here nothing does: the early writer is culled.
    let mut g = RenderGraph::new(&[]);
    g.add_pass("early", &[], &["x"], false).add_pass("late", &[], &["x"], false).add_pass("use", &["x"], &["out"], true);
    let c = g.compile().unwrap();
    assert_eq!(c.culled, ["early"], "only the latest writer feeds the reader");
    // A side-effect pass in the middle is a root even though later passes exist (and a pass after it that nothing needs is culled).
    let mut g = RenderGraph::new(&[]);
    g.add_pass("make", &[], &["a"], false).add_pass("readback", &["a"], &[], true).add_pass("dead", &[], &["z"], false);
    let c = g.compile().unwrap();
    assert_eq!(c.order, [0, 1]);
    assert_eq!(c.culled, ["dead"]);
}

// spec: REND-012
#[test]
fn aliasing_needs_equal_class_and_no_overlap_with_any_group_member() {
    // t0 (0..1) and t2 (2..3) are rgba8; mid (1..2) is depth. t4 (4..5) is rgba8 but overlaps nothing -> joins a group
    // whose members ALL ended; none of the unclassified resources may alias.
    let mut g = RenderGraph::new(&["sc"]);
    g.describe("t0", "rgba8").describe("mid", "depth").describe("t2", "rgba8");
    g.add_pass("p0", &[], &["t0"], false)
        .add_pass("p1", &["t0"], &["mid"], false)
        .add_pass("p2", &["mid"], &["t2"], false)
        .add_pass("p3", &["t2"], &["u1"], false)
        .add_pass("p4", &["u1"], &["u2"], false)
        .add_pass("p5", &["u2"], &["sc"], true);
    let c = g.compile().unwrap();
    let together = |x: &str, y: &str| c.alias_groups.iter().any(|grp| grp.contains(&x) && grp.contains(&y));
    assert!(together("t0", "t2"), "same class, disjoint lifetimes: {:?}", c.alias_groups);
    assert!(!together("t0", "mid") && !together("mid", "t2"), "different classes never alias: {:?}", c.alias_groups);
    assert!(!together("u1", "u2") && !together("u1", "t0"), "unclassified resources are never aliased: {:?}", c.alias_groups);
    // A third same-class resource alive during t2 must not join the {t0, t2} group even though it misses t0.
    let mut g = RenderGraph::new(&["sc"]);
    g.describe("a", "k").describe("b", "k").describe("c", "k");
    g.add_pass("p0", &[], &["a"], false).add_pass("p1", &["a"], &["b"], false).add_pass("p2", &["b"], &["c"], false).add_pass(
        "p3",
        &["b", "c"],
        &["sc"],
        true,
    );
    let c = g.compile().unwrap();
    let together = |x: &str, y: &str| c.alias_groups.iter().any(|grp| grp.contains(&x) && grp.contains(&y));
    assert!(together("a", "c") && !together("b", "c"), "b outlives the write of c: {:?}", c.alias_groups);
}
