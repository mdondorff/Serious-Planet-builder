//! ADR 0021 point 7 (M3 gate condition 2): the reads a stage makes while it generates are a subset of the reads its tile declared at plan time,
//! and its key follows them. Run over the whole stack of authored stages (stamps and lines on one layer, brush strokes on another) and over a set
//! of tiles from continental to metre scale, near the items and far from them. A stage that read an item its tile did not declare would key
//! wrongly (learning 2026-10-11); one that declared too little would not re-key the tile when that item changes.

use std::sync::{Arc, Mutex};

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::{ItemReader, StampStage, STAGE_HALO};
use planet_world_def::footprint::{angle_between, Cap, FootprintItem};
use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
use planet_world_def::{BodyId, ItemSpec, ReadRecord, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const BRUSH_LAYER: &str = "brush";
/// Every item of the test world with its layer.
const ITEMS: [(&str, u128); 8] = [
    (DEFAULT_ITEM_LAYER, 1),
    (DEFAULT_ITEM_LAYER, 2),
    (DEFAULT_ITEM_LAYER, 3),
    (DEFAULT_ITEM_LAYER, 4),
    (DEFAULT_ITEM_LAYER, 5),
    (DEFAULT_ITEM_LAYER, 6),
    (BRUSH_LAYER, 7),
    (BRUSH_LAYER, 8),
];

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

/// The point `d_m` metres from `anchor` towards `hint` on the sphere.
fn at_distance(anchor: Vec3, hint: Vec3, d_m: f64) -> Vec3 {
    let t = (hint - anchor * anchor.dot(hint)).normalized();
    let a = d_m / R;
    (anchor * libm::cos(a) + t * libm::sin(a)).normalized()
}

/// A point east of `p` on its tangent (a hint for [`at_distance`]).
fn east_of(p: Vec3) -> Vec3 {
    p + Vec3::new(0.0, 0.0, 1.0).cross(p).normalized()
}

fn stamp_item(id: u128, at: Vec3, reach_m: f64, kind: StampKind, params: [f64; 3]) -> ItemSpec {
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: id as i32,
        dir: at,
        content_hash: 0,
        reach_m,
        max_delta_m: 40.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(kind, params))
}

fn line_item(id: u128, kind: StampKind, amount: f64, width: f64, points: &[(f64, f64)]) -> ItemSpec {
    let dirs: Vec<Vec3> = points.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let far = dirs.iter().map(|d| angle_between(dirs[0], *d) * R).fold(0.0f64, f64::max);
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: dirs[0],
        content_hash: 0,
        reach_m: far + width,
        max_delta_m: amount,
        stamp: None,
    }
    .with_stamp(Stamp::new(kind, [amount, width, 0.0]).with_points(&dirs))
}

fn brush_item(id: u128, points: &[(f64, f64)]) -> ItemSpec {
    let dirs: Vec<Vec3> = points.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let far = dirs.iter().map(|d| angle_between(dirs[0], *d) * R).fold(0.0f64, f64::max);
    let stroke = Stroke { kind: BrushKind::Raise, radius_m: 300.0, strength_m: 20.0, target_m: None, points: dirs.clone() };
    ItemSpec {
        id,
        layer: BRUSH_LAYER.into(),
        priority: 0,
        dir: dirs[0],
        content_hash: 0,
        reach_m: far + 300.0,
        max_delta_m: 20.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(&[stroke]))
}

/// The authored world: stamps and two lines on the default layer, two brush items on the brush layer; some under the tiles of the test, some far.
fn world() -> Arc<WorldDefinition> {
    let a = unit(20.0, 30.0);
    let east = east_of(a);
    let items = [
        stamp_item(1, a, 900.0, StampKind::Hill, [15.0, 0.0, 0.0]),
        stamp_item(2, at_distance(a, east, 700.0), 1_200.0, StampKind::Crater, [15.0, 0.0, 0.0]),
        stamp_item(3, at_distance(a, east, 2_500.0), 3_000.0, StampKind::Plateau, [3.0, 0.0, 0.0]),
        stamp_item(4, at_distance(a, east, 60_000.0), 8_000.0, StampKind::Hill, [15.0, 0.0, 0.0]),
        line_item(5, StampKind::Ridge, 40.0, 400.0, &[(20.0, 30.0), (20.004, 30.003), (20.0, 30.006)]),
        line_item(6, StampKind::Channel, 30.0, 300.0, &[(25.0, 40.0), (25.0, 40.01)]),
        brush_item(7, &[(20.001, 30.001), (20.002, 30.004)]),
        brush_item(8, &[(-40.0, -100.0), (-40.0, -99.99)]),
    ];
    let mut d = WorldDefinition::single_planet(R, 1);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i).unwrap();
    }
    Arc::new(d)
}

/// A reader that records every item the stage asked for, under its layer (the recording context of ADR 0021 point 7).
struct Recording {
    inner: Arc<WorldDefinition>,
    seen: Arc<Mutex<Vec<(String, u128)>>>,
}

impl ItemReader for Recording {
    fn radius_m(&self, body: BodyId) -> Option<f64> {
        self.inner.radius_m(body)
    }
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem> {
        let items = self.inner.items(body, layer, cap);
        self.seen.lock().unwrap().extend(items.iter().map(|i| (layer.to_string(), i.id)));
        items
    }
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64 {
        self.inner.max_delta_sum(body, layer)
    }
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord> {
        self.inner.tile_reads(body, layer, tile, halo)
    }
    fn revision(&self) -> u64 {
        self.inner.revision()
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.inner.footprint_empty(body, layer, tile, halo)
    }
}

/// The stack of stages over the reference field: the default layer, then the brush layer on top, each reading through `reader`.
fn stack(reader: Arc<dyn ItemReader>) -> StampStage<'static> {
    let lower =
        StampStage::new(Arc::new(Reference(1)), reader.clone(), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).expect("a body with a radius");
    StampStage::new(Arc::new(lower), reader, BodyId::PLANET, BRUSH_LAYER, 1, 1).expect("a body with a radius")
}

/// The directions a tile samples: its grid and a one-sample halo.
fn tile_dirs(tile: TileId) -> Vec<Vec3> {
    let mut dirs = Vec::new();
    for l in -1..=17i64 {
        for k in -1..=17i64 {
            dirs.push(tile.sample_direction_ext(&TangentWarp, 16, k, l));
        }
    }
    dirs
}

/// The tiles of the test: continental to metre scale over an item, three kilometres from it, and on the far side of the planet.
fn tiles() -> Vec<TileId> {
    let a = unit(20.0, 30.0);
    let mut out = Vec::new();
    for centre in [a, at_distance(a, east_of(a), 3_000.0), unit(-40.0, -100.0)] {
        for level in [5u8, 8, 11, 13, 15, 17] {
            out.push(TileId::from_direction(&TangentWarp, centre, level));
        }
    }
    out
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|h| h.to_bits()).collect()
}

fn declared(def: &WorldDefinition, layer: &str, tile: TileId) -> Vec<u128> {
    def.footprint_items(BodyId::PLANET, layer, tile, STAGE_HALO).unwrap().iter().map(|i| i.id).collect()
}

// spec: GEN-026, GEN-028, GEN-030, WDEF-007
#[test]
fn every_item_a_stage_reads_while_it_generates_a_tile_is_declared_by_the_tile() {
    let def = world();
    let mut read_something = false;
    for tile in tiles() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let top = stack(Arc::new(Recording { inner: Arc::clone(&def), seen: Arc::clone(&seen) }));
        let _ = top.heights(&tile_dirs(tile));
        let seen = seen.lock().unwrap().clone();
        read_something |= !seen.is_empty();
        for (layer, id) in &seen {
            let declared = declared(&def, layer, tile);
            assert!(
                declared.contains(id),
                "tile {tile:?} (level {}): item {id} of layer {layer} was read but the tile did not declare it (declared {declared:?})",
                tile.level()
            );
        }
    }
    assert!(read_something, "the test world has items under some tile: a run that reads nothing proves nothing");
}

// spec: GEN-026, GEN-028, GEN-030, WDEF-007
#[test]
fn an_item_a_tile_does_not_declare_changes_none_of_its_heights_and_none_of_its_fine_keys() {
    let def = world();
    let mut pruned_any = false;
    for tile in tiles() {
        let dirs = tile_dirs(tile);
        let full = stack(def.clone() as Arc<dyn ItemReader>);
        let mut pruned = (*def).clone();
        for (layer, id) in ITEMS {
            if !declared(&def, layer, tile).contains(&id) {
                assert!(pruned.remove_item(BodyId::PLANET, id));
                pruned_any = true;
            }
        }
        let without = stack(Arc::new(pruned) as Arc<dyn ItemReader>);
        assert_eq!(
            bits(&full.heights(&dirs)),
            bits(&without.heights(&dirs)),
            "tile {tile:?} (level {}): removing the items it did not declare changed a height",
            tile.level()
        );
        // At the cell scale the key covers the declared items and nothing else: removing undeclared items does not re-key a fine tile.
        if tile.level() >= 11 {
            assert_eq!(
                full.inputs_hash_for(tile, &TangentWarp),
                without.inputs_hash_for(tile, &TangentWarp),
                "tile {tile:?} (level {}): removing undeclared items re-keyed it",
                tile.level()
            );
        }
    }
    assert!(pruned_any, "some item is out of reach of some tile");
}

// spec: GEN-026, GEN-028, GEN-030, WDEF-007
#[test]
fn the_key_of_a_tile_follows_every_item_the_stage_read_for_it() {
    let def = world();
    let mut checked = 0;
    for tile in tiles() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let top = stack(Arc::new(Recording { inner: Arc::clone(&def), seen: Arc::clone(&seen) }));
        let _ = top.heights(&tile_dirs(tile));
        let before = top.inputs_hash_for(tile, &TangentWarp);
        let mut ids: Vec<u128> = seen.lock().unwrap().iter().map(|(_, id)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let mut edited = (*def).clone();
            assert!(edited.remove_item(BodyId::PLANET, id));
            let after = stack(Arc::new(edited) as Arc<dyn ItemReader>).inputs_hash_for(tile, &TangentWarp);
            assert_ne!(before, after, "tile {tile:?} (level {}): item {id} was read but removing it did not re-key the tile", tile.level());
            checked += 1;
        }
    }
    assert!(checked > 0, "some tile reads some item");
}
