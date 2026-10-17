//! Tier A: the coarse level of the water field (ADR 0057, GEN-024, ATMO-012, ATMO-019): a tile that spans more than one block of the bake is
//! answered by the stored summary and reads no block; the answer is a superset of the point-by-point one, a parent is wet wherever its
//! children are, tiles at the block level and below keep their exact answers, and nothing depends on the thread.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{build_global, load_water_summary, GlobalWorld, StoreOceanSource, StoreSource};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_frame::water::{WaterBook, WaterTileCache};
use planet_generators::global::{bake, GridSource};
use planet_generators::ocean::{tiled_ocean, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::water::{SeaProducer, WaterField};
use planet_generators::water_summary::summary_of_grid;
use planet_world_def::{LiquidKindId, SeaParams, WorldDefinition};

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-water-summary-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&p);
        Dir(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tiles_of_level(level: u8) -> Vec<TileId> {
    let side = 1u32 << level;
    (0..6u8).flat_map(|f| (0..side).flat_map(move |y| (0..side).map(move |x| TileId::new(Face(f), level, x, y).unwrap()))).collect()
}

fn sea() -> SeaParams {
    SeaParams { level_m: 0.0, liquid: LiquidKindId::WATER, min_area_fraction: EARTH_OCEAN_MIN_AREA_FRACTION }
}

// spec: GEN-024
// spec: ATMO-019
#[test]
fn a_coarse_answer_is_a_superset_of_the_point_query_a_parent_is_wet_where_its_children_are_and_fine_tiles_keep_their_answers() {
    let (res, block) = (256u32, 32u32);
    let mut strictly_more = 0;
    let mut wet_coarse = 0;
    for seed in [1u64, 2, 3] {
        let grid = bake(seed, &TangentWarp, res, 4).unwrap();
        let (_, ocean) = tiled_ocean(&grid, &TangentWarp, block, 4, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
        let summary = summary_of_grid(&grid, &ocean, block, 4).unwrap();
        assert_eq!(summary.block_level(), 3);
        let src = GridSource(&grid);
        let exact_sea = SeaProducer::new(&ocean, sea());
        let exact_field = WaterField::new(&TangentWarp, &src, vec![&exact_sea]);
        let coarse_sea = SeaProducer::new(&ocean, sea()).with_summary(&summary);
        let coarse_field = WaterField::new(&TangentWarp, &src, vec![&coarse_sea]);
        let (mut exact, mut coarse) = (WaterTileCache::new(), WaterTileCache::new());
        let mut answers = std::collections::HashMap::new();
        for level in 0..=5u8 {
            for t in tiles_of_level(level) {
                let e = exact.tile_water(&exact_field, &TangentWarp, t, R).unwrap();
                let c = coarse.tile_water(&coarse_field, &TangentWarp, t, R).unwrap();
                if level <= 3 {
                    assert!(e.is_none() || c.is_some(), "seed {seed}: {t:?} has water by its points and none by the summary");
                    strictly_more += usize::from(e.is_none() && c.is_some());
                    wet_coarse += usize::from(c.is_some());
                } else {
                    assert_eq!(c, e, "seed {seed}: {t:?} is at or below the block level and keeps its exact answer");
                }
                answers.insert(t, c.is_some());
            }
        }
        for level in 1..=4u8 {
            for t in tiles_of_level(level) {
                if answers[&t] {
                    let p = t.parent().unwrap();
                    assert!(answers[&p], "seed {seed}: the parent {p:?} of the wet tile {t:?} is dry");
                }
            }
        }
    }
    assert!(wet_coarse > 100, "{wet_coarse}");
    // The coarse level is a union over the tile's area, so it finds water the 25 points of a large tile miss.
    println!("coarse tiles wet by the summary only: {strictly_more}");
}

// spec: STRM-025
// spec: ATMO-012
#[test]
fn a_tile_job_of_a_coarse_tile_reads_no_block_and_one_of_a_fine_tile_at_most_four() {
    let def = WorldDefinition::single_planet(R, 5);
    let world = GlobalWorld::from_definition(&def).unwrap().with_block_limit(8);
    let (res, d) = (64u32, Dir::new("jobs"));
    build_global(&d.0, &world, res, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(&d.0, world.params()).unwrap();
    let summary = load_water_summary(&store, &world, res).unwrap();
    assert_eq!(summary.block_level(), 3);
    let run = |threads: usize| {
        let bake_src = StoreSource::new(&store, &world, res).unwrap();
        let ocean_src = StoreOceanSource::new(&store, &world, res).unwrap();
        let producer = SeaProducer::new(&ocean_src, world.sea()).with_summary(&summary);
        let field = WaterField::new(&TangentWarp, &bake_src, vec![&producer]);
        let coarse: Vec<TileId> = (0..=3).flat_map(tiles_of_level).collect();
        let book = WaterBook::new();
        std::thread::scope(|s| {
            for t in 0..threads {
                let (book, field, coarse) = (&book, &field, &coarse);
                s.spawn(move || {
                    for k in 0..coarse.len() {
                        book.fill(field, &TangentWarp, coarse[(k + 97 * t) % coarse.len()], R).unwrap();
                    }
                });
            }
        });
        assert_eq!(
            (bake_src.stats().decodes, ocean_src.decodes()),
            (0, 0),
            "{} coarse tiles of levels 0 to 3 read {} sample blocks and {} ocean blocks",
            coarse.len(),
            bake_src.stats().decodes,
            ocean_src.decodes()
        );
        assert_eq!(book.filled(), coarse.len() as u64);
        let answers: Vec<_> = coarse.iter().map(|t| book.get(*t).unwrap()).collect();
        // A fine tile (level 4, half a block here): its exact answer, from the block it lies in and the neighbours that hold its far
        // edge (the vertex on a block edge is the first of the next block): at most 2 x 2 sample blocks.
        for t in tiles_of_level(4).into_iter().step_by(37) {
            let before = bake_src.stats().decodes;
            book.fill(&field, &TangentWarp, t, R).unwrap();
            let blocks = bake_src.stats().decodes - before;
            assert!(blocks <= 4, "{t:?} read {blocks} sample blocks");
        }
        answers
    };
    let (one, four) = (run(1), run(4));
    assert_eq!(one, four, "the coarse answers do not depend on the thread count");
    assert!(one.iter().any(Option::is_some) && one.iter().any(Option::is_none), "the test bake has both sea and land tiles");
    // The sea of a coarse answer is the definition's.
    let w = one.iter().flatten().next().unwrap();
    assert_eq!((w.level_m, w.kind), (world.sea().level_m, world.sea().liquid));
}
