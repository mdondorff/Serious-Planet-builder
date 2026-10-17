//! GEN-024: the coarse level of the water field (ADR 0057): the summary of a bake, its known answers, its tile query and its
//! independence of the order and thread count of the blocks.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::global::{bake, GlobalGrid, GlobalSample};
use planet_generators::ocean::{tiled_ocean, OceanBits, OceanGrid, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::water::{coast_band_elevation_m, WaterBodyId};
use planet_generators::water_summary::{
    summarize_block, summary_of_grid, SummaryCell, WaterSummary, FLAG_OCEAN, FLAG_WET, SUMMARY_BAKE_CELLS,
};
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
fn the_cells_of_a_hand_made_block_have_the_documented_flags_and_band_elevations() {
    // A block of 2 cells: one summary cell. Vertices row by row (k fastest). Continentalness 0.0 is sea, 0.5 is land.
    let land = sample(0.5, 300.0);
    let low_land = sample(0.5, 20.0);
    let sea_v = sample(0.0, -50.0);
    let nan = sample(f32::NAN, 5.0);
    let mut samples = vec![land; 9];
    let ocean_at = |k: u32, l: u32| {
        let mut b = OceanBits::zeros(2);
        b.set(k, l);
        b
    };

    // All land and no ocean vertex: dry, no ocean; the band elevation is the lowest corner of any lattice cell with a land corner.
    samples[4] = low_land;
    let c = summarize_block(2, &samples, &OceanBits::zeros(2)).unwrap();
    assert_eq!(c, vec![SummaryCell { flags: 0, band_elevation_m: 20.0 }]);

    // An ocean vertex with every corner of its lattice cells above the sea: ocean, not wet.
    let c = summarize_block(2, &samples, &ocean_at(0, 0)).unwrap();
    assert_eq!(c[0].flags, FLAG_OCEAN);

    // The ocean vertex at (0, 0) and a sea corner in its lattice cell: wet. A sea corner in another lattice cell does not make it wet.
    samples[1] = sea_v;
    let c = summarize_block(2, &samples, &ocean_at(0, 0)).unwrap();
    assert_eq!(c[0].flags, FLAG_OCEAN | FLAG_WET);
    let c = summarize_block(2, &samples, &ocean_at(2, 2)).unwrap();
    assert_eq!(
        c[0].flags, FLAG_OCEAN,
        "the sea corner at (1, 0) is in the cells at (0, 0) and (1, 0), the ocean vertex in the cell at (1, 1)"
    );
    // The lowest elevation among cells with a land corner: the sea vertex at -50 m belongs to cells that also have land corners.
    assert_eq!(c[0].band_elevation_m, -50.0);

    // A continentalness that is not a number counts as not above the sea (the producer's test is the negation of `c > L`).
    samples = vec![land; 9];
    samples[8] = nan;
    let c = summarize_block(2, &samples, &ocean_at(2, 2)).unwrap();
    assert_eq!(c[0].flags, FLAG_OCEAN | FLAG_WET);

    // A block of 128 cells has 2 x 2 summary cells of 64: the cell is chosen by position.
    let n = 129usize;
    let mut big = vec![land; n * n];
    big[100 * n + 5] = sea_v;
    let mut bits = OceanBits::zeros(128);
    bits.set(5, 100);
    let c = summarize_block(128, &big, &bits).unwrap();
    assert_eq!(c.len(), 4);
    assert_eq!(c[2].flags, FLAG_OCEAN | FLAG_WET, "column 5 and row 100 are in the cell at (0, 1)");
    assert!(c.iter().enumerate().all(|(i, cell)| i == 2 || cell.flags == 0));
    assert_eq!(SUMMARY_BAKE_CELLS, 64);
}

/// A summary with one cell per face (res 4, one block): face 0 is wet, faces 1 and 2 are band, face 3 is neither.
fn faces_summary() -> WaterSummary {
    let mut cells = vec![SummaryCell::EMPTY; 6];
    cells[0] = SummaryCell { flags: FLAG_WET | FLAG_OCEAN, band_elevation_m: f32::INFINITY };
    cells[1] = SummaryCell { flags: 0, band_elevation_m: 10.0 };
    cells[2] = SummaryCell { flags: 0, band_elevation_m: 60.0 };
    cells[3] = SummaryCell { flags: 0, band_elevation_m: 500.0 };
    WaterSummary::from_cells(4, 4, cells).unwrap()
}

// spec: GEN-024
#[test]
fn a_tile_is_wet_by_its_cells_or_by_the_band_within_the_reach_and_the_sea_level_moves_the_band() {
    let s = faces_summary();
    assert_eq!((s.block_level(), s.edge_cells(), s.bake_cells_per_cell()), (0, 1, 4));
    let map = &TangentWarp;
    let ask = |face: u8, sea: &SeaParams| s.tile_water(map, TileId::new(Face(face), 0, 0, 0).unwrap(), R, sea);
    let wet = Some(Some(planet_generators::water::WaterSample { level_m: 0.0, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }));
    // Faces are +x, -x, +y, -y, +z, -z. Face 0 holds sea. Face 2 (+y) is band and next to face 0: wet by the reach. Face 1 (-x) is band
    // but opposite face 0: farther than the reach (about 2.8 rad of the planet counted at the coarsest level) away. Face 3 is above the band.
    assert_eq!(ask(0, &sea()), wet);
    assert_eq!(ask(2, &sea()), wet);
    assert_eq!(ask(1, &sea()), Some(None), "band, but no ocean vertex within the reach");
    assert_eq!(ask(3, &sea()), Some(None), "above the band: dry whatever the reach");
    assert_eq!(ask(4, &sea()), Some(None), "no cell, no band");
    // The band rises with the level: at a level of 450 m the cell at 500 m is band (the carve rises faster than the level).
    let high = SeaParams { level_m: 450.0, ..sea() };
    assert!(coast_band_elevation_m(450.0) > 500.0);
    assert_eq!(ask(3, &sea()), Some(None));
    assert_eq!(ask(3, &high).unwrap().map(|w| w.level_m), Some(450.0), "band at the higher level, next to face 0");
    assert_eq!(ask(0, &high).unwrap().map(|w| w.level_m), Some(450.0), "the sea is at the definition's level");
    // A tile below the block level is not answered: the caller asks point by point.
    assert_eq!(s.tile_water(map, TileId::new(Face(0), 1, 0, 0).unwrap(), R, &sea()), None);
}

fn reference(grid: &GlobalGrid, ocean: &OceanGrid, d: u32) -> Vec<SummaryCell> {
    // The summary read off the whole faces, one summary cell at a time, with no block in sight.
    let (res, g) = (grid.res, grid.res / d);
    let mut out = Vec::new();
    for f in 0..6u8 {
        for cy in 0..g {
            for cx in 0..g {
                let (mut flags, mut band) = (0u8, f32::INFINITY);
                for j in cy * d..(cy + 1) * d {
                    for i in cx * d..(cx + 1) * d {
                        assert!(i < res && j < res);
                        let corners = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)];
                        let c: Vec<GlobalSample> = corners.iter().map(|&(a, b)| grid.at(Face(f), a, b)).collect();
                        let oc = corners.iter().any(|&(a, b)| ocean.get(Face(f), a, b));
                        let sea_corner = c.iter().any(|s| s.continent.partial_cmp(&0.1) != Some(std::cmp::Ordering::Greater));
                        let land_corner = c.iter().any(|s| s.continent > 0.1);
                        if oc {
                            flags |= FLAG_OCEAN;
                            if sea_corner {
                                flags |= FLAG_WET;
                            }
                        }
                        if land_corner {
                            band = band.min(c.iter().map(|s| s.elevation_m).fold(f32::INFINITY, f32::min));
                        }
                    }
                }
                out.push(SummaryCell { flags, band_elevation_m: band });
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
            assert_eq!((a.flags, a.band_elevation_m.to_bits()), (b.flags, b.band_elevation_m.to_bits()), "block {block}, cell {i}");
        }
        assert!(one.cells().iter().any(|c| c.flags & FLAG_WET != 0) && one.cells().iter().any(|c| c.flags == 0));
    }
    // The summary of a bake does not depend on how it was cut into blocks beyond the cell edge: 64-cell and 256-cell blocks both use
    // 64-cell summary cells.
    let a = summary_of_grid(&grid, &ocean, 64, 2).unwrap();
    let b = summary_of_grid(&grid, &ocean, 256, 2).unwrap();
    assert_eq!(a.cells(), b.cells());
    // Blocks set in any order give the same table.
    let mut rev = WaterSummary::new(256, 64).unwrap();
    let tiles = planet_generators::ocean::block_tiles(256, 64).unwrap();
    for t in tiles.iter().rev() {
        let samples = planet_generators::ocean::grid_block(&grid, *t, 64);
        let mut bits = OceanBits::zeros(64);
        for l in 0..=64 {
            for k in 0..=64 {
                if ocean.get(t.face(), t.x() * 64 + k, t.y() * 64 + l) {
                    bits.set(k, l);
                }
            }
        }
        rev.set_block(*t, &summarize_block(64, &samples, &bits).unwrap()).unwrap();
    }
    assert_eq!(rev.cells(), a.cells());
}
