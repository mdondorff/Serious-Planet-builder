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

use super::hydro::{Basin, BasinClass, Hydro, QUnit, CURVE_STEPS};
use super::route::{Routing, NONE};
use super::{Grid, Heights};
use std::collections::HashSet;

/// Version of the producer behaviour (the level rule, the water rule, the lake record): part of the key of a stored basin table, bumped with any change of what a lake is.
pub const LAKES_VERSION: u32 = 1;

/// A tile of a tile grid: the face and the tile's column and row on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TileRef {
    pub face: u8,
    pub ti: u32,
    pub tj: u32,
}

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
    /// The tiles that hold a cell of the lake's water, sorted (empty unless the producer was given a tile map: [`lakes_with_tiles`]).
    pub tiles: Vec<TileRef>,
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
    /// The water of the lakes as H1 cells with the lake's index in `lakes` (sorted by id), sorted by cell: 8 bytes an entry (GEN-167, D37; the mask's water blocks). Empty when the producer ran without them (the records alone).
    pub water: Vec<(u32, u32)>,
    /// The ring of each lake: the H1 cells next to its water that drain into it and belong to no other basin, sorted by cell (GEN-167; the mask's ring, D20).
    pub ring: Vec<(u32, u32)>,
}

/// What the producer reads the discharges and the routing for (D22, GEN-167): the river pieces and the lists of the water and the ring. Made before the release; the records follow after it.
#[derive(Clone, Debug, PartialEq)]
pub struct PreRelease {
    pub pieces: Vec<RiverPiece>,
    pub dropped_cells: usize,
    pub water: Vec<(u32, u32)>,
    pub ring: Vec<(u32, u32)>,
    /// Where the segments go under water (D54): `(segment index, first vertex, end vertex)`, vertices `first..end` of the segment's chain are under a lake's water (or its dead ground); the vertex before `first` is the
    /// lake entry and the one at `end` is the spill cell, where the river continues. Sorted by segment and vertex.
    pub cuts: Vec<(u32, u32, u32)>,
}

/// The runs of vertices of each segment that are under water, as [`PreRelease::cuts`]: the walk of [`cut_segments`] without the discharges (the last vertex of a segment, its `down` cell, is never cut: it starts the next).
pub fn lake_cut_ranges(hydro: &Hydro, routing: &Routing, water: &dyn Fn(usize) -> bool) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    for (index, s) in hydro.segments.iter().enumerate() {
        let mut cur = s.up as usize;
        let mut open: Option<u32> = None;
        for k in 0..s.cells {
            if water(cur) {
                open.get_or_insert(k);
            } else if let Some(first) = open.take() {
                out.push((index as u32, first, k));
            }
            let next = routing.receiver[cur];
            assert!(next != NONE, "a segment reaches its end cell");
            cur = next as usize;
        }
        if let Some(first) = open {
            out.push((index as u32, first, s.cells));
        }
    }
    out
}

/// The longest receiver chain followed from a cell next to a lake's water to see whether it drains into the lake (a cell that drains into it meets its water within a few steps; the bound keeps the cost
/// of the cells that drain elsewhere constant).
const RING_WALK: usize = 16;

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

/// The cells that carry no river because a lake is there (GEN-163, D10): the water, and the dead ground below it inside the basin.
///
/// - Water: a cell of a wet lake's basin with `z` under the lake's level (a test on the labels, no storage), and the shore cells under the level that drain into it along receiver chains (stored, sorted).
/// - Below the water: the routing runs on the filled surface, whose epsilon gradient inside a basin points to the spill, so a channel that reaches an endorheic lake's water would go on over dry
///   basin cells up to the spill; those labelled cells downstream of the water are not a river (the lake has no outflow there; at an overflowing lake they are water already). Stored, sorted.
///
/// The memory is the shore and the dead ground, not the water of the basins.
pub struct Water<'a, Z: Heights + ?Sized = [f64]> {
    label: &'a [u32],
    z: &'a Z,
    level: Vec<f64>,
    /// Sorted by cell: the shore cells with the index of the lake they drain into, and the dead ground with [`DEAD`].
    extra: Vec<(u32, u32)>,
}

/// The tag of a cell of the dead ground in `Water::extra`.
const DEAD: u32 = u32::MAX;

impl<Z: Heights + ?Sized> Water<'_, Z> {
    fn seed(&self, c: usize) -> bool {
        let l = self.label[c];
        l != NONE && self.z.at(c) < self.level[l as usize]
    }

    /// Whether no river is drawn on cell `c`.
    pub fn contains(&self, c: usize) -> bool {
        self.seed(c) || self.extra.binary_search_by_key(&(c as u32), |e| e.0).is_ok()
    }

    /// The tiles of every lake's water, by the index of the basin in the hydrology: `tile_of` maps a cell to its tile. Sorted and without repeats.
    pub fn tiles(&self, tile_of: &dyn Fn(usize) -> TileRef) -> Vec<Vec<TileRef>> {
        let mut out: Vec<Vec<TileRef>> = vec![Vec::new(); self.level.len()];
        for (c, &l) in self.label.iter().enumerate() {
            if self.seed(c) {
                out[l as usize].push(tile_of(c));
            }
        }
        for &(c, l) in self.extra.iter().filter(|e| e.1 != DEAD) {
            out[l as usize].push(tile_of(c as usize));
        }
        for v in &mut out {
            v.sort_unstable();
            v.dedup();
        }
        out
    }

    /// The water cells with the index of the lake they belong to, sorted by cell: `rank` maps the index of a basin in the hydrology to its place in the lakes by id. The dead ground is not water.
    pub fn lake_cells(&self, rank: &[u32]) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = Vec::new();
        for (c, &l) in self.label.iter().enumerate() {
            if self.seed(c) {
                v.push((c as u32, rank[l as usize]));
            }
        }
        v.extend(self.extra.iter().filter(|e| e.1 != DEAD).map(|&(c, l)| (c, rank[l as usize])));
        v.sort_unstable();
        v
    }

    /// The cells, in cell order.
    pub fn cells(&self) -> Vec<u32> {
        (0..self.z.len() as u32).filter(|&c| self.contains(c as usize)).collect()
    }
}

/// The water of the wet lakes and the ground below it, on the surface `z` H1 ran on.
pub fn water_cells<'a, Z: Heights + ?Sized>(grid: &Grid, routing: &Routing, hydro: &'a Hydro, z: &'a Z) -> Water<'a, Z> {
    assert!(z.len() == grid.len(), "one height per cell");
    let level: Vec<f64> = hydro.basins.iter().map(|b| if b.q_in > 0.0 { b.water_level_m } else { f64::NEG_INFINITY }).collect();
    let mut w = Water { label: &hydro.label, z, level, extra: Vec::new() };
    let mut shore: Vec<(u32, u32)> = Vec::new();
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for (c, &l) in hydro.label.iter().enumerate() {
        if w.seed(c) {
            stack.push((c as u32, l));
        }
        // The donors of a water cell under the level are water too: a shore drains into the lake. Every cell has one receiver, so a donor is reached once. A labelled donor is another basin's cell or
        // a seed of this one; the spill cell and everything below the sill drain away from the lake and are not reached.
        while let Some((x, l)) = stack.pop() {
            for d in 0..8 {
                let Some(q) = grid.neighbour(x as usize, d) else { continue };
                if routing.receiver[q] == x && hydro.label[q] == NONE && z.at(q) < w.level[l as usize] {
                    shore.push((q as u32, l));
                    stack.push((q as u32, l));
                }
            }
        }
    }
    shore.sort_unstable();
    w.extra = shore;
    // Below the water: from every water cell whose receiver is not water, the labelled cells down the receiver chain to the basin's edge.
    let mut dead: HashSet<u32> = HashSet::new();
    let mut walk = |w: &Water<Z>, c: usize| {
        let mut cur = routing.receiver[c];
        while cur != NONE && hydro.label[cur as usize] != NONE && !w.contains(cur as usize) && dead.insert(cur) {
            cur = routing.receiver[cur as usize];
        }
    };
    for c in 0..z.len() {
        if w.seed(c) {
            walk(&w, c);
        }
    }
    for k in 0..w.extra.len() {
        walk(&w, w.extra[k].0 as usize);
    }
    w.extra.extend(dead.into_iter().map(|c| (c, DEAD)));
    w.extra.sort_unstable();
    w
}

/// Cut the segments of a hydrology at the cells `water` says are under water: the pieces of the segments that lose a cell (first cell, count, `Q_w` at both ends) and the number of cells dropped.
pub fn cut_segments(hydro: &Hydro, routing: &Routing, water: &dyn Fn(usize) -> bool) -> (Vec<RiverPiece>, usize) {
    assert_discharge(hydro);
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
pub fn lakes_from_hydro(grid: &Grid, routing: &Routing, hydro: &Hydro, z: &(impl Heights + ?Sized)) -> Lakes {
    produce(grid, routing, hydro, z, None)
}

/// [`lakes_from_hydro`] with the tiles of each lake's water (`Lake::tiles`) from the tile map `tile_of`: the basin table's tile list (STRM-080).
pub fn lakes_with_tiles(
    grid: &Grid,
    routing: &Routing,
    hydro: &Hydro,
    z: &(impl Heights + ?Sized),
    tile_of: &dyn Fn(usize) -> TileRef,
) -> Lakes {
    produce(grid, routing, hydro, z, Some(tile_of))
}

/// The part of the producer that reads the discharges (`Q_w` at the ends of the pieces): the river pieces and the number of cells dropped. Run it before the discharges are released (D22); it holds nothing back, so the rest ([`lake_records`]) can run after the release.
pub fn river_pieces(grid: &Grid, routing: &Routing, hydro: &Hydro, z: &(impl Heights + ?Sized)) -> PreRelease {
    assert_discharge(hydro);
    assert!(hydro.qw.len() == grid.len(), "the producer needs Q_w: run it before the discharges are released");
    assert!(routing.receiver.len() == grid.len(), "one receiver per cell");
    let water = water_cells(grid, routing, hydro, z);
    let (pieces, dropped_cells) = cut_segments(hydro, routing, &|c| water.contains(c));
    let rank = basin_ranks(hydro);
    let cells = water.lake_cells(&rank);
    let ring = ring_cells(grid, routing, hydro, &rank, &cells);
    let cuts = lake_cut_ranges(hydro, routing, &|c| water.contains(c));
    PreRelease { pieces, dropped_cells, water: cells, ring, cuts }
}

/// The place of each basin of the hydrology among the basins by id (the index of its lake in [`Lakes::lakes`]).
fn basin_ranks(hydro: &Hydro) -> Vec<u32> {
    let mut order: Vec<usize> = (0..hydro.basins.len()).collect();
    order.sort_unstable_by_key(|&k| hydro.basins[k].id);
    let mut rank = vec![0u32; order.len()];
    for (r, &k) in order.iter().enumerate() {
        rank[k] = r as u32;
    }
    rank
}

/// The ring of every lake (D20): the H1 cells next to a water cell of the lake that are not water, belong to no other basin and drain into the lake along their receiver chain.
fn ring_cells(grid: &Grid, routing: &Routing, hydro: &Hydro, rank: &[u32], water: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let lake_of = |c: usize| water.binary_search_by_key(&(c as u32), |e| e.0).ok().map(|k| water[k].1);
    let other = |c: usize, id: u32| hydro.label[c] != NONE && rank[hydro.label[c] as usize] != id;
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &(c, id) in water {
        for d in 0..8 {
            let Some(q) = grid.neighbour(c as usize, d) else { continue };
            if lake_of(q).is_some() || other(q, id) {
                continue;
            }
            let mut cur = q;
            let mut drains = false;
            for _ in 0..RING_WALK {
                let r = routing.receiver[cur];
                if r == NONE {
                    break;
                }
                cur = r as usize;
                if let Some(l) = lake_of(cur) {
                    drains = l == id;
                    break;
                }
                if other(cur, id) {
                    break;
                }
            }
            if drains {
                out.push((q as u32, id));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// What the producer stores (`q_first`, `q_last`, `q_out`) is a discharge in m3/s: it refuses a hydrology run without the runoff, whose `Q` is an area in m2 (the units bug of 2026-11-10, D35).
fn assert_discharge(hydro: &Hydro) {
    assert!(hydro.q_unit == QUnit::Discharge, "the producer stores discharges (q_first, q_last, q_out) in m3/s: H1 returned an area in m2 (no runoff); pass the runoff to hydrology");
}

/// The lake records of the kept basins, by id (`q_out` is the basin's own, kept by H1; no discharge field is read).
pub fn lake_records(grid: &Grid, hydro: &Hydro) -> Vec<Lake> {
    assert_discharge(hydro);
    let mut lakes = basin_lakes(grid, hydro);
    lakes.sort_unstable_by_key(|l| l.id);
    lakes
}

fn basin_lakes(grid: &Grid, hydro: &Hydro) -> Vec<Lake> {
    hydro
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
                tiles: Vec::new(),
            }
        })
        .collect()
}
fn produce(
    grid: &Grid,
    routing: &Routing,
    hydro: &Hydro,
    z: &(impl Heights + ?Sized),
    tile_of: Option<&dyn Fn(usize) -> TileRef>,
) -> Lakes {
    assert_discharge(hydro);
    assert!(hydro.qw.len() == grid.len(), "the producer needs Q_w: run it before the discharges are released");
    assert!(routing.receiver.len() == grid.len(), "one receiver per cell");
    let mut lakes = basin_lakes(grid, hydro);
    let water = water_cells(grid, routing, hydro, z);
    if let Some(tile_of) = tile_of {
        let tiles = water.tiles(tile_of);
        assert_eq!(lakes.len(), tiles.len(), "one tile list per basin, before the lakes are sorted");
        for (lake, tiles) in lakes.iter_mut().zip(tiles) {
            lake.tiles = tiles;
        }
    }
    lakes.sort_unstable_by_key(|l| l.id);
    let (pieces, dropped_cells) = cut_segments(hydro, routing, &|c| water.contains(c));
    let rank = basin_ranks(hydro);
    let cells = water.lake_cells(&rank);
    let ring = ring_cells(grid, routing, hydro, &rank, &cells);
    Lakes { lakes, pieces, dropped_cells, water: cells, ring }
}
