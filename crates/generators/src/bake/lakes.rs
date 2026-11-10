//! The lake producer (GEN-163, A3 sections 5.3 and 5.5, K11's input): from H1's kept basins, one lake record each (level, class, outflow) and the river segments with the channel cells inside
//! kept basins dropped.
//!
//! - H1 already solves the steady balance of section 5.5 per basin (`Basin::water_level_m`, `q_out`, `overflows`); the producer adds no second balance. It turns each kept basin into a [`Lake`]
//!   and derives the wet surface at the level from the area curve.
//! - A basin with no inflow (`q_in = 0`) has the floor as its level and no water surface (`wet = false`): a playa.
//! - A channel cell under the water of a lake is dropped: the lake draws as water and a channel passes through it (A3 section 5.2). The water of a wet lake is the cells of its basin with `z` under the
//!   level, and the shore cells under the level that drain into it (a receiver chain of cells under the level, joined to the basin's water): a gentle shore under the level is water although H1 did not
//!   label it, and a labelled cell above the level of an endorheic lake is not water, so the channel continues down to the water's edge (architect D10). A dry lake (a playa) has no water. A segment
//!   that goes under water is cut into pieces: the piece before ends at the last cell before the water, the piece after starts at the first cell out of it (at the spill cell for an overflowing lake),
//!   whose `Q_w` includes the basin's `q_out`.
//! - Only the segments that lose a cell have pieces (a piece is its first cell and its cell count; the cells are the receiver chain); the whole segments stay in the hydrology. The producer's memory is the lakes
//!   and the cut segments, not a second copy of the rivers.
//! - The result is a pure function of the hydrology: lakes are in the order of their id, pieces in the order of the segments (CON-14).

use super::hydro::{Basin, BasinClass, Hydro, CURVE_STEPS};
use super::route::{Routing, NONE};
use super::Grid;
use std::collections::{HashSet, VecDeque};

/// A lake: the record of a kept basin the store keeps and the water pass reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Lake {
    /// The basin's identity (`basin_id`).
    pub id: u64,
    /// The global ids of the lowest cell and of the spill cell.
    pub floor_id: u64,
    pub spill_id: u64,
    pub floor_m: f64,
    pub spill_m: f64,
    /// The water level of the balance, between the floor and the spill.
    pub level_m: f64,
    pub class: BasinClass,
    /// The spill point is the active outlet.
    pub overflows: bool,
    /// Whether the lake has a water surface: false for a basin nothing drains into (a playa, level at the floor).
    pub wet: bool,
    /// The surface area at the level, in square metres (zero when dry).
    pub area_m2: f64,
    pub cells: u32,
    pub q_in: f64,
    pub q_out: f64,
}

/// A run of channel cells of one segment outside every kept basin. Only segments that lose a cell to a basin have pieces; every other segment of the hydrology is whole and is not listed (the memory of a
/// build is the cut segments, not a copy of every river cell, GEN-163).
#[derive(Clone, Debug, PartialEq)]
pub struct RiverPiece {
    /// The segment this piece is cut from, and its number among the pieces of that segment (0 first).
    pub segment: u64,
    pub part: u32,
    /// The first cell and the number of cells: the cells are the receiver chain from `first`.
    pub first: u32,
    pub cells: u32,
    /// `Q_w` at the first and at the last cell.
    pub q_first: f64,
    pub q_last: f64,
}

/// The producer's result.
#[derive(Clone, Debug, PartialEq)]
pub struct Lakes {
    /// One lake per kept basin, by id.
    pub lakes: Vec<Lake>,
    /// The pieces of the segments that have a channel cell inside a kept basin, in the order of the segments.
    pub pieces: Vec<RiverPiece>,
    /// The channel cells of the segments that lie inside kept basins and are in no piece.
    pub dropped_cells: usize,
}

/// The surface area of a basin at `level`: the area curve's steps are linear in the level, with zero at the floor.
fn area_at(b: &Basin, level: f64) -> f64 {
    let span = b.spill_m - b.floor_m;
    if span <= 0.0 || level <= b.floor_m {
        return 0.0;
    }
    if level >= b.spill_m {
        return b.area_m2[CURVE_STEPS - 1];
    }
    let x = ((level - b.floor_m) / span * CURVE_STEPS as f64).min(CURVE_STEPS as f64);
    let k = (x.floor() as usize).min(CURVE_STEPS - 1);
    let (a0, a1) = (if k == 0 { 0.0 } else { b.area_m2[k - 1] }, b.area_m2[k]);
    a0 + (x - k as f64) * (a1 - a0)
}

/// The cells under the water of the wet lakes (GEN-163, D10): the cells of a basin with `z` under its level and the cells under the level that drain into them. `z` is the surface H1 ran on.
pub fn water_cells(grid: &Grid, routing: &Routing, hydro: &Hydro, z: &[f64]) -> HashSet<u32> {
    assert!(z.len() == grid.len(), "one height per cell");
    let level: Vec<f64> = hydro.basins.iter().map(|b| if b.q_in > 0.0 { b.water_level_m } else { f64::NEG_INFINITY }).collect();
    let mut water: HashSet<u32> = HashSet::new();
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
    for (c, (&l, &zc)) in hydro.label.iter().zip(z).enumerate() {
        if l != NONE && zc < level[l as usize] {
            water.insert(c as u32);
            queue.push_back((c as u32, l));
        }
    }
    // The donors of a water cell under the level are water too: a shore drains into the lake. The spill cell and everything below the sill drain away from it and are not reached.
    while let Some((c, l)) = queue.pop_front() {
        for d in 0..8 {
            let Some(q) = grid.neighbour(c as usize, d) else { continue };
            if routing.receiver[q] == c && z[q] < level[l as usize] && water.insert(q as u32) {
                queue.push_back((q as u32, l));
            }
        }
    }
    water
}

/// Cut the segments of a hydrology at the cells `water` says are under water: the pieces of the segments that lose a cell (first cell, count, `Q_w` at both ends) and the number of cells dropped.
pub fn cut_segments(hydro: &Hydro, routing: &Routing, water: &dyn Fn(usize) -> bool) -> (Vec<RiverPiece>, usize) {
    let mut pieces: Vec<RiverPiece> = Vec::new();
    let mut dropped_cells = 0usize;
    for s in &hydro.segments {
        // One walk: the runs of cells out of the water as (first cell, count, last cell); the cells of a segment are `up` to the cell before `down`, which starts the next segment (or is the mouth).
        // H1 counted them: the walk is bounded by that count.
        let mut runs: Vec<(u32, u32, u32)> = Vec::new();
        let mut dropped = 0usize;
        let mut open = false;
        let mut cur = s.up as usize;
        for _ in 0..s.cells {
            if water(cur) {
                dropped += 1;
                open = false;
            } else if open {
                let r = runs.last_mut().expect("an open run");
                r.1 += 1;
                r.2 = cur as u32;
            } else {
                runs.push((cur as u32, 1, cur as u32));
                open = true;
            }
            let next = routing.receiver[cur];
            assert!(next != NONE, "a segment reaches its end cell");
            cur = next as usize;
        }
        assert_eq!(cur, s.down as usize, "the routing is the one H1 ran on: a segment ends at its end cell");
        if dropped == 0 {
            continue;
        }
        dropped_cells += dropped;
        for (part, (first, cells, last)) in runs.into_iter().enumerate() {
            pieces.push(RiverPiece {
                segment: s.id,
                part: part as u32,
                first,
                cells,
                q_first: hydro.qw[first as usize],
                q_last: hydro.qw[last as usize],
            });
        }
    }
    (pieces, dropped_cells)
}

/// Produce the lakes and the pieces of the river segments from a hydrology (GEN-163).
pub fn lakes_from_hydro(grid: &Grid, routing: &Routing, hydro: &Hydro, z: &[f64]) -> Lakes {
    assert!(hydro.qw.len() == grid.len(), "the producer needs Q_w: run it before the discharges are released");
    assert!(routing.receiver.len() == grid.len(), "one receiver per cell");
    let mut lakes: Vec<Lake> = hydro
        .basins
        .iter()
        .map(|b| {
            let wet = b.q_in > 0.0;
            let level_m = if wet { b.water_level_m } else { b.floor_m };
            Lake {
                id: b.id,
                floor_id: grid.global_id(b.floor_cell as usize),
                spill_id: grid.global_id(b.spill_cell as usize),
                floor_m: b.floor_m,
                spill_m: b.spill_m,
                level_m,
                class: b.class,
                overflows: b.overflows,
                wet,
                area_m2: if wet { area_at(b, level_m) } else { 0.0 },
                cells: b.cells,
                q_in: b.q_in,
                q_out: b.q_out,
            }
        })
        .collect();
    lakes.sort_unstable_by_key(|l| l.id);

    let water = water_cells(grid, routing, hydro, z);
    let (pieces, dropped_cells) = cut_segments(hydro, routing, &|c| water.contains(&(c as u32)));
    Lakes { lakes, pieces, dropped_cells }
}
