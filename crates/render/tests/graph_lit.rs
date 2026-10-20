//! Tier A: the lit frame as a compiled graph (REND-019) and the frame-target share asserted from it (REND-020). Pure, no GPU.

use planet_render::graph::*;

const P1440: (u32, u32) = (2560, 1440);

fn compile(size: (u32, u32), adaptive: bool) -> (RenderGraph, CompiledGraph) {
    let g = lit_frame_graph(&LitShape { size, adaptive, atmosphere: false, water: false });
    let c = g.compile().expect("the lit frame compiles");
    (g, c)
}

fn names(g: &RenderGraph, c: &CompiledGraph) -> Vec<&'static str> {
    c.order.iter().map(|&i| g.passes()[i].name).collect()
}

// spec: REND-019
#[test]
fn the_adaptive_lit_frame_runs_terrain_histogram_adaptation_and_tone_map_in_that_order() {
    let (g, c) = compile(P1440, true);
    assert_eq!(names(&g, &c), ["terrain", "exposure-histogram", "exposure-adapt", "tonemap"]);
    assert!(c.culled.is_empty(), "{:?}", c.culled);
    // The scene target lives from the terrain pass to the tone map; the histogram from its pass to the adaptation.
    let life = |r: &str| c.lifetimes.iter().find(|l| l.resource == r).unwrap_or_else(|| panic!("no lifetime for {r}")).clone();
    assert_eq!((life("hdr").first, life("hdr").last), (0, 3));
    assert_eq!((life("histogram").first, life("histogram").last), (1, 2));
    assert_eq!((life("depth").first, life("depth").last), (0, 0));
    assert!(c.lifetimes.iter().all(|l| l.resource != "presentable"), "the presentable target is external, not a frame target");
}

// spec: REND-019
#[test]
fn fixed_exposure_culls_the_histogram_in_the_compile_and_keeps_the_rest() {
    let (g, c) = compile(P1440, false);
    assert_eq!(names(&g, &c), ["terrain", "exposure-adapt", "tonemap"]);
    assert_eq!(c.culled, ["exposure-histogram"]);
    assert!(c.lifetimes.iter().all(|l| l.resource != "histogram"));
}

// spec: REND-019
#[test]
fn the_lit_frame_is_rejected_when_a_pass_loses_its_producer() {
    // A frame that tone maps without a terrain pass fails the compile with the names, as REND-012 requires.
    let mut g = RenderGraph::new(&["presentable"]);
    g.add_pass("tonemap", &["hdr", "exposure-state"], &["presentable"], true);
    let e = g.compile().unwrap_err();
    assert_eq!(e, GraphError::MissingProducer { pass: "tonemap", resource: "hdr" });
}

// spec: REND-019
#[test]
fn the_compile_is_a_pure_function_of_the_shape() {
    let a = lit_frame_graph(&LitShape { size: P1440, adaptive: true, atmosphere: false, water: false }).compile().unwrap();
    let b = lit_frame_graph(&LitShape { size: P1440, adaptive: true, atmosphere: false, water: false }).compile().unwrap();
    assert_eq!(a, b);
    let other = lit_frame_graph(&LitShape { size: (1280, 720), adaptive: true, atmosphere: false, water: false }).compile().unwrap();
    assert_ne!(a, other, "a new size is a new compiled frame");
    assert_eq!(a.order, other.order, "the order does not depend on the size");
}

// spec: REND-020
#[test]
fn the_frame_targets_at_1440p_are_counted_from_the_compiled_graph() {
    let (_, c) = compile(P1440, true);
    let bytes = |r: &str| c.lifetimes.iter().find(|l| l.resource == r).unwrap().bytes;
    // ADR 0042 point 5: HDR 29.5 MB, depth 14.7 MB.
    assert_eq!(bytes("hdr"), 2560 * 1440 * 8);
    assert_eq!(bytes("hdr"), 29_491_200);
    assert_eq!(bytes("depth"), 2560 * 1440 * 4);
    assert_eq!(bytes("depth"), 14_745_600);
    assert_eq!(bytes("histogram"), 256);
    assert_eq!(c.persistent, [("exposure-state", 16)]);
    assert_eq!(c.transient_bytes(), 29_491_200 + 14_745_600 + 256);
    assert_eq!(c.frame_target_bytes(), 44_237_072, "the adaptive lit frame at 1440p");
    // Fixed exposure allocates no histogram pass; the renderer still owns the buffer (see the tier C allocation test).
    let (_, f) = compile(P1440, false);
    assert_eq!(f.frame_target_bytes(), 44_237_072 - 256);
}

// spec: REND-020
#[test]
fn the_frame_target_share_holds_and_a_growth_beyond_it_fails_with_the_numbers() {
    assert_eq!(frame_target_share_bytes(P1440), 40 * 2560 * 1440 + 32 * 1024 * 1024);
    assert_eq!(frame_target_share_bytes(P1440), 181_010_432);
    for size in [(64, 64), (128, 96), (1920, 1080), P1440, (3840, 2160)] {
        let (_, c) = compile(size, true);
        let (used, cap) = (c.frame_target_bytes(), frame_target_share_bytes(size));
        assert!(used <= cap, "{size:?}: the lit frame's targets take {used} B, over the frame-target share of {cap} B");
    }
    // The share is a small fraction of what the terrain share leaves of the mid tier (8 GiB - 2 GiB).
    let rest_of_mid_tier = (8u64 - 2) * 1024 * 1024 * 1024;
    assert!(frame_target_share_bytes(P1440) * 100 / rest_of_mid_tier < 3);
    // A deliberately wasteful frame (eight more HDR-sized copies) is caught with its numbers: this is how the
    // water pass's refraction copy will be checked when it lands.
    let mut g = RenderGraph::new(&["presentable"]);
    let px = 2560u64 * 1440;
    g.describe_sized("hdr", "a", px * 8).describe_sized("copy", "b", px * 8 * 8);
    g.add_pass("terrain", &[], &["hdr", "copy"], false).add_pass("tonemap", &["hdr", "copy"], &["presentable"], true);
    let c = g.compile().unwrap();
    assert!(c.frame_target_bytes() > frame_target_share_bytes(P1440), "{} B", c.frame_target_bytes());
}

// spec: REND-020
#[test]
fn equal_class_targets_with_disjoint_lifetimes_could_share_memory() {
    let mut g = RenderGraph::new(&["out"]);
    g.describe_sized("a", "k", 100).describe_sized("b", "k", 100).describe_sized("c", "k", 100);
    g.add_pass("p0", &[], &["a"], false).add_pass("p1", &["a"], &["b"], false).add_pass("p2", &["b"], &["c"], false).add_pass(
        "p3",
        &["c"],
        &["out"],
        true,
    );
    let c = g.compile().unwrap();
    assert_eq!(c.transient_bytes(), 300);
    assert_eq!(c.aliased_transient_bytes(), 200, "a and c can share: two allocations of 100 instead of three");
}

fn compile_atmosphere(size: (u32, u32), adaptive: bool) -> (RenderGraph, CompiledGraph) {
    let g = lit_frame_graph(&LitShape { size, adaptive, atmosphere: true, water: false });
    let c = g.compile().expect("the lit frame with an atmosphere compiles");
    (g, c)
}

// spec: ATMO-011
#[test]
fn the_atmosphere_frame_runs_the_tables_the_products_the_terrain_and_the_composite_in_that_order() {
    let (g, c) = compile_atmosphere(P1440, true);
    assert_eq!(
        names(&g, &c),
        [
            "atmosphere-luts",
            "sky-view",
            "aerial-perspective",
            "terrain",
            "sky-composite",
            "exposure-histogram",
            "exposure-adapt",
            "tonemap"
        ]
    );
    assert!(c.culled.is_empty(), "{:?}", c.culled);
    let life = |r: &str| c.lifetimes.iter().find(|l| l.resource == r).unwrap_or_else(|| panic!("no lifetime for {r}")).clone();
    // The terrain draws into the scene target and the one depth buffer; the composite reads both and writes the HDR target.
    assert_eq!((life("hdr-scene").first, life("hdr-scene").last), (3, 4));
    assert_eq!((life("depth").first, life("depth").last), (3, 4), "the composite reads the terrain's depth");
    assert_eq!((life("hdr").first, life("hdr").last), (4, 7));
    assert_eq!((life("sky-view-lut").first, life("sky-view-lut").last), (1, 4));
    assert_eq!((life("aerial-perspective-volume").first, life("aerial-perspective-volume").last), (2, 4));
    // Fixed exposure still culls only the histogram.
    let (g, f) = compile_atmosphere(P1440, false);
    assert_eq!(f.culled, ["exposure-histogram"]);
    assert_eq!(names(&g, &f).len(), 7);
}

// spec: ATMO-011
#[test]
fn a_composite_without_its_inputs_fails_the_compile_with_the_names() {
    let mut g = RenderGraph::new(&["presentable"]);
    g.add_pass("sky-composite", &["hdr-scene", "depth", "sky-view-lut"], &["hdr"], false).add_pass(
        "tonemap",
        &["hdr"],
        &["presentable"],
        true,
    );
    assert_eq!(g.compile().unwrap_err(), GraphError::MissingProducer { pass: "sky-composite", resource: "hdr-scene" });
}

// spec: ATMO-011, REND-020
#[test]
fn the_atmosphere_adds_its_targets_to_the_frame_target_share_and_stays_inside_it() {
    let (_, plain) = compile(P1440, true);
    let (_, c) = compile_atmosphere(P1440, true);
    let bytes = |r: &str| c.lifetimes.iter().find(|l| l.resource == r).unwrap().bytes;
    assert_eq!(bytes("hdr-scene"), 29_491_200, "a second HDR-sized target");
    assert_eq!(bytes("sky-view-lut"), 192 * 108 * 8);
    assert_eq!(bytes("aerial-perspective-volume"), 32 * 32 * 32 * 8);
    // The half-resolution haze of the camera march (ATMO-021): two Rgba16Float targets of 1280 x 720.
    assert_eq!(
        c.persistent,
        [("exposure-state", 16), ("haze-lowres", 14_745_600), ("multiscatter-lut", 16_384), ("transmittance-lut", 262_144)]
    );
    let extra = c.frame_target_bytes() - plain.frame_target_bytes();
    assert_eq!(extra, 29_491_200 + 165_888 + 262_144 + 262_144 + 16_384 + 14_745_600);
    assert!(c.frame_target_bytes() <= frame_target_share_bytes(P1440), "{} B over the share", c.frame_target_bytes());
    for size in [(64, 64), (128, 96), (1920, 1080), (3840, 2160)] {
        let (_, c) = compile_atmosphere(size, true);
        assert!(c.frame_target_bytes() <= frame_target_share_bytes(size), "{size:?}: {} B", c.frame_target_bytes());
    }
}
