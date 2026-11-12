//! Routing: the jittered D8 receivers on the filled surface, the order from the receivers (the donor stack of Braun and Willett) and the drainage area, which is multiple-flow-direction (Freeman) over the filled surface: the receivers stay D8 for the implicit carve, the area that drives it does not follow the grid's eight directions (GEN-100).

use super::flood::priority_flood_with;
use super::{Grid, Heights};

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

/// How the multiple-flow-direction area treats a lower neighbour in another D8 catchment (GEN-141, A3 section 6.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MfdClip {
    /// Today's rule: a cell shares its area among all its lower neighbours.
    #[default]
    Off,
    /// The weights stay over all lower neighbours, and the weight of a neighbour in another catchment goes to the cell's own receiver. The area stays in the catchment, but a member's area still
    /// depends on the levels of the foreign neighbours (they are in the denominator). Kept to compare with `Local`.
    ToReceiver,
    /// Lower neighbours in another catchment are dropped from the weights and from their sum: the cell shares among its own catchment's lower neighbours (its receiver is one). A catchment's
    /// area depends on its own cells only.
    Local,
}

/// The two kernel changes that make a catchment's round a function of its inputs (GEN-141, A3 section 6.4). Off by default: today's kernels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoundRules {
    pub clip_mfd: MfdClip,
    /// The flood visits by (level, cell id) with no FIFO queue ([`priority_flood_with`]).
    pub canonical_flood: bool,
}

impl RoundRules {
    /// The rules of the round solver: the local clip and the canonical flood.
    pub const ROUND: RoundRules = RoundRules { clip_mfd: MfdClip::Local, canonical_flood: true };

    pub fn is_default(&self) -> bool {
        *self == RoundRules::default()
    }
}

/// The catchment of each cell and the receivers, for the clipped area.
struct Clip<'a> {
    root: &'a [u32],
    receiver: &'a [u32],
    mode: MfdClip,
}

/// The exponent of Freeman's multiple-flow-direction split of a cell's area among its lower neighbours.
pub const MFD_EXPONENT: f64 = 1.1;

/// Routes `z`: fills it ([`priority_flood`], `epsilon`), takes for every other cell the receiver of the steepest jittered slope
/// (`slope (1 + 0.5 u)` for a downhill neighbour, `u` the [`jitter`] of the global cell), orders the cells and accumulates the area by multiple flow direction over the filled surface
/// (highest cells first, ties by the cell index: a total order, so the sums are the same bits on any machine). `max_area` caps the area when positive
/// (SPIKE-70's local carve). Every cell that is not an outlet has a receiver after the flood; a cell without one is an error naming it.
pub fn route(grid: &Grid, z: &(impl Heights + ?Sized), outlet: &[bool], epsilon: f64, seed: u64, max_area: f64) -> Routing {
    route_weighted(grid, z, outlet, epsilon, seed, max_area, None)
}

/// [`route`] with the runoff weight of each cell (GEN-129): every cell starts with the area `w dx^2`, so the returned `area` is the discharge `Q = sum(w dA)` (the area for weights of 1).
/// The cap applies to `Q`.
pub fn route_weighted(
    grid: &Grid,
    z: &(impl Heights + ?Sized),
    outlet: &[bool],
    epsilon: f64,
    seed: u64,
    max_area: f64,
    weights: Option<&[f32]>,
) -> Routing {
    route_rules(grid, z, outlet, epsilon, seed, max_area, weights, RoundRules::default())
}

/// [`route_weighted`] under the given [`RoundRules`].
#[allow(clippy::too_many_arguments)]
pub fn route_rules(
    grid: &Grid,
    z: &(impl Heights + ?Sized),
    outlet: &[bool],
    epsilon: f64,
    seed: u64,
    max_area: f64,
    weights: Option<&[f32]>,
    rules: RoundRules,
) -> Routing {
    route_core(grid, z, outlet, epsilon, seed, max_area, weights, rules, false).0
}

/// [`route_rules`] that also returns the filled surface the routing was made on (the flood of `z` under the same rules, epsilon and outlets: the bits a separate flood would give), for a caller that reads it next (H1);
/// under the unclipped rules the surface is then not dropped before the donor lists and the order are built.
#[allow(clippy::too_many_arguments)]
pub fn route_rules_filled(
    grid: &Grid,
    z: &(impl Heights + ?Sized),
    outlet: &[bool],
    epsilon: f64,
    seed: u64,
    max_area: f64,
    weights: Option<&[f32]>,
    rules: RoundRules,
) -> (Routing, Vec<f64>) {
    let (r, f) = route_core(grid, z, outlet, epsilon, seed, max_area, weights, rules, true);
    (r, f.expect("the filled surface is kept"))
}

#[allow(clippy::too_many_arguments)]
fn route_core(
    grid: &Grid,
    z: &(impl Heights + ?Sized),
    outlet: &[bool],
    epsilon: f64,
    seed: u64,
    max_area: f64,
    weights: Option<&[f32]>,
    rules: RoundRules,
    keep: bool,
) -> (Routing, Option<Vec<f64>>) {
    if let Some(w) = weights {
        assert_eq!(w.len(), grid.len(), "one runoff weight per cell");
        assert!(w.iter().all(|v| v.is_finite() && *v > 0.0), "the runoff weights are finite and positive");
    }
    let filled = priority_flood_with(grid, z, outlet, epsilon, rules.canonical_flood);
    let receiver = receivers(grid, &filled, outlet, seed);
    let (area, order);
    let filled = if rules.clip_mfd == MfdClip::Off {
        // The area comes first so that the filled surface (8 bytes a cell) is dropped before the donor lists and the order are built: a face at 12,288 cells has 1.6e8 cells.
        area = mfd_area(grid, &filled, outlet, weights, None);
        let filled = keep.then_some(filled);
        order = topological_order(&receiver);
        filled
    } else {
        // The clip needs the catchments, hence the order, before the area.
        order = topological_order(&receiver);
        let root = catchment_roots(&receiver, &order);
        area = mfd_area(grid, &filled, outlet, weights, Some(Clip { root: &root, receiver: &receiver, mode: rules.clip_mfd }));
        keep.then_some(filled)
    };
    let mut area = area;
    if max_area > 0.0 {
        for a in &mut area {
            *a = a.min(max_area);
        }
    }
    (Routing { receiver, order, area }, filled)
}

/// The root (outlet) of the D8 tree each cell drains to: the catchment label.
pub fn catchment_roots(receiver: &[u32], order: &[u32]) -> Vec<u32> {
    let mut root = vec![NONE; receiver.len()];
    for &c in order {
        let r = receiver[c as usize];
        root[c as usize] = if r == NONE { c } else { root[r as usize] };
    }
    root
}

/// The cells in a fixed topological order: each cell after its receiver (a depth-first walk of the donors from the roots in index order).
pub(super) fn topological_order(receiver: &[u32]) -> Vec<u32> {
    let n = receiver.len();
    // The donors in compressed form, then the stack from the roots: each cell comes after its receiver.
    let mut order = Vec::with_capacity(n);
    {
        let mut start = vec![0u32; n + 1];
        for &r in receiver {
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
    order
}

/// The receiver of every cell on the filled surface: [`NONE`] for an outlet.
fn receivers(grid: &Grid, filled: &[f64], outlet: &[bool], seed: u64) -> Vec<u32> {
    let mut receiver = vec![NONE; grid.len()];
    for p in 0..grid.len() {
        if outlet[p] || !grid.is_active(p) {
            continue;
        }
        receiver[p] = receiver_of(grid, filled, p, seed);
    }
    receiver
}

/// The receiver of the non-outlet cell `p`: the neighbour of the steepest jittered slope on the filled surface (it reads `p` and its 8 neighbours only).
pub(super) fn receiver_of(grid: &Grid, filled: &[f64], p: usize, seed: u64) -> u32 {
    let mut best = (0.0f64, NONE);
    for d in 0..8 {
        let Some(q) = grid.neighbour(p, d) else { continue };
        let mut s = (filled[p] - filled[q]) / grid.distance_at(p, d);
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
    best.1
}

/// Freeman's multiple flow direction (exponent [`MFD_EXPONENT`]): every cell starts with its own area and, from the highest cell down (ties by the cell index), shares what it holds
/// among its lower neighbours in proportion to `(drop / distance)^1.1`. An outlet keeps what it receives (it passes nothing on, as in the D8 tree), and so does a cell with no lower
/// neighbour.
fn mfd_area(grid: &Grid, filled: &[f64], outlet: &[bool], weights: Option<&[f32]>, clip: Option<Clip>) -> Vec<f64> {
    let n = grid.len();
    let mut by_height: Vec<u32> = (0..n as u32).collect();
    by_height.sort_unstable_by(|&a, &b| filled[b as usize].total_cmp(&filled[a as usize]).then(a.cmp(&b)));
    let mut area: Vec<f64> = match weights {
        Some(w) => w.iter().enumerate().map(|(p, v)| grid.cell_area(p) * f64::from(*v)).collect(),
        None => (0..n).map(|p| grid.cell_area(p)).collect(),
    };
    for &p in &by_height {
        let p = p as usize;
        if outlet[p] {
            continue;
        }
        let mut weight = [0.0f64; 8];
        let mut sum = 0.0;
        for (d, w) in weight.iter_mut().enumerate() {
            if let Some(q) = grid.neighbour(p, d) {
                let slope = (filled[p] - filled[q]) / grid.distance_at(p, d);
                if slope > 0.0 {
                    if let Some(c) = &clip {
                        if c.mode == MfdClip::Local && c.root[q] != c.root[p] {
                            continue;
                        }
                    }
                    *w = libm::pow(slope, MFD_EXPONENT);
                    sum += *w;
                }
            }
        }
        if sum > 0.0 {
            let held = area[p];
            if let Some(c) = clip.as_ref().filter(|c| c.mode == MfdClip::ToReceiver) {
                // The weight of a neighbour in another catchment goes to the cell's own receiver (a lower neighbour of its own tree, so its weight is positive).
                let mut redirect = 0.0;
                let mut own = usize::MAX;
                for (d, w) in weight.iter_mut().enumerate() {
                    if *w > 0.0 {
                        let q = grid.neighbour(p, d).expect("a weighted neighbour exists");
                        if q as u32 == c.receiver[p] {
                            own = d;
                        } else if c.root[q] != c.root[p] {
                            redirect += *w;
                            *w = 0.0;
                        }
                    }
                }
                assert!(own != usize::MAX || redirect == 0.0, "cell {p}: its receiver is not a lower neighbour");
                if own != usize::MAX {
                    weight[own] += redirect;
                }
            }
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
