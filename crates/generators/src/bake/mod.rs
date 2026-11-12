//! The bake's kernels (ADR 0088 point 3, GEN-100, GEN-101; m9-bake PR 3a): the routing and carve of pass 1 on a rectangular grid. A port of SPIKE-70's prototype
//! (`spikes/m9-global-routing/results/proto/s70carve.rs.txt`) to f64 with `libm`, the outlets an explicit input.
//!
//! Every kernel is single-threaded with a fixed visiting order (the flood's pop order with ties broken by the cell index, the donor stack, the topological order), so the
//! result is the same bits on any machine and thread count (CON-14); the jitter of the receivers is an integer hash of the GLOBAL cell (the grid's origin plus the cell) and the direction,
//! so a cell routes the same in every window that holds it. The grids do not wrap: a cube face does not route across its edges (GEN-100), its edge cells are outlets like the sea.

/// Version of the pass-1 kernels' behaviour (flood, receivers, jitter, carve, breach, the face grid): part of the layer's key (CON-07), bumped with any change that moves a bit. History: 1 = PR 3a/4; 2 = the drainage area by multiple flow direction (Freeman, p 1.1) with D8 receivers kept (SPIKE-98 diagnosis, owner 2026-11-03); 3 = the uplift tapered by the height above the sea (coast diagnosis, owner 2026-11-04); 4 = the breach limit 500 m instead of 50 m (SPIKE-101, owner 2026-11-04), and the optional height scaling of the limit. 5 = the pits kept (GEN-120): a cell below its receiver gains its uplift instead of being raised to the filled level. 6 = the flats of the canonical flood graded by Euclidean distance to their lower rim up to the bound of halo - 1 cells (GEN-173, architect, 2026-11-12); 7 = the breach lowers a cell by at most its limit in a round, however many donors breach it (GEN-174, architect, 2026-11-12).
pub const BAKE_KERNEL_VERSION: u32 = 7;

/// Version of pass 2's behaviour (the window and the face-change blend of `map::change_at`, the local carve, thermal, deposition, the quantisation): part of every tile's key. History: 1 = PR 3b/5; 2 = the pass-1 change read by Catmull-Rom (C1) instead of bilinear, and the multiple-flow-direction area in the local carve and the deposition sweep; 3 = the base sampled every 4 cells and interpolated (Catmull-Rom) in a window, thermal 300 passes of 1/8 (SPIKE-98), halo 128; 4 = the depression fill capped by the receiver and the one-cell pits spread over their neighbours (volume kept). 5 = pass 1 keeps its pits (GEN-120); 6 = the K7 burn targets the lowest of the level and the heights found within the halo upstream along the channel tree (GEN-169), the width of a trench is capped at 32 cells, and a final channel pass lowers the path cells to the lowest height within the halo upstream (architect, 2026-11-11); 7 = the flats of the canonical flood graded by Euclidean distance (GEN-173, architect, 2026-11-12); 8 = the breach cap per cell per round (GEN-174, architect, 2026-11-12); 9 = K7 river beds at or above the sea level on land and the channel cells fixed for every stage after the burn (GEN-176, architect, 2026-11-12).
pub const PASS2_KERNEL_VERSION: u32 = 9;

pub mod breach;
pub mod build;
pub mod carve;
pub mod channel_graph;
pub mod compact;
pub mod cube;
pub mod deposit;
pub mod face;
pub mod fillgeom;
pub mod flood;
pub mod h0;
pub mod hydro;
pub mod inputs;
pub mod k7;
pub mod lakes;
pub mod map;
pub mod memo;
pub mod metric;
pub mod partition;
pub mod precision;
pub mod preview;
pub mod probe;
pub mod round;
pub mod route;
pub mod sediment;
pub mod spill;
pub mod thermal;
pub mod tile;
pub mod water_mask;

/// A `w x h` grid of cells `dx` metres apart, row-major (`p = j w + i`), 8-connected, not wrapping, whose cell `(0, 0)` is the global cell `(origin.0, origin.1)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    w: usize,
    h: usize,
    dx: f64,
    origin: (u32, u32),
    /// The physical cell metric (GEN-138); `None` is the uniform metric: every cell `dx^2`, neighbours `dx` and `sqrt 2 dx` apart.
    metric: Option<metric::Metric>,
    /// The cells that exist (GEN-141): a cell outside the mask has no neighbours and is no one's neighbour, so the kernels see only the masked cells. `None`: every cell.
    active: Option<std::sync::Arc<[bool]>>,
    /// The six cube faces as one grid (GEN-144): the neighbours across the cube edges, the sphere's areas and distances.
    cube: Option<std::sync::Arc<cube::Cube>>,
    /// With `cube`: only the active cells, indexed by rank (GEN-147); `w` is their number and `h` is 1.
    compact: Option<std::sync::Arc<compact::ActiveIndex>>,
}

/// The 8 neighbours as `(di, dj)`: the four axis neighbours first, then the diagonals.
pub const NEIGHBOURS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

impl Grid {
    pub fn new(w: usize, h: usize, dx: f64) -> Grid {
        assert!(w >= 3 && h >= 3, "a grid is at least 3 x 3 cells, not {w} x {h}");
        assert!(dx.is_finite() && dx > 0.0, "the cell size is finite and positive, not {dx}");
        assert!(w.checked_mul(h).is_some_and(|n| n < u32::MAX as usize), "the cell index is a u32: {w} x {h} cells are too many");
        Grid { w, h, dx, origin: (0, 0), metric: None, active: None, cube: None, compact: None }
    }

    /// The same grid with its cell `(0, 0)` at the global cell `origin` (a window of a larger grid: the jitter of a cell follows its global position).
    pub fn with_origin(mut self, origin: (u32, u32)) -> Grid {
        assert!(self.cube.is_none(), "a cube grid has its own cell ids: no origin");
        self.origin = origin;
        self
    }

    /// The six faces of a cube sphere of `n` cells an edge as one `n x 6n` grid (GEN-144): cell `(face, i, j)` is `face n^2 + j n + i`, the neighbours join across the cube edges as in H0
    /// (GEN-139), the areas and distances are the sphere's. There is no border: every cell has neighbours, the outlets are the sea.
    pub fn cube(n: usize, radius_m: f64) -> Grid {
        let mut g = Grid::new(n, 6 * n, radius_m * core::f64::consts::FRAC_PI_2 / n as f64);
        g.cube = Some(std::sync::Arc::new(cube::Cube::new(n, radius_m)));
        g
    }

    /// The landmass of a cube sphere of `n` cells an edge as a compact grid (GEN-147): `bits` is the bitset of the active cube ids (bit `g % 64` of word `g / 64`). Cell `p` is the `p`-th active cell
    /// by cube id; neighbours, areas, distances and the global ids are the cube grid's, so a solve gives the bits of the same solve on the masked cube grid with `6 n^2` cells.
    pub fn compact_cube(n: usize, radius_m: f64, bits: Vec<u64>) -> Grid {
        assert!(bits.len() == (6 * n * n).div_ceil(64), "one bit per cube cell");
        let tail = (6 * n * n) % 64;
        assert!(tail == 0 || bits[bits.len() - 1] >> tail == 0, "a bit is set past the last cube cell");
        let index = compact::ActiveIndex::new(bits);
        let len = index.len();
        assert!(len >= 3 && len < u32::MAX as usize, "a compact grid has at least 3 cells and fewer than 2^32");
        let mut g = Grid::cube(n, radius_m);
        g.w = len;
        g.h = 1;
        g.compact = Some(std::sync::Arc::new(index));
        g
    }

    /// Whether this is a compact grid ([`Grid::compact_cube`]): `w` is the number of cells and `h` is 1, not a rectangle.
    pub fn is_compact(&self) -> bool {
        self.compact.is_some()
    }

    /// The compact index of the cube cell `g` on a compact grid, if it is one of its cells.
    pub fn compact_index_of(&self, g: usize) -> Option<usize> {
        self.compact.as_ref().expect("a compact grid").rank(g)
    }

    /// The cube id of compact cell `p`.
    pub fn cube_id_of(&self, p: usize) -> usize {
        self.compact.as_ref().expect("a compact grid").cube_id(p)
    }

    /// Whether this is the cube grid of [`Grid::cube`].
    pub fn is_cube(&self) -> bool {
        self.cube.is_some()
    }

    /// The same grid with a physical cell metric (GEN-138): [`Grid::cell_area`] and [`Grid::distance_at`] follow it.
    pub fn with_metric(mut self, metric: metric::Metric) -> Grid {
        assert!(self.cube.is_none(), "a cube grid has the sphere's metric: no plane metric");
        self.metric = Some(metric);
        self
    }

    /// The same grid restricted to the `active` cells (one flag per cell): the others do not exist for the kernels. Used to route a catchment alone (G-LOCAL).
    pub fn with_active(mut self, active: Vec<bool>) -> Grid {
        assert!(self.compact.is_none(), "a compact grid has only its cells: no mask");
        assert_eq!(active.len(), self.len(), "one flag per cell");
        self.active = Some(active.into());
        self
    }

    /// Whether cell `p` exists on this grid.
    pub fn is_active(&self, p: usize) -> bool {
        self.active.as_ref().is_none_or(|a| a[p])
    }

    /// Whether the grid has the sphere's metric: the cube metric of GEN-138 or the cube grid of GEN-144.
    pub fn has_metric(&self) -> bool {
        self.metric.is_some() || self.cube.is_some()
    }

    /// The area of cell `p` in square metres: `dx^2` on the uniform metric.
    pub fn cell_area(&self, p: usize) -> f64 {
        if let Some(c) = &self.cube {
            return c.area(self.compact.as_ref().map_or(p, |k| k.cube_id(p)));
        }
        match &self.metric {
            Some(m) => m.area(p % self.w, p / self.w),
            None => self.dx * self.dx,
        }
    }

    /// The distance from cell `p` to its neighbour `d`: [`Grid::distance`] on the uniform metric.
    pub fn distance_at(&self, p: usize, d: usize) -> f64 {
        if let Some(c) = &self.cube {
            let q = self.neighbour(p, d).expect("a distance to a neighbour on the grid");
            return match &self.compact {
                Some(k) => c.distance(k.cube_id(p), k.cube_id(q)),
                None => c.distance(p, q),
            };
        }
        match &self.metric {
            Some(m) => {
                let q = self.neighbour(p, d).expect("a distance to a neighbour on the grid");
                m.distance(p % self.w, p / self.w, q % self.w, q / self.w)
            }
            None => self.distance(d),
        }
    }

    /// The rectangle the cells are laid out in: `(w, h)`, and for a compact cube grid the six faces stacked, `(n, 6 n)` (GEN-149).
    pub fn layout(&self) -> (usize, usize) {
        match &self.compact {
            Some(_) => {
                let n = self.cube.as_ref().expect("a compact grid is a cube").n();
                (n, 6 * n)
            }
            None => (self.w, self.h),
        }
    }

    /// The column and row of cell `p` in [`Grid::layout`].
    pub fn xy(&self, p: usize) -> (usize, usize) {
        match &self.compact {
            Some(k) => {
                let (g, n) = (k.cube_id(p), self.cube.as_ref().expect("a compact grid is a cube").n());
                (g % n, g / n)
            }
            None => (p % self.w, p / self.w),
        }
    }

    /// The cell at column `x`, row `y` of [`Grid::layout`]; on a compact grid `None` when it is not one of its cells.
    pub fn at_xy(&self, x: usize, y: usize) -> Option<usize> {
        match &self.compact {
            Some(k) => k.rank(y * self.cube.as_ref().expect("a compact grid is a cube").n() + x),
            None => Some(y * self.w + x),
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
        if let Some(k) = &self.compact {
            let (g, n) = (k.cube_id(p) as u64, self.cube.as_ref().expect("a compact grid is a cube").n() as u64);
            return (g / n) << 32 | (g % n);
        }
        (u64::from(self.origin.1) + (p / self.w) as u64) << 32 | (u64::from(self.origin.0) + (p % self.w) as u64)
    }

    /// The cell of this grid at the H1 coordinates `(i, j)` of `face`, also one step outside the face along one axis (resolved across the cube edge, [`cube::Cube::resolve`]): `None` when there is no such
    /// cell (a corner quadrant, further out, a cell the compact grid does not hold). On a plane grid only face 0 and the cells inside exist.
    pub fn cell_at_face(&self, face: u8, i: i64, j: i64) -> Option<usize> {
        if let Some(c) = &self.cube {
            let id = c.resolve(face, i, j)?;
            return match &self.compact {
                Some(k) => k.rank(id),
                None => Some(id),
            };
        }
        (face == 0 && i >= 0 && j >= 0 && (i as usize) < self.w && (j as usize) < self.h).then(|| j as usize * self.w + i as usize)
    }

    /// The neighbour `d` of cell `p`, if it is on the grid.
    pub fn neighbour(&self, p: usize, d: usize) -> Option<usize> {
        if let Some(c) = &self.cube {
            if let Some(k) = &self.compact {
                return k.rank(c.neighbour(k.cube_id(p), d)?);
            }
            let q = c.neighbour(p, d)?;
            return match &self.active {
                Some(a) if !a[p] || !a[q] => None,
                _ => Some(q),
            };
        }
        let (i, j) = ((p % self.w) as i64 + NEIGHBOURS[d].0, (p / self.w) as i64 + NEIGHBOURS[d].1);
        let q = (i >= 0 && j >= 0 && i < self.w as i64 && j < self.h as i64).then(|| j as usize * self.w + i as usize)?;
        match &self.active {
            Some(a) if !a[p] || !a[q] => None,
            _ => Some(q),
        }
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
        if self.cube.is_some() {
            return false;
        }
        let (i, j) = (p % self.w, p / self.w);
        i == 0 || j == 0 || i == self.w - 1 || j == self.h - 1
    }

    /// The window-cut outlets (GEN-172): the border cells above `sea_level`. Those below it are sea outlets; none on a region bounded by the sea.
    pub fn window_cut(&self, z: &[f64], sea_level: f64) -> Vec<bool> {
        assert_eq!(z.len(), self.len(), "one height per cell");
        (0..self.len()).map(|p| self.is_edge(p) && z[p] > sea_level).collect()
    }

    /// The outlets of a face: its border cells and every cell at or below `sea_level` (water leaves through them).
    pub fn outlets(&self, z: &[f64], sea_level: f64) -> Vec<bool> {
        assert_eq!(z.len(), self.len(), "one height per cell");
        (0..self.len()).map(|p| self.is_edge(p) || z[p] <= sea_level).collect()
    }
}

/// Refuses a height field with a value that is not finite (a failed read of the base): a NaN would be left in place by every kernel.
/// A read-only height per cell (GEN-166 PR 2): `[f64]` and `[f32]` (and their `Vec`s) give the same values, the f32 ones widened exactly, so a reader generic over it is today's code on f64 and reads the
/// f32-stored round surface without a widened copy.
pub trait Heights: Sync {
    /// The number of cells.
    fn len(&self) -> usize;
    /// Whether there is no cell.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The height of cell `i` in metres.
    fn at(&self, i: usize) -> f64;
}

impl Heights for [f64] {
    #[inline(always)]
    fn len(&self) -> usize {
        <[f64]>::len(self)
    }
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        self[i]
    }
}

impl Heights for [f32] {
    #[inline(always)]
    fn len(&self) -> usize {
        <[f32]>::len(self)
    }
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        f64::from(self[i])
    }
}

impl Heights for Vec<f64> {
    #[inline(always)]
    fn len(&self) -> usize {
        Vec::len(self)
    }
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        self[i]
    }
}

impl Heights for Vec<f32> {
    #[inline(always)]
    fn len(&self) -> usize {
        Vec::len(self)
    }
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        f64::from(self[i])
    }
}

impl<const N: usize> Heights for [f64; N] {
    #[inline(always)]
    fn len(&self) -> usize {
        N
    }
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        self[i]
    }
}

impl<const N: usize> Heights for [f32; N] {
    #[inline(always)]
    fn len(&self) -> usize {
        N
    }
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        f64::from(self[i])
    }
}

/// [`assert_finite`] for any heights.
pub(crate) fn assert_finite_heights(what: &str, v: &(impl Heights + ?Sized)) {
    if let Some(p) = (0..v.len()).find(|&i| !v.at(i).is_finite()) {
        panic!("{what} holds a value that is not finite at cell {p} ({})", v.at(p));
    }
}

pub(crate) fn assert_finite(what: &str, v: &[f64]) {
    if let Some(p) = v.iter().position(|x| !x.is_finite()) {
        panic!("{what} holds a value that is not finite at cell {p} ({})", v[p]);
    }
}
