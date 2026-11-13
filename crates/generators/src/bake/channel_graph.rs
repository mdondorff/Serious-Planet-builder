//! The channel graph of H1 as compact arrays (item 15 PR 7, GEN-170; architect D51): the segments' cell chains, the routed discharge `Q_w` and the bed level at every vertex, in the form the store keeps
//! and K7's burn and the river drawing read.
//!
//! - A segment is its vertices from `up` to `down` (both included); consecutive vertices are 8-neighbours, so the chain is one cell index and one direction (`Grid::neighbour`) a step.
//! - `Q_w` is a `u16` on a log scale (D51, ADR 0114 point 4): code 0 is no discharge, codes 1..=65535 span [`Q_CODE_MIN`], [`Q_CODE_MAX`] m3/s in equal ratios, a relative step of 3.87e-4 (so a read is within
//!   1.94e-4 of the stored discharge and a width `7.2 sqrt(Q)` within 0.97e-4). K7's burn reads the dequantised value, so the burn and the drawing use the same width (D6) and a fresh build and a re-solve agree (CON-29).
//! - The bed level is the running minimum of the H1 surface along the segment (the channel never rises downstream), as an `i16` count of [`BED_STEP_M`]; the minimum of quantised values is the quantised minimum.
//! - The arrays are built in the pre-release step, from the routing and `Q_w` that are released afterwards; they hold no pointers into either.

use super::hydro::{Hydro, QUnit};
use super::k7::Channel;
use super::route::{Routing, NONE};
use super::{Grid, Heights};

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

/// The code of a bed level in metres: the step at or below it (clamped to the `i16` range: a level below -16,384 m reads as -16,384 m, above it; the range holds the Earth and is a stated limit for deeper bodies), so the stored bed never lies above the surface it was made from and K7's burn (which only lowers) cuts every cell the
/// unquantised bed would cut. The floor is as monotone as the level, so the quantised running minimum is the minimum of the quantised levels.
pub fn bed_code(z: f64) -> i16 {
    libm::floor(z / BED_STEP_M).clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
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
    /// The largest relative error of a stored `Q_w` against the f64 it was made from, over every vertex (at most half a step, 1.94e-4; the width `7.2 sqrt(Q)` is within half of it).
    pub max_q_rel_error: f64,
    /// The first cell (the `up` cell) and the number of vertices of each segment.
    first: Vec<u32>,
    verts: Vec<u32>,
    /// Per vertex, all segments one after another: the direction of the step from the previous vertex (0 for a segment's first), the `Q_w` code and the bed code.
    dir: Vec<u8>,
    q: Vec<u16>,
    bed: Vec<i16>,
    /// Where segments go under a lake (D54): `(segment index, first vertex, end vertex)`, sorted; see [`PreRelease::cuts`](super::lakes::PreRelease).
    cuts: Vec<(u32, u32, u32)>,
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
    pub fn from_hydro(grid: &Grid, routing: &Routing, hydro: &Hydro, zs: &(impl Heights + ?Sized)) -> ChannelGraph {
        assert!(
            hydro.qw.iter().all(|q| q.is_finite()) && (0..zs.len()).all(|c| zs.at(c).is_finite()),
            "the graph needs Q_w and the surface finite (no NaN, no infinity)"
        );
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
            cuts: Vec::new(),
        };
        for sg in &hydro.segments {
            let mut cur = sg.up as usize;
            let mut level = f64::INFINITY;
            g.first.push(sg.up);
            let mut count = 0u32;
            let mut into = 0u8; // the direction of the step into the current vertex (0 for the first)
            loop {
                level = level.min(zs.at(cur));
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

    /// The burn levels of the whole network (GEN-185): every vertex's bed becomes the running minimum down the channel tree of its H1 level and of `start` (the start surface `B + regional` of K7) at every
    /// local line cell up to it, the cells of the line between consecutive vertices rasterised as K7's burn rasterises them (`to_local` maps an H1 cell to its point in the face's local cell-index space).
    /// A tributary is a segment whose last cell is this segment's first cell; its end level carries into this segment. A function of the network and of `start` alone, so every tile that burns a line
    /// reads the same levels whatever window it holds (partition independence) and the burn's carry only bridges the cells between two vertices. A level outside the bed code's `i16` range
    /// (-16,384 m to 16,383.5 m) is clamped by the code (a stated limit: the region's height bounds lie inside it). Returns the number of vertices lowered and the largest
    /// lowering in metres. Run it before the graph is stored and before the channels are made from it: the stored levels are the burn's input (STRM-084).
    pub fn deepen_beds(&mut self, grid: &Grid, to_local: &dyn Fn(u32) -> (f64, f64), start: &(dyn Fn(i64, i64) -> f64 + Sync)) -> (usize, f64) {
        let n = self.first.len();
        let mut offset = Vec::with_capacity(n);
        let mut at = 0usize;
        for &v in &self.verts {
            offset.push(at);
            at += v as usize;
        }
        let cells: Vec<Vec<u32>> = {
            let mut out = Vec::with_capacity(n);
            self.for_each(|s| out.push(s.cells(grid)));
            out
        };
        // the segments that end at each cell (a one-vertex segment is nobody's tributary)
        let mut ending: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
        for (s, c) in cells.iter().enumerate() {
            if c.len() > 1 {
                ending.entry(*c.last().expect("a vertex")).or_default().push(s);
            }
        }
        // a segment's end level, computed after its tributaries' (an explicit stack: the trees are deep)
        let mut end: Vec<Option<f64>> = vec![None; n];
        // A segment on the stack whose tributaries are being computed: a cycle (malformed input only; H1's receivers make none) reads such a tributary as no carry instead of looping.
        let mut visiting = vec![false; n];
        let (mut lowered, mut largest) = (0usize, 0.0f64);
        for root in 0..n {
            let mut stack = vec![(root, false)];
            while let Some((s, ready)) = stack.pop() {
                if end[s].is_some() || (!ready && visiting[s]) {
                    continue;
                }
                let tribs: Vec<usize> = ending.get(&cells[s][0]).map_or(Vec::new(), |v| v.iter().copied().filter(|&u| u != s).collect());
                if !ready {
                    visiting[s] = true;
                    stack.push((s, true));
                    for &u in &tribs {
                        if end[u].is_none() && !visiting[u] {
                            stack.push((u, false));
                        }
                    }
                    continue;
                }
                let mut run = tribs.iter().filter_map(|&u| end[u]).fold(f64::INFINITY, f64::min);
                let c = &cells[s];
                for k in 0..c.len() {
                    let (x1, y1) = to_local(c[k]);
                    let (x0, y0) = if k == 0 { (x1, y1) } else { to_local(c[k - 1]) };
                    let step = Channel { points: vec![(x0, y0), (x1, y1)], levels: vec![0.0, 0.0], q: Vec::new(), q_unit: QUnit::Discharge };
                    for (x, y) in super::k7::line_cells(&step) {
                        let v = start(x, y);
                        assert!(v.is_finite(), "the start surface at local cell ({x}, {y}) is {v}: not finite");
                        run = run.min(v);
                    }
                    let i = offset[s] + k;
                    let old = bed_value(self.bed[i]);
                    run = run.min(old);
                    let code = bed_code(run);
                    if code < self.bed[i] {
                        lowered += 1;
                        largest = largest.max(old - bed_value(code));
                        self.bed[i] = code;
                    }
                }
                end[s] = Some(run);
                visiting[s] = false;
            }
        }
        (lowered, largest)
    }

    /// Records where the segments go under a lake (the producer's cuts, [`super::lakes::lake_cut_ranges`]): sorted, inside the segments and not empty.
    pub fn set_cuts(&mut self, cuts: Vec<(u32, u32, u32)>) {
        assert!(cuts.windows(2).all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1)), "the cuts are sorted by segment and vertex");
        assert!(
            cuts.iter().all(|&(s, a, b)| (s as usize) < self.verts.len() && a < b && b < self.verts[s as usize]),
            "a cut lies inside its segment and leaves its last vertex"
        );
        self.cuts = cuts;
    }

    /// The cuts of segment `index`: vertex ranges under water.
    pub fn cuts_of(&self, index: usize) -> &[(u32, u32, u32)] {
        let lo = self.cuts.partition_point(|c| (c.0 as usize) < index);
        let hi = self.cuts.partition_point(|c| (c.0 as usize) <= index);
        &self.cuts[lo..hi]
    }

    pub fn cuts(&self) -> &[(u32, u32, u32)] {
        &self.cuts
    }

    pub fn segments(&self) -> usize {
        self.first.len()
    }

    pub fn vertices(&self) -> usize {
        self.q.len()
    }

    /// The bytes the arrays hold.
    pub fn bytes(&self) -> usize {
        self.first.len() * 4 + self.verts.len() * 4 + self.dir.len() + self.q.len() * 2 + self.bed.len() * 2 + self.cuts.len() * 12
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

/// The channels of a graph on the local grid (K7's input from the compact arrays): one [`Channel`] per segment with the dequantised `Q_w` and bed levels, the points `to_local` of each cell.
pub fn channels_from_graph(graph: &ChannelGraph, grid: &Grid, to_local: &dyn Fn(u32) -> (f64, f64)) -> Vec<Channel> {
    let mut out = Vec::with_capacity(graph.segments());
    graph.for_each(|s| {
        out.push(Channel {
            points: s.cells(grid).iter().map(|&c| to_local(c)).collect(),
            levels: s.bed.iter().map(|&b| bed_value(b)).collect(),
            q: s.q.iter().map(|&q| q_value(q)).collect(),
            q_unit: graph.q_unit,
        });
    });
    out
}
