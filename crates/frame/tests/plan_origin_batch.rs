//! LOD-009, GEN-020: the plan evaluates the origin heights of the tiles its cache lacks as ONE batch (read block by block), not one
//! `height` per node in tree order, and the plan is bit for bit the plan of the per-node evaluation, at any thread count.
//! The bake is a synthetic multi-block lattice behind a source that counts block decodes like the store's cache (an LRU of
//! `capacity` blocks), so the counts are those of a cold plan over the stored 16384-cell bake, scaled down.

// The synthetic bake, the probe and the cameras are shared with `oracle_plan_origin_batch.rs` (tier B).
include!("support/origin_batch.rs");

// spec: LOD-009
// spec: GEN-020
#[test]
fn a_cold_plan_evaluates_its_origin_heights_as_one_batch_that_decodes_each_block_once() {
    // Altitude views: their cold plans are the cost of the dive (hundreds of blocks for tens of nodes) and select cheaply.
    let any = Blocks::new(usize::MAX);
    let cams = cameras(&eroded(&any));
    let mut checked = 0;
    for (name, camera) in
        cams.iter().filter(|(n, _)| ["orbit-20000km", "high-2000km", "dive-5000000m", "dive-500000m"].contains(&n.as_str()))
    {
        // A cache of ONE block: any block read twice is decoded twice.
        let small = Blocks::new(1);
        let probe = Probe::new(&small);
        let req = request(*camera, &probe);
        for threads in [1usize, 20] {
            probe.reset();
            let plan = plan_terrain_threaded(&req, &probe, &mut SelectionCache::new(), threads).unwrap();
            assert_eq!(probe.singles.load(Relaxed), 0, "{name}: no origin is evaluated one `height` at a time");
            assert_eq!(probe.batches.load(Relaxed), 1, "{name}: one batch per plan (at most the tree depth), {threads} threads");
            let dirs = probe.batch_dirs.lock().unwrap().clone();
            assert_eq!(dirs.len(), plan.nodes.len(), "{name}: the batch holds the origin of every node of a cold plan");
            // The decodes of an unbounded cache over the same directions: every distinct block once.
            let unbounded = Blocks::new(usize::MAX);
            eroded(&unbounded).heights(&dirs);
            let distinct = unbounded.decodes();
            let decodes = probe.batch_decodes.load(Relaxed);
            println!("{name} ({threads} threads): {} nodes, {distinct} distinct blocks, {decodes} decodes in the batch", plan.nodes.len());
            assert!(distinct > 0 && decodes <= distinct, "{name}: {decodes} decodes for {distinct} distinct blocks with a one-block cache");
            checked += 1;
        }
    }
    assert_eq!(checked, 8);
}

// spec: LOD-009
// spec: GEN-020
#[test]
fn the_plan_equals_the_per_node_evaluation_at_every_thread_count() {
    let any = Blocks::new(usize::MAX);
    let field = eroded(&any);
    let map = TangentWarp;
    // The small variant (tier A): every third camera plus the cube-corner and pole views, one thread against three; every camera against 20 threads is
    // `oracle_plan_origin_batch.rs` (tier B).
    let every_third = |i: usize, name: &str| i.is_multiple_of(3) || name == "cube-corner-1km" || name == "north-pole-1km";
    for (name, camera) in cameras(&field).into_iter().enumerate().filter(|(i, (n, _))| every_third(*i, n)).map(|(_, c)| c) {
        let req = request(camera, &field);
        let one = plan_terrain_cached(&req, &field, &mut SelectionCache::new()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let many = plan_terrain_threaded(&req, &field, &mut SelectionCache::new(), 3).unwrap();
        assert_eq!(one, many, "{name}: the plan of 3 threads equals the plan of one");
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

// spec: LOD-009
#[test]
fn a_warm_plan_makes_no_batch_and_a_moved_camera_batches_only_its_new_tiles() {
    let any = Blocks::new(usize::MAX);
    let probe = Probe::new(&any);
    let a = request(dive_camera(&probe, 5.0e5), &probe);
    let mut b = a.clone();
    b.camera.position = PlanetFixed(b.camera.position.0 + b.camera.forward * 20_000.0);
    let mut cache = SelectionCache::new();
    let first = plan_terrain_cached(&a, &probe, &mut cache).unwrap();
    probe.reset();
    let again = plan_terrain_cached(&a, &probe, &mut cache).unwrap();
    assert_eq!((probe.singles.load(Relaxed), probe.batches.load(Relaxed)), (0, 0), "a warm plan evaluates no origin height");
    assert_eq!(first, again);
    let moved = plan_terrain_cached(&b, &probe, &mut cache).unwrap();
    let (singles, batches, batched) = (probe.singles.load(Relaxed), probe.batches.load(Relaxed), probe.batch_dirs.lock().unwrap().len());
    let known: std::collections::HashSet<TileId> = first.nodes.iter().map(|n| n.tile).collect();
    let new = moved.nodes.iter().filter(|n| !known.contains(&n.tile)).count();
    assert!(new > 0 && new < moved.nodes.len(), "the step keeps some tiles and brings new ones ({new} of {})", moved.nodes.len());
    assert_eq!((singles, batches, batched), (0, 1, new), "one batch of exactly the new tiles");
    let fresh = plan_terrain_cached(&b, &probe, &mut SelectionCache::new()).unwrap();
    assert_eq!(moved, fresh, "the cached plan of the moved camera equals the plan of a fresh cache");
}
