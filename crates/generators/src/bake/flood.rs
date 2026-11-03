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
        if outlet[p] && (0..8).filter_map(|d| grid.neighbour(p, d)).any(|q| !outlet[q]) {
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
            if z[q] <= lowest {
                filled[q] = lowest;
                flat.push_back(q as u32);
            } else {
                filled[q] = z[q];
                heap.push(Reverse((key(z[q]), q as u32)));
            }
        }
    }
    filled
}
