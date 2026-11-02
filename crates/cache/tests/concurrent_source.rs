//! STRM-014: the stored global bake read by many threads (M4 item 7b).

// The store, the directories and the readers are shared with `oracle_concurrent_source.rs` (tier B).
include!("support/concurrent_source.rs");

// spec: STRM-014
#[test]
fn tiles_over_the_store_are_identical_at_1_and_3_threads_and_to_the_in_memory_bake() {
    // The small variant (tier A): one thread against three, on four tiles of every face (levels 0 to 2 and a fine edge tile, so a face
    // edge and block borders still occur); 1, 4 and 20 threads on all tiles is `oracle_concurrent_source.rs` (tier B).
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let grid = bake(7, &TangentWarp, RES, 2).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let key = w.stage_key(RES).0;
    // Tiles at several levels, across face edges and block borders.
    let mut ids = Vec::new();
    for f in 0..6u8 {
        for (l, x, y) in [(0u8, 0u32, 0u32), (1, 1, 0), (2, 3, 2), (4, 9, 15)] {
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
    for threads in [1usize, 3] {
        // A fresh source each time; a budget of a few blocks makes evictions happen under contention too.
        let tight = StoreSource::with_budget(&store, &w, RES, 6 * BLOCK_BYTES).unwrap();
        assert!(mesh_bits(&tight, threads) == memory, "{threads} threads, tight budget");
        let roomy = StoreSource::new(&store, &w, RES).unwrap();
        assert!(mesh_bits(&roomy, threads) == memory, "{threads} threads, default budget");
    }
}

// spec: STRM-014
#[test]
fn threads_that_want_one_block_share_one_decode() {
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let grid = bake(7, &TangentWarp, RES, 2).unwrap();
    let src = StoreSource::with_budget(&store, &w, RES, BLOCKS * BLOCK_BYTES).unwrap();
    std::thread::scope(|s| {
        for t in 0..4 {
            let (src, grid) = (&src, &grid);
            s.spawn(move || read_all(src, &GridSource(grid), t));
        }
    });
    let st = src.stats();
    assert_eq!(st.decodes, BLOCKS, "every block decoded once, not once per thread: {st:?}");
    assert_eq!((st.resident_blocks as u64, st.resident_bytes), (BLOCKS, BLOCKS * BLOCK_BYTES), "{st:?}");
}

// spec: STRM-014
#[test]
fn resident_bytes_never_exceed_the_budget_and_reads_stay_exact() {
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let grid = bake(7, &TangentWarp, RES, 2).unwrap();
    for budget_blocks in [0u64, 1, 5, 40] {
        // Less than one block still holds one (a source that cannot hold a block could not read).
        let src = StoreSource::with_budget(&store, &w, RES, budget_blocks * BLOCK_BYTES).unwrap();
        let cap = budget_blocks.max(1) * BLOCK_BYTES;
        assert_eq!(src.stats().budget_bytes, cap);
        // The source counts the most blocks its cache ever held, under the cache lock after every insert and eviction, so no
        // moment above the budget can be missed (a sampling thread could miss one, and a spinning one cost a core of `test fast`).
        std::thread::scope(|s| {
            let readers: Vec<_> = (0..4)
                .map(|t| {
                    let (src, grid) = (&src, &grid);
                    s.spawn(move || read_all(src, &GridSource(grid), t))
                })
                .collect();
            for r in readers {
                r.join().unwrap();
            }
        });
        let st = src.stats();
        assert!(st.peak_resident_blocks as u64 * BLOCK_BYTES <= cap, "budget {budget_blocks} blocks: peak {st:?}");
        assert_eq!(st.peak_resident_blocks as u64, budget_blocks.max(1), "the cache was filled to its budget and no further: {st:?}");
        assert!(st.resident_bytes <= cap && st.resident_blocks as u64 * BLOCK_BYTES == st.resident_bytes, "{st:?}");
    }
}

// spec: STRM-014
#[test]
fn a_missing_block_is_an_error_for_every_reader_and_is_not_cached() {
    let w = world(7);
    let empty = Dir::new("missing");
    fs::create_dir_all(empty.path()).unwrap();
    let nothing = TileStore::open(empty.path(), w.params()).unwrap();
    let src = StoreSource::new(&nothing, &w, RES).unwrap();
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| {
                for _ in 0..3 {
                    let e = src.at(Face(2), 5, 9).unwrap_err().to_string();
                    assert!(e.contains("not stored under the current key") && e.contains("face 2 level 4"), "{e}");
                }
            });
        }
    });
    let st = src.stats();
    assert_eq!((st.resident_blocks, st.resident_bytes), (0, 0), "{st:?}");
}

// spec: STRM-014
#[test]
fn the_default_budget_holds_more_than_the_sixteen_blocks_it_replaces() {
    let w = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 7)).unwrap();
    let dir = Dir::new("default-budget");
    fs::create_dir_all(dir.path()).unwrap();
    let store = TileStore::open(dir.path(), w.params()).unwrap();
    let src = StoreSource::new(&store, &w, 16384).unwrap();
    let block = 16 * 1025u64 * 1025;
    assert_eq!(src.stats().budget_bytes, 31 * block, "512 MiB in whole blocks");
    assert!(SOURCE_CACHE_BYTES == 512 << 20 && SOURCE_CACHE_BYTES / block >= 31, "31 blocks of 1024 cells fit in 512 MiB");
}

/// Counts every lattice read that goes through it.
struct CountingSource<'a> {
    inner: &'a dyn LatticeSource,
    reads: AtomicU64,
}

impl LatticeSource for CountingSource<'_> {
    fn res(&self) -> u32 {
        self.inner.res()
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, planet_generators::global::ReadError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.at(face, i, j)
    }
}

/// Tiles of the eroded field generated by `threads` threads over `src`; returns the mesh keys (a witness that all ran).
fn generate_tiles(src: &dyn LatticeSource, w: &GlobalWorld, ids: &[TileId], threads: usize) -> Vec<u64> {
    let def = WorldDefinition::single_planet(R, 7);
    let key = w.stage_key(RES).0;
    let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).unwrap();
    let out: Vec<AtomicU64> = ids.iter().map(|_| AtomicU64::new(0)).collect();
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= ids.len() {
                    break;
                }
                out[i].store(tile_mesh_field(&field, &TangentWarp, ids[i], 8, R).key, Ordering::Relaxed);
            });
        }
    });
    assert!(field.error().is_none(), "{:?}", field.error());
    out.iter().map(|k| k.load(Ordering::Relaxed)).collect()
}

const PROXY_BLOCK_CELLS: u32 = 16;
const PROXY_BLOCKS: u64 = 6 * 4 * 4;
const PROXY_BLOCK_BYTES: u64 = 17 * 17 * 16;

fn work_tiles() -> Vec<TileId> {
    let mut ids = Vec::new();
    for f in 0..6u8 {
        for (l, x, y) in [(3u8, 7u32, 7u32), (3, 0, 4), (3, 3, 3), (4, 9, 15), (4, 8, 8), (4, 2, 11), (2, 1, 1)] {
            ids.push(TileId::new(Face(f), l, x, y).unwrap());
        }
    }
    ids
}

// spec: STRM-014
#[test]
fn twenty_threads_take_the_shared_lock_only_on_a_change_of_block_and_decode_each_block_once() {
    // The deterministic proxy of "the 20-thread rate over the store is at least 80 % of the in-memory rate": what made the
    // rate 20 tiles/s on 20 threads was a lock per lattice read. The measured rate is not asserted (wall clock is not stable
    // on shared CI, and this 64-cell grid has blocks so small that tiles change block often): run by hand on the real store (the removed `store_blocks` example did: the ratio to the 20-thread tiles/s of the same tiles over an in-memory bake, PASS at 80 %).
    // Recorded (ad hoc, Rasierklinge, 2026-10-15, M4 item 7b, cells 32, ground-1m draw set): 1,000-1,190 tiles/s on 20 threads
    // over the store against 1,207 in memory (83-98 %); 152 on one thread (111 before); 20 on 20 threads with a mutex per read.
    // Blocks of 16 cells (a 64-cell grid is 4 x 4 blocks per face): the shape of the product, where a tile lies in a few blocks.
    let d = Dir::new("lock-proxy");
    let w = world_with_limit(7, PROXY_BLOCK_CELLS);
    build_global(d.path(), &w, RES, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let ids = work_tiles();

    let one = StoreSource::with_budget(&store, &w, RES, PROXY_BLOCKS * PROXY_BLOCK_BYTES).unwrap();
    let one_counted = CountingSource { inner: &one, reads: AtomicU64::new(0) };
    let keys_one = generate_tiles(&one_counted, &w, &ids, 1);
    let (reads_one, st_one) = (one_counted.reads.load(Ordering::Relaxed), one.stats());

    let many = StoreSource::with_budget(&store, &w, RES, PROXY_BLOCKS * PROXY_BLOCK_BYTES).unwrap();
    let many_counted = CountingSource { inner: &many, reads: AtomicU64::new(0) };
    let keys_many = generate_tiles(&many_counted, &w, &ids, 20);
    let (reads_many, st_many) = (many_counted.reads.load(Ordering::Relaxed), many.stats());

    assert_eq!(keys_one, keys_many);
    assert_eq!(reads_one, reads_many, "the same tiles read the same vertices at any thread count");
    assert!(st_one.decodes > 0 && st_one.decodes <= PROXY_BLOCKS, "{st_one:?}");
    assert_eq!(st_many.decodes, st_one.decodes, "one decode per distinct block at any thread count: {st_many:?} vs {st_one:?}");

    // The shared lock is taken on a change of block, not per read (the lock per read is what made 20 threads 5x slower than
    // one). A thread's pinned block can only save a lookup, and which tiles follow each other on a thread varies with
    // scheduling by at most one lookup per tile; measured 12 % of the reads on this 64-cell grid (far less on the product's
    // 16384-cell grid, where a tile lies in one or two blocks).
    assert!(
        st_many.shared_lookups <= st_one.shared_lookups + ids.len() as u64,
        "20 threads took the lock {} times, one thread {} for {} tiles",
        st_many.shared_lookups,
        st_one.shared_lookups,
        ids.len()
    );
    assert!(
        st_many.shared_lookups * 4 <= reads_many,
        "the lock was taken {} times for {reads_many} lattice reads: it must be taken on a change of block only",
        st_many.shared_lookups
    );
}

// spec: GEN-020
#[test]
fn a_tile_decodes_each_block_it_touches_once_even_with_a_cache_of_one_block() {
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let key = w.stage_key(RES).0;
    let decodes = |budget_blocks: u64, id: TileId, cells: u32| {
        let src = StoreSource::with_budget(&store, &w, RES, budget_blocks * BLOCK_BYTES).unwrap();
        assert_eq!(src.block_cells(), 4, "the store reports its block size");
        let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, key).unwrap();
        let mesh = tile_mesh_field(&field, &TangentWarp, id, cells, R);
        assert!(field.error().is_none(), "{:?}", field.error());
        (src.stats().decodes, mesh.content_hash())
    };
    for (id, cells) in [
        (TileId::new(Face(1), 0, 0, 0).unwrap(), 16),
        (TileId::new(Face(4), 2, 1, 3).unwrap(), 8),
        (TileId::new(Face(0), 3, 0, 0).unwrap(), 8),
    ] {
        let (distinct, hash) = decodes(BLOCKS, id, cells);
        let (one, hash_one) = decodes(1, id, cells);
        assert_eq!(hash, hash_one, "{id:?}: the budget never changes a bit");
        assert!(one <= distinct, "{id:?}: {one} decodes with a cache of one block for {distinct} distinct blocks");
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
    // The small variant (tier A): threads 1 and 3 over budgets of 1 and 31 blocks; 1, 2 and 20 threads over 1, 3 and 31 blocks
    // is `oracle_concurrent_source.rs` (tier B).
    for blocks in [1u64, 31] {
        for threads in [1usize, 3] {
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

// spec: STRM-022
#[test]
fn the_source_counts_its_cache_behaviour_exactly_and_the_reads_stay_the_stored_bits() {
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let grid = bake(7, &TangentWarp, RES, 2).unwrap();
    // One thread, a budget of five blocks: every miss decodes and inserts one block, so the evictions are the decodes beyond the
    // budget, nothing waits for another thread's decode, and the lookups are the hits plus the misses.
    let (before, decoded_before) = (planet_cache::global_store::thread_block_wait_ns(), planet_cache::global_store::thread_block_decodes());
    let tight = StoreSource::with_budget(&store, &w, RES, 5 * BLOCK_BYTES).unwrap();
    read_all(&tight, &GridSource(&grid), 0);
    let st = tight.stats();
    let misses = st.shared_lookups - st.shared_hits;
    assert_eq!(misses, st.decodes, "{st:?}");
    assert_eq!(st.evictions, st.decodes - 5, "{st:?}");
    assert_eq!((st.decode_waits, st.budget_blocks, st.resident_blocks), (0, 5, 5), "{st:?}");
    assert!(st.decodes > 5, "{st:?}");
    assert!(planet_cache::global_store::thread_block_wait_ns() >= before, "the thread's block wait only grows");
    assert_eq!(
        planet_cache::global_store::thread_block_decodes() - decoded_before,
        st.decodes,
        "the one thread decoded every block itself"
    );
    // Eight threads over a roomy budget: one decode per block, no eviction, a thread that finds a block another is decoding
    // counts a wait, and no more waits than hits.
    let roomy = StoreSource::with_budget(&store, &w, RES, BLOCKS * BLOCK_BYTES).unwrap();
    std::thread::scope(|s| {
        for t in 0..8 {
            let (src, grid) = (&roomy, &grid);
            s.spawn(move || read_all(src, &GridSource(grid), t));
        }
    });
    let st = roomy.stats();
    assert_eq!((st.decodes, st.evictions), (BLOCKS, 0), "{st:?}");
    assert_eq!(st.shared_lookups - st.shared_hits, BLOCKS, "{st:?}");
    assert!(st.decode_waits <= st.shared_hits, "{st:?}");
}

// spec: STRM-022
#[test]
fn the_pack_counts_its_lookups_and_the_ones_that_found_a_record() {
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    assert_eq!(store.read_stats(), planet_cache::pack::StoreReadStats::default());
    let src = StoreSource::with_budget(&store, &w, RES, BLOCKS * BLOCK_BYTES).unwrap();
    read_all(&src, &GridSource(&bake(7, &TangentWarp, RES, 2).unwrap()), 3);
    let st = src.stats();
    let reads = store.read_stats();
    // Every decode asked the pack once and found its block.
    assert_eq!((reads.gets, reads.found), (st.decodes, st.decodes), "{reads:?} {st:?}");
    // A key the pack does not hold is a lookup that found nothing.
    let tile = TileId::new(Face(0), 4, 0, 0).unwrap();
    let missing = planet_cache::key::CacheKey([0xAB; 16]);
    assert!(store.get(planet_cache::global_store::KIND_GLOBAL_BLOCK, tile, missing).unwrap().is_none());
    let after = store.read_stats();
    assert_eq!((after.gets, after.found), (reads.gets + 1, reads.found));
}

// spec: VIEW-061
#[test]
fn pinned_blocks_stay_resident_under_loader_pressure() {
    // The camera's ground read touches a block or two; pinned, they survive loader threads that read every block of the world through
    // a budget of 4 blocks, so the next ground read decodes nothing.
    let (d, w) = stored();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let src = StoreSource::with_budget(&store, &w, RES, 4 * BLOCK_BYTES).unwrap();
    // A read across a block corner touches several blocks.
    let ground = || src.at(Face(2), 4, 4).unwrap().elevation_m + src.at(Face(2), 3, 3).unwrap().elevation_m;
    let (h0, touched) = planet_cache::global_store::record_touched_blocks(ground);
    assert!(touched.len() >= 2, "a corner read touches two or more blocks: {touched:?}");
    src.pin_touched(&touched);
    assert_eq!(src.pinned_blocks().len(), touched.len().min(3), "the pins stay fewer than the budget");
    let grid = bake(7, &TangentWarp, RES, 2).unwrap();
    std::thread::scope(|s| {
        for t in 0..4 {
            let (src, grid) = (&src, &grid);
            s.spawn(move || read_all(src, &GridSource(grid), t));
        }
    });
    let st = src.stats();
    assert!(st.evictions > BLOCKS, "the loaders pushed blocks out: {st:?}");
    assert!(st.peak_resident_blocks <= 4, "the budget holds with pins: {st:?}");
    // A fresh thread (no thread-local block) reads the ground again: no decode.
    let before = src.stats().decodes;
    let h1 = std::thread::scope(|s| s.spawn(ground).join().unwrap());
    assert_eq!(h1, h0, "the same ground");
    assert_eq!(src.stats().decodes, before, "the pinned blocks were still resident");
    // Unpinned, the same pressure pushes them out.
    src.pin_touched(&[]);
    std::thread::scope(|s| {
        let (src, grid) = (&src, &grid);
        s.spawn(move || read_all(src, &GridSource(grid), 1));
    });
    let before = src.stats().decodes;
    std::thread::scope(|s| s.spawn(ground).join().unwrap());
    assert!(src.stats().decodes > before, "without the pins the ground decodes again");
}
