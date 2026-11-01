//! WDEF-005: the spatial cell index and its read recording.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_world_def::cells::{cell_level, cell_of, cells_covering, empty_cell_hash, with_margin};
use planet_world_def::{cell_query_all_path, cell_query_path, BodyId, GenContext, ItemError, NodeValue, WorldDefinition};

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
    assert!(matches!(&e, ItemError::Radius(m) if m.0.contains("body.2.radius_m")), "{e}");
    assert!(e.to_string().contains("radius of its body"));
    d.set("body.0.radius_m", NodeValue::U64(5));
    assert!(matches!(d.put_item(BodyId::PLANET, 1, dir(1.0, 0.0, 0.0), 5), Err(ItemError::Radius(m)) if m.0.contains("not a number")));
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        d.set_body_radius(BodyId::PLANET, bad);
        assert!(matches!(d.put_item(BodyId::PLANET, 1, dir(1.0, 0.0, 0.0), 5), Err(ItemError::Radius(_))), "radius {bad}");
    }
    d.set_body_radius(BodyId::PLANET, R);
    for bad in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(f64::NAN, 1.0, 0.0), Vec3::new(f64::INFINITY, 0.0, 0.0), Vec3::new(1e-300, 0.0, 0.0)] {
        assert_eq!(d.put_item(BodyId::PLANET, 1, bad, 5), Err(ItemError::Direction), "{bad:?}");
    }
    assert!(!d.has_items());
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
    let items = ctx.query_tile(BodyId::PLANET, tile, 0, 16).unwrap();
    assert_eq!(items, vec![(1, 1)]);
    assert_eq!(ctx.reads().iter().filter(|(p, _)| p.starts_with("derived.cell.0.")).count(), 4);
    // A margin of one cell reads the ring around the four cells too.
    let ringed = GenContext::new(&with_far);
    assert_eq!(ringed.query_tile(BodyId::PLANET, tile, 1, 64).unwrap(), vec![(1, 1)]);
    assert!(ringed.reads().iter().filter(|(p, _)| p.starts_with("derived.cell.0.")).count() >= 16);
}

// spec: WDEF-005
#[test]
fn a_tile_over_the_cell_limit_reads_the_whole_body_and_is_never_silently_stale() {
    let near = dir(1.0, 0.2, 0.1);
    let far = dir(-1.0, 0.3, 0.2);
    let tile = TileId::from_direction(&TangentWarp, near, 5); // 4^3 = 64 cells
    let hash_of = |d: &WorldDefinition, max: usize| {
        let ctx = GenContext::new(d);
        let items = ctx.query_tile(BodyId::PLANET, tile, 0, max).unwrap();
        (ctx.read_set_hash(), items, ctx.reads())
    };
    let plain = planet();
    let mut other_place = planet();
    other_place.put_item(BodyId::PLANET, 1, far, 1).unwrap();
    // Within the limit the tile does not depend on an item elsewhere; over it, it reads everything.
    assert_eq!(hash_of(&plain, 64).0, hash_of(&other_place, 64).0);
    let (over_plain, items, reads) = hash_of(&plain, 63);
    let (over_other, items_other, _) = hash_of(&other_place, 63);
    assert!(items.is_empty() && items_other == vec![(1, 1)]);
    assert_ne!(over_plain, over_other, "the whole-body read sees every item");
    assert!(reads.iter().any(|(p, _)| *p == cell_query_all_path(BodyId::PLANET)), "{reads:?}");
}

// spec: WDEF-005
#[test]
fn changing_the_radius_across_a_cell_level_boundary_refiles_the_items() {
    // 4.60e6 m is level 7 and 4.62e6 m is level 8 (the boundary is at about 4.6096e6 m).
    assert_eq!((cell_level(4_600_000.0), cell_level(4_620_000.0)), (7, 8));
    let d_item = dir(1.0, 0.2, 0.1);
    let mut d = WorldDefinition::single_planet(4_600_000.0, 7);
    d.put_item(BodyId::PLANET, 1, d_item, 42).unwrap();
    let tile = TileId::from_direction(&TangentWarp, d_item, 11);
    let read = |d: &WorldDefinition| GenContext::new(d).query_tile(BodyId::PLANET, tile, 0, 64).unwrap();
    assert_eq!(read(&d), vec![(1, 42)]);
    d.set_body_radius(BodyId::PLANET, 4_620_000.0);
    assert_eq!(read(&d), vec![(1, 42)], "the item must follow the body to its new cell level");
    assert_eq!(d.items_in_cell(BodyId::PLANET, cell_of(4_620_000.0, d_item)), vec![(1, 42)]);
    assert!(d.items_in_cell(BodyId::PLANET, cell_of(4_600_000.0, d_item)).is_empty());
    // The same definition built the other way round is the same definition.
    let mut e = WorldDefinition::single_planet(4_620_000.0, 7);
    e.put_item(BodyId::PLANET, 1, d_item, 42).unwrap();
    assert_eq!(d, e);
    assert_eq!(d.hash(), e.hash());
    // Removing after the change empties the right cell.
    assert!(d.remove_item(BodyId::PLANET, 1));
    assert!(!d.has_items());
    assert_eq!(d, WorldDefinition::single_planet(4_620_000.0, 7));
}

// spec: WDEF-005
#[test]
fn moving_an_item_inside_its_cell_changes_the_definition_hash_and_the_cell_hash() {
    let a = dir(1.0, 0.2, 0.1);
    let b = dir(1.0, 0.2000001, 0.1);
    assert_eq!(cell_of(R, a), cell_of(R, b), "the two places are in one cell");
    let mut one = planet();
    one.put_item(BodyId::PLANET, 1, a, 5).unwrap();
    let mut two = planet();
    two.put_item(BodyId::PLANET, 1, b, 5).unwrap();
    assert_ne!(one.hash(), two.hash());
    let read = |d: &WorldDefinition| {
        let ctx = GenContext::new(d);
        ctx.query_cell(BodyId::PLANET, cell_of(R, a)).unwrap();
        ctx.read_set_hash()
    };
    assert_ne!(read(&one), read(&two), "a tile that read the cell must re-key when an item moves within it");
}

// spec: WDEF-005
#[test]
fn the_margin_by_grid_coordinates_equals_the_margin_by_walking_the_neighbours_for_every_cell_and_margin() {
    use planet_world_def::cells::with_margin_walk;
    let level = 8u8;
    let n = 1u32 << level;
    let mut cases = 0;
    // Every face, cells in the interior, on the edges one and two cells in, at the corners, and bundles of several cells.
    for face in 0..6u8 {
        let xs = [0, 1, 2, 3, 5, n / 2, n - 4, n - 3, n - 2, n - 1];
        for &x in &xs {
            for &y in &xs {
                let c = TileId::new(Face(face), level, x, y).unwrap();
                for margin in 0..=6 {
                    assert_eq!(with_margin(&[c], margin), with_margin_walk(&[c], margin), "face {face} cell ({x}, {y}) margin {margin}");
                    cases += 1;
                }
            }
        }
        let bundle: Vec<TileId> =
            (0..4).flat_map(|i| (0..4).map(move |j| TileId::new(Face(face), level, n / 2 + i, n / 3 + j).unwrap())).collect();
        let edge_bundle: Vec<TileId> = (0..3).map(|i| TileId::new(Face(face), level, n - 2 + (i % 2), 5 + i).unwrap()).collect();
        for margin in 0..=5 {
            assert_eq!(with_margin(&bundle, margin), with_margin_walk(&bundle, margin), "bundle, face {face} margin {margin}");
            assert_eq!(
                with_margin(&edge_bundle, margin),
                with_margin_walk(&edge_bundle, margin),
                "edge bundle, face {face} margin {margin}"
            );
        }
    }
    // Cells of two levels: the walk decides.
    let mixed = [TileId::new(Face(0), 8, 100, 100).unwrap(), TileId::new(Face(0), 7, 50, 20).unwrap()];
    assert_eq!(with_margin(&mixed, 2), with_margin_walk(&mixed, 2));
    assert!(cases > 3000);
}

// spec: WDEF-005
#[test]
fn the_margin_ring_reaches_across_face_edges_and_cube_corners() {
    let interior = cell_of(R, dir(1.0, 0.2, 0.1));
    let ring = with_margin(&[interior], 1);
    assert_eq!(ring.len(), 9, "an interior cell and its eight neighbours");
    assert!(ring.contains(&interior));
    assert!(ring.windows(2).all(|w| w[0].raw() < w[1].raw()));
    assert_eq!(with_margin(&[interior], 0), vec![interior]);
    assert_eq!(with_margin(&[interior], 2).len(), 25);
    // On a face edge the ring continues on the next face.
    let edge = cell_of(R, dir(1.0, 0.999999, 0.1));
    let faces: std::collections::BTreeSet<u8> = with_margin(&[edge], 1).iter().map(|c| c.face().0).collect();
    assert!(faces.len() >= 2, "{faces:?}");
    // At a cube corner there is no diagonal tile: eight cells, not nine.
    let corner = cell_of(R, dir(1.0, 1.0, 1.0));
    let ring = with_margin(&[corner], 1);
    assert!(ring.len() == 7 || ring.len() == 8, "{}", ring.len());
    let corner_faces: std::collections::BTreeSet<u8> = ring.iter().map(|c| c.face().0).collect();
    assert_eq!(corner_faces.len(), 3, "a cube corner ring touches three faces");
}

// spec: WDEF-005
#[test]
fn the_margin_by_integer_seam_folding_equals_the_walk_on_every_cell_of_the_small_levels() {
    use planet_world_def::cells::with_margin_walk;
    for level in 0..=3u8 {
        let n = 1u32 << level;
        for face in 0..6u8 {
            for x in 0..n {
                for y in 0..n {
                    let c = TileId::new(Face(face), level, x, y).unwrap();
                    for margin in 0..=3 {
                        assert_eq!(
                            with_margin(&[c], margin),
                            with_margin_walk(&[c], margin),
                            "level {level} face {face} ({x}, {y}) margin {margin}"
                        );
                    }
                }
            }
        }
    }
}

// spec: WDEF-005
#[test]
#[ignore = "wall clock: cargo xtask perf wallclock"]
fn the_margin_of_a_cell_on_a_face_edge_or_a_cube_corner_costs_a_small_multiple_of_one_inside_a_face() {
    // The editor's picking field asks for the margin of its point's cell on every height query (VIEW-022): the walk across a seam cost 50
    // to 250 times the interior square (78 against 0.3 microseconds at three rings). A wall-clock check: it runs with `cargo xtask perf
    // wallclock` on the reference machine, not in tier A (a timing ratio failed on a shared runner, #1017); the cells it returns stay tier A.
    let level = cell_level(R);
    let time = |c: TileId| {
        let _ = with_margin(&[c], 3);
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            for _ in 0..400 {
                std::hint::black_box(with_margin(&[std::hint::black_box(c)], 3));
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        best
    };
    let centre = time(cell_of(R, dir(1.0, 0.0, 0.0)));
    let edge = time(cell_of(R, dir(1.0, 1.0, 0.0)));
    let corner = time(cell_of(R, dir(-1.0, 1.0, 1.0)));
    assert!(
        edge < 30.0 * centre && corner < 30.0 * centre,
        "level {level}: centre {centre:e} s, edge {edge:e} s, corner {corner:e} s per 400 margins"
    );
}
