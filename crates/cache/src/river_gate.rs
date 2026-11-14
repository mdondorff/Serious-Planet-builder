//! The river gate (BUILD-055): the numbers every river candidate of P3 is judged by, on one surface at L against the start surface. The definitions come from SPIKE-114's and SPIKE-115's tools
//! (`rivers_at_l`, `erosion_at_l`) with the limits agreed by crusader and spikes for SPIKE-117 and SPIKE-118 (2026-11-14). The measure is a pure function of its input: the `river_gate` example of
//! this crate builds the input from a window or a store, and `cargo xtask river-gate` runs it under the heavy lock.
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::route::{route_rules_filled, RoundRules, Routing, NONE};
use planet_generators::bake::Grid;

/// The routing's flood grade between cells (the spikes' 1e-3 m).
pub const ROUTE_EPSILON_M: f64 = 1.0e-3;
/// The basin flood's grade between cells (SPIKE-117/118's 1e-7 m): its graded flats stand far below [`BASIN_WET_M`].
pub const BASIN_EPSILON_M: f64 = 1.0e-7;
/// A cell lies in a basin when the basin flood stands more than this above it (a flat graded at 1e-7 m a cell reaches it only after 10,000 cells).
pub const BASIN_WET_M: f64 = 1.0e-3;
/// SPIKE-114's reference: a river cell in a pit of the routing's flood standing more than 5 cm above it, with no depth or cell limit.
pub const REFERENCE_WET_M: f64 = 0.05;
/// The valley floor's cross-section around a river cell: the cells 2 to 4 cells out (Chebyshev distance).
pub const VALLEY_RING: (i64, i64) = (2, 4);

/// The gate's limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateParams {
    /// A closed basin is a pit at least this deep (metres)
    pub basin_depth_m: f64,
    /// and at least this many cells.
    pub basin_cells: usize,
    /// A basin deeper than D stays a lake; the others are breached (metres).
    pub lake_depth_m: f64,
    /// A river cell more than this below its valley (the canyon) or below the start surface (the erosion depth) is counted (metres).
    pub cut_m: f64,
    /// A cell is a river cell when its drainage area is at least this (square metres) and it is land.
    pub channel_area_m2: f64,
    /// An H1 crossing of the region's edge is matched by an L crossing of the same direction within this distance (metres)
    pub edge_m: f64,
    /// of the H1 crossings carrying at least this discharge (m3/s).
    pub edge_q_m3s: f64,
}

impl Default for GateParams {
    fn default() -> GateParams {
        GateParams {
            basin_depth_m: 0.5,
            basin_cells: 16,
            lake_depth_m: 20.0,
            cut_m: 50.0,
            channel_area_m2: planet_generators::bake::hydro::HydroParams::default().initiation_m2,
            edge_m: 2000.0,
            edge_q_m3s: 1.0,
        }
    }
}

/// A river crossing the region's edge, in the grid's cell coordinates: `inside` is the side the river comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub inside: bool,
    pub x: f64,
    pub y: f64,
    pub q_m3s: f64,
}

/// What the gate measures on: one grid at L.
pub struct GateInput<'a> {
    pub grid: &'a Grid,
    /// The start surface (before the candidate's stage) and the candidate's surface, metres, one per cell.
    pub start: &'a [f64],
    pub surface: &'a [f64],
    /// The outlets of the routing (the sea and the window's edge).
    pub outlet: &'a [bool],
    /// The cells the numbers count: the region (its commit's tiles, or its box).
    pub inside: &'a [bool],
    /// The region's box for the edge crossings (a cell inside it or not).
    pub region: &'a [bool],
    /// H1's crossings of the region's edge; empty when the input has none (the edge row then says so).
    pub h1: &'a [Crossing],
    /// The routing's jitter seed.
    pub seed: u64,
}

/// The p99 and the maximum of a set of values (0 for none).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Tail {
    pub count_over: usize,
    pub p50_m: f64,
    pub p99_m: f64,
    pub max_m: f64,
}

fn tail(mut v: Vec<f64>, over: f64) -> Tail {
    v.sort_by(f64::total_cmp);
    let at = |f: f64| v.get(((v.len() as f64 * f) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0.0);
    Tail { count_over: v.iter().filter(|&&x| x > over).count(), p50_m: at(0.5), p99_m: at(0.99), max_m: v.last().copied().unwrap_or(0.0) }
}

/// The gate's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct GateReport {
    pub params: GateParams,
    /// River cells inside the region.
    pub river_cells: usize,
    /// Closed basins in the window, and the river cells inside that lie in one.
    pub basins: usize,
    pub basin_river_cells: usize,
    /// SPIKE-114's reference count: river cells inside in any pit of 5 cm.
    pub reference_river_cells: usize,
    /// River cells inside that lie in a basin deeper than D (lakes).
    pub lake_river_cells: usize,
    /// The canyon: the river level below the p50 of its valley ring, with the lakes of D kept and with every basin breached.
    pub canyon: Tail,
    pub canyon_breached: Tail,
    /// The erosion depth: the start surface minus the river level, with the lakes of D kept.
    pub erosion: Tail,
    /// The p50 of the height range over land blocks of 2 x 2 and 10 x 10 cells inside, on the start and on the surface (metres).
    pub relief_2_start_m: f64,
    pub relief_2_m: f64,
    pub relief_10_start_m: f64,
    pub relief_10_m: f64,
    /// H1 crossings of the edge (Q at least the limit), and those an L crossing matches.
    pub edge_h1: usize,
    pub edge_matched: usize,
}

fn share(n: usize, of: usize) -> f64 {
    100.0 * n as f64 / of.max(1) as f64
}

fn percent(v: f64, of: f64) -> f64 {
    if of > 0.0 {
        100.0 * v / of
    } else {
        0.0
    }
}

impl GateReport {
    pub fn basin_share(&self) -> f64 {
        share(self.basin_river_cells, self.river_cells)
    }
    pub fn lake_share(&self) -> f64 {
        share(self.lake_river_cells, self.river_cells)
    }
    pub fn canyon_share(&self) -> f64 {
        share(self.canyon.count_over, self.river_cells)
    }

    /// The printed report: one line per measure, the same bits for the same input.
    pub fn lines(&self) -> Vec<String> {
        let p = &self.params;
        let edge = if self.edge_h1 == 0 {
            "RIVER-GATE edge: no H1 crossing of the region's edge in the input".to_string()
        } else {
            format!(
                "RIVER-GATE edge: {} of {} H1 crossings (Q >= {} m3/s) have an L crossing of the same direction within {:.0} m ({:.1} %)",
                self.edge_matched,
                self.edge_h1,
                p.edge_q_m3s,
                p.edge_m,
                share(self.edge_matched, self.edge_h1)
            )
        };
        let (r0, r1) = VALLEY_RING;
        vec![
            format!("RIVER-GATE rivers: {} river cells inside (drainage area >= {:.0} km2, land)", self.river_cells, p.channel_area_m2 / 1.0e6),
            format!(
                "RIVER-GATE basins: {:.2} % of river length in closed basins ({} of {} cells; {} basins of >= {} m depth and >= {} cells, fill epsilon {BASIN_EPSILON_M:e} m; bar <= 10 %); SPIKE-114 reference (any pit of {REFERENCE_WET_M} m): {:.2} %",
                self.basin_share(),
                self.basin_river_cells,
                self.river_cells,
                self.basins,
                p.basin_depth_m,
                p.basin_cells,
                share(self.reference_river_cells, self.river_cells)
            ),
            format!(
                "RIVER-GATE canyon: {:.2} % of river cells over {} m below the p50 of their valley ({r0} to {r1} cells out) at D {} m ({} cells, p99 {:.2} m, max {:.2} m; bar <= 1 %); every basin breached {:.2} % ({} cells, max {:.2} m)",
                self.canyon_share(),
                p.cut_m,
                p.lake_depth_m,
                self.canyon.count_over,
                self.canyon.p99_m,
                self.canyon.max_m,
                share(self.canyon_breached.count_over, self.river_cells),
                self.canyon_breached.count_over,
                self.canyon_breached.max_m
            ),
            format!(
                "RIVER-GATE erosion depth: the river level under the start surface at D {} m p50 {:.2} p99 {:.2} max {:.2} m, {} cells over {} m ({:.2} %)",
                p.lake_depth_m,
                self.erosion.p50_m,
                self.erosion.p99_m,
                self.erosion.max_m,
                self.erosion.count_over,
                p.cut_m,
                share(self.erosion.count_over, self.river_cells)
            ),
            format!("RIVER-GATE lakes: {:.2} % of river cells in lakes at D {} m ({} cells)", self.lake_share(), p.lake_depth_m, self.lake_river_cells),
            format!(
                "RIVER-GATE relief: p50 over 2 x 2 cells {:.2} m against the start's {:.2} m ({:.0} %), over 10 x 10 cells {:.2} m against {:.2} m ({:.0} %)",
                self.relief_2_m,
                self.relief_2_start_m,
                percent(self.relief_2_m, self.relief_2_start_m),
                self.relief_10_m,
                self.relief_10_start_m,
                percent(self.relief_10_m, self.relief_10_start_m)
            ),
            edge,
        ]
    }
}

/// The pits of `z` under its flood `filled`: the 8-connected components of the cells with `filled - z > wet`, each cell's component (or `u32::MAX`), and each component's depth and cells.
fn pits(grid: &Grid, z: &[f64], filled: &[f64], wet: f64) -> (Vec<u32>, Vec<(f64, usize)>) {
    let (w, h) = (grid.w(), grid.h());
    let is_wet = |c: usize| filled[c] - z[c] > wet;
    let mut label = vec![u32::MAX; w * h];
    let mut pits: Vec<(f64, usize)> = Vec::new();
    let mut stack = Vec::new();
    for s in 0..w * h {
        if !is_wet(s) || label[s] != u32::MAX {
            continue;
        }
        let id = pits.len() as u32;
        let (mut depth, mut cells) = (0f64, 0usize);
        label[s] = id;
        stack.push(s);
        while let Some(c) = stack.pop() {
            depth = depth.max(filled[c] - z[c]);
            cells += 1;
            let (cx, cy) = ((c % w) as i64, (c / w) as i64);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let q = ny as usize * w + nx as usize;
                    if label[q] == u32::MAX && is_wet(q) {
                        label[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
        pits.push((depth, cells));
    }
    (label, pits)
}

/// The drainage area of every cell along the receivers (square metres).
fn areas(grid: &Grid, routing: &Routing) -> Vec<f64> {
    let cell = grid.dx() * grid.dx();
    let mut area = vec![cell; grid.len()];
    for &c in routing.order.iter().rev() {
        let r = routing.receiver[c as usize];
        if r != NONE {
            area[r as usize] += area[c as usize];
        }
    }
    area
}

/// The river levels: walking downstream, a river cell's level is the lowest drained height on its way so far (a breached basin's cell drains to its own height, a lake's to its water).
fn levels(routing: &Routing, river: &[bool], drained: impl Fn(usize) -> f64) -> Vec<f64> {
    let mut level = vec![f64::INFINITY; river.len()];
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        if !river[c] {
            continue;
        }
        let v = level[c].min(drained(c));
        level[c] = v;
        let r = routing.receiver[c];
        if r != NONE && river[r as usize] {
            level[r as usize] = level[r as usize].min(v);
        }
    }
    level
}

/// The p50 of the surface over the valley ring of cell `c` (the cells of the grid at a Chebyshev distance in [`VALLEY_RING`]).
fn valley_floor(grid: &Grid, z: &[f64], c: usize) -> f64 {
    let (w, h) = (grid.w() as i64, grid.h() as i64);
    let (cx, cy) = ((c % grid.w()) as i64, (c / grid.w()) as i64);
    let (r0, r1) = VALLEY_RING;
    let mut ring: Vec<f64> = Vec::with_capacity(((2 * r1 + 1) * (2 * r1 + 1)) as usize);
    for dy in -r1..=r1 {
        for dx in -r1..=r1 {
            let (x, y) = (cx + dx, cy + dy);
            if dx.abs().max(dy.abs()) >= r0 && x >= 0 && y >= 0 && x < w && y < h {
                ring.push(z[(y * w + x) as usize]);
            }
        }
    }
    ring.sort_by(f64::total_cmp);
    ring.get(ring.len() / 2).copied().unwrap_or(z[c])
}

/// The p50 of the height range of the land blocks of `k x k` cells that lie inside (blocks tiled from the grid's corner).
fn relief(grid: &Grid, z: &[f64], inside: &[bool], k: usize) -> f64 {
    let (w, h) = (grid.w(), grid.h());
    let mut r: Vec<f64> = Vec::new();
    for by in (0..h.saturating_sub(k - 1)).step_by(k) {
        for bx in (0..w.saturating_sub(k - 1)).step_by(k) {
            let (mut lo, mut hi, mut all_inside) = (f64::INFINITY, f64::NEG_INFINITY, true);
            for y in by..by + k {
                for x in bx..bx + k {
                    let c = y * w + x;
                    all_inside &= inside[c];
                    lo = lo.min(z[c]);
                    hi = hi.max(z[c]);
                }
            }
            if all_inside && lo > 0.0 {
                r.push(hi - lo);
            }
        }
    }
    r.sort_by(f64::total_cmp);
    r.get(r.len() / 2).copied().unwrap_or(0.0)
}

/// The H1 crossings of at least the discharge limit that an L crossing of the same direction matches within the distance: (matched, counted).
pub fn edge_agreement(l: &[Crossing], h1: &[Crossing], dx_m: f64, params: &GateParams) -> (usize, usize) {
    let big: Vec<&Crossing> = h1.iter().filter(|c| c.q_m3s >= params.edge_q_m3s).collect();
    let matched = big
        .iter()
        .filter(|hc| l.iter().filter(|c| c.inside == hc.inside).any(|c| libm::hypot(c.x - hc.x, c.y - hc.y) * dx_m <= params.edge_m))
        .count();
    (matched, big.len())
}

/// Measures `input` under `params`: routes the surface at L, finds the closed basins and the river cells, the levels for every basin breached and for the lakes of D, the canyon against the valley,
/// the erosion depth against the start surface, the relief and the edge.
pub fn measure(input: &GateInput, params: &GateParams) -> GateReport {
    let g = input.grid;
    let n = g.len();
    assert!(
        [input.start.len(), input.surface.len(), input.outlet.len(), input.inside.len(), input.region.len()].iter().all(|&l| l == n),
        "one start height, surface height, outlet, inside and region flag per cell ({n} cells)"
    );
    let z = input.surface;
    let (routing, route_filled) = route_rules_filled(g, z, input.outlet, ROUTE_EPSILON_M, input.seed, 0.0, None, RoundRules::default());
    let area = areas(g, &routing);
    let river: Vec<bool> = (0..n).map(|c| area[c] >= params.channel_area_m2 && z[c] > 0.0).collect();
    let (reference, _) = pits(g, z, &route_filled, REFERENCE_WET_M);
    drop(route_filled);
    let filled = priority_flood_with(g, z, input.outlet, BASIN_EPSILON_M, false);
    let (label, pits) = pits(g, z, &filled, BASIN_WET_M);
    let is_basin = |&(d, k): &(f64, usize)| d >= params.basin_depth_m && k >= params.basin_cells;
    let basin = |c: usize| label[c] != u32::MAX && is_basin(&pits[label[c] as usize]);
    let lake = |c: usize| basin(c) && pits[label[c] as usize].0 > params.lake_depth_m;
    let breached = levels(&routing, &river, |c| z[c]);
    let with_lakes = levels(&routing, &river, |c| if lake(c) { filled[c] } else { z[c] });
    let (mut river_cells, mut basin_cells, mut reference_cells, mut lake_cells) = (0usize, 0usize, 0usize, 0usize);
    let (mut canyon, mut canyon_b, mut erosion) = (Vec::new(), Vec::new(), Vec::new());
    for c in 0..n {
        if !river[c] || !input.inside[c] {
            continue;
        }
        river_cells += 1;
        basin_cells += usize::from(basin(c));
        reference_cells += usize::from(reference[c] != u32::MAX);
        lake_cells += usize::from(lake(c));
        let floor = valley_floor(g, z, c);
        canyon.push(floor - with_lakes[c]);
        canyon_b.push(floor - breached[c]);
        erosion.push(input.start[c] - with_lakes[c]);
    }
    let w = g.w();
    let mut l_cross: Vec<Crossing> = Vec::new();
    for c in 0..n {
        let r = routing.receiver[c];
        if river[c] && r != NONE && input.region[c] != input.region[r as usize] {
            l_cross.push(Crossing { inside: input.region[c], x: (c % w) as f64, y: (c / w) as f64, q_m3s: 0.0 });
        }
    }
    let (edge_matched, edge_h1) = edge_agreement(&l_cross, input.h1, g.dx(), params);
    GateReport {
        params: *params,
        river_cells,
        basins: pits.iter().filter(|p| is_basin(p)).count(),
        basin_river_cells: basin_cells,
        reference_river_cells: reference_cells,
        lake_river_cells: lake_cells,
        canyon: tail(canyon, params.cut_m),
        canyon_breached: tail(canyon_b, params.cut_m),
        erosion: tail(erosion, params.cut_m),
        relief_2_start_m: relief(g, input.start, input.inside, 2),
        relief_2_m: relief(g, z, input.inside, 2),
        relief_10_start_m: relief(g, input.start, input.inside, 10),
        relief_10_m: relief(g, z, input.inside, 10),
        edge_h1,
        edge_matched,
    }
}
