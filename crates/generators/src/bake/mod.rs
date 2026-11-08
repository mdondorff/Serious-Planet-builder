//! The bake's kernels (ADR 0088 point 3, GEN-100, GEN-101; m9-bake PR 3a): the routing and carve of pass 1 on a rectangular grid. A port of SPIKE-70's prototype
//! (`spikes/m9-global-routing/results/proto/s70carve.rs.txt`) to f64 with `libm`, the outlets an explicit input.
//!
//! Every kernel is single-threaded with a fixed visiting order (the flood's pop order with ties broken by the cell index, the donor stack, the topological order), so the
//! result is the same bits on any machine and thread count (CON-14); the jitter of the receivers is an integer hash of the GLOBAL cell (the grid's origin plus the cell) and the direction,
//! so a cell routes the same in every window that holds it. The grids do not wrap: a cube face does not route across its edges (GEN-100), its edge cells are outlets like the sea.

/// Version of the pass-1 kernels' behaviour (flood, receivers, jitter, carve, breach, the face grid): part of the layer's key (CON-07), bumped with any change that moves a bit. History: 1 = PR 3a/4; 2 = the drainage area by multiple flow direction (Freeman, p 1.1) with D8 receivers kept (SPIKE-98 diagnosis, owner 2026-11-03); 3 = the uplift tapered by the height above the sea (coast diagnosis, owner 2026-11-04); 4 = the breach limit 500 m instead of 50 m (SPIKE-101, owner 2026-11-04), and the optional height scaling of the limit. 5 = the pits kept (GEN-120): a cell below its receiver gains its uplift instead of being raised to the filled level,.
pub const BAKE_KERNEL_VERSION: u32 = 5;

/// Version of pass 2's behaviour (the window and the face-change blend of `map::change_at`, the local carve, thermal, deposition, the quantisation): part of every tile's key. History: 1 = PR 3b/5; 2 = the pass-1 change read by Catmull-Rom (C1) instead of bilinear, and the multiple-flow-direction area in the local carve and the deposition sweep; 3 = the base sampled every 4 cells and interpolated (Catmull-Rom) in a window, thermal 300 passes of 1/8 (SPIKE-98), halo 128; 4 = the depression fill capped by the receiver and the one-cell pits spread over their neighbours (volume kept). 5 = pass 1 keeps its pits (GEN-120),.
pub const PASS2_KERNEL_VERSION: u32 = 5;

pub mod carve;
pub mod deposit;
pub mod face;
pub mod fillgeom;
pub mod flood;
pub mod inputs;
pub mod map;
pub mod metric;
pub mod route;
pub mod spill;
pub mod thermal;
pub mod tile;

/// A `w x h` grid of cells `dx` metres apart, row-major (`p = j w + i`), 8-connected, not wrapping, whose cell `(0, 0)` is the global cell `(origin.0, origin.1)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    w: usize,
    h: usize,
    dx: f64,
    origin: (u32, u32),
    /// The physical cell metric (GEN-138); `None` is the uniform metric: every cell `dx^2`, neighbours `dx` and `sqrt 2 dx` apart.
    metric: Option<metric::Metric>,
}

/// The 8 neighbours as `(di, dj)`: the four axis neighbours first, then the diagonals.
pub const NEIGHBOURS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

impl Grid {
    pub fn new(w: usize, h: usize, dx: f64) -> Grid {
        assert!(w >= 3 && h >= 3, "a grid is at least 3 x 3 cells, not {w} x {h}");
        assert!(dx.is_finite() && dx > 0.0, "the cell size is finite and positive, not {dx}");
        assert!(w.checked_mul(h).is_some_and(|n| n < u32::MAX as usize), "the cell index is a u32: {w} x {h} cells are too many");
        Grid { w, h, dx, origin: (0, 0), metric: None }
    }

    /// The same grid with its cell `(0, 0)` at the global cell `origin` (a window of a larger grid: the jitter of a cell follows its global position).
    pub fn with_origin(mut self, origin: (u32, u32)) -> Grid {
        self.origin = origin;
        self
    }

    /// The same grid with a physical cell metric (GEN-138): [`Grid::cell_area`] and [`Grid::distance_at`] follow it.
    pub fn with_metric(mut self, metric: metric::Metric) -> Grid {
        self.metric = Some(metric);
        self
    }

    /// Whether the grid carries a physical metric.
    pub fn has_metric(&self) -> bool {
        self.metric.is_some()
    }

    /// The area of cell `p` in square metres: `dx^2` on the uniform metric.
    pub fn cell_area(&self, p: usize) -> f64 {
        match &self.metric {
            Some(m) => m.area(p % self.w, p / self.w),
            None => self.dx * self.dx,
        }
    }

    /// The distance from cell `p` to its neighbour `d`: [`Grid::distance`] on the uniform metric.
    pub fn distance_at(&self, p: usize, d: usize) -> f64 {
        match &self.metric {
            Some(m) => {
                let q = self.neighbour(p, d).expect("a distance to a neighbour on the grid");
                m.distance(p % self.w, p / self.w, q % self.w, q / self.w)
            }
            None => self.distance(d),
        }
    }

    pub fn w(&self) -> usize {
        self.w
    }

    pub fn h(&self) -> usize {
        self.h
    }

    pub fn dx(&self) -> f64 {
        self.dx
    }

    pub fn len(&self) -> usize {
        self.w * self.h
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The global cell id of cell `p`: the row of the global cell in the high half, its column in the low half.
    pub fn global_id(&self, p: usize) -> u64 {
        (u64::from(self.origin.1) + (p / self.w) as u64) << 32 | (u64::from(self.origin.0) + (p % self.w) as u64)
    }

    /// The neighbour `d` of cell `p`, if it is on the grid.
    pub fn neighbour(&self, p: usize, d: usize) -> Option<usize> {
        let (i, j) = ((p % self.w) as i64 + NEIGHBOURS[d].0, (p / self.w) as i64 + NEIGHBOURS[d].1);
        (i >= 0 && j >= 0 && i < self.w as i64 && j < self.h as i64).then(|| j as usize * self.w + i as usize)
    }

    /// The distance from a cell to its neighbour `d`.
    pub fn distance(&self, d: usize) -> f64 {
        if d < 4 {
            self.dx
        } else {
            self.dx * core::f64::consts::SQRT_2
        }
    }

    /// Whether `p` lies on the border of the grid.
    pub fn is_edge(&self, p: usize) -> bool {
        let (i, j) = (p % self.w, p / self.w);
        i == 0 || j == 0 || i == self.w - 1 || j == self.h - 1
    }

    /// The outlets of a face: its border cells and every cell at or below `sea_level` (water leaves through them).
    pub fn outlets(&self, z: &[f64], sea_level: f64) -> Vec<bool> {
        assert_eq!(z.len(), self.len(), "one height per cell");
        (0..self.len()).map(|p| self.is_edge(p) || z[p] <= sea_level).collect()
    }
}

/// Refuses a height field with a value that is not finite (a failed read of the base): a NaN would be left in place by every kernel.
pub(crate) fn assert_finite(what: &str, v: &[f64]) {
    if let Some(p) = v.iter().position(|x| !x.is_finite()) {
        panic!("{what} holds a value that is not finite at cell {p} ({})", v[p]);
    }
}
