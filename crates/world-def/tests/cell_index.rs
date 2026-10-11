//! WDEF-005: the spatial cell index and its read recording.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_world_def::cells::{cell_level, cell_of, cells_covering, empty_cell_hash};
use planet_world_def::{cell_query_path, BodyId, GenContext, NodeValue, WorldDefinition};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn planet() -> WorldDefinition {
    WorldDefinition::single_planet(R, 7)
}

// spec: WDEF-005
#[test]
fn cells_are_about_forty_kilometres_wide_whatever_the_body() {
    assert_eq!(cell_level(R), 8, "ADR 0007: level 8 at Earth's radius");
    assert_eq!(cell_level(1_737_400.0), 6);
    assert_eq!(cell_level(1.0), 0);
    assert_eq!(cell_level(0.0), 0);
    assert_eq!(cell_level(f64::NAN), 0);
    assert_eq!(cell_level(1e18), 20, "clamped");
    // The cell edge at the chosen level is within a factor of sqrt(2) of the target.
    for r in [500_000.0, 1_737_400.0, R, 69_911_000.0] {
        let edge = std::f64::consts::FRAC_PI_2 * r / f64::from(1u32 << cell_level(r));
        assert!((28_000.0..=57_000.0).contains(&edge) || cell_level(r) == 20, "{r}: {edge}");
    }
}

// spec: WDEF-005
#[test]
fn a_tile_touches_its_ancestor_cell_or_every_descendant_cell() {
    let d = dir(1.0, 0.2, 0.1);
    let cell = cell_of(R, d);
    assert_eq!(cell.level(), 8);
    // At and below the cell level: the one ancestor cell.
    let finer = TileId::from_direction(&TangentWarp, d, 12);
    assert_eq!(cells_covering(finer, 8, 16), Some(vec![cell]));
    assert_eq!(cells_covering(cell, 8, 16), Some(vec![cell]));
    // Above: all 4^k descendants, sorted, each the ancestor of the cell of a direction inside the tile.
    let coarse = TileId::from_direction(&TangentWarp, d, 6);
    let cover = cells_covering(coarse, 8, 16).unwrap();
    assert_eq!(cover.len(), 16);
    assert!(cover.windows(2).all(|w| w[0].raw() < w[1].raw()));
    assert!(cover.contains(&cell));
    assert!(cover.iter().all(|c| c.level() == 8));
    // Too many cells: the tile reads the whole body instead.
    assert_eq!(cells_covering(coarse, 8, 15), None);
    assert_eq!(cells_covering(TileId::new(Face(0), 0, 0, 0).unwrap(), 8, 1000), None);
    // Every direction inside the tile lands in one of its cells.
    for i in 0..200 {
        let t = f64::from(i) / 200.0;
        let probe = coarse.sample_direction(&TangentWarp, 64, (t * 63.0) as u32, ((1.0 - t) * 63.0) as u32);
        assert!(cover.contains(&cell_of(R, probe)), "probe {i}");
    }
}

// spec: WDEF-005
#[test]
fn items_change_the_definition_hash_but_not_the_base_hash_and_order_does_not_matter() {
    let plain = planet();
    let mut a = planet();
    a.put_item(BodyId::PLANET, 1, dir(1.0, 0.2, 0.1), 111).unwrap();
    a.put_item(BodyId::PLANET, 2, dir(-1.0, 0.3, 0.2), 222).unwrap();
    assert_ne!(a.hash(), plain.hash());
    assert_eq!(a.base_hash(), plain.base_hash(), "authored items are not part of the base");
    assert_eq!(plain.hash(), plain.base_hash(), "no items: exactly the node hash");
    let mut b = planet();
    b.put_item(BodyId::PLANET, 2, dir(-1.0, 0.3, 0.2), 222).unwrap();
    b.put_item(BodyId::PLANET, 1, dir(1.0, 0.2, 0.1), 111).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.hash(), b.hash());
    // Content, id and place each matter; removing everything restores the plain hash.
    let mut c = a.clone();
    c.put_item(BodyId::PLANET, 1, dir(1.0, 0.2, 0.1), 112).unwrap();
    assert_ne!(c.hash(), a.hash());
    c.put_item(BodyId::PLANET, 1, dir(0.2, 1.0, 0.1), 111).unwrap();
    assert_ne!(c.hash(), a.hash());
    assert!(c.remove_item(BodyId::PLANET, 1));
    assert!(!c.remove_item(BodyId::PLANET, 1));
    assert!(c.remove_item(BodyId::PLANET, 2));
    assert_eq!(c.hash(), plain.hash());
    assert_eq!(c, plain);
}

// spec: WDEF-005
#[test]
fn an_item_needs_its_bodys_radius_and_moves_between_cells() {
    let mut d = WorldDefinition::new(1);
    let e = d.put_item(BodyId(2), 1, dir(1.0, 0.0, 0.0), 5).unwrap_err();
    assert!(e.0.contains("body.2.radius_m"), "{e}");
    d.set("body.0.radius_m", NodeValue::U64(5));
    assert!(d.put_item(BodyId::PLANET, 1, dir(1.0, 0.0, 0.0), 5).unwrap_err().0.contains("not a number"));
    let mut d = planet();
    let from = d.put_item(BodyId::PLANET, 9, dir(1.0, 0.2, 0.1), 5).unwrap();
    assert_eq!(d.items_in_cell(BodyId::PLANET, from), vec![(9, 5)]);
    let to = d.put_item(BodyId::PLANET, 9, dir(-1.0, 0.3, 0.2), 5).unwrap();
    assert_ne!(from, to);
    assert!(d.items_in_cell(BodyId::PLANET, from).is_empty(), "moving an item empties its old cell");
    assert_eq!(d.items_in_cell(BodyId::PLANET, to), vec![(9, 5)]);
}

// spec: WDEF-005
#[test]
fn a_cell_query_is_recorded_even_when_the_cell_is_empty() {
    let d = planet();
    let cell = cell_of(R, dir(1.0, 0.2, 0.1));
    let ctx = GenContext::new(&d);
    assert!(ctx.query_cell(BodyId::PLANET, cell).unwrap().is_empty());
    let reads = ctx.reads();
    let path = cell_query_path(BodyId::PLANET, cell);
    assert!(reads.iter().any(|(p, h)| *p == path && *h == empty_cell_hash()), "{reads:?}");
    // The radius that fixes the cell level is a read too.
    assert!(reads.iter().any(|(p, _)| p == "body.0.radius_m"));
    // A cell of the wrong level is an error naming the expected one.
    let wrong = TileId::from_direction(&TangentWarp, dir(1.0, 0.2, 0.1), 5);
    assert!(ctx.query_cell(BodyId::PLANET, wrong).unwrap_err().0.contains("level 8"));
}

// spec: WDEF-005
#[test]
fn adding_an_item_changes_the_read_set_of_a_queried_cell_and_of_no_other() {
    let near = dir(1.0, 0.2, 0.1);
    let far = dir(-1.0, 0.3, 0.2);
    let read_set = |d: &WorldDefinition, around: Vec3| {
        let ctx = GenContext::new(d);
        ctx.query_cell(BodyId::PLANET, cell_of(R, around)).unwrap();
        ctx.read_set_hash()
    };
    let plain = planet();
    let mut with_far = planet();
    with_far.put_item(BodyId::PLANET, 1, far, 1).unwrap();
    // Queried and found empty before; an item there changes the key. An item elsewhere does not.
    assert_eq!(read_set(&plain, near), read_set(&with_far, near));
    assert_ne!(read_set(&plain, far), read_set(&with_far, far));
    // Whole-tile queries record every cell of the footprint, and say so when the footprint is too large.
    let ctx = GenContext::new(&with_far);
    let tile = TileId::from_direction(&TangentWarp, far, 7);
    let items = ctx.query_tile(BodyId::PLANET, tile, 16).unwrap().unwrap();
    assert_eq!(items, vec![(1, 1)]);
    assert_eq!(ctx.reads().iter().filter(|(p, _)| p.starts_with("derived.cell.0.")).count(), 4);
    assert_eq!(ctx.query_tile(BodyId::PLANET, tile, 3).unwrap(), None);
}
