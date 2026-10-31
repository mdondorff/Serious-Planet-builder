//! The production store opens with the current build without a rebuild (ADR 0086, decision 3; TEST-075). Read only.

use planet::coupledworld::{load_stored, run_cells, Origin, COUPLED_STEPS};
use planet::worldfield::WorldTerrain;
use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_cache::pack::TileStore;
use planet_world_def::WorldDefinition;

const STORE: &str = "C:/DEV/SeriousPlanetData/global16384";
const RES: u32 = 16384;

/// The store records no whole-definition hash: a record is found by the keys its definition derives (body parameters, the node set id, the bake's
/// stage key; the coupled run's key adds the bake's identity). So "the definition hash is the one the store was built under" is shown by the build
/// finding the records of the default world (seed 1, Earth radius) with those keys: the packs load with no problem, the bake, the ocean channel,
/// the wet floor and the water summary open (`WorldTerrain::open_store`, the viewer's own check), and the stored coupled run loads by its key.
/// `PLANET_REAL_STORE=DIR` names another store. Nothing is written: readers take no lock and no function here appends. `cargo test -p planet --release --test it the_real_store_opens -- --ignored
/// --nocapture`; prints `real store: MATCH`, or `real store: NOT RUN (reason)` and fails when the store is absent.
// spec: TEST-075
#[test]
#[ignore = "reads the real store C:/DEV/SeriousPlanetData/global16384 (read only); NOT RUN when it is absent"]
fn the_real_store_opens_without_a_rebuild() {
    let given = std::env::var_os("PLANET_REAL_STORE");
    let dir = given.as_deref().map_or(std::path::Path::new(STORE), std::path::Path::new);
    if !dir.join("pack-00000001.tpack").is_file() {
        println!("real store: NOT RUN ({} has no pack: the check needs the production store)", dir.display());
        panic!("real store: NOT RUN ({} has no pack)", dir.display());
    }
    let def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
    println!("definition hash {:016x}, base hash {:016x}", def.hash(), def.base_hash());
    let world = GlobalWorld::from_definition(&def).expect("the default world is generable");
    let store = TileStore::open(dir, world.params()).expect("the store opens");
    assert!(store.problems().is_empty(), "packs skipped or damaged under the current build: {:?}", store.problems());
    WorldTerrain::open_store(def.clone(), dir, RES).unwrap_or_else(|e| panic!("real store: MISMATCH, the viewer's open refuses it: {e}"));
    let src = StoreSource::new(&store, &world, RES).expect("the bake of 16384 cells");
    let run = load_stored(&def, Some(dir), &src, run_cells(RES), COUPLED_STEPS)
        .expect("the key of the coupled run is made")
        .expect("real store: MISMATCH, the stored coupled run is not found under the current key");
    assert!(matches!(run.origin, Origin::Stored));
    println!("real store: MATCH (packs load, the bake, ocean, wet floor and summary open, the coupled run loads by its key)");
}
