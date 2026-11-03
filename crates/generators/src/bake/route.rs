//! Routing: the jittered D8 receivers on the filled surface, the order from the receivers (the donor stack of Braun and Willett) and the drainage area (GEN-100).

use super::flood::priority_flood;
use super::Grid;

/// `u32::MAX` marks a cell without a receiver (an outlet).
pub const NONE: u32 = u32::MAX;

/// A routing of a grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Routing {
    /// The receiver of each cell, or [`NONE`] for an outlet.
    pub receiver: Vec<u32>,
    /// The cells outlets first, each cell after its receiver (a fixed topological order: the donor stack from the outlets in index order).
    pub order: Vec<u32>,
    /// The drainage area of each cell in square metres: its own cell plus the areas of its donors, capped at the largest area when one is given.
    pub area: Vec<f64>,
    /// The filled surface the receivers were taken on.
    pub filled: Vec<f64>,
}

/// The jitter of the receiver choice of cell `p` in direction `d`: a number in `[-1, 1)`, an integer hash of the two (the seed names the world's stream).
pub fn jitter(seed: u64, p: u64, d: usize) -> f64 {
    let x = (seed ^ p.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ (d as u64 + 1).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = (x ^ (x >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let x = x ^ (x >> 32);
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// Routes `z`: fills it ([`priority_flood`], `epsilon`), takes for every other cell the receiver of the steepest jittered slope
/// (`slope (1 + 0.5 u)` for a downhill neighbour, `u` the [`jitter`]), orders the cells and accumulates the area. `max_area` caps the area when positive (SPIKE-70's local carve).
pub fn route(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64, seed: u64, max_area: f64) -> Routing {
    let n = grid.len();
    let filled = priority_flood(grid, z, outlet, epsilon);
    let mut receiver = vec![NONE; n];
    for p in 0..n {
        if outlet[p] {
            continue;
        }
        let mut best = (0.0f64, NONE);
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            let mut s = (filled[p] - filled[q]) / grid.distance(d);
            if s > 0.0 {
                s *= 1.0 + 0.5 * jitter(seed, p as u64, d);
            }
            if s > best.0 {
                best = (s, q as u32);
            }
        }
        receiver[p] = best.1;
    }
    // The donors in compressed form, then the stack from the outlets: each cell comes after its receiver.
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
    let mut order = Vec::with_capacity(n);
    let mut stack: Vec<u32> = (0..n as u32).rev().filter(|&p| receiver[p as usize] == NONE).collect();
    while let Some(p) = stack.pop() {
        order.push(p);
        let (a, b) = (start[p as usize] as usize, start[p as usize + 1] as usize);
        stack.extend(donors[a..b].iter().rev());
    }
    let mut area = vec![grid.dx * grid.dx; n];
    for &p in order.iter().rev() {
        let r = receiver[p as usize];
        if r != NONE {
            area[r as usize] += area[p as usize];
        }
    }
    if max_area > 0.0 {
        for a in &mut area {
            *a = a.min(max_area);
        }
    }
    Routing { receiver, order, area, filled }
}
