//! Tier A, REND-059: the river coverage of a tile. One width function for the burn and the drawing, the signed distance to the river edge on the cell centres in
//! 8-bit codes (+-4 cells), smooth banks, the one-cell minimum, the 32-cell cap and the same texels from any window.

use planet_frame::river::{distance_code, drawn_width_cells, river_coverage, river_distance, CODE_PER_CELL, CODE_ZERO, RANGE_CELLS};
use planet_generators::bake::k7::{burn_channels_wide, trench_width_cells, Channel, QUnit, WidthLaw, DISC_RADIUS_MAX};

const LAW: WidthLaw = WidthLaw::STANDARD;
const CELL_M: f64 = 100.0;

fn straight(from: (f64, f64), to: (f64, f64), q: f64) -> Channel {
    Channel { points: vec![from, to], levels: vec![10.0, 5.0], q: vec![q, q], q_unit: QUnit::Discharge }
}

/// The discharge whose width is exactly `cells` cells.
fn q_for(cells: f64) -> f64 {
    let w = cells * CELL_M;
    (w / LAW.a) * (w / LAW.a)
}

// spec: REND-059
#[test]
fn the_drawn_width_is_the_burned_width_with_the_one_cell_minimum_and_the_32_cell_cap() {
    assert_eq!(drawn_width_cells(&LAW, CELL_M, 0.0), 1.0, "no discharge: the one-cell line");
    assert_eq!(drawn_width_cells(&LAW, CELL_M, 1e-9), 1.0, "narrower than a cell: one cell");
    let q = q_for(6.0);
    assert!((drawn_width_cells(&LAW, CELL_M, q) - 6.0).abs() < 1e-9);
    assert_eq!(drawn_width_cells(&LAW, CELL_M, 1e12), 2.0 * DISC_RADIUS_MAX as f64, "capped at 32 cells");
    for q in [0.0, 0.5, 3.0, q_for(2.0), q_for(11.0), q_for(40.0), 1e9] {
        // The burn's width function is the one the drawing takes: max(trench, 1 cell).
        assert_eq!(drawn_width_cells(&LAW, CELL_M, q), trench_width_cells(&LAW, CELL_M, q).max(1.0), "q = {q}");
    }
}

// spec: REND-059
#[test]
fn the_signed_distance_of_a_straight_river_is_exact_and_coded_in_thirty_second_cell_steps() {
    // A horizontal river 10 cells wide on the row 20: the edge is 5 cells from the line.
    let ch = straight((0.0, 20.0), (40.0, 20.0), q_for(10.0));
    let d = river_distance(std::slice::from_ref(&ch), (0, 10), (41, 21), &LAW, CELL_M);
    let at = |x: usize, y: i64| d[(y - 10) as usize * 41 + x];
    for y in 11..=29 {
        let want = (y as f64 - 20.0).abs() - 5.0;
        assert!((at(20, y) - want).abs() < 1e-12, "row {y}: {} against {want}", at(20, y));
    }
    let code = river_coverage(&[ch], (0, 10), (41, 21), &LAW, CELL_M);
    let cat = |x: usize, y: i64| code[(y - 10) as usize * 41 + x];
    assert_eq!(cat(20, 25), CODE_ZERO, "the edge is the zero code");
    assert_eq!(cat(20, 26), CODE_ZERO + CODE_PER_CELL as u8, "one cell outside the edge");
    assert_eq!(cat(20, 24), CODE_ZERO - CODE_PER_CELL as u8, "one cell inside");
    assert_eq!(cat(20, 20), 0, "deep inside clamps at the range: -4 cells and beyond");
    assert_eq!(cat(20, 30), 255, "far outside clamps at +4 cells and beyond");
    assert_eq!(RANGE_CELLS, 4.0);
    // Water where the code is at or under the zero code.
    let wet: usize = (10..31).filter(|&y| cat(20, y) <= CODE_ZERO).count();
    assert_eq!(wet, 11, "the rows 15 to 25 (the edge rows included)");
}

// spec: REND-059
#[test]
fn the_banks_of_a_diagonal_river_are_smooth_there_is_no_cell_staircase() {
    // A 45-degree river 7 cells wide: the distance field is the Euclidean one, so along any row the code changes by a bounded, near constant step (a staircase would jump by 0 and then 2 or more cells).
    let ch = straight((0.0, 0.0), (60.0, 60.0), q_for(7.0));
    let d = river_distance(&[ch], (0, 0), (61, 61), &LAW, CELL_M);
    let row = 30usize;
    let mut steps = Vec::new();
    for x in 22..38usize {
        steps.push(d[row * 61 + x + 1] - d[row * 61 + x]);
    }
    for s in &steps {
        assert!((s.abs() - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9, "the distance changes by cos 45 deg a cell along a row, got {s}");
    }
    // The codes of the row are the distance coded at 32 a cell: consecutive texels differ by 22 or 23 codes (32 cos 45 = 22.6), never a staircase's 0 and 45.
    for x in 22..38usize {
        let (a, b) = (distance_code(d[row * 61 + x]) as i32, distance_code(d[row * 61 + x + 1]) as i32);
        assert!((a - b).abs() == 22 || (a - b).abs() == 23 || a == 0 || b == 0 || a == 255 || b == 255, "codes {a} and {b}");
    }
}

// spec: REND-059
#[test]
fn a_river_under_a_cell_is_one_cell_wide_and_a_confluence_takes_the_nearest_edge() {
    let thin = straight((0.0, 5.0), (20.0, 5.0), 1e-6);
    let d = river_distance(std::slice::from_ref(&thin), (0, 0), (21, 11), &LAW, CELL_M);
    assert!((d[5 * 21 + 10] + 0.5).abs() < 1e-12, "on the line: half a cell inside the one-cell river");
    assert!((d[6 * 21 + 10] - 0.5).abs() < 1e-12, "the next row is half a cell outside");
    // A confluence: a tributary meeting the trunk; the distance is the smaller of the two.
    let trunk = straight((0.0, 5.0), (20.0, 5.0), q_for(4.0));
    let trib = straight((10.0, 0.0), (10.0, 5.0), q_for(2.0));
    let both = river_distance(&[trunk.clone(), trib.clone()], (0, 0), (21, 11), &LAW, CELL_M);
    let (a, b) = (river_distance(&[trunk], (0, 0), (21, 11), &LAW, CELL_M), river_distance(&[trib], (0, 0), (21, 11), &LAW, CELL_M));
    for i in 0..both.len() {
        assert_eq!(both[i], a[i].min(b[i]), "texel {i}");
    }
    // No channel: nothing is wet and the code is the far-outside one.
    let none = river_coverage(&[], (0, 0), (4, 4), &LAW, CELL_M);
    assert!(none.iter().all(|&c| c == 255));
}

// spec: REND-059, CON-30
#[test]
fn two_windows_of_one_river_give_the_same_texels_where_they_overlap() {
    let chans = vec![straight((3.0, 7.5), (58.0, 41.25), q_for(5.0)), straight((30.0, 0.0), (30.0, 24.0), q_for(3.0))];
    let whole = river_coverage(&chans, (0, 0), (64, 48), &LAW, CELL_M);
    // Tiles of 24 by 16 cut from it: each tile is made from the channels alone, with no knowledge of the others.
    for (tx, ty) in [(0i64, 0i64), (24, 0), (40, 16), (13, 29)] {
        let tile = river_coverage(&chans, (tx, ty), (24, 16), &LAW, CELL_M);
        for y in 0..16usize {
            for x in 0..24usize {
                let (gx, gy) = (tx as usize + x, ty as usize + y);
                if gx < 64 && gy < 48 {
                    assert_eq!(tile[y * 24 + x], whole[gy * 64 + gx], "texel ({gx}, {gy})");
                }
            }
        }
    }
}

// spec: REND-059
#[test]
fn the_coverage_agrees_with_the_burn_on_which_cells_are_the_river() {
    // The burn lowers the cells within w / 2 of a straight line (and the line's own cells): the cells the coverage puts half a cell inside the edge are all burnt, and a burnt cell outside the coverage is within
    // the line's rasterisation (at most 0.75 cell beyond the edge).
    for (from, to, cells) in [
        ((4.0, 20.0), (44.0, 20.0), 9.0),
        ((4.0, 4.0), (40.0, 36.0), 6.0),
        ((6.0, 30.0), (40.0, 8.0), 3.0),
        ((4.0, 20.0), (44.0, 20.0), 1.0),
    ] {
        let ch = straight(from, to, q_for(cells));
        let w = 48usize;
        let mut z = vec![100.0; w * w];
        burn_channels_wide(&mut z, w, (0, 0), std::slice::from_ref(&ch), Some((&LAW, CELL_M)));
        let d = river_distance(&[ch], (0, 0), (w, w), &LAW, CELL_M);
        let (mut inside, mut burnt_n) = (0, 0);
        for i in 0..w * w {
            inside += usize::from(d[i] <= -0.5);
            burnt_n += usize::from(z[i] < 100.0);
            let burnt = z[i] < 100.0;
            if d[i] <= -0.5 && cells > 1.0 {
                assert!(burnt, "cell {i}: inside the edge by {} but not burnt (width {cells})", d[i]);
            }
            if burnt {
                assert!(d[i] <= 0.75, "cell {i}: burnt but {} cells outside the edge (width {cells})", d[i]);
            }
        }
        assert!(burnt_n > 30, "the burn lowered cells (width {cells})");
        assert!(cells < 2.0 || inside > 10, "the river has cells well inside its edge (width {cells})");
    }
}

// spec: REND-059
#[test]
fn a_river_that_widens_along_a_segment_is_drawn_as_the_burns_union_of_discs() {
    // A tributary that joins a trunk over one long segment: one cell wide at the start, 32 cells at the end (the burn cuts a disc of its own radius at every sample).
    for (from, to, q0, q1) in [
        ((4.0, 20.0), (14.0, 20.0), q_for(1.0), q_for(32.0)),
        ((4.0, 6.0), (30.0, 22.0), q_for(2.0), q_for(14.0)),
        ((30.0, 22.0), (4.0, 6.0), q_for(14.0), q_for(2.0)),
    ] {
        let ch = Channel { points: vec![from, to], levels: vec![10.0, 5.0], q: vec![q0, q1], q_unit: QUnit::Discharge };
        let w = 64usize;
        let mut z = vec![100.0; w * w];
        burn_channels_wide(&mut z, w, (0, 0), std::slice::from_ref(&ch), Some((&LAW, CELL_M)));
        let d = river_distance(std::slice::from_ref(&ch), (0, 0), (w, w), &LAW, CELL_M);
        let (mut burnt_n, mut wet) = (0, 0);
        for i in 0..w * w {
            let burnt = z[i] < 100.0;
            burnt_n += usize::from(burnt);
            wet += usize::from(d[i] <= 0.0);
            // Every burnt cell is drawn (within the line's own rasterisation) and every cell well inside the drawing is burnt.
            if burnt {
                assert!(d[i] <= 0.75, "cell {i}: burnt but {} cells outside the edge ({from:?} to {to:?})", d[i]);
            }
            if d[i] <= -0.5 {
                assert!(burnt, "cell {i}: inside the edge by {} but not burnt ({from:?} to {to:?})", d[i]);
            }
        }
        assert!(burnt_n > 100 && wet > 100, "the river is a real shape ({burnt_n} burnt, {wet} wet)");
    }
}

// spec: REND-059
#[test]
fn a_single_point_is_a_disc_and_a_nan_distance_codes_as_far_outside() {
    let one = Channel { points: vec![(10.0, 10.0)], levels: vec![1.0], q: vec![q_for(6.0)], q_unit: QUnit::Discharge };
    let d = river_distance(&[one], (0, 0), (21, 21), &LAW, CELL_M);
    assert!((d[10 * 21 + 10] + 3.0).abs() < 1e-12, "the centre is 3 cells inside a disc of 6 cells across");
    assert!((d[10 * 21 + 16] - 3.0).abs() < 1e-12, "6 cells to the side is 3 cells outside");
    assert_eq!(distance_code(f64::NAN), 255);
}

// spec: REND-059
#[test]
#[should_panic(expected = "discharge in m3/s")]
fn a_discharge_given_as_an_area_is_refused_as_the_burn_refuses_it() {
    let ch = Channel { points: vec![(0.0, 0.0), (5.0, 0.0)], levels: vec![1.0, 1.0], q: vec![1e6, 1e6], q_unit: QUnit::Area };
    river_distance(&[ch], (0, 0), (8, 8), &LAW, CELL_M);
}

// spec: REND-059
#[test]
#[should_panic(expected = "a discharge for every point")]
fn a_discharge_list_of_another_length_is_refused_with_a_message() {
    let ch = Channel { points: vec![(0.0, 0.0), (5.0, 0.0)], levels: vec![1.0, 1.0], q: vec![1.0], q_unit: QUnit::Discharge };
    river_distance(&[ch], (0, 0), (8, 8), &LAW, CELL_M);
}

// spec: REND-059, CON-30
#[test]
fn windows_at_production_cell_indices_give_the_same_texels_as_one_window() {
    // Face cell indices of a 3,072-cell-tile planet reach about 2^21: the same river is coded from two windows there.
    let o = 2_000_000i64;
    let chans = vec![Channel {
        points: vec![(o as f64 + 3.0, o as f64 + 7.5), (o as f64 + 58.0, o as f64 + 41.25)],
        levels: vec![2.0, 1.0],
        q: vec![q_for(2.0), q_for(20.0)],
        q_unit: QUnit::Discharge,
    }];
    let whole = river_coverage(&chans, (o, o), (64, 48), &LAW, CELL_M);
    for (tx, ty) in [(0i64, 0i64), (24, 0), (40, 16), (13, 29)] {
        let tile = river_coverage(&chans, (o + tx, o + ty), (24, 16), &LAW, CELL_M);
        for y in 0..16usize {
            for x in 0..24usize {
                let (gx, gy) = (tx as usize + x, ty as usize + y);
                if gx < 64 && gy < 48 {
                    assert_eq!(tile[y * 24 + x], whole[gy * 64 + gx], "texel ({gx}, {gy})");
                }
            }
        }
    }
}
