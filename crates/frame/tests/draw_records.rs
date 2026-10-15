//! REND-015, TEST-011: the argument records of an indirect multi-draw equal the plan's nodes, and the bytes a frame writes for
//! them are counted per standard view (a declared addition to the pinned counts: no existing pin changes).

use planet_frame::draw::{build_draw_list, indirect_frame_bytes, DRAW_RECORD_BYTES, NODE_RECORD_BYTES};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_streaming::{PoolKind, SlotRef};

const R: f64 = 6_371_000.0;

/// Node counts of the standard views over the reference field (the same table as `terrain.rs`, VIEW_NODE_PINS).
const VIEW_NODES: [(&str, usize); 10] = [
    ("orbit-20000km", 52),
    ("high-2000km", 428),
    ("aerial-200km", 630),
    ("aerial-20km", 892),
    ("low-2km", 1279),
    ("ground-200m", 1694),
    ("ground-1m", 2897),
    ("north-pole-1km", 1180),
    ("cube-corner-1km", 1606),
    ("face-edge-1km", 1574),
];

// spec: REND-015, TEST-011
#[test]
fn draw_records_equal_the_plan_nodes_and_their_bytes_are_counted_per_standard_view() {
    let size = (2560u32, 1440u32);
    let lod = LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let views = scripted_views(R, f64::from(size.0) / f64::from(size.1), 1);
    let mut report = Vec::new();
    for ((name, camera), (pinned, nodes)) in views.into_iter().zip(VIEW_NODES) {
        assert_eq!(name, pinned);
        let plan = plan_terrain(&TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        // Slots as the renderer's free list would hand them out: the plan's order, lowest first, 256 per block.
        let slots = (0..plan.nodes.len() as u32).map(|i| SlotRef { pool: PoolKind::Main, index: i });
        let (vertices, indices) = (1_221u32, 6_144u32);
        let list = build_draw_list(slots, 256, vertices, indices);
        assert_eq!(list.records.len(), plan.nodes.len(), "{name}: one record per node");
        for (i, r) in list.records.iter().enumerate() {
            assert_eq!(r.first_instance as usize, i, "{name}: first_instance is the node index");
            assert_eq!(r.base_vertex as u64, (i as u64 % 256) * u64::from(vertices), "{name}: base_vertex is the slot's first vertex");
            assert_eq!((r.index_count, r.instance_count), (indices, 1), "{name}");
        }
        assert_eq!(list.runs.iter().map(|r| r.count as usize).sum::<usize>(), plan.nodes.len(), "{name}: the runs cover the records");
        // The bytes a frame writes: node records plus argument records (counts against the per-frame upload limit).
        let bytes = indirect_frame_bytes(plan.nodes.len());
        assert_eq!(bytes, plan.nodes.len() as u64 * (NODE_RECORD_BYTES + DRAW_RECORD_BYTES), "{name}");
        assert_eq!(list.argument_bytes() + plan.nodes.len() as u64 * NODE_RECORD_BYTES, bytes, "{name}");
        assert!(bytes <= nodes as u64 * 212, "{name}: {bytes} B per frame against the pin of {nodes} nodes x 212 B");
        report.push(format!("{name}: {} nodes, {bytes} B per frame", plan.nodes.len()));
    }
    println!("record bytes per frame (indirect path):\n{}", report.join("\n"));
}

// spec: REND-015, STRM-006
#[test]
fn the_shares_node_record_bytes_hold_a_ring_for_the_largest_plan() {
    use planet_frame::pool::{tier_pool, MAX_PLAN_NODES};
    // The renderer keeps two sets (node records, argument records, 96 B of globals) and refuses plans over MAX_PLAN_NODES.
    let need = 2 * (MAX_PLAN_NODES * (NODE_RECORD_BYTES + DRAW_RECORD_BYTES) + 96);
    let pool = tier_pool(planet_streaming::Tier::Mid, 32, 2).unwrap();
    assert!(pool.node_record_bytes >= need, "{} B reserved, {need} B needed", pool.node_record_bytes);
}
