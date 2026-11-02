//! ATMO-012, tier A: the water book is bounded (`waterbook-eviction`). Over a long synthetic flight (thousands of tiles asked for, each once)
//! the book holds at most its capacity; the least recently used entries go, the tiles in use stay; an evicted tile is computed again to the
//! same bits.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_frame::water::{WaterBook, WATER_BOOK_CAP};
use planet_generators::global::ReadError;
use planet_generators::height::{HeightField, Reference};
use planet_generators::water::{WaterBodyId, WaterSample};
use planet_world_def::LiquidKindId;

const R: f64 = 6_371_000.0;

/// A sea over the low ground of the reference field: a pure function of the direction.
struct Sea<'a>(&'a Reference);

impl planet_frame::water::WaterSource for Sea<'_> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok((self.0.height(dir) <= 40.0).then_some(WaterSample { level_m: 0.0, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }))
    }
}

/// The tiles of a flight: a strip of level-8 tiles along a face, one new row of the strip each step.
fn flight(steps: u32) -> Vec<TileId> {
    (0..steps).map(|i| TileId::new(Face::ALL[(i / 256 % 6) as usize], 8, i % 256, (i / 6) % 256).expect("a tile")).collect()
}

// spec: ATMO-012
#[test]
fn the_book_holds_its_capacity_over_a_long_flight_and_keeps_what_is_in_use() {
    let field = Reference(1);
    let src = Sea(&field);
    let book = WaterBook::with_capacity(32);
    let tiles = flight(600);
    let anchor = tiles[0];
    book.fill(&src, &TangentWarp, anchor, R).unwrap();
    let mut peak = 0;
    for (i, t) in tiles.iter().enumerate() {
        book.fill(&src, &TangentWarp, *t, R).unwrap();
        // The tile of the camera is asked for every frame: it is never the least recently used.
        assert!(book.get(anchor).is_some(), "the tile in use was evicted at step {i}");
        peak = peak.max(book.len());
        assert!(book.len() <= book.capacity(), "step {i}: {} tiles over the capacity {}", book.len(), book.capacity());
    }
    assert!(peak > 16, "the book filled up: {peak}");
    assert!(book.evicted() > 400, "most of the flight's tiles were evicted: {}", book.evicted());
    assert!(WaterBook::new().capacity() == WATER_BOOK_CAP);
}

// spec: ATMO-012
#[test]
fn an_evicted_tile_is_computed_again_to_the_same_bits_with_its_minimum() {
    let field = Reference(1);
    let src = Sea(&field);
    let book = WaterBook::with_capacity(16);
    let tiles = flight(80);
    // A wet tile with a minimum, then enough other tiles that it is evicted.
    let wet = tiles
        .iter()
        .copied()
        .find(|t| {
            let b = WaterBook::with_capacity(16);
            b.fill(&src, &TangentWarp, *t, R).unwrap();
            matches!(b.get(*t), Some(Some(_)))
        })
        .expect("a wet tile in the flight");
    book.fill(&src, &TangentWarp, wet, R).unwrap();
    let first = book.get(wet).unwrap();
    book.set_min_height(wet, -3.5);
    assert_eq!(book.min_height(wet), Some(-3.5));
    for t in tiles.iter().filter(|t| **t != wet) {
        book.fill(&src, &TangentWarp, *t, R).unwrap();
    }
    assert!(book.get(wet).is_none() && book.min_height(wet).is_none(), "the old entry is gone, with its minimum");
    // Asked again: computed again, the same answer; the minimum is the job's to leave once more.
    book.fill(&src, &TangentWarp, wet, R).unwrap();
    assert_eq!(book.get(wet), Some(first), "the same answer to the bit");
    book.set_min_height(wet, -3.5);
    assert_eq!(book.min_height(wet), Some(-3.5));
}
