//! LOD-009, GEN-020, tier B: the plan of every camera at 20 threads is the plan of one thread and the per-node evaluation.
//! `plan_origin_batch.rs` (tier A) runs every third camera against three threads.
#![allow(dead_code, unused_imports)]

include!("support/origin_batch.rs");

// spec: LOD-009
// spec: GEN-020
#[test]
fn the_plan_equals_the_per_node_evaluation_at_every_thread_count() {
    let any = Blocks::new(usize::MAX);
    let field = eroded(&any);
    let map = TangentWarp;
    for (name, camera) in cameras(&field) {
        let req = request(camera, &field);
        let one = plan_terrain_cached(&req, &field, &mut SelectionCache::new()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let many = plan_terrain_threaded(&req, &field, &mut SelectionCache::new(), 20).unwrap();
        assert_eq!(one, many, "{name}: the plan of 20 threads equals the plan of one");
        // The old plan: one `height(centre)` per node.
        for n in &one.nodes {
            let centre = n.tile.center_direction(&map);
            let origin = PlanetFixed(centre * (R + field.height(centre)));
            assert_eq!(
                (n.origin.0.x.to_bits(), n.origin.0.y.to_bits(), n.origin.0.z.to_bits()),
                (origin.0.x.to_bits(), origin.0.y.to_bits(), origin.0.z.to_bits()),
                "{name}: origin of {:?} (level {})",
                n.tile,
                n.level
            );
        }
    }
}
