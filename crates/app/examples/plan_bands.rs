//! M4 fix F3 measurement (docs/archive/M4/learnings.md, change cold-plan-origin-heights): where a cold plan at altitude goes, over the stored
//! global bake, per altitude band of the dive, and the dive's plan column replayed the way the plan worker runs it. Not production code.
//! `cargo run --release -p planet --example plan_bands -- --store DIR --global-res N [--part bands|dive|tilegen] [--threads 20] [--bands 20000000,5000000,...]`.
//!
//! `bands`: per band, a fresh `StoreSource` and `SelectionCache` for each measurement: the selection alone, the origin heights of its
//! nodes one `height()` at a time in selection order (the plan before F3), the same as one `heights()` batch, and the whole plan
//! (`plan_terrain_threaded`) with the field read through a counting source (lattice reads, distinct blocks, decodes).
//! `dive`: one cold source and one cache for the whole dive; the worker's loop: plan the camera of the script time `t`, then R, and
//! the next plan is for the camera at `t + (plan + R) time` (a job takes the newest request when it ends).

// Platform maths only picks which script frame a band uses; nothing is keyed or compared on it.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Instant;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::Face;
use planet_frame::pipeline::residency_of;
use planet_frame::scenes::{build_script, dive_altitude_m, StreamScene, DIVE_DESCENT_S, FRAME_STEP_S};
use planet_frame::{plan_terrain_threaded, select_nodes_in_view_cached, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{GlobalSample, LatticeSource, ReadError};
use planet_generators::height::HeightField;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const RADIUS_M: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

struct Recording<'a> {
    inner: &'a StoreSource<'a>,
    cells: u32,
    blocks: Mutex<BTreeSet<(u8, u32, u32)>>,
    calls: AtomicU64,
}

impl LatticeSource for Recording<'_> {
    fn res(&self) -> u32 {
        self.inner.res()
    }
    fn block_cells(&self) -> u32 {
        self.inner.block_cells()
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.calls.fetch_add(1, Relaxed);
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
        max_level: planet_frame::production_finest_level(cells),
        ..LodParams::earth_1080p()
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn main() -> Result<(), String> {
    let dir = arg("--store").ok_or("--store DIR needed")?;
    let res: u32 = arg("--global-res").ok_or("--global-res N needed")?.parse().map_err(|e| format!("{e}"))?;
    let part = arg("--part").unwrap_or_else(|| "bands".into());
    let threads: usize = arg("--threads").map_or(20, |t| t.parse().expect("threads"));
    let cells = 32u32;
    let def = WorldDefinition::single_planet(RADIUS_M, 1);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
    let key = world.stage_key(res).0;
    let (_, block_cells) = world.layout(res).map_err(|e| format!("{e:?}"))?;
    let map = TangentWarp;
    fn make_field<'a>(
        def: &WorldDefinition,
        src: &'a dyn LatticeSource,
        key: [u8; 16],
    ) -> Result<planet_generators::erosion::ErosionField<'a>, String> {
        ErosionGenerator.field(&GenContext::new(def), BodyId::PLANET, src, key).map_err(|e| format!("{e:?}"))
    }

    // The script's cameras need the terrain height under the site: a throwaway source.
    let probe = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
    let probe_field = make_field(&def, &probe, key)?;
    let script = build_script(StreamScene::Dive, RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), &probe_field, 1);
    let lod = lod(&probe_field, cells);
    let seed = probe_field.seed();
    let request = |camera| TerrainRequest {
        camera,
        size: SIZE,
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: map.id().to_string(),
        seed,
    };

    if part == "tilegen" {
        // The tile generation time per level: one tile of the level under the dive's site, cold source, one thread (the compact
        // format the app streams), then the next tile east of it over the same source (its blocks partly cached).
        let site = script.cameras[script.cameras.len() - 1].position.0.normalized();
        println!("TILEGEN cold compact tile under the site, 1 thread: level, seconds, decodes (distinct blocks), then the neighbour over the warm source");
        for level in [0u8, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 22] {
            let tile = planet_core::TileId::from_direction(&map, site, level);
            let src = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
            let rec = Recording { inner: &src, cells: block_cells, blocks: Mutex::new(BTreeSet::new()), calls: AtomicU64::new(0) };
            let field = make_field(&def, &rec, key)?;
            let t = Instant::now();
            let c = planet_generators::compact::compact_tile_field(&field, &map, tile, cells, RADIUS_M);
            let cold = ms(t);
            std::hint::black_box(&c);
            let (dec, blocks) = (src.stats().decodes, rec.blocks.lock().unwrap().len());
            let next = planet_core::TileId::new(tile.face(), level, (tile.x() + 1).min((1u32 << level) - 1), tile.y()).unwrap();
            let t = Instant::now();
            std::hint::black_box(planet_generators::compact::compact_tile_field(&field, &map, next, cells, RADIUS_M));
            println!(
                "TILEGEN level {level:>2}: {:.3} s, {dec} decodes ({blocks} blocks); neighbour {:.3} s, {} more decodes",
                cold / 1e3,
                ms(t) / 1e3,
                src.stats().decodes - dec
            );
        }
        return Ok(());
    }
    if part == "dive" {
        let src = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
        let rec = Recording { inner: &src, cells: block_cells, blocks: Mutex::new(BTreeSet::new()), calls: AtomicU64::new(0) };
        let field = make_field(&def, &rec, key)?;
        let mut cache = SelectionCache::new();
        let mut t = 0.0f64;
        let mut worst = 0.0f64;
        println!(
            "DIVE plan column ({threads} threads, cold source, one cache): script t, altitude, nodes, plan ms, R ms, new blocks, decodes"
        );
        while t < DIVE_DESCENT_S + 1.0 {
            let frame = ((t / FRAME_STEP_S).round() as usize).min(script.cameras.len() - 1);
            let req = request(script.cameras[frame]);
            let before = (rec.blocks.lock().unwrap().len(), src.stats().decodes);
            let started = Instant::now();
            let plan = plan_terrain_threaded(&req, &field, &mut cache, threads).map_err(|e| e.to_string())?;
            let plan_ms = ms(started);
            let started = Instant::now();
            let r = residency_of(&req, &field, &mut cache).map_err(|e| e.to_string())?;
            let r_ms = ms(started);
            worst = worst.max(plan_ms);
            let (lo, hi) = plan.nodes.iter().fold((u8::MAX, 0u8), |(l, h), n| (l.min(n.level), h.max(n.level)));
            println!(
                "DIVE t {t:6.2} s alt {:>12.0} m nodes {:>6} levels {lo}-{hi} R {:>6} plan {plan_ms:>8.1} ms R {r_ms:>8.1} ms blocks +{} decodes +{}",
                dive_altitude_m(t),
                plan.nodes.len(),
                r.tiles.len(),
                rec.blocks.lock().unwrap().len() - before.0,
                src.stats().decodes - before.1
            );
            t += ((plan_ms + r_ms) / 1e3).max(FRAME_STEP_S);
        }
        println!("DIVE worst plan {worst:.1} ms");
        return Ok(());
    }

    let bands: Vec<f64> = arg("--bands").map_or_else(
        || vec![2.0e7, 5.0e6, 2.0e6, 5.0e5, 2.0e5, 2.0e4, 2.0e3, 200.0, 1.0],
        |s| s.split(',').map(|b| b.parse().expect("band")).collect(),
    );
    println!("BANDS cold plan per altitude band ({threads} threads for the threaded plan); fresh source and cache per measurement");
    for alt in bands {
        // The dive frame at this altitude: h = START * (END/START)^(t/T).
        let u =
            (alt / planet_frame::scenes::DIVE_START_M).ln() / (planet_frame::scenes::DIVE_END_M / planet_frame::scenes::DIVE_START_M).ln();
        let frame = ((u * DIVE_DESCENT_S / FRAME_STEP_S).round() as usize).min(script.cameras.len() - 1);
        let req = request(script.cameras[frame]);
        let fresh =
            || -> Result<(StoreSource<'_>, ()), String> { Ok((StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?, ())) };
        // 1. selection alone.
        let (s1, ()) = fresh()?;
        let r1 = Recording { inner: &s1, cells: block_cells, blocks: Mutex::new(BTreeSet::new()), calls: AtomicU64::new(0) };
        let f1 = make_field(&def, &r1, key)?;
        let t = Instant::now();
        let sel = select_nodes_in_view_cached(&map, &req.camera, &f1, &lod, &mut SelectionCache::new());
        let sel_ms = ms(t);
        let (sel_reads, sel_blocks, sel_dec) = (r1.calls.load(Relaxed), r1.blocks.lock().unwrap().len(), s1.stats().decodes);
        let dirs: Vec<_> = sel.nodes.iter().map(|n| n.tile.center_direction(&map)).collect();
        let mut levels = std::collections::BTreeMap::new();
        for n in &sel.nodes {
            *levels.entry(n.tile.level()).or_insert(0u32) += 1;
        }
        // 2. origin heights one at a time, in selection order.
        let (s2, ()) = fresh()?;
        let r2 = Recording { inner: &s2, cells: block_cells, blocks: Mutex::new(BTreeSet::new()), calls: AtomicU64::new(0) };
        let f2 = make_field(&def, &r2, key)?;
        let t = Instant::now();
        let single: Vec<f64> = dirs.iter().map(|&d| f2.height(d)).collect();
        let single_ms = ms(t);
        let (o_reads, o_blocks, o_dec) = (r2.calls.load(Relaxed), r2.blocks.lock().unwrap().len(), s2.stats().decodes);
        // 3. the same as one batch.
        let (s3, ()) = fresh()?;
        let r3 = Recording { inner: &s3, cells: block_cells, blocks: Mutex::new(BTreeSet::new()), calls: AtomicU64::new(0) };
        let f3 = make_field(&def, &r3, key)?;
        let t = Instant::now();
        let batch = f3.heights(&dirs);
        let batch_ms = ms(t);
        let (b_reads, b_blocks, b_dec) = (r3.calls.load(Relaxed), r3.blocks.lock().unwrap().len(), s3.stats().decodes);
        assert!(single.iter().zip(&batch).all(|(a, b)| a.to_bits() == b.to_bits()), "batch differs from single evaluation");
        // 4. whole plans, 1 thread and `threads` threads.
        let mut plans = String::new();
        for th in [1usize, threads] {
            let (s4, ()) = fresh()?;
            let r4 = Recording { inner: &s4, cells: block_cells, blocks: Mutex::new(BTreeSet::new()), calls: AtomicU64::new(0) };
            let f4 = make_field(&def, &r4, key)?;
            let t = Instant::now();
            let plan = plan_terrain_threaded(&req, &f4, &mut SelectionCache::new(), th).map_err(|e| e.to_string())?;
            plans += &format!(
                " | plan {th}t {:.0} ms ({} nodes, {} reads, {} blocks, {} decodes)",
                ms(t),
                plan.nodes.len(),
                r4.calls.load(Relaxed),
                r4.blocks.lock().unwrap().len(),
                s4.stats().decodes
            );
        }
        println!(
            "BAND {alt:>10.0} m (frame {frame}) nodes {} levels {levels:?} | selection {sel_ms:.0} ms [{sel_reads} reads, {sel_blocks} blocks, {sel_dec} decodes] | origin singly {single_ms:.0} ms [{o_reads} reads, {o_blocks} blocks, {o_dec} decodes] | origin batch {batch_ms:.0} ms [{b_reads} reads, {b_blocks} blocks, {b_dec} decodes]{plans}",
            dirs.len()
        );
    }
    Ok(())
}
