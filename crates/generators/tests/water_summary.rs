//! GEN-024: the coarse level of the water field (ADR 0058, ADR 0071): the summary of a bake, its known answers, its tile query and its
//! independence of the order and thread count of the blocks.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{bake, GlobalGrid, GlobalSample};
use planet_generators::ocean::{tiled_ocean, OceanGrid, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::water::{coast_band_elevation_m, WaterBodyId};
use planet_generators::water_summary::{summarize_block, summary_of_grid, SummaryCell, WaterSummary, FLAG_SEA, SUMMARY_BAKE_CELLS};
use planet_generators::wet_floor::{bound_code, code_of, reference_floor_grid, INF};
use planet_world_def::{LiquidKindId, SeaParams};

const R: f64 = 6_371_000.0;

fn sample(continent: f32, elevation_m: f32) -> GlobalSample {
    GlobalSample { continent, elevation_m, temperature_c: 10.0, precipitation_mm: 800.0 }
}

fn sea() -> SeaParams {
    SeaParams { level_m: 0.0, liquid: LiquidKindId::WATER, min_area_fraction: EARTH_OCEAN_MIN_AREA_FRACTION }
}

// spec: GEN-024
#[test]
fn the_cells_of_a_hand_made_block_have_the_documented_flags_floors_and_band_elevations() {
    // A block of 2 cells: one summary cell. Vertices row by row (k fastest). Continentalness 0.0 is sea, 0.5 is land.
    let land = sample(0.5, 300.0);
    let low_land = sample(0.5, 20.0);
    let sea_v = sample(0.0, -50.0);
    let nan = sample(f32::NAN, 5.0);
    let mut samples = vec![land; 9];
    let floors = vec![INF; 9];

    // All land and no floor reached: no sea corner, floor 0xFFFF; the band elevation is the lowest corner of any lattice cell with a
    // land corner.
    samples[4] = low_land;
    let c = summarize_block(2, &samples, &floors).unwrap();
    assert_eq!(c, vec![SummaryCell { flags: 0, band_elevation_m: 20.0, floor: INF }]);

    // The floor of the cell is the lowest over every corner of its lattice cells.
    let mut f = floors.clone();
    f[7] = 24_500;
    f[2] = 24_300;
    assert_eq!(summarize_block(2, &samples, &f).unwrap()[0].floor, 24_300);
    f[0] = 0;
    assert_eq!(summarize_block(2, &samples, &f).unwrap()[0].floor, 0);

    // A sea corner makes the cell able to hold sea; a continentalness that is not a number counts as not above the sea (the producer's
    // test is the negation of `c > L`).
    samples[1] = sea_v;
    assert_eq!(summarize_block(2, &samples, &floors).unwrap()[0].flags, FLAG_SEA);
    // The lowest elevation among cells with a land corner: the sea vertex at -50 m belongs to cells that also have land corners.
    assert_eq!(summarize_block(2, &samples, &floors).unwrap()[0].band_elevation_m, -50.0);
    samples = vec![land; 9];
    samples[8] = nan;
    assert_eq!(summarize_block(2, &samples, &floors).unwrap()[0].flags, FLAG_SEA);

    // A block of 128 cells has 2 x 2 summary cells of 64: the cell is chosen by position.
    let n = 129usize;
    let mut big = vec![land; n * n];
    big[100 * n + 5] = sea_v;
    let mut bf = vec![INF; n * n];
    bf[100 * n + 5] = 24_100;
    let c = summarize_block(128, &big, &bf).unwrap();
    assert_eq!(c.len(), 4);
    assert_eq!((c[2].flags, c[2].floor), (FLAG_SEA, 24_100), "column 5 and row 100 are in the cell at (0, 1)");
    assert!(c.iter().enumerate().all(|(i, cell)| i == 2 || (cell.flags == 0 && cell.floor == INF)));
    assert_eq!(SUMMARY_BAKE_CELLS, 64);

    // A block of the wrong size is refused.
    assert!(summarize_block(2, &samples, &floors[..8]).is_err());
}

/// A summary with one cell per face (res 4, one block): face 0 holds sea at floor 0, faces 1 and 2 are band with different floors,
/// face 3 is band-free, face 5 is band behind high ground.
fn faces_summary() -> WaterSummary {
    let bound = bound_code(coast_band_elevation_m(0.0));
    let mut cells = vec![SummaryCell::EMPTY; 6];
    cells[0] = SummaryCell { flags: FLAG_SEA, band_elevation_m: f32::INFINITY, floor: 0 };
    cells[1] = SummaryCell { flags: 0, band_elevation_m: 10.0, floor: INF };
    cells[2] = SummaryCell { flags: 0, band_elevation_m: 60.0, floor: bound };
    cells[3] = SummaryCell { flags: 0, band_elevation_m: 500.0, floor: 0 };
    cells[5] = SummaryCell { flags: 0, band_elevation_m: 10.0, floor: bound + 1 };
    WaterSummary::from_cells(4, 4, cells).unwrap()
}

// spec: GEN-024
#[test]
fn a_tile_is_wet_by_a_reached_cell_that_may_hold_sea_or_band_and_the_sea_level_moves_the_bound() {
    let s = faces_summary();
    assert_eq!((s.block_level(), s.edge_cells(), s.bake_cells_per_cell()), (0, 1, 4));
    let map = &TangentWarp;
    let ask = |face: u8, sea: &SeaParams| s.tile_water(map, TileId::new(Face(face), 0, 0, 0).unwrap(), R, sea);
    let wet = Some(Some(planet_generators::water::WaterSample { level_m: 0.0, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }));
    assert_eq!(ask(0, &sea()), wet, "a sea corner reached at floor 0");
    assert_eq!(ask(2, &sea()), wet, "band at 60 m, reached at the bound itself");
    assert_eq!(ask(1, &sea()), Some(None), "band, but the sea does not reach it: connection decides, not distance");
    assert_eq!(ask(5, &sea()), Some(None), "band behind ground one code above the bound");
    assert_eq!(ask(3, &sea()), Some(None), "reached, but above the band and no sea corner");
    assert_eq!(ask(4, &sea()), Some(None), "no cell, no band");
    // The bound rises with the level: at a level of 450 m the cell at 500 m is band and the floor 0 is at or below the bound.
    let high = SeaParams { level_m: 450.0, ..sea() };
    assert!(coast_band_elevation_m(450.0) > 500.0);
    assert_eq!(ask(3, &high).unwrap().map(|w| w.level_m), Some(450.0), "band at the higher level, reached");
    assert_eq!(ask(0, &high).unwrap().map(|w| w.level_m), Some(450.0), "the sea is at the definition's level");
    assert_eq!(ask(5, &high).unwrap().map(|w| w.level_m), Some(450.0), "the cell behind the low ridge is reached at the higher bound");
    // A tile below the block level is not answered: the caller asks point by point.
    assert_eq!(s.tile_water(map, TileId::new(Face(0), 1, 0, 0).unwrap(), R, &sea()), None);
}

fn reference(grid: &GlobalGrid, ocean: &OceanGrid, d: u32) -> Vec<SummaryCell> {
    // The summary read off the whole faces, one summary cell at a time, with no block in sight; the floors from the serial flood.
    let floors = reference_floor_grid(grid, ocean);
    let (res, g) = (grid.res, grid.res / d);
    let mut out = Vec::new();
    for f in 0..6u8 {
        for cy in 0..g {
            for cx in 0..g {
                let (mut flags, mut band, mut floor) = (0u8, f32::INFINITY, INF);
                for j in cy * d..(cy + 1) * d {
                    for i in cx * d..(cx + 1) * d {
                        assert!(i < res && j < res);
                        let corners = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)];
                        let c: Vec<GlobalSample> = corners.iter().map(|&(a, b)| grid.at(Face(f), a, b)).collect();
                        let sea_corner = c.iter().any(|s| s.continent.partial_cmp(&0.1) != Some(std::cmp::Ordering::Greater));
                        let land_corner = c.iter().any(|s| s.continent > 0.1);
                        if sea_corner {
                            flags |= FLAG_SEA;
                        }
                        for &(a, b) in &corners {
                            floor = floor.min(floors.at(Face(f), a, b));
                        }
                        if land_corner {
                            band = band.min(c.iter().map(|s| s.elevation_m).fold(f32::INFINITY, f32::min));
                        }
                    }
                }
                out.push(SummaryCell { flags, band_elevation_m: band, floor });
            }
        }
    }
    out
}

// spec: GEN-024
#[test]
fn the_summary_of_a_bake_equals_the_reference_read_off_whole_faces_and_does_not_depend_on_threads_or_block_order() {
    let grid = bake(2, &TangentWarp, 256, 4).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 64, 3, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    for block in [32u32, 64, 256] {
        let one = summary_of_grid(&grid, &ocean, block, 1).unwrap();
        let four = summary_of_grid(&grid, &ocean, block, 4).unwrap();
        assert_eq!(one.cells(), four.cells(), "block {block}");
        let d = block.min(64);
        let expected = reference(&grid, &ocean, d);
        assert_eq!(one.cells().len(), expected.len());
        for (i, (a, b)) in one.cells().iter().zip(&expected).enumerate() {
            assert_eq!(
                (a.flags, a.band_elevation_m.to_bits(), a.floor),
                (b.flags, b.band_elevation_m.to_bits(), b.floor),
                "block {block}, cell {i}"
            );
        }
        assert!(one.cells().iter().any(|c| c.floor == 0) && one.cells().iter().any(|c| c.floor == INF || c.flags == 0));
    }
    // The summary of a bake does not depend on how it was cut into blocks beyond the cell edge: 64-cell and 256-cell blocks both use
    // 64-cell summary cells.
    let a = summary_of_grid(&grid, &ocean, 64, 2).unwrap();
    let b = summary_of_grid(&grid, &ocean, 256, 2).unwrap();
    assert_eq!(a.cells(), b.cells());
    // Blocks set in any order give the same table.
    let floors = reference_floor_grid(&grid, &ocean);
    let mut rev = WaterSummary::new(256, 64).unwrap();
    let tiles = planet_generators::ocean::block_tiles(256, 64).unwrap();
    for t in tiles.iter().rev() {
        let samples = planet_generators::ocean::grid_block(&grid, *t, 64);
        let codes: Vec<u16> = (0..65 * 65).map(|v| floors.at(t.face(), t.x() * 64 + v % 65, t.y() * 64 + v / 65)).collect();
        rev.set_block(*t, &summarize_block(64, &samples, &codes).unwrap()).unwrap();
    }
    assert_eq!(rev.cells(), a.cells());
}

// spec: GEN-024
#[test]
fn a_tile_counts_the_cells_just_below_its_low_edge_because_its_border_samples_can_weigh_the_vertices_there() {
    // 256 cells in blocks of 64: 4 x 4 summary cells per face, block level 2. The tile (1, 1) of level 2 starts at cell 64.
    let tile = TileId::new(Face(0), 2, 1, 1).unwrap();
    for (cx, cy) in [(0usize, 1usize), (1, 0), (0, 0)] {
        let mut cells = vec![SummaryCell::EMPTY; 6 * 16];
        cells[cy * 4 + cx] = SummaryCell { flags: FLAG_SEA, band_elevation_m: f32::INFINITY, floor: 0 };
        let s = WaterSummary::from_cells(256, 64, cells).unwrap();
        assert!(s.tile_water(&TangentWarp, tile, R, &sea()).unwrap().is_some(), "wet cell ({cx}, {cy}) below the low edge");
    }
    // Two cells away it does not count.
    let mut cells = vec![SummaryCell::EMPTY; 6 * 16];
    cells[0] = SummaryCell { flags: FLAG_SEA, band_elevation_m: f32::INFINITY, floor: 0 };
    let s = WaterSummary::from_cells(256, 64, cells).unwrap();
    assert!(s.tile_water(&TangentWarp, TileId::new(Face(0), 2, 2, 2).unwrap(), R, &sea()).unwrap().is_none());
}

// spec: GEN-024
#[test]
fn a_parent_is_wet_wherever_a_child_is_at_every_level_of_a_real_bake() {
    let grid = bake(3, &TangentWarp, 256, 4).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 64, 3, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    let s = summary_of_grid(&grid, &ocean, 64, 2).unwrap();
    let mut wet_children = 0;
    for level in 1..=s.block_level() {
        let side = 1u32 << level;
        for face in 0..6u8 {
            for y in 0..side {
                for x in 0..side {
                    let child = TileId::new(Face(face), level, x, y).unwrap();
                    let wet = s.tile_water(&TangentWarp, child, R, &sea()).unwrap().is_some();
                    if wet {
                        wet_children += 1;
                        let parent = s.tile_water(&TangentWarp, child.parent().unwrap(), R, &sea()).unwrap();
                        assert!(parent.is_some(), "a parent is wet wherever its children are: {child:?}");
                    }
                }
            }
        }
    }
    assert!(wet_children > 0);
    // The code of the lowest wet floor is the code of the sea.
    assert_eq!(code_of(f32::NEG_INFINITY), 1);
}

// spec: GEN-024
#[test]
fn a_summary_that_implies_too_many_cells_is_refused_before_it_is_allocated() {
    assert!(WaterSummary::new(65536, 1).is_err());
    assert!(WaterSummary::new(16384, 1024).is_ok());
    assert_eq!(planet_generators::water_summary::summary_cell_count(16384, 1024), Some(6 * 256 * 256));
}
