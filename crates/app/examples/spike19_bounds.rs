//! SPIKE-19 (spike branch only): the height bounds the plan reads for the level-4 tile of the refusal and its neighbours.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::lod::tile_extent;
use planet_frame::LodParams;
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let res: u32 = std::env::var("SP19_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(128);
    let seed: u64 = std::env::var("SP19_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, seed), res).unwrap();
    let field = world.field().unwrap();
    println!("field {} v{} max_abs_height {:.0} m", field.implementation_id(), field.version(), field.max_abs_height_m());
    let lod = LodParams { cells: 32, viewport_h_px: 67.0 * 4.0, tau_px: 4.0, max_level: 22, max_height_m: 40_000.0, ..LodParams::earth_1080p() };
    let t = TileId::from_raw(288230376151711948).unwrap();
    println!("lod morph_band {} split(3) {:.1} split(4) {:.1} node_radius_bound(4) {:.1}", lod.morph_band, lod.split_distance(3), lod.split_distance(4), lod.node_radius_bound(4));
    let (a, b) = lod.morph_interval(4);
    println!("morph_interval(4) own [{a:.1}, {b:.1}]");
    for (name, tile) in std::iter::once(("tile", t)).chain(planet_core::Side::ALL.iter().map(|s| ("neighbour", t.neighbor(&TangentWarp, *s)))) {
        let e = tile_extent(&TangentWarp, tile, &lod);
        let (lo, hi) = field.height_bounds(e.dir, e.angular_radius);
        println!("{name} {:?} chord_radius {:.0} m, height_bounds [{lo:.0}, {hi:.0}] m, centre height {:.0}", tile, e.chord_radius_m, field.height(e.dir));
    }
}
