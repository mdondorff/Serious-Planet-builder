//! Tier A: the coastal band of the water selection (ATMO-019, ADR 0056): a tile with no sea of its own carries the sea when a sample is
//! band (the bake says land, the ground is low enough for erosion to cut it below the level) and an ocean vertex lies within the reach;
//! an inland basin stays dry; the answer is monotone in the level and does not depend on the order the tiles are asked in.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::{WaterSource, WaterTileCache};
use planet_generators::global::ReadError;
use planet_generators::water::{coast_band_elevation_m, WaterBodyId, WaterSample, COAST_REACH_M};
use planet_world_def::LiquidKindId;

const R: f64 = 6_371_000.0;

/// A planet whose southern hemisphere (z <= 0) is ocean, and whose bake elevation of the land rises 20 km per unit of z.
struct Stub {
    level: f64,
    /// Whether the ocean channel exists at all (false: every basin is inland).
    ocean: bool,
}

impl Stub {
    fn sea(&self) -> WaterSample {
        WaterSample { level_m: self.level, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }
    }
}

impl WaterSource for Stub {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok((self.ocean && dir.z <= 0.0).then(|| self.sea()))
    }

    fn band_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        let elevation = dir.z * 20_000.0;
        Ok((dir.z > 0.0 && elevation <= coast_band_elevation_m(self.level)).then(|| self.sea()))
    }

    fn ocean_bit_at(&self, dir: Vec3) -> Result<bool, ReadError> {
        Ok(self.ocean && dir.z <= 0.0)
    }
}

/// Tiles of level 12 (2.4 km) along a meridian from the equator northwards, `km` kilometres from it.
fn tile_at(km: f64) -> TileId {
    let lat = km * 1000.0 / R;
    let d = Vec3::new(libm::cos(lat), 0.0, libm::sin(lat));
    TileId::from_direction(&TangentWarp, d, 12)
}

// spec: ATMO-019
#[test]
fn a_band_tile_within_the_reach_of_the_ocean_carries_the_sea_and_one_beyond_it_or_inland_does_not() {
    let s = Stub { level: 0.0, ocean: true };
    let band_m = coast_band_elevation_m(0.0);
    assert!((60.0..68.0).contains(&band_m), "the band is about 64 m of elevation at the level 0: {band_m}");
    let mut cache = WaterTileCache::new();
    // 5 km north of the coast: elevation 15.7 m (the band is about 64 m), within the band and the reach.
    let near = cache.tile_water(&s, &TangentWarp, tile_at(5.0), R).unwrap();
    assert_eq!(near, Some(s.sea()), "the band tile next to the ocean carries the sea at the sea's level");
    // 40 km north: above the band (elevation 125 m), dry.
    assert_eq!(cache.tile_water(&s, &TangentWarp, tile_at(40.0), R).unwrap(), None);
    // The same band with no ocean channel (an inland basin below the level, owner constraint 2): dry.
    let inland = Stub { level: 0.0, ocean: false };
    assert_eq!(WaterTileCache::new().tile_water(&inland, &TangentWarp, tile_at(5.0), R).unwrap(), None, "no ocean vertex: dry");
    assert_eq!(COAST_REACH_M, 100_000.0);
}

// spec: ATMO-019
#[test]
fn the_wet_tiles_only_grow_with_the_sea_level_and_the_answer_does_not_depend_on_the_order() {
    let tiles: Vec<TileId> = (0..400).map(|i| tile_at(f64::from(i) * 0.5)).collect();
    let wet = |level: f64, order: &[TileId]| -> std::collections::BTreeSet<u64> {
        let s = Stub { level, ocean: true };
        let mut cache = WaterTileCache::new();
        order.iter().filter(|t| cache.tile_water(&s, &TangentWarp, **t, R).unwrap().is_some()).map(|t| t.raw()).collect()
    };
    let mut last = std::collections::BTreeSet::new();
    for level in [-100.0, -58.0, -20.0, 0.0, 10.0, 40.0, 150.0] {
        let now = wet(level, &tiles);
        assert!(last.is_subset(&now), "level {level}: tiles dried when the level rose");
        last = now;
    }
    let reversed: Vec<TileId> = tiles.iter().rev().copied().collect();
    assert_eq!(wet(0.0, &tiles), wet(0.0, &reversed), "a tile's answer is a pure function of the tile and the field");
}

// spec: ATMO-019
#[test]
fn the_book_filled_from_many_threads_equals_a_sequential_cache_and_a_parent_is_wet_where_its_children_are() {
    use planet_frame::water::WaterBook;
    let s = Stub { level: 0.0, ocean: true };
    let tiles: Vec<TileId> = (0..300)
        .map(|i| tile_at(f64::from(i) * 0.4))
        .chain((0..40).map(|i| {
            TileId::from_direction(&TangentWarp, Vec3::new(libm::cos(f64::from(i) * 0.001), 0.0, libm::sin(f64::from(i) * 0.001)), 8)
        }))
        .collect();
    let book = WaterBook::new();
    std::thread::scope(|scope| {
        for t in 0..8usize {
            let (book, s, tiles) = (&book, &s, &tiles);
            scope.spawn(move || {
                // Every thread fills every tile, from a different start, so the same tile is filled by several jobs at once.
                for k in 0..tiles.len() {
                    book.fill(s, &TangentWarp, tiles[(k + 37 * t) % tiles.len()], R).unwrap();
                }
            });
        }
    });
    let mut cache = WaterTileCache::new();
    for t in &tiles {
        assert_eq!(book.get(*t), Some(cache.tile_water(&s, &TangentWarp, *t, R).unwrap()), "{t:?}");
    }
    let distinct: std::collections::BTreeSet<u64> = tiles.iter().map(|t| t.raw()).collect();
    assert_eq!(book.filled(), distinct.len() as u64, "a tile filled by several jobs counts once");
    // Parent and children: where a child carries the sea its parent does (the reach counts from the nearest point of the tile).
    let mut checked = 0;
    for km in (0..120).map(|i| f64::from(i) * 1.0) {
        let child = tile_at(km);
        let parent = child.parent().expect("a parent");
        if cache.tile_water(&s, &TangentWarp, child, R).unwrap().is_some() {
            assert!(cache.tile_water(&s, &TangentWarp, parent, R).unwrap().is_some(), "the parent of a wet tile at {km} km is wet");
            checked += 1;
        }
    }
    assert!(checked > 5, "{checked}");
}
