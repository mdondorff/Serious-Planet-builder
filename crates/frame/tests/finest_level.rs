//! LOD-010, tier A: the finest geometry level is one LOD parameter (`LodParams::max_level`), with the production default
//! `27 - ceil(log2(cells))` (22 at cells 32, 21 at 64: equal vertex spacing of about 7.5 cm); selection, the residency set R,
//! prefetch, the cover plan and the draw set all read it from there, and the mesh key carries whether a tile is a finest tile of
//! the view, so the compact format's CON-13 guard is a function of what the key identifies.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::prefetch::prefetch_set;
use planet_frame::{
    cover_selection, plan_terrain_field, production_finest_level, residency_set_field, scripted_views_field, select_nodes_in_view_field,
    LodParams, SelectionCache, TerrainDraw, TerrainRequest, TerrainView,
};
use planet_generators::compact::tile_data_field;
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::{finest_class_key, mesh_key_field};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1280, 720);

fn lod(cells: u32, max_level: u8) -> LodParams {
    LodParams { cells, viewport_h_px: f64::from(SIZE.1), tau_px: 6.0, max_level, ..LodParams::earth_1080p() }
}

fn ground_1m() -> planet_frame::Camera {
    let field = Reference(1);
    scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &field).into_iter().find(|(n, _)| *n == "ground-1m").unwrap().1
}

fn request(lod: LodParams) -> TerrainRequest {
    TerrainRequest { camera: ground_1m(), size: SIZE, view: TerrainView::Level, lod, face_mapping: TangentWarp.id().to_string(), seed: 1 }
}

// spec: LOD-010
#[test]
fn the_production_finest_level_keeps_the_vertex_spacing_whatever_the_tile_size() {
    assert_eq!(production_finest_level(32), 22);
    assert_eq!(production_finest_level(64), 21);
    assert_eq!(production_finest_level(16), 23);
    assert_eq!(production_finest_level(8), 24);
    assert_eq!(production_finest_level(1), 26, "never deeper than the reference cap 26");
    assert_eq!(production_finest_level(u32::MAX), 0, "an absurd tile size does not overflow");
    assert_eq!(production_finest_level(48), 21, "a tile size that is not a power of two rounds the spacing up, never down");
    assert_eq!(LodParams::earth_1080p().cells, 32);
    assert_eq!(LodParams::earth_1080p().max_level, 22, "the default LOD parameters are the production ones");
    // The relation: 2^level * cells is constant, so the spacing is: 7.46 cm at Earth's radius.
    let spacing = |cells: u32| {
        let p = lod(cells, production_finest_level(cells));
        p.sample_spacing(p.max_level)
    };
    for cells in [8u32, 16, 32, 64, 128] {
        assert!((spacing(cells) / spacing(32) - 1.0).abs() < 1e-12, "cells {cells}: {} m against {} m", spacing(cells), spacing(32));
    }
    assert!((spacing(32) - 0.0746).abs() < 5e-4, "{} m", spacing(32));
}

fn deepest(tiles: impl IntoIterator<Item = TileId>) -> u8 {
    tiles.into_iter().map(|t| t.level()).max().unwrap()
}

// spec: LOD-010
#[test]
fn selection_r_prefetch_and_the_cover_plan_stop_at_the_cap_and_the_cap_is_reached() {
    let field = Reference(1);
    let camera = ground_1m();
    for cap in [22u8, 23, 24] {
        let params = LodParams { max_height_m: field.max_abs_height_m(), ..lod(32, cap) };
        let draw = select_nodes_in_view_field(&TangentWarp, &camera, &field, &params);
        assert_eq!(deepest(draw.nodes.iter().map(|n| n.tile)), cap, "the draw set reaches the cap {cap} at 1 m and not beyond");
        let mut cache = SelectionCache::new();
        let r = residency_set_field(&TangentWarp, camera.position, &field, &params, &mut cache);
        assert_eq!(deepest(r.tiles.iter().map(|e| e.tile)), cap, "R is selected with the same cap");
        // Prefetch is the set R of a predicted position, minus R now: the same parameter, so the same cap.
        let ahead_pos = PlanetFixed(camera.position.0 + Vec3::new(30.0, 0.0, 0.0));
        let ahead = residency_set_field(&TangentWarp, ahead_pos, &field, &params, &mut cache);
        let fetch = prefetch_set(&r, &ahead);
        assert!(fetch.iter().all(|(t, _)| t.level() <= cap), "prefetch beyond the cap {cap}");
        // The cover plan over a fully resident tree.
        let map = TangentWarp;
        let resident = |t: TileId| Some(PlanetFixed(t.center_direction(&map) * R));
        let cover = cover_selection(&map, &camera, &params, &resident);
        assert!(!cover.is_empty() && deepest(cover.iter().copied()) <= cap, "the cover plan stops at the cap {cap}");
    }
}

fn draws_by_tile(plan: &planet_frame::TerrainPlan) -> std::collections::BTreeMap<TileId, TerrainDraw> {
    plan.nodes.iter().map(|d| (d.tile, *d)).collect()
}

// spec: LOD-010, LOD-009, GEN-010
#[test]
fn a_tile_at_the_cap_has_the_key_of_its_class_and_other_tiles_keep_their_base_key() {
    let field = Reference(1);
    let params = |cap| LodParams { max_height_m: field.max_abs_height_m(), ..lod(32, cap) };
    let p22 = plan_terrain_field(&request(params(22)), &field).unwrap();
    let p23 = plan_terrain_field(&request(params(23)), &field).unwrap();
    let base = |t: TileId| mesh_key_field(&field, &TangentWarp, t, 32, R);
    let mut at_cap = 0;
    for d in &p22.nodes {
        if d.level == 22 {
            at_cap += 1;
            assert_eq!(d.mesh_key, finest_class_key(base(d.tile), d.tile, 22));
            assert_ne!(d.mesh_key, base(d.tile), "a finest tile is another key than the same tile below the cap");
        } else {
            assert_eq!(d.mesh_key, base(d.tile), "{:?} is below the cap and keeps its base key", d.tile);
        }
    }
    assert!(at_cap > 0, "the 1 m view reaches the cap");
    // The same tile at level 22 is a finest tile under cap 22 and an ordinary one under cap 23: another key, so a resident tile
    // decided under one cap is not taken for the other.
    let p23 = draws_by_tile(&p23);
    let t23 = p23.keys().find(|t| t.level() == 23).copied().expect("a level-23 draw under cap 23");
    let a22 = t23.parent().unwrap();
    assert_ne!(finest_class_key(base(a22), a22, 22), finest_class_key(base(a22), a22, 23), "level 22 is finest under cap 22 only");
    assert_eq!(finest_class_key(base(a22), a22, 23), base(a22));
    // A cap change invalidates a warm plan cache's keys (LOD-009): the plan equals the plan of a fresh cache.
    let mut cache = SelectionCache::new();
    let warm22 = planet_frame::plan_terrain_cached(&request(params(22)), &field, &mut cache).unwrap();
    let warm23 = planet_frame::plan_terrain_cached(&request(params(23)), &field, &mut cache).unwrap();
    assert_eq!(warm22.nodes, plan_terrain_field(&request(params(22)), &field).unwrap().nodes);
    assert_eq!(warm23.nodes, plan_terrain_field(&request(params(23)), &field).unwrap().nodes);
}

// spec: LOD-010, GEN-019, CON-13
#[test]
fn the_format_of_a_resident_tile_is_a_function_of_its_key() {
    use planet_generators::compact::RenderTile;
    use planet_generators::stress::Steep;
    // The steep field has far more than 65 m of relief on a level-9 tile: it is a vertex-format tile where level 9 is the cap, and
    // a compact one where the cap is deeper. The two must not share a key, or a resident tile decided for one cap would be reused
    // for the other (the compact tile's guard is decided once, at build).
    let steep = Steep(1);
    let tile = TileId::new(planet_core::Face::ALL[4], 9, 100, 100).unwrap();
    let (capped, deeper) = (tile_data_field(&steep, &TangentWarp, tile, 32, R, 9), tile_data_field(&steep, &TangentWarp, tile, 32, R, 10));
    assert!(matches!(capped, RenderTile::Vertex(_)) && deeper.is_compact(), "fixture: the cap decides the format of this tile");
    assert_ne!(capped.key(), deeper.key(), "the key must identify the class the format was decided for");
    // Each key is the plan's key for that cap, whichever format the tile has.
    let base = mesh_key_field(&steep, &TangentWarp, tile, 32, R);
    assert_eq!(capped.key(), finest_class_key(base, tile, 9));
    assert_eq!(deeper.key(), base);
    // Tiles with the same class share a key: the format decision no longer depends on the cap's value.
    assert_eq!(tile_data_field(&steep, &TangentWarp, tile, 32, R, 8).key(), capped.key());
}
