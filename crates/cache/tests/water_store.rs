//! GEN-023 over the store: the water-level field reads the stored bake and the stored ocean channel and gives the answers of the
//! in-memory computation, with the sea of the definition (its level and its liquid).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::{build_global, GlobalWorld, StoreOceanSource, StoreSource};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::hash::hash_u64s;
use planet_core::Vec3;
use planet_generators::global::{bake, GridSource};
use planet_generators::ocean::tiled_ocean;
use planet_generators::water::{SeaProducer, WaterBodyId, WaterField};
use planet_world_def::{BodyId, LiquidKind, LiquidKindId, SeaParams, WorldDefinition};

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-water-store-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&p);
        Dir(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn directions(n: u64) -> Vec<Vec3> {
    (0..n)
        .map(|k| {
            let c = |a: u64| (hash_u64s(0x7761_7465, &[k, a]) >> 11) as f64 / (1u64 << 52) as f64 * 2.0 - 1.0;
            Vec3::new(c(0), c(1), c(2)).normalized()
        })
        .collect()
}

// spec: GEN-023
// spec: STRM-023
#[test]
fn the_field_over_the_store_equals_the_field_over_the_in_memory_bake_and_takes_the_definitions_sea() {
    let mut def = WorldDefinition::single_planet(6_371_000.0, 5);
    def.set_liquid(LiquidKindId(0), &LiquidKind::WATER).unwrap();
    let lava = LiquidKind {
        colour: [0.8, 0.1, 0.02],
        emission: [40.0, 9.0, 0.5],
        viscosity_pa_s: 1.0e3,
        wave_amplitude_m: 0.0,
        wave_length_m: 0.0,
    };
    def.set_liquid(LiquidKindId(1), &lava).unwrap();
    def.set_sea(BodyId::PLANET, &SeaParams { level_m: 3.5, liquid: LiquidKindId(1), min_area_fraction: 0.005 });
    let world = GlobalWorld::from_definition(&def).unwrap().with_block_limit(8);
    assert_eq!(world.sea(), SeaParams { level_m: 3.5, liquid: LiquidKindId(1), min_area_fraction: 0.005 });
    let d = Dir::new("field");
    let res = 64;
    build_global(&d.0, &world, res, 3, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(&d.0, world.params()).unwrap();

    let bake_src = StoreSource::new(&store, &world, res).unwrap();
    let ocean_src = StoreOceanSource::new(&store, &world, res).unwrap();
    let sea = SeaProducer::new(&ocean_src, world.sea());
    let stored = WaterField::new(&TangentWarp, &bake_src, vec![&sea]);

    let grid = bake(world.seed(), &TangentWarp, res, 2).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 8, 2, 0.005).unwrap();
    let mem_bake = GridSource(&grid);
    let mem_sea = SeaProducer::new(&ocean, world.sea());
    let memory = WaterField::new(&TangentWarp, &mem_bake, vec![&mem_sea]);

    let mut wet = 0;
    for dir in directions(3000) {
        let (a, b) = (stored.water_at(dir).unwrap(), memory.water_at(dir).unwrap());
        assert_eq!(a, b, "direction {dir:?}");
        if let Some(w) = a {
            wet += 1;
            assert_eq!((w.level_m, w.body, w.kind), (3.5, WaterBodyId::SEA, LiquidKindId(1)), "the sea of the definition");
        }
    }
    assert!(wet > 500, "{wet}");
    // The kind id names a row of the world's table.
    let table = def.liquids().unwrap();
    assert!(table.get(world.sea().liquid).unwrap().emission[0] > 0.0);

    // An ocean channel of another bake size is refused by the field, naming both.
    let other = Dir::new("other-res");
    build_global(&other.0, &world, 32, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let store32 = TileStore::open(&other.0, world.params()).unwrap();
    let ocean32 = StoreOceanSource::new(&store32, &world, 32).unwrap();
    let wrong = SeaProducer::new(&ocean32, world.sea());
    let e = WaterField::new(&TangentWarp, &bake_src, vec![&wrong]).water_at(directions(1)[0]).unwrap_err().to_string();
    assert!(e.contains("32-cell") && e.contains("64-cell"), "{e}");
}

// spec: WDEF-006
#[test]
fn a_sea_whose_liquid_is_not_in_the_table_is_refused_before_anything_is_baked() {
    let mut def = WorldDefinition::single_planet(6_371_000.0, 5);
    def.set_sea(BodyId::PLANET, &SeaParams { liquid: LiquidKindId(5), ..SeaParams::EARTH });
    let e = GlobalWorld::from_definition(&def).err().expect("a dangling kind").to_string();
    assert!(e.contains("body.0.sea.liquid") && e.contains("liquid kind 5"), "{e}");
    def.set_liquid(LiquidKindId(5), &LiquidKind::WATER).unwrap();
    assert!(GlobalWorld::from_definition(&def).is_ok(), "defining kind 5 makes the table hold it");
}
