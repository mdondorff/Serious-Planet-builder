//! VIEW-011: the editor UI is one more node of the lit frame graph, after the tone map, and touches nothing of the HDR frame.

use planet_render::graph::*;

fn names(g: &RenderGraph, c: &CompiledGraph) -> Vec<&'static str> {
    c.order.iter().map(|&i| g.passes()[i].name).collect()
}

// spec: VIEW-011
#[test]
fn the_ui_node_follows_the_tone_map_and_changes_no_frame_target() {
    for (atmosphere, water) in [(false, false), (true, false), (true, true)] {
        let shape = LitShape { size: (2560, 1440), adaptive: true, atmosphere, water };
        let plain = lit_frame_graph(&shape);
        let with = lit_frame_graph_with_ui(&shape);
        let (a, b) = (plain.compile().unwrap(), with.compile().unwrap());
        let order = names(&with, &b);
        assert_eq!(&order[order.len() - 2..], ["tonemap", "ui"], "atmosphere {atmosphere} water {water}: {order:?}");
        let mut expected = names(&plain, &a);
        expected.push("ui");
        assert_eq!(order, expected, "every other pass keeps its place");
        assert_eq!(a.frame_target_bytes(), b.frame_target_bytes(), "the UI adds no frame target bytes");
        assert_eq!(a.lifetimes, b.lifetimes, "no HDR, depth or histogram lifetime moves");
        assert!(b.culled.is_empty());
    }
}
