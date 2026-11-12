//! Tier A, REND-058: the runtime's read of a commit's lakes. The L writer's fixture (one lake on face 2, its water block and ring in tile (3, 3)) is baked to a commit; `CommitLakes` answers every core cell
//! with the lake the tile's mask has there (its face, table index and level) and dry elsewhere, a tile the commit does not hold and a cell outside the face are dry, and the lake producer answers a direction
//! with the lake's body and level.

use std::sync::Arc;

use crate::l_writer::{bake, basin, expected, lake, lakes, spec, tiles, world, Dir, Feed, RES, T};
use planet_cache::l_writer::read_basin_table;
use planet_cache::lake_read::CommitLakes;
use planet_cache::pack::TileStore;
use planet_core::Face;
use planet_core::TangentWarp;
use planet_generators::bake::tile::TileGrid;
use planet_generators::bake::water_mask::NO_LAKE;
use planet_generators::lake_water::{lake_body, lake_of_body, LakeMaskSource, LakeProducer};
use planet_world_def::LiquidKindId;

fn lakes_of(d: &Dir) -> CommitLakes {
    let w = world();
    let c = bake(d, &w, spec(7), &Feed(true), &lakes(), 2).expect("the bake");
    let store = TileStore::open(d.path(), w.params()).unwrap();
    assert!(read_basin_table(&store, &w, RES, &basin(), &c, Face(2)).unwrap().is_some());
    CommitLakes::new(Arc::new(store), Arc::new(w), c, 4).expect("the lakes of the commit")
}

// spec: REND-058
#[test]
fn every_core_cell_reads_the_lake_its_tile_mask_has_and_the_rest_is_dry() {
    let d = Dir::new("lake-read");
    let l = lakes_of(&d);
    assert_eq!(l.table(Face(2)), &[lake()][..]);
    for id in tiles() {
        let want = expected(id.ti, id.tj);
        for (k, &w) in want.iter().enumerate() {
            let (i, j) = (i64::from(id.ti) * T as i64 + (k % T) as i64, i64::from(id.tj) * T as i64 + (k / T) as i64);
            let got = l.lake_at(Face(2), i, j).unwrap();
            if w == NO_LAKE {
                assert_eq!(got, None, "cell ({i}, {j}) is dry in the mask");
            } else {
                let c = got.unwrap_or_else(|| panic!("cell ({i}, {j}) is wet in the mask"));
                assert_eq!((c.face, c.index, c.level_m), (Face(2), 0, lake().level_m), "cell ({i}, {j})");
            }
        }
    }
    assert!(tiles().iter().any(|id| expected(id.ti, id.tj).iter().any(|&x| x != NO_LAKE)), "the fixture has wet cells");
    // A tile the commit does not hold, another face and a cell outside the face are dry.
    assert_eq!(l.lake_at(Face(2), 6 * T as i64, 6 * T as i64).unwrap(), None);
    assert_eq!(l.lake_at(Face(1), 80, 80).unwrap(), None);
    assert_eq!(l.lake_at(Face(2), -1, 80).unwrap(), None);
    assert_eq!(l.lake_at(Face(2), 192, 80).unwrap(), None);
}

// spec: REND-058
#[test]
fn the_lake_producer_maps_a_direction_to_its_cell_and_names_the_lake_as_its_body() {
    let d = Dir::new("lake-producer");
    let l = lakes_of(&d);
    let p = LakeProducer::new(&TangentWarp, &l, LiquidKindId::WATER);
    let g = TileGrid { cells: 192, tile: T, halo: 8, radius_m: 6_371_000.0 };
    // The centre of every cell maps back to the cell; a wet cell answers with the lake's body.
    let mut wet = 0;
    for (i, j) in [(80i64, 80i64), (85, 82), (72, 81), (10, 10), (100, 100)] {
        assert_eq!(p.cell_of(g.direction(Face(2), i, j)), (Face(2), i, j));
        if let Some(c) = l.lake_at(Face(2), i, j).unwrap() {
            wet += 1;
            assert_eq!(lake_of_body(lake_body(c.face, c.index)), Some((Face(2), 0)));
        }
    }
    assert!(wet > 0, "a probe cell is in the lake");
}
