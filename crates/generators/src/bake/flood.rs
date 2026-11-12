//! Barnes' improved priority flood with an epsilon (Barnes, Lehman and Mulla 2014; SPIKE-62): the surface with every depression filled to its pour point and a
//! gradient of `epsilon` per cell over the flats, so that every cell drains. The improved variant keeps the cells that fill to the level of their parent in a plain queue and
//! only the others in the heap.

#![allow(clippy::needless_range_loop)]

use super::{assert_finite_heights, Grid, Heights, NEIGHBOURS};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// The order key of a height: monotone in the value under the total order, so a heap can compare it as an integer.
fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b & (1 << 63) != 0 {
        !b
    } else {
        b | (1 << 63)
    }
}

/// The filled surface of `z`: `outlet` cells keep their height, every other cell is raised until it has a neighbour lower than itself on a path to an outlet (by `epsilon` on the flats).
/// Ties of the heap break by the cell index, so the result does not depend on anything but the input. Refused: a height that is not finite, no outlet at all, and an `epsilon`
/// that is not positive.
pub fn priority_flood(grid: &Grid, z: &(impl Heights + ?Sized), outlet: &[bool], epsilon: f64) -> Vec<f64> {
    priority_flood_with(grid, z, outlet, epsilon, false)
}

/// [`priority_flood`] with the canonical tie rule when `canonical` (GEN-141, A3 section 6.4): there is no FIFO queue for the flats, every cell enters the heap at its filled level and cells are
/// visited by (level, cell id), so a cell's level comes from its lowest visited neighbour by a total order that no other cell's presence changes. The id order of the cells of a rectangular grid
/// is the global id order (row-major), so it is the same in every window.
pub fn priority_flood_with(grid: &Grid, z: &(impl Heights + ?Sized), outlet: &[bool], epsilon: f64, canonical: bool) -> Vec<f64> {
    let mut filled = priority_flood_ungraded(grid, z, outlet, epsilon, canonical);
    if canonical {
        let _ = regrade_flats(grid, z, outlet, epsilon, FLAT_BOUND, &mut filled);
    }
    filled
}

/// [`priority_flood_with`] before the flats are graded (GEN-173): the chessboard levels the grade starts from.
pub fn priority_flood_ungraded(grid: &Grid, z: &(impl Heights + ?Sized), outlet: &[bool], epsilon: f64, canonical: bool) -> Vec<f64> {
    let n = grid.len();
    assert!(z.len() == n && outlet.len() == n, "one height and one outlet flag per cell");
    assert!(epsilon > 0.0 && epsilon.is_finite(), "the epsilon is finite and positive (a flat could not drain), not {epsilon}");
    assert_finite_heights("the heights", z);
    assert!(outlet.iter().any(|&o| o), "a flood needs an outlet: the mask has none");
    let mut filled = vec![0.0f64; n];
    let mut seen = vec![false; n];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    let mut flat: VecDeque<u32> = VecDeque::new();
    for p in 0..n {
        if outlet[p] {
            filled[p] = z.at(p);
            seen[p] = true;
        }
    }
    // Only an outlet with a neighbour that is not one expands anything: the others are seen and finished (the same filled surface, a far smaller heap on a sea face).
    for p in 0..n {
        if outlet[p] && grid.is_active(p) && (0..8).filter_map(|d| grid.neighbour(p, d)).any(|q| !outlet[q]) {
            heap.push(Reverse((key(z.at(p)), p as u32)));
        }
    }
    loop {
        let p = if let Some(p) = flat.pop_front() {
            p as usize
        } else if let Some(Reverse((_, p))) = heap.pop() {
            p as usize
        } else {
            break;
        };
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            if seen[q] {
                continue;
            }
            seen[q] = true;
            let lowest = filled[p] + epsilon;
            assert!(lowest > filled[p], "the epsilon {epsilon} is below the spacing of the heights near {}", filled[p]);
            if !canonical && z.at(q) <= lowest {
                filled[q] = lowest;
                flat.push_back(q as u32);
            } else {
                filled[q] = z.at(q).max(lowest);
                heap.push(Reverse((key(filled[q]), q as u32)));
            }
        }
    }
    filled
}

/// The largest Chebyshev diameter in cells of a flat that is graded by Euclidean distance (GEN-173): the production profile's halo (128 cells) minus 1. A flat that size lies, with its lower
/// rim, inside the core plus halo of every tile of its cells, so every tiling sees the same flat.
pub const FLAT_BOUND: usize = 127;

/// The step added to a flat cell's level over its parent's, as a share of `epsilon`: a cell has more than the neighbour it drains to.
const FLAT_STEP: f64 = 1e-6;

/// The flats of a canonical filled surface graded by Euclidean distance (GEN-173, SPIKE-108). A cell is lifted when `filled > z`; its parent is the neighbour whose level plus `epsilon` is its level
/// (the lowest by (level, id)); a flat is a connected set of lifted cells with one pour level (the level of the non-lifted cell their parent chains end at). A flat whose Chebyshev diameter is at most
/// `bound`, and none of whose cells lies on the border of the grid (it does not touch the window's outer ring: it may be larger than the window shows), is regraded to `base + epsilon d / sqrt 2` where `d`
/// is the Euclidean distance in cells to the nearest lower rim cell (propagated offsets, Dijkstra by (d squared, cell id)); each cell stays strictly above its parent. A larger flat keeps the
/// chessboard grade. The result depends only on the cells of the flat and its rim, never on a tiling. Cube grids are left as they are.
pub fn regrade_flats(
    grid: &Grid,
    z: &(impl Heights + ?Sized),
    outlet: &[bool],
    epsilon: f64,
    bound: usize,
    filled: &mut [f64],
) -> Vec<FlatInfo> {
    let flats = lifted_flats(grid, z, outlet, epsilon, filled);
    let mut infos = Vec::with_capacity(flats.len());
    // The levels are written after every flat is read, so the rim of a flat is always the chessboard surface: no neighbouring flat decides a bit.
    let mut writes: Vec<(u32, f64)> = Vec::new();
    for f in &flats {
        let small = !f.touches_border && f.diameter <= bound;
        infos.push(FlatInfo {
            cells: f.cells.len(),
            diameter: f.diameter,
            touches_border: f.touches_border,
            graded: small,
            max_lift_m: f.max_lift_m,
        });
        if small {
            regrade_one(grid, z, epsilon, &f.cells, f.base, filled, &mut writes);
        }
    }
    for (q, v) in writes {
        filled[q as usize] = v;
    }
    infos
}

/// A flat of a canonical filled surface: the lifted cells (the fill raised them) of one pour level that are connected, in the order a breadth-first walk from the cell of the lowest level
/// (then the lowest id) finds them; `base` is the pour level, `diameter` the Chebyshev diameter in cells (`max(x1 - x0, y1 - y0)`), `touches_border` whether a cell lies on or beside the border of the grid,
/// `max_lift_m` the deepest lift in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct Flat {
    pub cells: Vec<u32>,
    pub base: f64,
    pub diameter: usize,
    pub touches_border: bool,
    pub max_lift_m: f64,
}

/// The flats of the canonical filled surface `filled` of `z` (planar grids; none for a cube grid), in the order of their lowest cell by (level, id). A cell is lifted when `filled > z`; its parent
/// is the neighbour whose level plus `epsilon` is its level (the lowest by (level, id)); the base of a cell is the level of the non-lifted cell its parent chain ends at; a flat is a connected set
/// of lifted cells of one base. Used by the grade of the flats (GEN-173) and the breach of the basins (GEN-174).
pub fn lifted_flats(grid: &Grid, z: &(impl Heights + ?Sized), outlet: &[bool], epsilon: f64, filled: &[f64]) -> Vec<Flat> {
    let mut out = Vec::new();
    if grid.is_cube() {
        return out;
    }
    let n = grid.len();
    let lifted = |q: usize| !outlet[q] && grid.is_active(q) && filled[q] > z.at(q);
    let mut ids: Vec<u32> = (0..n).filter(|&q| lifted(q)).map(|q| q as u32).collect();
    if ids.is_empty() {
        return out;
    }
    ids.sort_unstable_by(|&a, &b| filled[a as usize].total_cmp(&filled[b as usize]).then(a.cmp(&b)));
    let mut slot = vec![u32::MAX; n];
    for (k, &q) in ids.iter().enumerate() {
        slot[q as usize] = k as u32;
    }
    let mut base = vec![f64::NAN; ids.len()];
    for (k, &q) in ids.iter().enumerate() {
        let q = q as usize;
        let mut parent: Option<usize> = None;
        for d in 0..8 {
            let Some(s) = grid.neighbour(q, d) else { continue };
            if filled[s] + epsilon == filled[q] && parent.is_none_or(|b| (filled[s], s) < (filled[b], b)) {
                parent = Some(s);
            }
        }
        base[k] = match parent {
            Some(s) if slot[s] != u32::MAX => base[slot[s] as usize],
            Some(s) => filled[s],
            None => f64::NAN,
        };
    }
    let mut seen = vec![false; ids.len()];
    for k0 in 0..ids.len() {
        if seen[k0] || base[k0].is_nan() {
            continue;
        }
        let mut members = vec![k0];
        seen[k0] = true;
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
        let mut touches = false;
        let mut i = 0;
        while i < members.len() {
            let p = ids[members[i]] as usize;
            i += 1;
            let (x, y) = grid.xy(p);
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
            // On the grid border or beside it: the border cells of a window are outlets, so a flat cut by the window ends one cell inside.
            touches |= x <= 1 || y <= 1 || x + 2 >= grid.w() || y + 2 >= grid.h();
            for d in 0..8 {
                if let Some(q) = grid.neighbour(p, d) {
                    let kq = slot[q];
                    if kq != u32::MAX && !seen[kq as usize] && base[kq as usize] == base[k0] {
                        seen[kq as usize] = true;
                        members.push(kq as usize);
                    }
                }
            }
        }
        let max_lift_m = members.iter().map(|&k| filled[ids[k] as usize] - z.at(ids[k] as usize)).fold(0.0f64, f64::max);
        out.push(Flat {
            cells: members.iter().map(|&k| ids[k]).collect(),
            base: base[k0],
            diameter: (x1 - x0).max(y1 - y0),
            touches_border: touches,
            max_lift_m,
        });
    }
    out
}

/// One flat seen by [`regrade_flats`]: its size, whether it was graded, and its deepest lift in metres (a lake bed is deep, a plain is a fraction of a metre).
#[derive(Clone, Debug, PartialEq)]
pub struct FlatInfo {
    pub cells: usize,
    pub diameter: usize,
    pub touches_border: bool,
    pub graded: bool,
    pub max_lift_m: f64,
}

/// One flat's Euclidean grade (see [`regrade_flats`]).
#[allow(clippy::too_many_arguments)]
fn regrade_one(
    grid: &Grid,
    z: &(impl Heights + ?Sized),
    epsilon: f64,
    cells: &[u32],
    base: f64,
    filled: &[f64],
    writes: &mut Vec<(u32, f64)>,
) {
    let m = cells.len();
    let mut local = std::collections::HashMap::with_capacity(m);
    for (j, &c) in cells.iter().enumerate() {
        local.insert(c as usize, j);
    }
    let mut best = vec![u64::MAX; m];
    let mut off = vec![(0i32, 0i32); m];
    let mut parent = vec![u32::MAX; m];
    let mut rank = vec![0.0f64; m];
    let mut done = vec![false; m];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    for (j, &c) in cells.iter().enumerate() {
        let q = c as usize;
        for d in 0..8 {
            let Some(s) = grid.neighbour(q, d) else { continue };
            if local.contains_key(&s) || filled[s] > base {
                continue;
            }
            // The offset from the source `s` to `q` is the opposite of the step from `q` to `s`.
            let v = (-(NEIGHBOURS[d].0 as i32), -(NEIGHBOURS[d].1 as i32));
            let d2 = (v.0 * v.0 + v.1 * v.1) as u64;
            if d2 < best[j] {
                best[j] = d2;
                off[j] = v;
            }
        }
        if best[j] != u64::MAX {
            heap.push(Reverse((best[j], q as u32)));
        }
    }
    while let Some(Reverse((d2, p))) = heap.pop() {
        let p = p as usize;
        let jp = local[&p];
        if done[jp] || d2 != best[jp] {
            continue;
        }
        done[jp] = true;
        let below = if parent[jp] == u32::MAX { 0.0 } else { rank[local[&(parent[jp] as usize)]] };
        // Never below the cell's own height: a cell lifted by less than the chessboard count may stand above its grade.
        let own = (z.at(p) - base) / epsilon + FLAT_STEP;
        rank[jp] = (libm::sqrt(d2 as f64) / std::f64::consts::SQRT_2).max(below + FLAT_STEP).max(own);
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            let Some(&jq) = local.get(&q) else { continue };
            if done[jq] {
                continue;
            }
            let v = (off[jp].0 + NEIGHBOURS[d].0 as i32, off[jp].1 + NEIGHBOURS[d].1 as i32);
            let c = (i64::from(v.0) * i64::from(v.0) + i64::from(v.1) * i64::from(v.1)) as u64;
            if c < best[jq] {
                best[jq] = c;
                off[jq] = v;
                parent[jq] = p as u32;
                heap.push(Reverse((c, q as u32)));
            }
        }
    }
    assert!(done.iter().all(|&d| d), "a flat of {m} cells at {base} m has a cell its lower rim does not reach (cell {})", cells[0]);
    for (j, &c) in cells.iter().enumerate() {
        writes.push((c, base + epsilon * rank[j]));
    }
}

/// The depth each cell of `z` lies below the surface [`priority_flood`] makes of it, in metres, for the cells that hold a pit of at least `min_m` and 0 elsewhere (GEN-120): the epsilon
/// gradient over flats and a pit shallower than `min_m` count as no pit, so a plain is not lifted by the flood's tilt. An outlet is 0.
pub fn fill_depth(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64, min_m: f64) -> Vec<f32> {
    let filled = priority_flood(grid, z, outlet, epsilon);
    filled.iter().zip(z).map(|(f, h)| if f - h >= min_m { (f - h) as f32 } else { 0.0 }).collect()
}
