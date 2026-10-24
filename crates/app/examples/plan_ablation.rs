//! M4 item 7a ablation (docs/archive/M4/learnings.md): where the draw plan's time goes over a stored global bake. For each standard
//! view, cold and warm: the selection alone, the per-node origin height, the mesh keys, the whole plan, and the counts of
//! field calls (`height`, `height_bounds`, `inputs_hash`, `read_records*`) per plan. Not production code.
//! `cargo run --release -p planet --example plan_ablation -- --store DIR --global-res N [--mem-res N] [--cells 32]`.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{
    plan_terrain_cached, scripted_views_field, select_nodes_in_view_cached, LodParams, SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{GlobalCoarseGenerator, GridSource, LatticeSource};
use planet_generators::height::HeightField;
use planet_generators::mesh::MeshKeyer;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const RADIUS_M: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

#[derive(Default)]
struct Counting<'a> {
    inner: Option<&'a dyn HeightField>,
    height: AtomicU64,
    bounds: AtomicU64,
    inputs: AtomicU64,
    records: AtomicU64,
}

impl<'a> Counting<'a> {
    fn f(&self) -> &'a dyn HeightField {
        self.inner.unwrap()
    }
    fn take(&self) -> String {
        format!(
            "height {} bounds {} inputs_hash {} records {}",
            self.height.swap(0, Relaxed),
            self.bounds.swap(0, Relaxed),
            self.inputs.swap(0, Relaxed),
            self.records.swap(0, Relaxed)
        )
    }
}

impl HeightField for Counting<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.height.fetch_add(1, Relaxed);
        self.f().height(d)
    }
    fn height_at_level(&self, d: Vec3, level: u8) -> f64 {
        self.height.fetch_add(1, Relaxed);
        self.f().height_at_level(d, level)
    }
    fn level_aware(&self) -> bool {
        self.f().level_aware()
    }
    fn heights_and_climate(
        &self,
        map: &dyn FaceMapping,
        id: TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, planet_generators::climate::ClimateTexture) {
        self.f().heights_and_climate(map, id, dirs, stop)
    }
    fn seed(&self) -> u64 {
        self.f().seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.f().implementation_id()
    }
    fn version(&self) -> u32 {
        self.f().version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.f().max_abs_height_m()
    }
    fn inputs_hash(&self) -> u64 {
        self.inputs.fetch_add(1, Relaxed);
        self.f().inputs_hash()
    }
    fn read_records(&self) -> Vec<planet_world_def::ReadRecord> {
        self.records.fetch_add(1, Relaxed);
        self.f().read_records()
    }
    fn read_records_for(&self, t: TileId, m: &dyn FaceMapping) -> Vec<planet_world_def::ReadRecord> {
        self.records.fetch_add(1, Relaxed);
        self.f().read_records_for(t, m)
    }
    fn reads_cells(&self) -> bool {
        self.f().reads_cells()
    }
    fn inputs_hash_for(&self, t: TileId, m: &dyn FaceMapping) -> u64 {
        self.inputs.fetch_add(1, Relaxed);
        self.f().inputs_hash_for(t, m)
    }
    fn body(&self) -> BodyId {
        self.f().body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.f().source_key()
    }
    fn upstream(&self) -> Option<&dyn planet_generators::height::HeightField> {
        self.f().upstream()
    }
    fn stage_params(&self) -> Option<(&str, u32)> {
        self.f().stage_params()
    }
    fn item_revision(&self) -> u64 {
        self.f().item_revision()
    }
    fn passes_through(&self, tile: planet_core::TileId, map: &dyn planet_core::FaceMapping) -> bool {
        self.f().passes_through(tile, map)
    }
    fn surface_id(&self) -> &'static str {
        self.f().surface_id()
    }
    fn height_bounds(&self, c: Vec3, r: f64) -> (f64, f64) {
        self.bounds.fetch_add(1, Relaxed);
        self.f().height_bounds(c, r)
    }
    fn bounds_monotone(&self) -> bool {
        self.f().bounds_monotone()
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn main() -> Result<(), String> {
    let dir = arg("--store").ok_or("--store DIR needed")?;
    let res: u32 = arg("--global-res").ok_or("--global-res N needed")?.parse().map_err(|e| format!("{e}"))?;
    let cells: u32 = arg("--cells").map_or(32, |c| c.parse().expect("cells"));
    let def = WorldDefinition::single_planet(RADIUS_M, 1);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
    let stored = StoreSource::new(&store, &world, res).map_err(|e| e.to_string())?;
    let grid = match arg("--mem-res") {
        Some(n) => {
            let n: u32 = n.parse().map_err(|e| format!("{e}"))?;
            let g = GlobalCoarseGenerator.field(&GenContext::new(&def), BodyId::PLANET).map_err(|e| format!("{e:?}"))?;
            Some((n, g.bake(&TangentWarp, n, 20).map_err(|e| format!("{e:?}"))?))
        }
        None => None,
    };
    let mem = grid.as_ref().map(|(_, g)| GridSource(g));
    let (src, res): (&dyn LatticeSource, u32) = match (&mem, &grid) {
        (Some(m), Some((n, _))) => (m, *n),
        _ => (&stored, res),
    };
    let key = world.stage_key(res).0;
    let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, key).map_err(|e| format!("{e:?}"))?;
    let counting = Counting { inner: Some(&eroded), ..Default::default() };
    let field: &dyn HeightField = &counting;
    let map = TangentWarp;
    let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), field);
    counting.take();
    println!("plan_ablation: bake {res}, cells {cells}, tau 6, {}x{}, field {}", SIZE.0, SIZE.1, field.implementation_id());
    for (name, cam) in &views {
        let lod = LodParams {
            radius_m: RADIUS_M,
            max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
            cells,
            viewport_h_px: f64::from(SIZE.1),
            tau_px: 6.0,
            max_level: 26,
            ..LodParams::earth_1080p()
        };
        let req = TerrainRequest {
            camera: *cam,
            size: SIZE,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: map.id().to_string(),
            seed: field.seed(),
        };
        let mut cache = SelectionCache::new();
        let mut line = format!("ABL {name:>16}");
        for pass in ["cold", "warm"] {
            counting.take();
            let t = Instant::now();
            let sel = select_nodes_in_view_cached(&map, cam, field, &lod, &mut cache);
            let sel_ms = ms(t);
            let sel_calls = counting.take();
            let t = Instant::now();
            let origins: Vec<PlanetFixed> = sel
                .nodes
                .iter()
                .map(|n| {
                    let c = n.tile.center_direction(&map);
                    PlanetFixed(c * (RADIUS_M + field.height(c)))
                })
                .collect();
            let origin_ms = ms(t);
            std::hint::black_box(&origins);
            counting.take();
            let t = Instant::now();
            let keyer = MeshKeyer::new(field, &map, cells, RADIUS_M);
            let keys: Vec<u64> = sel.nodes.iter().map(|n| keyer.key(n.tile)).collect();
            let key_ms = ms(t);
            std::hint::black_box(&keys);
            let key_calls = counting.take();
            let t = Instant::now();
            let plan = plan_terrain_cached(&req, field, &mut cache).map_err(|e| e.to_string())?;
            let plan_ms = ms(t);
            let plan_calls = counting.take();
            line += &format!(
                " | {pass}: nodes {} sel {sel_ms:.2} ms [{sel_calls}] origin {origin_ms:.2} keys {key_ms:.2} [{key_calls}] plan {plan_ms:.2} [{plan_calls}]",
                plan.nodes.len()
            );
        }
        println!("{line}");
    }
    Ok(())
}
