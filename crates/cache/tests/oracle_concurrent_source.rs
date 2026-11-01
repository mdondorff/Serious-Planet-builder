//! STRM-014, GEN-020, tier B: the stored global bake read at 1, 2, 4 and 20 threads, in full. `concurrent_source.rs` (tier A)
//! runs the small variants of the same two tests (1 against 3 threads on fewer tiles and budgets).
#![allow(dead_code, unused_imports)]

include!("support/concurrent_source.rs");

// spec: STRM-014
#[test]
fn tiles_over_the_store_are_identical_at_1_4_and_20_threads_and_to_the_in_memory_bake() {
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let grid = bake(7, &TangentWarp, RES, 2).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let key = w.stage_key(RES).0;
    // Tiles at several levels, across face edges and block borders.
    let mut ids = Vec::new();
    for f in 0..6u8 {
        for (l, x, y) in [(0u8, 0u32, 0u32), (1, 1, 0), (2, 3, 2), (3, 7, 7), (3, 0, 4), (4, 9, 15)] {
            ids.push(TileId::new(Face(f), l, x, y).unwrap());
        }
    }
    let mesh_bits = |src: &dyn LatticeSource, threads: usize| -> Vec<Vec<u32>> {
        let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).unwrap();
        let out: Vec<Mutex<Vec<u32>>> = ids.iter().map(|_| Mutex::new(Vec::new())).collect();
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= ids.len() {
                        break;
                    }
                    let m = tile_mesh_field(&field, &TangentWarp, ids[i], 8, R);
                    let mut v = vec![m.key as u32, (m.key >> 32) as u32];
                    for p in &m.vertices {
                        v.extend(p.pos.iter().chain(&p.normal).chain(&p.coarse).chain([&p.height, &p.coarse_height]).map(|x| x.to_bits()));
                    }
                    *out[i].lock().unwrap() = v;
                });
            }
        });
        assert!(field.error().is_none(), "{:?}", field.error());
        out.into_iter().map(|m| m.into_inner().unwrap()).collect()
    };
    let memory = mesh_bits(&GridSource(&grid), 1);
    for threads in [1usize, 4, 20] {
        // A fresh source each time; a budget of a few blocks makes evictions happen under contention too.
        let tight = StoreSource::with_budget(&store, &w, RES, 6 * BLOCK_BYTES).unwrap();
        assert!(mesh_bits(&tight, threads) == memory, "{threads} threads, tight budget");
        let roomy = StoreSource::new(&store, &w, RES).unwrap();
        assert!(mesh_bits(&roomy, threads) == memory, "{threads} threads, default budget");
    }
}

// spec: STRM-014
// spec: GEN-020
#[test]
fn a_threaded_batch_over_the_store_decodes_each_block_once_whatever_the_budget_and_the_thread_count() {
    use planet_core::hash::SplitMix64;
    use planet_core::Vec3;
    use planet_generators::height::HeightField;
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let key = w.stage_key(RES).0;
    let mut rng = SplitMix64(21);
    let mut axis = || rng.next_f64() * 2.0 - 1.0;
    let dirs: Vec<Vec3> = (0..120).map(|_| Vec3::new(axis(), axis(), axis()).normalized()).collect();
    let make = |src: &dyn LatticeSource| ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).unwrap().heights(&dirs);
    let reference: Vec<u64> = {
        let roomy = StoreSource::new(&store, &w, RES).unwrap();
        make(&roomy).iter().map(|h| h.to_bits()).collect()
    };
    let distinct = {
        let roomy = StoreSource::new(&store, &w, RES).unwrap();
        make(&roomy);
        roomy.stats().decodes
    };
    assert!(distinct > 8, "{distinct} distinct blocks");
    for blocks in [1u64, 3, 31] {
        for threads in [1usize, 2, 20] {
            let src = StoreSource::with_budget(&store, &w, RES, blocks * BLOCK_BYTES).unwrap();
            let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, key).unwrap();
            let got: Vec<u64> = field.heights_threaded(&dirs, &threads).iter().map(|h| h.to_bits()).collect();
            assert!(got == reference, "budget {blocks} blocks, {threads} threads: heights differ");
            assert!(
                src.stats().decodes <= distinct,
                "budget {blocks} blocks, {threads} threads: {} decodes for {distinct} distinct blocks",
                src.stats().decodes
            );
        }
    }
}
