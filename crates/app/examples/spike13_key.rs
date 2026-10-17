#![allow(clippy::disallowed_methods)]
use planet::spike13::*;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{plan_terrain_threaded, LodParams, SelectionCache};
use planet_generators::compact::tile_data_field;
use planet_generators::mesh::MeshKeyer;
use std::sync::Arc;

fn main() {
    let base = base_world(7, 16);
    let set = Arc::new(StampSet { def: base.0.clone(), stamps: Vec::new() });
    let field = StampField { set, base: Arc::clone(&base.1) };
    let id = TileId::from_raw(648518346341588470).unwrap();
    let keyer = MeshKeyer::new(&field, &TangentWarp, 32, R).at_finest_level(22);
    let t = tile_data_field(&field, &TangentWarp, id, 32, R, 22);
    println!("level {} keyer {:#x} tile {:#x}", id.level(), keyer.key(id), t.key());
    let (la, lo) = (20f64.to_radians(), 30f64.to_radians());
    let dir = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    let stamp = Stamp { id: 1, dir, radius_m: 150.0, amp_m: 30.0 };
    let cam = camera_around(&stamp, &field, 2000.0, 2000.0, 0.0, 16.0 / 9.0);
    let lod = LodParams { cells: 32, viewport_h_px: 720.0, tau_px: 6.0, max_level: 22, max_height_m: 10_000.0, ..LodParams::earth_1080p() };
    let req = request(cam, (1280, 720), &lod);
    let mut cache = SelectionCache::new();
    let plan = plan_terrain_threaded(&req, &field, &mut cache, 4).unwrap();
    println!("plan {} nodes, max_level {}", plan.nodes.len(), lod.max_level);
    for n in plan.nodes.iter().filter(|n| n.tile == id) {
        println!("node key {:#x}", n.mesh_key);
    }
    let any = plan.nodes.iter().find(|n| n.level >= 3).unwrap();
    let t2 = tile_data_field(&field, &TangentWarp, any.tile, 32, R, 22);
    println!("sample node level {} plan key {:#x} tile key {:#x}", any.level, any.mesh_key, t2.key());
}
