//! Routing: the jittered D8 receivers on the filled surface, the order from the receivers (the donor stack of Braun and Willett) and the drainage area, which is multiple-flow-direction (Freeman) over the filled surface: the receivers stay D8 for the implicit carve, the area that drives it does not follow the grid's eight directions (GEN-100).

use super::flood::priority_flood;
use super::Grid;

/// `u32::MAX` marks a cell without a receiver (an outlet).
pub const NONE: u32 = u32::MAX;

/// A routing of a grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Routing {
    /// The receiver of each cell, or [`NONE`] for an outlet.
    pub receiver: Vec<u32>,
    /// The cells in a fixed topological order: each cell after its receiver (a depth-first walk of the donors from the outlets in index order).
    pub order: Vec<u32>,
    /// The drainage area of each cell in square metres: its own cell plus what the cells above it send to it by multiple flow direction (a cell shares its area among its lower
    /// neighbours in proportion to `slope^1.1`, [`MFD_EXPONENT`]), capped at the largest area when one is given.
    pub area: Vec<f64>,
}

/// The jitter of the receiver choice of the global cell `id` in direction `d`: a number in `[-1, 1)`, an integer hash of the two (the seed names the world's stream).
pub fn jitter(seed: u64, id: u64, d: usize) -> f64 {
    let x = (seed ^ id.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ (d as u64 + 1).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = (x ^ (x >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let x = x ^ (x >> 32);
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// The exponent of Freeman's multiple-flow-direction split of a cell's area among its lower neighbours.
pub const MFD_EXPONENT: f64 = 1.1;

/// Routes `z`: fills it ([`priority_flood`], `epsilon`), takes for every other cell the receiver of the steepest jittered slope
/// (`slope (1 + 0.5 u)` for a downhill neighbour, `u` the [`jitter`] of the global cell), orders the cells and accumulates the area by multiple flow direction over the filled surface
/// (highest cells first, ties by the cell index: a total order, so the sums are the same bits on any machine). `max_area` caps the area when positive
/// (SPIKE-70's local carve). Every cell that is not an outlet has a receiver after the flood; a cell without one is an error naming it.
pub fn route(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64, seed: u64, max_area: f64) -> Routing {
    let n = grid.len();
    let filled = priority_flood(grid, z, outlet, epsilon);
    let receiver = receivers(grid, &filled, outlet, seed);
    // The donors in compressed form, then the stack from the outlets: each cell comes after its receiver.
    let mut order = Vec::with_capacity(n);
    {
        let mut start = vec![0u32; n + 1];
        for &r in &receiver {
            if r != NONE {
                start[r as usize + 1] += 1;
            }
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut next = start.clone();
        let mut donors = vec![0u32; n];
        for (p, &r) in receiver.iter().enumerate() {
            if r != NONE {
                donors[next[r as usize] as usize] = p as u32;
                next[r as usize] += 1;
            }
        }
        drop(next);
        let mut stack: Vec<u32> = (0..n as u32).rev().filter(|&p| receiver[p as usize] == NONE).collect();
        while let Some(p) = stack.pop() {
            order.push(p);
            let (a, b) = (start[p as usize] as usize, start[p as usize + 1] as usize);
            stack.extend(donors[a..b].iter().rev());
        }
    }
    assert_eq!(order.len(), n, "the order does not reach every cell: a cell drains in a cycle");
    let mut area = mfd_area(grid, &filled, outlet);
    if max_area > 0.0 {
        for a in &mut area {
            *a = a.min(max_area);
        }
    }
    Routing { receiver, order, area }
}

/// The receiver of every cell on the filled surface: [`NONE`] for an outlet.
fn receivers(grid: &Grid, filled: &[f64], outlet: &[bool], seed: u64) -> Vec<u32> {
    let mut receiver = vec![NONE; grid.len()];
    for p in 0..grid.len() {
        if outlet[p] {
            continue;
        }
        let mut best = (0.0f64, NONE);
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            let mut s = (filled[p] - filled[q]) / grid.distance(d);
            if s > 0.0 {
                s *= 1.0 + 0.5 * jitter(seed, grid.global_id(p), d);
            }
            if s > best.0 {
                best = (s, q as u32);
            }
        }
        assert!(
            best.1 != NONE,
            "cell {p} is not an outlet and has no lower neighbour after the flood (the heights are too flat for the epsilon)"
        );
        receiver[p] = best.1;
    }
    receiver
}

/// Freeman's multiple flow direction (exponent [`MFD_EXPONENT`]): every cell starts with its own area and, from the highest cell down (ties by the cell index), shares what it holds
/// among its lower neighbours in proportion to `(drop / distance)^1.1`. An outlet keeps what it receives (it passes nothing on, as in the D8 tree), and so does a cell with no lower
/// neighbour.
fn mfd_area(grid: &Grid, filled: &[f64], outlet: &[bool]) -> Vec<f64> {
    let n = grid.len();
    let mut by_height: Vec<u32> = (0..n as u32).collect();
    by_height.sort_by(|&a, &b| filled[b as usize].total_cmp(&filled[a as usize]).then(a.cmp(&b)));
    let mut area = vec![grid.dx() * grid.dx(); n];
    for &p in &by_height {
        let p = p as usize;
        if outlet[p] {
            continue;
        }
        let mut weight = [0.0f64; 8];
        let mut sum = 0.0;
        for (d, w) in weight.iter_mut().enumerate() {
            if let Some(q) = grid.neighbour(p, d) {
                let slope = (filled[p] - filled[q]) / grid.distance(d);
                if slope > 0.0 {
                    *w = libm::pow(slope, MFD_EXPONENT);
                    sum += *w;
                }
            }
        }
        if sum > 0.0 {
            let held = area[p];
            for (d, w) in weight.iter().enumerate() {
                if *w > 0.0 {
                    let q = grid.neighbour(p, d).expect("a weighted neighbour exists");
                    area[q] += held * w / sum;
                }
            }
        }
    }
    area
}
