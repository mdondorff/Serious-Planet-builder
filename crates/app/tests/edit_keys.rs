//! STRM-026, WDEF-007: an edit refreshes every resident tile whose key it changes. The editor names the tiles to refresh by the edit's
//! footprint (`planet_editor::feed::edited_tiles`); a tile whose key changed but that the footprint does not name keeps its old version
//! for good (it is never marked stale, so it is never requested again), and the frame draws it as a substitute (edit-stale-diagnostics).

use std::collections::BTreeSet;
use std::sync::Arc;

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_editor::commands::Affected;
use planet_generators::mesh::mesh_key_field;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;
const RES: u32 = 16;
const CELLS: u32 = 32;

fn hill(id: u128, dir: Vec3, reach_m: f64) -> ItemSpec {
    ItemSpec { id, layer: "stamps".into(), priority: 0, dir, content_hash: 0, reach_m, max_delta_m: 60.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [40.0, 0.0, 0.0]))
}

/// Every tile of levels 0 to 5, and the tile under `dir` with its eight neighbours at levels 6 to 12.
fn tiles_around(dir: Vec3) -> Vec<TileId> {
    let mut out = BTreeSet::new();
    for f in 0..6u8 {
        for level in 0..=5u8 {
            let n = 1u32 << level;
            for x in 0..n {
                for y in 0..n {
                    out.insert(TileId::new(Face(f), level, x, y).unwrap());
                }
            }
        }
    }
    for level in 6..=12u8 {
        let t = TileId::from_direction(&TangentWarp, dir, level);
        out.insert(t);
        for n in t.neighbors8() {
            out.insert(n);
            out.extend(n.neighbors8());
        }
    }
    out.into_iter().collect()
}

/// The tiles the editor names for refresh after an edit, as `EditRuntime::refresh_tiles` does with the definition after the edit.
fn named_by_editor(def: &WorldDefinition, affected: &[Affected], tiles: &[TileId]) -> BTreeSet<TileId> {
    planet_editor::feed::edited_tiles_reading(affected, BodyId::PLANET, tiles, R, &TangentWarp, &|t| {
        planet::editwin::reads_summary(def, BodyId::PLANET, t)
    })
    .into_iter()
    .collect()
}

fn keys(world: &WorldTerrain, tiles: &[TileId]) -> Vec<u64> {
    let field = world.field().unwrap();
    tiles.iter().map(|t| mesh_key_field(field.as_ref(), &TangentWarp, *t, CELLS, R)).collect()
}

/// The re-keyed tiles of levels `from..` that the footprint of an edit at `at` with `reach_m` does not name.
fn missed(at: Vec3, reach_m: f64, from: u8) -> (usize, Vec<String>) {
    let existing = Vec3::new(0.3, -0.8, 0.5).normalized();
    let mut def = WorldDefinition::single_planet(R, 5);
    def.put_item_spec(BodyId::PLANET, hill(1, existing, 3_000.0)).unwrap();
    let world = Arc::new(WorldTerrain::open(def.clone(), RES).unwrap());
    let tiles = tiles_around(at);
    let before = keys(&world, &tiles);
    // The edit: a new hill, as the editor puts it.
    def.put_item_spec(BodyId::PLANET, hill(2, at, reach_m)).unwrap();
    world.set_definition(Arc::new(def.clone())).unwrap();
    let after = keys(&world, &tiles);
    let affected = [Affected { body: BodyId::PLANET, dir: at, reach_m, brush: None }];
    let named = named_by_editor(&def, &affected, &tiles);
    let changed: Vec<TileId> = tiles.iter().zip(before.iter().zip(&after)).filter(|(_, (a, b))| a != b).map(|(t, _)| *t).collect();
    assert!(!changed.is_empty(), "the edit changes some keys");
    let missed = changed
        .iter()
        .filter(|t| t.level() >= from && !named.contains(t))
        .map(|t| format!("face {} level {} ({}, {})", t.face().0, t.level(), t.x(), t.y()))
        .collect();
    (changed.len(), missed)
}

// spec: STRM-070, STRM-026, WDEF-007, WDEF-008
#[test]
fn every_tile_an_edit_re_keys_is_named_by_its_footprint() {
    // The find's halo is the stamp stage's: the summary covers it walks are the ones the keys read.
    assert_eq!(planet_editor::feed::REFRESH_HALO.to_bits(), planet_generators::stamp::STAGE_HALO.to_bits());
    let cases = [
        ("inside a face", Vec3::new(1.0, 0.2, 0.1).normalized(), 3_000.0),
        ("near a face edge", Vec3::new(1.0, 0.999, 0.1).normalized(), 3_000.0),
        ("near a cube corner", Vec3::new(1.0, 0.999, 0.998).normalized(), 3_000.0),
        ("a wide reach", Vec3::new(1.0, 0.2, 0.1).normalized(), 40_000.0),
    ];
    let mut report = Vec::new();
    for (name, at, reach_m) in cases {
        let (changed, missed) = missed(at, reach_m, 0);
        if !missed.is_empty() {
            report.push(format!("{name}: {} of {changed} re-keyed tiles not named: {:?}", missed.len(), missed));
        }
    }
    assert!(
        report.is_empty(),
        "re-keyed tiles the edit footprint does not name (they would stay on their old version):
{}",
        report.join(
            "
"
        )
    );
}

// spec: STRM-070, STRM-026, WDEF-007, WDEF-008
#[test]
fn a_coarse_tile_with_an_item_is_named_when_an_edit_lands_in_its_summary_margin() {
    // A coarse tile reads its region summary only when an item touches it (a tile with an empty footprint keeps the base's key). Its cover
    // reaches past its cap by the longest reach and whole cells, so an edit anchored there re-keys it although the edit's reach does not
    // touch the tile: the tile must still be named (numerics review of edit-stale-diagnostics).
    let n = TileId::from_direction(&TangentWarp, Vec3::new(1.0, 0.05, 0.05).normalized(), 5);
    let centre = n.center_direction(&TangentWarp);
    // 250 km outside the tile's edge along the face's v axis: the tile's half edge is about 156 km at level 5.
    let up = Vec3::new(0.0, 1.0, 0.0);
    let tangent = (up - centre * up.dot(centre)).normalized();
    let angle = (156_000.0 + 250_000.0) / R;
    let at = (centre * libm::cos(angle) + tangent * libm::sin(angle)).normalized();
    let mut def = WorldDefinition::single_planet(R, 5);
    def.put_item_spec(BodyId::PLANET, hill(1, centre, 3_000.0)).unwrap();
    let world = Arc::new(WorldTerrain::open(def.clone(), RES).unwrap());
    let tiles: Vec<TileId> = (3..=5u8)
        .flat_map(|l| {
            let t = TileId::from_direction(&TangentWarp, centre, l);
            let mut v = vec![t];
            v.extend(t.neighbors8());
            v
        })
        .collect();
    let before = keys(&world, &tiles);
    def.put_item_spec(BodyId::PLANET, hill(2, at, 3_000.0)).unwrap();
    world.set_definition(Arc::new(def.clone())).unwrap();
    let after = keys(&world, &tiles);
    let affected = [Affected { body: BodyId::PLANET, dir: at, reach_m: 3_000.0, brush: None }];
    let named = named_by_editor(&def, &affected, &tiles);
    let at_n = tiles.iter().position(|t| *t == n).unwrap();
    assert_ne!(before[at_n], after[at_n], "the edit in the tile's summary margin re-keys it");
    let missed: Vec<String> = tiles
        .iter()
        .zip(before.iter().zip(&after))
        .filter(|(t, (a, b))| a != b && !named.contains(t))
        .map(|(t, _)| format!("face {} level {} ({}, {})", t.face().0, t.level(), t.x(), t.y()))
        .collect();
    assert!(missed.is_empty(), "re-keyed coarse tiles not named: {missed:?}");
    // Exact: no coarse tile is named whose key did not change (a false name would only cost a refresh, but the find walks the cover exactly).
    let extra: Vec<TileId> =
        tiles.iter().zip(before.iter().zip(&after)).filter(|(t, (a, b))| a == b && named.contains(t)).map(|(t, _)| *t).collect();
    assert!(extra.is_empty(), "named although their key did not change: {extra:?}");
}
