//! Barnes' improved priority flood with an epsilon (Barnes, Lehman and Mulla 2014; SPIKE-62): the surface with every depression filled to its pour point and a
//! gradient of `epsilon` per cell over the flats, so that every cell drains. The improved variant keeps the cells that fill to the level of their parent in a plain queue and
//! only the others in the heap.

use super::{assert_finite, Grid};
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
pub fn priority_flood(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64) -> Vec<f64> {
    priority_flood_with(grid, z, outlet, epsilon, false)
}

/// [`priority_flood`] with the canonical tie rule when `canonical` (GEN-141, A3 section 6.4): there is no FIFO queue for the flats, every cell enters the heap at its filled level and cells are
/// visited by (level, cell id), so a cell's level comes from its lowest visited neighbour by a total order that no other cell's presence changes. The id order of the cells of a rectangular grid
/// is the global id order (row-major), so it is the same in every window.
pub fn priority_flood_with(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64, canonical: bool) -> Vec<f64> {
    let n = grid.len();
    assert!(z.len() == n && outlet.len() == n, "one height and one outlet flag per cell");
    assert!(epsilon > 0.0 && epsilon.is_finite(), "the epsilon is finite and positive (a flat could not drain), not {epsilon}");
    assert_finite("the heights", z);
    assert!(outlet.iter().any(|&o| o), "a flood needs an outlet: the mask has none");
    let mut filled = vec![0.0f64; n];
    let mut seen = vec![false; n];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    let mut flat: VecDeque<u32> = VecDeque::new();
    for p in 0..n {
        if outlet[p] {
            filled[p] = z[p];
            seen[p] = true;
        }
    }
    // Only an outlet with a neighbour that is not one expands anything: the others are seen and finished (the same filled surface, a far smaller heap on a sea face).
    for p in 0..n {
        if outlet[p] && grid.is_active(p) && (0..8).filter_map(|d| grid.neighbour(p, d)).any(|q| !outlet[q]) {
            heap.push(Reverse((key(z[p]), p as u32)));
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
            if !canonical && z[q] <= lowest {
                filled[q] = lowest;
                flat.push_back(q as u32);
            } else {
                filled[q] = z[q].max(lowest);
                heap.push(Reverse((key(filled[q]), q as u32)));
            }
        }
    }
    filled
}

/// The depth each cell of `z` lies below the surface [`priority_flood`] makes of it, in metres, for the cells that hold a pit of at least `min_m` and 0 elsewhere (GEN-120): the epsilon
/// gradient over flats and a pit shallower than `min_m` count as no pit, so a plain is not lifted by the flood's tilt. An outlet is 0.
pub fn fill_depth(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64, min_m: f64) -> Vec<f32> {
    let filled = priority_flood(grid, z, outlet, epsilon);
    filled.iter().zip(z).map(|(f, h)| if f - h >= min_m { (f - h) as f32 } else { 0.0 }).collect()
}
