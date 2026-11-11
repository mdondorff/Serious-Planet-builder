//! The channel graph of H1 as compact arrays (item 15 PR 7, GEN-170; architect D51): the segments' cell chains, the routed discharge `Q_w` and the bed level at every vertex, in the form the store keeps
//! and K7's burn and the river drawing read.
//!
//! - A segment is its vertices from `up` to `down` (both included); consecutive vertices are 8-neighbours, so the chain is one cell index and one direction (`Grid::neighbour`) a step.
//! - `Q_w` is a `u16` on a log scale (D51, ADR 0114 point 4): code 0 is no discharge, codes 1..=65535 span [`Q_CODE_MIN`], [`Q_CODE_MAX`] m3/s in equal ratios, a relative step of about 3.9e-4 (so a read is within
//!   1.9e-4 of the stored discharge and a width `7.2 sqrt(Q)` within 1e-4). K7's burn reads the dequantised value, so the burn and the drawing use the same width (D6) and a fresh build and a re-solve agree (CON-29).
//! - The bed level is the running minimum of the H1 surface along the segment (the channel never rises downstream), as an `i16` count of [`BED_STEP_M`]; the minimum of quantised values is the quantised minimum.
//! - The arrays are built in the pre-release step, from the routing and `Q_w` that are released afterwards; they hold no pointers into either.

use super::hydro::{Hydro, QUnit};
use super::route::{Routing, NONE};
use super::Grid;

/// The discharge of code 1, m3/s (a discharge below it reads as it).
pub const Q_CODE_MIN: f64 = 1.0e-4;
/// The discharge of code 65535, m3/s (a discharge above it reads as it).
pub const Q_CODE_MAX: f64 = 1.0e7;
/// The step of the bed level, metres.
pub const BED_STEP_M: f64 = 0.5;

fn ln_step() -> f64 {
    (libm::log(Q_CODE_MAX) - libm::log(Q_CODE_MIN)) / 65534.0
}

/// The code of a discharge: 0 for none (zero, negative or not a number), else the nearest step of the log scale between [`Q_CODE_MIN`] and [`Q_CODE_MAX`] (clamped).
pub fn q_code(q: f64) -> u16 {
    if q.is_nan() || q <= 0.0 {
        return 0;
    }
    let x = (libm::log(q.clamp(Q_CODE_MIN, Q_CODE_MAX)) - libm::log(Q_CODE_MIN)) / ln_step();
    (libm::round(x) as i64 + 1).clamp(1, 65535) as u16
}

/// The discharge of a code (m3/s): 0 for code 0.
pub fn q_value(code: u16) -> f64 {
    if code == 0 {
        0.0
    } else {
        libm::exp(libm::log(Q_CODE_MIN) + f64::from(code - 1) * ln_step())
    }
}

/// The code of a bed level in metres (clamped to the `i16` range).
pub fn bed_code(z: f64) -> i16 {
    libm::round(z / BED_STEP_M).clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

/// The bed level of a code, metres.
pub fn bed_value(code: i16) -> f64 {
    f64::from(code) * BED_STEP_M
}

/// The segments of a hydrology as compact arrays, in the order of `Hydro::segments`.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelGraph {
    /// The unit of `Q_w` (a discharge in m3/s or an area): an area is refused by the consumers (D33).
    pub q_unit: QUnit,
    /// The largest relative error of a stored `Q_w` against the f64 it was made from, over every vertex (at most half a step, 1.9e-4; the width `7.2 sqrt(Q)` is within half of it).
    pub max_q_rel_error: f64,
    /// The first cell (the `up` cell) and the number of vertices of each segment.
    first: Vec<u32>,
    verts: Vec<u32>,
    /// Per vertex, all segments one after another: the direction of the step from the previous vertex (0 for a segment's first), the `Q_w` code and the bed code.
    dir: Vec<u8>,
    q: Vec<u16>,
    bed: Vec<i16>,
}

/// One segment of the graph.
pub struct SegmentView<'a> {
    pub index: usize,
    pub first: u32,
    pub dir: &'a [u8],
    pub q: &'a [u16],
    pub bed: &'a [i16],
}

impl SegmentView<'_> {
    /// The cells of the chain: `first`, then the neighbour of the previous cell in each step's direction.
    pub fn cells(&self, grid: &Grid) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.dir.len());
        let mut cur = self.first;
        out.push(cur);
        for &d in &self.dir[1..] {
            cur = grid.neighbour(cur as usize, usize::from(d)).expect("a stored step leads to a neighbour") as u32;
            out.push(cur);
        }
        out
    }
}

impl ChannelGraph {
    /// The graph of `hydro` on `grid`: the chains by the routing's receivers, `Q_w` and the running minimum of `zs` (the H1 surface) along them. `Q_w` must still be there (run before the discharges are released).
    pub fn from_hydro(grid: &Grid, routing: &Routing, hydro: &Hydro, zs: &[f64]) -> ChannelGraph {
        assert!(
            hydro.qw.len() == grid.len() && zs.len() == grid.len(),
            "the graph needs Q_w and the surface: run it before the discharges are released"
        );
        let n = hydro.segments.len();
        let mut g = ChannelGraph {
            q_unit: hydro.q_unit,
            max_q_rel_error: 0.0,
            first: Vec::with_capacity(n),
            verts: Vec::with_capacity(n),
            dir: Vec::new(),
            q: Vec::new(),
            bed: Vec::new(),
        };
        for sg in &hydro.segments {
            let mut cur = sg.up as usize;
            let mut level = f64::INFINITY;
            g.first.push(sg.up);
            let mut count = 0u32;
            let mut into = 0u8; // the direction of the step into the current vertex (0 for the first)
            loop {
                level = level.min(zs[cur]);
                g.dir.push(into);
                let code = q_code(hydro.qw[cur]);
                if hydro.qw[cur] >= Q_CODE_MIN && hydro.qw[cur] <= Q_CODE_MAX {
                    g.max_q_rel_error = g.max_q_rel_error.max(((q_value(code) - hydro.qw[cur]) / hydro.qw[cur]).abs());
                }
                g.q.push(code);
                g.bed.push(bed_code(level));
                count += 1;
                if cur == sg.down as usize {
                    break;
                }
                let next = routing.receiver[cur];
                assert!(next != NONE, "a segment reaches its end cell");
                into = (0..8).find(|&d| grid.neighbour(cur, d) == Some(next as usize)).expect("a receiver is a neighbour") as u8;
                cur = next as usize;
            }
            g.verts.push(count);
        }
        g
    }

    pub fn segments(&self) -> usize {
        self.first.len()
    }

    pub fn vertices(&self) -> usize {
        self.q.len()
    }

    /// The bytes the arrays hold.
    pub fn bytes(&self) -> usize {
        self.first.len() * 4 + self.verts.len() * 4 + self.dir.len() + self.q.len() * 2 + self.bed.len() * 2
    }

    /// Calls `f` for every segment in order.
    pub fn for_each(&self, mut f: impl FnMut(SegmentView<'_>)) {
        let mut at = 0usize;
        for (index, (&first, &v)) in self.first.iter().zip(&self.verts).enumerate() {
            let v = v as usize;
            f(SegmentView { index, first, dir: &self.dir[at..at + v], q: &self.q[at..at + v], bed: &self.bed[at..at + v] });
            at += v;
        }
    }
}
