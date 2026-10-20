//! Tier A: the water pass in the lit frame graph (ADR 0041 point 4, ADR 0042 point 2 pass 4, ADR 0048 point 1): its place between the
//! terrain and the composite, the copy of the scene target it reads, the frame-target share re-asserted from the compiled graph.

use planet_render::graph::{frame_target_share_bytes, lit_frame_graph, CompiledGraph, GraphError, LitShape, RenderGraph};

const P1440: (u32, u32) = (2560, 1440);

fn compile(atmosphere: bool, adaptive: bool, water: bool) -> (RenderGraph, CompiledGraph) {
    let g = lit_frame_graph(&LitShape { size: P1440, adaptive, atmosphere, water });
    let c = g.compile().expect("the lit frame compiles");
    (g, c)
}

fn names(g: &RenderGraph, c: &CompiledGraph) -> Vec<&'static str> {
    c.order.iter().map(|&i| g.passes()[i].name).collect()
}

// spec: ATMO-016
#[test]
fn the_water_pass_runs_after_the_terrain_and_before_the_composite_with_an_atmosphere() {
    let (g, c) = compile(true, true, true);
    assert_eq!(
        names(&g, &c),
        [
            "atmosphere-luts",
            "sky-view",
            "aerial-perspective",
            "terrain",
            "scene-copy",
            "water",
            "sky-composite",
            "exposure-histogram",
            "exposure-adapt",
            "tonemap"
        ]
    );
    assert!(c.culled.is_empty(), "{:?}", c.culled);
    let life = |r: &str| c.lifetimes.iter().find(|l| l.resource == r).unwrap_or_else(|| panic!("no lifetime for {r}")).clone();
    // The copy lives from its pass to the water pass; the scene target and the depth buffer live through the water to the composite.
    assert_eq!((life("scene-copy").first, life("scene-copy").last), (4, 5));
    assert_eq!((life("hdr-scene").first, life("hdr-scene").last), (3, 6));
    assert_eq!((life("depth").first, life("depth").last), (3, 6), "the composite reads the depth the water wrote");
    // Fixed exposure still culls only the histogram.
    let (g, f) = compile(true, false, true);
    assert_eq!(f.culled, ["exposure-histogram"]);
    assert_eq!(names(&g, &f).len(), 9);
}

// spec: ATMO-016
#[test]
fn without_an_atmosphere_the_water_draws_over_the_hdr_target_and_a_frame_without_water_is_unchanged() {
    let (g, c) = compile(false, true, true);
    assert_eq!(names(&g, &c), ["terrain", "scene-copy", "water", "exposure-histogram", "exposure-adapt", "tonemap"]);
    // The frames without water are the graphs of before the water pass: same passes, same bytes.
    for (atmosphere, passes, bytes) in [(false, 4usize, 44_237_072u64), (true, 8, 74_434_832 + 14_745_600)] {
        let (g, c) = compile(atmosphere, true, false);
        assert_eq!(names(&g, &c).len(), passes);
        assert_eq!(c.frame_target_bytes(), bytes, "atmosphere {atmosphere}");
        assert!(!names(&g, &c).contains(&"water") && !names(&g, &c).contains(&"scene-copy"));
    }
}

// spec: ATMO-016
#[test]
fn a_water_pass_without_its_copy_fails_the_compile_with_the_names() {
    let mut g = RenderGraph::new(&["presentable"]);
    g.add_pass("terrain", &[], &["hdr", "depth"], false)
        .add_pass("water", &["scene-copy", "hdr", "depth"], &["hdr", "depth"], false)
        .add_pass("tonemap", &["hdr"], &["presentable"], true);
    assert_eq!(g.compile().unwrap_err(), GraphError::MissingProducer { pass: "water", resource: "scene-copy" });
}

// spec: ATMO-016, REND-020
#[test]
fn the_water_pass_adds_one_hdr_sized_copy_and_the_frame_stays_inside_the_share_at_1440p() {
    let (_, without) = compile(true, true, false);
    let (_, with) = compile(true, true, true);
    let bytes = |c: &CompiledGraph, r: &str| c.lifetimes.iter().find(|l| l.resource == r).unwrap().bytes;
    assert_eq!(bytes(&with, "scene-copy"), 29_491_200, "an HDR-sized copy of the scene target (Rgba16Float)");
    assert_eq!(with.frame_target_bytes() - without.frame_target_bytes(), 29_491_200);
    assert_eq!(
        with.frame_target_bytes(),
        118_671_632,
        "the atmosphere frame with water at 1440p (the half-resolution haze of the camera march included, 14,745,600 B)"
    );
    let share = frame_target_share_bytes(P1440);
    assert!(with.frame_target_bytes() <= share, "{} B against the share of {share} B", with.frame_target_bytes());
    // The plain lit frame with water too.
    let (_, plain) = compile(false, true, true);
    assert_eq!(plain.frame_target_bytes(), 44_237_072 + 29_491_200);
    assert!(plain.frame_target_bytes() <= share);
    println!("the lit frame with the atmosphere and water at 1440p: {} B of the share's {share} B", with.frame_target_bytes());
}
