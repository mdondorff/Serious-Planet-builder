//! M4 item 7b measurement (docs/archive/M4/learnings.md, change store-source-concurrency): over a stored global bake, per standard view
//! the cold draw plan (a fresh `StoreSource` and `SelectionCache`), the lattice reads it makes, the distinct blocks it touches
//! and the second plan (blocks cached, fresh `SelectionCache`); then the 1, 4 and 20 thread tile rate with a bit-identity
//! check of the tiles across thread counts. Not production code.
//! `cargo run --release -p planet --example store_blocks -- --store DIR --global-res N [--part plans|rate|decode|roots|tiles|cells] [--order raster|block] [--threads 1,6] [--skip-per-root] [--view NAME] [--memory-rate TILES_PER_S]`.
//! `--order raster` reads the lattice without a block-size hint (the sample order before change block-order-sampling, GEN-020); `block` (default) is the product.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::Face;
use planet_frame::{plan_terrain_cached, scripted_views_field, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{GlobalSample, LatticeSource, ReadError};
use planet_generators::height::HeightField;
use planet_generators::mesh::tile_mesh_field;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const RADIUS_M: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

/// The store as a lattice source, with or without the block-size hint (`--order raster|block`).
struct Ordered<'a>(&'a StoreSource<'a>);

fn raster() -> bool {
    arg("--order").as_deref() == Some("raster")
}

impl LatticeSource for Ordered<'_> {
    fn res(&self) -> u32 {
        self.0.res()
    }
    fn block_cells(&self) -> u32 {
        if raster() {
            0
        } else {
            self.0.block_cells()
        }
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.0.at(face, i, j)
    }
}

struct Recording<'a> {
    inner: &'a StoreSource<'a>,
    cells_seen: Mutex<std::collections::HashSet<(u8, u32, u32)>>,
    record_cells: bool,
    cells: u32,
    blocks: Mutex<BTreeSet<(u8, u32, u32)>>,
    calls: AtomicU64,
    align: [AtomicU64; 15],
}

impl LatticeSource for Recording<'_> {
    fn res(&self) -> u32 {
        self.inner.res()
    }
    fn block_cells(&self) -> u32 {
        if raster() {
            0
        } else {
            self.inner.block_cells()
        }
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.calls.fetch_add(1, Relaxed);
        if self.record_cells {
            self.cells_seen.lock().unwrap().insert((face.0, i, j));
        }
        self.align[((i | j | 1 << 14).trailing_zeros() as usize).min(14)].fetch_add(1, Relaxed);
        let side = self.inner.res() / self.cells;
        self.blocks.lock().unwrap().insert((face.0, (i / self.cells).min(side - 1), (j / self.cells).min(side - 1)));
        self.inner.at(face, i, j)
    }
}

fn lod(field: &dyn HeightField, cells: u32) -> LodParams {
    LodParams {
        radius_m: RADIUS_M,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        cells,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 6.0,
        max_level: 26,
        ..LodParams::earth_1080p()
    }
}

fn fnv(h: &mut u64, x: u32) {
    *h = (*h ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
}

fn main() -> Result<(), String> {
    let dir = arg("--store").ok_or("--store DIR needed")?;
    let res: u32 = arg("--global-res").ok_or("--global-res N needed")?.parse().map_err(|e| format!("{e}"))?;
    let part = arg("--part").unwrap_or_else(|| "plans".into());
    let cells = 32u32;
    let def = WorldDefinition::single_planet(RADIUS_M, 1);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
    let key = world.stage_key(res).0;
    let map = TangentWarp;
    if part == "decode" {
        use planet_cache::global_store::{decode_block, KIND_GLOBAL_BLOCK};
        use planet_core::TileId;
        let (level, bc) = world.layout(res).map_err(|e| format!("{e:?}"))?;
        for k in 0..6u32 {
            let tile = TileId::new(Face((k % 6) as u8), level, k, 3).unwrap();
            let t = Instant::now();
            let rec = store.get(KIND_GLOBAL_BLOCK, tile, world.block_key(tile, bc)).map_err(|e| e.to_string())?.ok_or("missing")?;
            let get = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let (_, s) = decode_block(&rec.payload).map_err(|e| e.to_string())?;
            let dec = t.elapsed().as_secs_f64() * 1e3;
            println!(
                "DECODE block {k}: payload {:.1} MB, get {get:.1} ms, decode {dec:.1} ms, {} samples",
                rec.payload.len() as f64 / 1e6,
                s.len()
            );
        }
        return Ok(());
    }
    if part == "roots" {
        // Root-load measurement (change parallel-root-load): (1) per root, a fresh unbounded-budget source: distinct blocks,
        // decoded bytes, time; (2) the six roots one after another over one fresh source (the old path); (3) the same
        // through `Streamer::load_roots_blocking` on a pool of N threads over a fresh source.
        use planet_core::TileId;
        use planet_streaming::{Spawner, StreamConfig, Streamer, SystemClock, ThreadPool, TileSource};
        let store: &'static planet_cache::pack::TileStore = Box::leak(Box::new(store));
        let world: &'static GlobalWorld = Box::leak(Box::new(world));
        let roots: Vec<TileId> = Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).unwrap()).collect();
        if arg("--skip-per-root").is_none() {
            let mut union = BTreeSet::new();
            for &root in &roots {
                let stored = StoreSource::with_budget(store, world, res, 64 << 30).map_err(|e| e.to_string())?;
                let (_, block_cells) = world.layout(res).map_err(|e| format!("{e:?}"))?;
                let rec = Recording {
                    inner: &stored,
                    cells_seen: Mutex::new(Default::default()),
                    record_cells: false,
                    cells: block_cells,
                    blocks: Mutex::new(BTreeSet::new()),
                    calls: AtomicU64::new(0),
                    align: Default::default(),
                };
                let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &rec, key).map_err(|e| format!("{e:?}"))?;
                let t = Instant::now();
                let m = tile_mesh_field(&eroded, &map, root, cells, RADIUS_M);
                let s = stored.stats();
                let blocks = rec.blocks.lock().unwrap();
                println!(
                    "ROOT {root:?}: {:.1} s, {} reads, {} distinct blocks (face {}), {} decodes, {:.0} MB decoded, {} vertices",
                    t.elapsed().as_secs_f64(),
                    rec.calls.load(Relaxed),
                    blocks.len(),
                    root.face().0,
                    s.decodes,
                    s.resident_bytes as f64 / 1e6,
                    m.vertices.len()
                );
                union.extend(blocks.iter().copied());
            }
            println!("ROOTS union: {} distinct blocks", union.len());
        }
        struct Roots {
            src: StoreSource<'static>,
            cells: u32,
        }
        let make = |threads: usize| -> Result<(Streamer<usize>, Arc<Roots>), String> {
            let src = StoreSource::new(store, world, res).map_err(|e| e.to_string())?;
            let r = Arc::new(Roots { src, cells });
            let pool: Arc<dyn Spawner> = Arc::new(ThreadPool::new(threads));
            Ok((
                Streamer::new(Arc::clone(&r) as Arc<dyn TileSource<usize>>, pool, Arc::new(SystemClock::new()), StreamConfig::default()),
                r,
            ))
        };
        impl TileSource<usize> for Roots {
            fn load(&self, tile: TileId) -> Result<usize, String> {
                let def = WorldDefinition::single_planet(RADIUS_M, 1);
                let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
                let key = world.stage_key(self.src.res()).0;
                let ordered = Ordered(&self.src);
                let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &ordered, key).map_err(|e| format!("{e:?}"))?;
                Ok(tile_mesh_field(&eroded, &TangentWarp, tile, self.cells, RADIUS_M).vertices.len())
            }
        }
        for threads in arg("--threads").unwrap_or_else(|| "1,6".into()).split(',').map(|t| t.parse::<usize>().unwrap()) {
            let (mut streamer, r) = make(threads)?;
            let t = Instant::now();
            streamer.load_roots_blocking()?;
            let s = r.src.stats();
            println!(
                "LOAD_ROOTS pool of {threads}: {:.1} s, {} decodes, {:.0} MB resident of {:.0} MB budget, vertices {:?}",
                t.elapsed().as_secs_f64(),
                s.decodes,
                s.resident_bytes as f64 / 1e6,
                s.budget_bytes as f64 / 1e6,
                roots.iter().map(|id| *streamer.get(*id).unwrap()).collect::<Vec<_>>()
            );
        }
        return Ok(());
    }
    if part == "tiles" || part == "cells" {
        // Per level 0..4 (change block-order-sampling): one tile, cold cache, one thread: time, decodes against the distinct
        // blocks and lattice cells it reads; then (tiles) up to 16 tiles of the level on `--threads` workers over one fresh
        // source. `cells` also lists how the distinct lattice cells relate to the tile's own sample spacing.
        use planet_core::TileId;
        let per_level = |level: u8| -> Vec<TileId> {
            let side = 1u32 << level;
            let all: Vec<TileId> =
                (0..side).flat_map(|y| (0..side).map(move |x| (x, y))).map(|(x, y)| TileId::new(Face(2), level, x, y).unwrap()).collect();
            let n = all.len().min(16);
            (0..n).map(|k| all[k * all.len() / n]).collect()
        };
        let order = if raster() { "raster" } else { "block" };
        for level in 0..=4u8 {
            let tile = per_level(level)[0];
            let stored = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
            let (_, block_cells) = world.layout(res).map_err(|e| format!("{e:?}"))?;
            let rec = Recording {
                inner: &stored,
                cells_seen: Mutex::new(Default::default()),
                record_cells: part == "cells",
                cells: block_cells,
                blocks: Mutex::new(BTreeSet::new()),
                calls: AtomicU64::new(0),
                align: Default::default(),
            };
            let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &rec, key).map_err(|e| format!("{e:?}"))?;
            let t = Instant::now();
            let m = tile_mesh_field(&eroded, &map, tile, cells, RADIUS_M);
            let secs = t.elapsed().as_secs_f64();
            println!(
                "TILE level {level} {tile:?} [{order}]: {secs:.2} s, {} reads, {} distinct blocks, {} decodes, {} vertices",
                rec.calls.load(Relaxed),
                rec.blocks.lock().unwrap().len(),
                stored.stats().decodes,
                m.vertices.len()
            );
            if part == "cells" {
                let seen = rec.cells_seen.lock().unwrap();
                let spacing = (res / (cells << level)).max(1);
                let on = seen.iter().filter(|(_, i, j)| i % spacing == 0 && j % spacing == 0).count();
                let mut hist = [0usize; 16];
                for (_, i, j) in seen.iter() {
                    hist[((i | j | 1 << 15).trailing_zeros() as usize).min(15)] += 1;
                }
                println!(
                    "CELLS level {level}: {} distinct lattice cells; sample spacing {spacing} cells; {on} on the spacing grid (i and j multiples of it); alignment histogram (trailing zeros of i|j) {hist:?}",
                    seen.len()
                );
            }
            if part == "tiles" {
                let tiles = per_level(level);
                for threads in arg("--threads").unwrap_or_else(|| "6".into()).split(',').map(|t| t.parse::<usize>().unwrap()) {
                    let src = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
                    let ordered = Ordered(&src);
                    let field =
                        ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &ordered, key).map_err(|e| format!("{e:?}"))?;
                    let next = AtomicU64::new(0);
                    let t = Instant::now();
                    std::thread::scope(|s| {
                        for _ in 0..threads {
                            s.spawn(|| loop {
                                let i = next.fetch_add(1, Relaxed) as usize;
                                if i >= tiles.len() {
                                    break;
                                }
                                std::hint::black_box(tile_mesh_field(&field, &map, tiles[i], cells, RADIUS_M));
                            });
                        }
                    });
                    println!(
                        "TILES level {level} [{order}]: {} tiles on {threads} threads over one cold source: {:.2} s wall, {} decodes",
                        tiles.len(),
                        t.elapsed().as_secs_f64(),
                        src.stats().decodes
                    );
                }
            }
        }
        return Ok(());
    }
    if part == "plans" {
        let only = arg("--view");
        for name in ["orbit-20000km", "high-2000km", "aerial-200km", "aerial-20km", "low-2km", "ground-200m", "ground-1m"] {
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let stored = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
            let (_, block_cells) = world.layout(res).map_err(|e| format!("{e:?}"))?;
            let rec = Recording {
                inner: &stored,
                cells_seen: Mutex::new(Default::default()),
                record_cells: false,
                cells: block_cells,
                blocks: Mutex::new(BTreeSet::new()),
                calls: AtomicU64::new(0),
                align: Default::default(),
            };
            let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &rec, key).map_err(|e| format!("{e:?}"))?;
            let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), &eroded);
            let (_, cam) = views.iter().find(|(n, _)| *n == name).expect("view");
            let req = TerrainRequest {
                camera: *cam,
                size: SIZE,
                view: TerrainView::Level,
                lod: lod(&eroded, cells),
                face_mapping: map.id().to_string(),
                seed: eroded.seed(),
            };
            let mut line = format!("PLAN {name:>13}");
            for pass in ["cold", "blocks-warm"] {
                rec.calls.store(0, Relaxed);
                rec.blocks.lock().unwrap().clear();
                let t = Instant::now();
                let plan = plan_terrain_cached(&req, &eroded, &mut SelectionCache::new()).map_err(|e| e.to_string())?;
                let ms = t.elapsed().as_secs_f64() * 1e3;
                line += &format!(
                    " | {pass}: {ms:.0} ms, {} nodes, {} reads, {} blocks",
                    plan.nodes.len(),
                    rec.calls.load(Relaxed),
                    rec.blocks.lock().unwrap().len()
                );
            }
            let al: Vec<u64> = rec.align.iter().map(|a| a.load(Relaxed)).collect();
            println!(
                "{line}
    alignment (trailing zeros of i|j): {al:?}"
            );
        }
        return Ok(());
    }
    // rate: tiles of the ground-1m draw set at 1, 4 and 20 threads, a fresh source each; a hash of every tile's mesh bits.
    let stored = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
    let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &stored, key).map_err(|e| format!("{e:?}"))?;
    let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), &eroded);
    let (_, cam) = views.iter().find(|(n, _)| *n == "ground-1m").expect("view");
    let req = TerrainRequest {
        camera: *cam,
        size: SIZE,
        view: TerrainView::Level,
        lod: lod(&eroded, cells),
        face_mapping: map.id().to_string(),
        seed: eroded.seed(),
    };
    let plan = plan_terrain_cached(&req, &eroded, &mut SelectionCache::new()).map_err(|e| e.to_string())?;
    let n = plan.nodes.len();
    let count: usize = arg("--tiles").map_or(320, |c| c.parse().unwrap());
    let pick: Vec<_> = (0..count).map(|i| plan.nodes[i * n / count].tile).collect();
    let mut all = Vec::new();
    for threads in [1usize, 4, 20] {
        let src = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
        let ordered = Ordered(&src);
        let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &ordered, key).map_err(|e| format!("{e:?}"))?;
        let out: Vec<AtomicU64> = pick.iter().map(|_| AtomicU64::new(0)).collect();
        let next = AtomicU64::new(0);
        let t = Instant::now();
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Relaxed) as usize;
                    if i >= pick.len() {
                        break;
                    }
                    let m = tile_mesh_field(&field, &map, pick[i], cells, RADIUS_M);
                    let mut h = 0xcbf2_9ce4_8422_2325u64;
                    fnv(&mut h, m.key as u32);
                    fnv(&mut h, (m.key >> 32) as u32);
                    for v in &m.vertices {
                        for x in v.pos.iter().chain(&v.coarse).chain(&v.normal).chain([&v.height, &v.coarse_height]) {
                            fnv(&mut h, x.to_bits());
                        }
                    }
                    out[i].store(h, Relaxed);
                });
            }
        });
        let secs = t.elapsed().as_secs_f64();
        println!(
            "RATE {threads:>2} threads: {} tiles in {secs:.2} s = {:.1} tiles/s (fresh source, includes cold block decodes)",
            pick.len(),
            pick.len() as f64 / secs
        );
        if threads == 20 {
            if let Some(memory) = arg("--memory-rate").map(|m| m.parse::<f64>().expect("--memory-rate is tiles/s")) {
                let rate = pick.len() as f64 / secs;
                println!(
                    "STRM-014 rate vs in-memory {memory:.0} tiles/s: {:.0} % -> {}",
                    100.0 * rate / memory,
                    if rate >= 0.8 * memory { "PASS" } else { "FAIL" }
                );
            }
        }
        all.push(out.iter().map(|a| a.load(Relaxed)).collect::<Vec<_>>());
    }
    println!("IDENTICAL across 1/4/20 threads: {}", all[0] == all[1] && all[1] == all[2]);
    Ok(())
}
