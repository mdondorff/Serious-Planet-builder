//! The river gate (BUILD-055): the numbers every river candidate of P3 is judged by, on one surface at L against the start surface. The definitions come from SPIKE-114's and SPIKE-115's tools
//! (`rivers_at_l`, `erosion_at_l`) with the limits agreed by crusader and spikes for SPIKE-117 and SPIKE-118 (2026-11-14). The measure is a pure function of its input: the `river_gate` example of
//! this crate builds the input from a window or a store, and `cargo xtask river-gate` runs it under the heavy lock.
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::route::{route_rules_filled, MfdClip, RoundRules, Routing, NONE};
use planet_generators::bake::Grid;

use crate::river_gate_graph::{
    agreement, bars_line, budget_field, budget_report, g2_bars, graph_report, kept_interiors, kept_steps, partition_report, rim_mismatch,
    Bar, BudgetReport, G2Input, G2Report, CORRIDOR_MIN_PCT, CORRIDOR_MIN_PCT_MOUNTAIN,
};
use crate::river_suite::{
    corridor_bar_m, measure_suite, network, ridge_cells, ridge_report, trunk_burn, trunk_corridor, Network, RidgeReport, Snap, SuiteInput,
    SuiteReport, TrunkReport, CORRIDOR_K, NETWORK_KM2, SADDLE_KM2,
};

/// The crest rounding bar (lead #172; review B Q5): the p95 of the ridge lowering against `S_KH` at most this (metres), and on a mountain window (M1).
pub const CREST_P95_MAX_M: f64 = 15.0;
pub const CREST_P95_MAX_M_MOUNTAIN: f64 = 35.0;
/// The gate's outlets (BUILD-055, lead 2026-11-23, the chain's connected sea mask of readings 12 and 39): the window's edge cells, and the ocean cells still at or under
/// `sea` on the measured surface `surface`. The ocean is decided on `z`, the window's start surface, as the chain decides it: the cells at or under `sea` joined through
/// such cells (8-connected) to an edge cell or to a `seed` cell (the L cells of an H1 sea cell inside the region). An inland cell under the sea level is no outlet, and its
/// pit fills; a start-sea cell the variant raised above the sea is land; a sea cut off by a raised coast stays sea.
pub fn connected_sea_outlets(
    grid: &planet_generators::bake::Grid,
    z: &[f64],
    surface: &[f64],
    sea: f64,
    seed: &dyn Fn(usize) -> bool,
) -> Vec<bool> {
    let n = z.len();
    let mut out: Vec<bool> = (0..n).map(|p| grid.is_edge(p)).collect();
    let mut stack: Vec<usize> = (0..n).filter(|&p| (grid.is_edge(p) || seed(p)) && z[p] <= sea).collect();
    let mut seen = vec![false; n];
    for &p in &stack {
        seen[p] = true;
    }
    while let Some(c) = stack.pop() {
        out[c] = out[c] || surface[c] <= sea;
        for k in 0..8 {
            if let Some(q) = grid.neighbour(c, k) {
                if !seen[q] && z[q] <= sea {
                    seen[q] = true;
                    stack.push(q);
                }
            }
        }
    }
    out
}

/// The cells of `a` outside `b`, and their 8-connected components (lead #185b: `g_ocean_extra` is the chain's `ocean.u8` outside the gate's own ocean, `g_ocean_missing` the reverse; a cross-check, 0 on a chain row).
pub fn mask_difference(grid: &Grid, a: &[bool], b: &[bool]) -> (usize, usize) {
    let n = a.len();
    let diff: Vec<bool> = (0..n).map(|p| a[p] && !b[p]).collect();
    let mut seen = vec![false; n];
    let (mut cells, mut comps) = (0usize, 0usize);
    for s in 0..n {
        if !diff[s] || seen[s] {
            continue;
        }
        comps += 1;
        seen[s] = true;
        let mut stack = vec![s];
        while let Some(c) = stack.pop() {
            cells += 1;
            for k in 0..8 {
                if let Some(q) = grid.neighbour(c, k) {
                    if diff[q] && !seen[q] {
                        seen[q] = true;
                        stack.push(q);
                    }
                }
            }
        }
    }
    (cells, comps)
}

/// The ponds bar (BUILD-060, owner 2026-11-20, option C): river length in closed basins outside the kept lakes, over the river length outside them, at most this (percent).
pub const PONDS_MAX_PCT: f64 = 10.0;
/// The finer river network of the uphill and ponds bars (G3-1, lead #186, A4): river cells drain at least this (km2), beside the 75 km2 network.
pub const FINE_NETWORK_KM2: f64 = 1.0;
/// The bars on the finer network (G3-1): uphill steps at most `UPHILL_MAX_PCT`, as on the 75 km2 network, and ponds at most this (percent; proposed for the owner, O10).
pub const PONDS_FINE_MAX_PCT: f64 = 10.0;
/// G3-1 (lead #186): hump cuts against `S0` ([`crate::river_suite::hump_cuts`]) per 10,000 km2 of counted land, at most this (B's row: 0.04 to 0.24 on the five windows,
/// rounded up to 1); and corner-only links over 5 m that the surface adds to `S0`'s ([`crate::river_suite::corner_links`]), per 10,000 km2, at most this (both proposed for
/// the owner, O10).
pub const HUMP_CUTS_MAX_PER_10K_KM2: f64 = 1.0;
pub const CORNER_LINKS_MAX_PER_10K_KM2: f64 = 10.0;
/// The kept-river bar: river length in kept lakes, over all river length, at most this (percent) on a lowland window and `KEPT_RIVER_MAX_PCT_MOUNTAIN` on a mountain one.
pub const KEPT_RIVER_MAX_PCT: f64 = 5.0;
pub const KEPT_RIVER_MAX_PCT_MOUNTAIN: f64 = 10.0;
/// A kept basin whose depression level on the measured surface lies more than this under its recorded spill is a spill mismatch (metres): a graph violation.
pub const SPILL_MISMATCH_M: f64 = 0.5;

/// The basin flood's grade between cells (SPIKE-117/118's 1e-7 m): its graded flats stand far below [`BASIN_WET_M`].
pub const BASIN_EPSILON_M: f64 = 1.0e-7;
/// The routing's flood grade between cells (lead #185c): a flat drains by GEN-173's grade (distance to its exit), and a flat as wide as a window (8,160 cells) rises 8 cm, under the uphill
/// step (`UPHILL_M`). The spikes' 1e-3 m lifted a flat 1 m over 1,000 cells, and R3a's coastal flats drained onto land up to 2.44 m higher. It is the smallest grade whose regrade step
/// (1e-6 of the grade) stays above the f64 spacing of heights up to about 9 km; the basin grade (1e-7 m) left a cell with no lower neighbour on B's R3a store.
pub const ROUTE_EPSILON_M: f64 = 1.0e-5;
/// The routing's flood (lead #185c): the canonical flood with GEN-173's Euclidean grade of the flats.
pub const ROUTE_RULES: RoundRules = RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true };
/// A cell lies in a basin when the basin flood stands more than this above it (a flat graded at 1e-7 m a cell reaches it only after 10,000 cells).
pub const BASIN_WET_M: f64 = 1.0e-3;
/// SPIKE-114's reference: a river cell in a pit of the routing's flood standing more than 5 cm above it, with no depth or cell limit.
pub const REFERENCE_WET_M: f64 = 0.05;
/// The valley floor's cross-section around a river cell: the cells 2 to 4 cells out (Chebyshev distance).
pub const VALLEY_RING: (i64, i64) = (2, 4);
/// The near ring of the valley depth (the canyon a viewer sees): the cells 1 to 2 cells out (102-204 m at L; lead, 2026-11-14, DEC crusader-4).
pub const NEAR_RING: (i64, i64) = (1, 2);

/// The relief index of the relief-indexed bars (G3-1, lead #186, A5; owner 2026-11-21): the window's start relief at 10 x 10 cells (`relief10_start_m`). A bar holds its lowland value
/// up to `RELIEF_LOWLAND_M` and its mountain value from `RELIEF_MOUNTAIN_M`, linear in the logarithm of the relief between (both endpoints proposed by the lead for the owner, O10).
pub const RELIEF_LOWLAND_M: f64 = 60.0;
pub const RELIEF_MOUNTAIN_M: f64 = 300.0;

/// The weight of the mountain value at start relief `relief_m` (0 at or under `RELIEF_LOWLAND_M`, 1 at or over `RELIEF_MOUNTAIN_M`).
pub fn relief_weight(relief_m: f64) -> f64 {
    // a window without land samples (NaN relief) takes the lowland values
    if relief_m.is_nan() || relief_m <= RELIEF_LOWLAND_M {
        0.0
    } else if relief_m >= RELIEF_MOUNTAIN_M {
        1.0
    } else {
        libm::log(relief_m / RELIEF_LOWLAND_M) / libm::log(RELIEF_MOUNTAIN_M / RELIEF_LOWLAND_M)
    }
}

/// A relief-indexed limit: `lowland` and `mountain` weighted by [`relief_weight`].
pub fn relief_indexed(lowland: f64, mountain: f64, relief_m: f64) -> f64 {
    let t = relief_weight(relief_m);
    lowland + (mountain - lowland) * t
}

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
    /// The H1 cell size (metres): a trunk corridor's radius is `k_c` of it.
    pub h1_cell_m: f64,
    /// The channel thresholds of the network measure (BUILD-059), km2.
    pub network_km2: [f64; 2],
    /// The corridor bar of the G2 bars (lead #168): the share of snapped k_c 2 trunk reaches passing, percent; its lowland value (95 at mountain relief), unless `corridor_fixed`.
    pub corridor_min_pct: f64,
    /// The crest rounding bar: the p95 of the ridge lowering, metres; its lowland value (35 at mountain relief).
    pub crest_max_m: f64,
    /// The kept-river bar (percent); its lowland value (10 at mountain relief).
    pub kept_river_max_pct: f64,
    /// G3-1: the three limits above are relief-indexed ([`relief_indexed`]) by the row's start relief; off, each is the value given.
    pub relief_indexed: bool,
    /// The corridor bar given by hand (`--corridor-bar`): `corridor_min_pct` as it is, not relief-indexed.
    pub corridor_fixed: bool,
    /// G3-1 (lead review of #1648): the relief index, the window's own start relief ([`relief_index`] of main's S0 over the region's box), the same for every candidate on the
    /// window; without it the row's start relief (`relief10_start_m`).
    pub relief_index_m: Option<f64>,
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
            h1_cell_m: 814.0,
            network_km2: NETWORK_KM2,
            corridor_min_pct: CORRIDOR_MIN_PCT,
            crest_max_m: CREST_P95_MAX_M,
            kept_river_max_pct: KEPT_RIVER_MAX_PCT,
            relief_indexed: true,
            corridor_fixed: false,
            relief_index_m: None,
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
    /// The sea level (metres): land is above it.
    pub sea_level_m: f64,
    /// Every cell outside `inside` is an outlet: a store's tiles drain to their border, so the start surface around them (without the candidate's burn) dams no river leaving the tiles.
    pub outside_drains: bool,
    /// H1's trunk reaches in the grid's cells, each upstream first (empty when the input has none; the trunk line then says so).
    pub trunks: &'a [Vec<(f64, f64)>],
    /// The G2 inputs (BUILD-060): a canonical river graph, the per-stage budget fields, a partition pair; `None` for a row without them (B).
    pub g2: Option<&'a G2Input<'a>>,
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

/// The column names of one corridor form (`trunk_c<k>[s]_...`; `s` the snapped form).
macro_rules! corridor_names {
    ($p:literal) => {
        [
            concat!("trunk_", $p, "_reaches"),
            concat!("trunk_", $p, "_reaches_over"),
            concat!("trunk_", $p, "_reaches_passing_pct"),
            concat!("trunk_", $p, "_vertices"),
            concat!("trunk_", $p, "_vertices_over"),
            concat!("trunk_", $p, "_vertices_over_pct"),
            concat!("trunk_", $p, "_deepest_m"),
            concat!("trunk_", $p, "_deepest_reach"),
        ]
    };
}

/// The column names of the corridor trunk measure: one row per [`CORRIDOR_K`] (k_c 1, 2, 4) with raw H1 ends, then the same snapped (lead #159).
const CORRIDOR_FIELDS: [[&str; 8]; 6] = [
    corridor_names!("c1"),
    corridor_names!("c2"),
    corridor_names!("c4"),
    corridor_names!("c1s"),
    corridor_names!("c2s"),
    corridor_names!("c4s"),
];

/// A raw grid's cell size (SPIKE-119's prepared GLO-30 tiles, metres) and its counted border (cells; BUILD-059).
pub const GRID_CELL_M: f64 = 101.8;
pub const GRID_BORDER: usize = 10;

/// A raw height grid input (BUILD-059): `file,w,h,sea_level_m`.
#[derive(Clone, Debug, PartialEq)]
pub struct RawGrid {
    pub file: String,
    pub w: usize,
    pub h: usize,
    pub sea_level_m: f64,
}

/// Parses `file,w,h,sea_level_m` (the file may hold commas); refuses a grid not wider and higher than twice [`GRID_BORDER`].
pub fn raw_grid_arg(arg: &str) -> Result<RawGrid, String> {
    let usage = || format!("--grid file,w,h,sea_level_m, not '{arg}'");
    let p: Vec<&str> = arg.rsplitn(4, ',').collect();
    let [sea, h, w, file] = p[..] else { return Err(usage()) };
    let g = RawGrid {
        file: file.to_string(),
        w: w.parse().map_err(|_| usage())?,
        h: h.parse().map_err(|_| usage())?,
        sea_level_m: sea.parse().map_err(|_| usage())?,
    };
    if g.file.is_empty() || g.w <= 2 * GRID_BORDER || g.h <= 2 * GRID_BORDER {
        return Err(format!(
            "--grid {} x {}: a file and a grid wider and higher than twice the {GRID_BORDER}-cell border, not '{arg}'",
            g.w, g.h
        ));
    }
    Ok(g)
}

/// The heights of a raw grid: `w x h` little-endian f64, row-major; refused when the byte count differs.
pub fn raw_grid_heights(bytes: &[u8], w: usize, h: usize) -> Result<Vec<f64>, String> {
    if bytes.len() != w * h * 8 {
        return Err(format!("{} bytes, {w} x {h} f64 heights are {}", bytes.len(), w * h * 8));
    }
    Ok(bytes.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect())
}

/// The counted cells of a raw grid: every cell at least `border` cells from each edge.
pub fn border_inside(w: usize, h: usize, border: usize) -> Vec<bool> {
    (0..w * h)
        .map(|c| (border..w.saturating_sub(border)).contains(&(c % w)) && (border..h.saturating_sub(border)).contains(&(c / w)))
        .collect()
}

/// The column names of the network measure (BUILD-059): one row per channel threshold of `GateParams::network_km2`.
const NETWORK_FIELDS: [[&str; 7]; 2] = [
    ["net1_km2", "net1_channel_km", "net1_land_km2", "net1_density_km_per_km2", "net1_streams_by_order", "net1_rb_orders", "net1_rb"],
    ["net2_km2", "net2_channel_km", "net2_land_km2", "net2_density_km_per_km2", "net2_streams_by_order", "net2_rb_orders", "net2_rb"],
];

/// The gate's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct GateReport {
    pub params: GateParams,
    /// River cells inside the region.
    pub river_cells: usize,
    /// Closed basins in the window, and the river cells inside that lie in one.
    pub basins: usize,
    pub basin_river_cells: usize,
    /// With a graph (BUILD-060, owner 2026-11-20, option C; `None` without one): the river cells inside in a closed basin outside every kept-lake interior (the ponds), the
    /// river cells inside in a kept-lake interior (BUILD-060's kept-basin interior), the interiors' cells inside, and the kept basins whose depression level at the floor lies
    /// more than `SPILL_MISMATCH_M` under the recorded spill.
    pub ponds_river_cells: Option<usize>,
    pub kept_river_cells: Option<usize>,
    pub kept_lake_cells: Option<usize>,
    /// The 1 km2 network (G3-1, [`FINE_NETWORK_KM2`]): river cells inside on land, their river steps and uphill steps outside kept-basin interiors, and the river cells outside
    /// the interiors with those of them in a closed basin (on a row without a graph every river cell is outside an interior).
    pub fine: FineReport,
    /// G3-1: the window's margin outside the region's box ([`MarginReport`]).
    pub margin: MarginReport,
    /// G3-1 (lead #186): hump cuts of the surface against `S0`, and against `S_KH` on a row with KH's stage field (printed beside, not barred); corner-only links over 5 m on
    /// the surface and on `S0`, over the counted cells.
    pub hump_cuts: usize,
    pub hump_cuts_s_kh: Option<usize>,
    /// Lead review of #1648: S0's humps over 10 m and those the surface lowers by more than 10 m at a crest ([`crate::river_suite::humps_removed`]); reported, not barred.
    pub humps: (usize, usize),
    pub corner_links: usize,
    pub corner_links_s0: usize,
    pub spill_mismatch: Option<usize>,
    /// With a graph: test (a), the kept basins whose rim holds an L vertex with its bed more than `SPILL_MISMATCH_M` under the recorded spill ([`rim_mismatch`]).
    pub spill_rim_mismatch: Option<usize>,
    /// With a graph (lead #183, moved from the spike by lead #192): the kept basins with their floor in the window, by how far their recorded spill sits above the
    /// measured surface's pour level at the floor (the basin flood's level; over 0.01, 0.1 and 0.5 m: a notch cut partly below the record), the largest gap (m), and the
    /// dry ones (the flood stands no more than `BASIN_WET_M` over the floor).
    pub spill_notch: Option<(usize, [usize; 3], f64, usize)>,
    /// With a graph (lead #182a): the lake_inlet nodes by origin and the line-end ones in no kept-basin depression ([`crate::river_gate_graph::InletReport`]).
    pub inlets: Option<crate::river_gate_graph::InletReport>,
    /// The area of an L cell (m2), for the kept lakes' area.
    pub cell_area_m2: f64,
    /// SPIKE-114's reference count: river cells inside in any pit of 5 cm.
    pub reference_river_cells: usize,
    /// River cells inside that lie in a basin deeper than D (lakes).
    pub lake_river_cells: usize,
    /// The valley depth (the canyon a viewer sees; lead, 2026-11-14, DEC crusader-4): the p50 of the surface 1 to 2 cells out ([`NEAR_RING`]) above the surface
    /// at the river cell.
    pub valley: Tail,
    /// The burn (K7's cut): the surface at the river cell above the river level, with every basin breached and with the lakes of D kept.
    pub burn_breached: Tail,
    pub burn: Tail,
    /// Ring 2-4 (the first canyon measure, kept as a line): the p50 of the surface 2 to 4 cells out ([`VALLEY_RING`]) above the river level, with the lakes of D
    /// kept and with every basin breached; it is the valley depth at 2 to 4 cells plus the burn.
    pub canyon: Tail,
    pub canyon_breached: Tail,
    /// The erosion depth: the start surface minus the river level, with the lakes of D kept.
    pub erosion: Tail,
    /// The p50 of the height range over land blocks of 2 x 2 and 10 x 10 cells inside, on the start and on the surface (metres).
    pub relief_2_start_m: f64,
    pub relief_2_m: f64,
    pub relief_10_start_m: f64,
    pub relief_10_m: f64,
    /// Land cells inside (the start above the sea) and those whose surface lies more than `cut_m` below the start (#160: ground lowered over 50 m).
    pub land_cells: usize,
    pub lowered_cells: usize,
    /// H1 crossings of the edge (Q at least the limit), and those an L crossing matches.
    pub edge_h1: usize,
    pub edge_matched: usize,
    /// The measurement suite (BUILD-057).
    pub suite: SuiteReport,
    /// H1's trunk reaches against the surface (BUILD-057): the vertices more than `cut_m` above their reach's running minimum; `trunks_given` says whether the input had reaches.
    pub trunk: TrunkReport,
    pub trunks_given: bool,
    /// The same along each reach's minimax route inside its corridor of `k_c x h1_cell_m`, one per [`CORRIDOR_K`] (lead #158), against the window's bar
    /// `corridor_bar_m` (#159): from H1's raw ends, and snapped (the nodes to the lowest cell within one H1 cell, the region's edge crossed at its lowest cell).
    pub trunk_corridors: Vec<TrunkReport>,
    pub trunk_corridors_snapped: Vec<TrunkReport>,
    pub corridor_bar_m: f64,
    /// The channel network at each of `params.network_km2` (BUILD-059): drainage density and the Strahler bifurcation ratio.
    pub networks: Vec<Network>,
    /// The G2 columns (BUILD-060), when the input names a graph, stages or a partition pair.
    pub g2: Option<G2Report>,
    /// The ridge measures (BUILD-060, lead #172) on the start's ridge mask: against `S_KH` (the start less KH's stage field, reading 21; the start itself without one,
    /// `ridge_vs_kh` false) and against the start `S0`.
    pub ridge: RidgeReport,
    pub ridge_s0: RidgeReport,
    pub ridge_vs_kh: bool,
}

/// The margin (G3-1, lead #186; Carbon M4.7): the window's cells outside the region's box, which no bar counts: river cells (75 km2 network) on land, those in a closed
/// basin and those in one outside the kept-basin interiors, river steps and uphill steps outside the interiors, and the interiors' cells. Reported, not barred.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MarginReport {
    pub cells: usize,
    pub river_cells: usize,
    pub basin_cells: usize,
    pub ponds_cells: usize,
    pub steps: usize,
    pub uphill_steps: usize,
    pub kept_lake_cells: usize,
}

/// The uphill and ponds counts on the 1 km2 network (G3-1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FineReport {
    pub river_cells: usize,
    pub steps: usize,
    pub uphill_steps: usize,
    pub outside_cells: usize,
    pub ponds_cells: usize,
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
    /// The ponds' share (BUILD-060, option C): river cells in closed basins outside the kept lakes, over the river cells outside them; `None` without a graph.
    pub fn ponds_pct(&self) -> Option<f64> {
        self.ponds_river_cells.zip(self.kept_river_cells).map(|(pd, k)| share(pd, self.river_cells - k))
    }

    pub fn basin_share(&self) -> f64 {
        share(self.basin_river_cells, self.river_cells)
    }
    pub fn lake_share(&self) -> f64 {
        share(self.lake_river_cells, self.river_cells)
    }
    pub fn canyon_share(&self) -> f64 {
        share(self.canyon.count_over, self.river_cells)
    }
    pub fn valley_share(&self) -> f64 {
        share(self.valley.count_over, self.river_cells)
    }
    pub fn burn_share(&self) -> f64 {
        share(self.burn_breached.count_over, self.river_cells)
    }

    /// Every number of the report as (column, value), counts beside their percentages: the columns of `cargo xtask river-gate --table` (BUILD-057). The names are fixed; a new number is a new column.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let s = &self.suite;
        let n = self.river_cells;
        let pct = |k: usize, of: usize| format!("{:.2}", share(k, of));
        let m = |v: f64| format!("{v:.2}");
        let r = |v: f64| format!("{v:.3}");
        let c = |k: usize| k.to_string();
        let mut f = vec![
            ("river_cells", c(n)),
            ("basins", c(self.basins)),
            ("basin_cells", c(self.basin_river_cells)),
            ("basin_pct", pct(self.basin_river_cells, n)),
            ("ponds_cells", self.ponds_river_cells.map_or("-".to_string(), c)),
            ("ponds_pct", self.ponds_pct().map_or("-".to_string(), |v| format!("{v:.2}"))),
            ("fine_river_cells", c(self.fine.river_cells)),
            ("fine_uphill_steps", c(self.fine.uphill_steps)),
            ("fine_uphill_pct", format!("{:.2}", share(self.fine.uphill_steps, self.fine.steps))),
            ("fine_ponds_cells", c(self.fine.ponds_cells)),
            ("fine_ponds_pct", format!("{:.2}", share(self.fine.ponds_cells, self.fine.outside_cells))),
            ("m_cells", c(self.margin.cells)),
            ("m_river_cells", c(self.margin.river_cells)),
            ("m_basin_cells", c(self.margin.basin_cells)),
            ("m_basin_pct", format!("{:.2}", share(self.margin.basin_cells, self.margin.river_cells))),
            ("m_ponds_cells", c(self.margin.ponds_cells)),
            ("m_ponds_pct", format!("{:.2}", share(self.margin.ponds_cells, self.margin.river_cells))),
            ("m_uphill_steps", c(self.margin.uphill_steps)),
            ("m_uphill_pct", format!("{:.2}", share(self.margin.uphill_steps, self.margin.steps))),
            ("m_kept_lake_km2", format!("{:.2}", self.margin.kept_lake_cells as f64 * self.cell_area_m2 / 1.0e6)),
            ("relief_index_m", m(self.relief_index())),
            ("humps", c(self.humps.0)),
            ("humps_removed", c(self.humps.1)),
            ("humps_removed_per_10k_km2", format!("{:.2}", self.per_10k_km2(self.humps.1 as f64))),
            ("hump_cuts", c(self.hump_cuts)),
            ("hump_cuts_s_kh", self.hump_cuts_s_kh.map_or("-".to_string(), c)),
            ("hump_cuts_per_10k_km2", format!("{:.2}", self.per_10k_km2(self.hump_cuts as f64))),
            ("corner_links", c(self.corner_links)),
            ("corner_links_s0", c(self.corner_links_s0)),
            ("corner_links_added_per_10k_km2", format!("{:.2}", self.per_10k_km2(self.corner_links as f64 - self.corner_links_s0 as f64))),
            ("kept_river_cells", self.kept_river_cells.map_or("-".to_string(), c)),
            ("kept_river_pct", self.kept_river_cells.map_or("-".to_string(), |k| pct(k, n))),
            ("kept_lake_cells", self.kept_lake_cells.map_or("-".to_string(), c)),
            ("kept_lake_km2", self.kept_lake_cells.map_or("-".to_string(), |k| format!("{:.2}", k as f64 * self.cell_area_m2 / 1.0e6))),
            ("g_spill_mismatch", self.spill_mismatch.map_or("-".to_string(), c)),
            ("g_spill_rim", self.spill_rim_mismatch.map_or("-".to_string(), c)),
            ("g_kept_in_window", self.spill_notch.map_or("-".to_string(), |x| x.0.to_string())),
            ("g_notch_over_0_01", self.spill_notch.map_or("-".to_string(), |x| x.1[0].to_string())),
            ("g_notch_over_0_1", self.spill_notch.map_or("-".to_string(), |x| x.1[1].to_string())),
            ("g_notch_over_0_5", self.spill_notch.map_or("-".to_string(), |x| x.1[2].to_string())),
            ("g_notch_max_m", self.spill_notch.map_or("-".to_string(), |x| format!("{:.2}", x.2))),
            ("g_kept_dry", self.spill_notch.map_or("-".to_string(), |x| x.3.to_string())),
            ("g_lake_inlets", self.inlets.map_or("-".to_string(), |x| x.inside.to_string())),
            ("g_inlet_line_ends", self.inlets.map_or("-".to_string(), |x| x.line_ends.to_string())),
            ("g_inlet_spill_targets", self.inlets.map_or("-".to_string(), |x| x.spill_targets.to_string())),
            ("g_inlet_orphans", self.inlets.map_or("-".to_string(), |x| x.orphans.to_string())),
            ("g_inlet_no_basin", self.inlets.map_or("-".to_string(), |x| x.line_end_no_basin.to_string())),
            ("g_inlet_no_basin_far", self.inlets.map_or("-".to_string(), |x| x.line_end_no_basin_far.to_string())),
            ("reference_cells", c(self.reference_river_cells)),
            ("reference_pct", pct(self.reference_river_cells, n)),
            ("valley_over_cells", c(self.valley.count_over)),
            ("valley_over_pct", pct(self.valley.count_over, n)),
            ("valley_p50_m", m(self.valley.p50_m)),
            ("valley_p99_m", m(self.valley.p99_m)),
            ("burn_over_cells", c(self.burn_breached.count_over)),
            ("burn_over_pct", pct(self.burn_breached.count_over, n)),
            ("burn_p99_m", m(self.burn_breached.p99_m)),
            ("burn_lakes_over_cells", c(self.burn.count_over)),
            ("burn_lakes_over_pct", pct(self.burn.count_over, n)),
            ("ring24_over_cells", c(self.canyon.count_over)),
            ("ring24_over_pct", pct(self.canyon.count_over, n)),
            ("ring24_breached_over_cells", c(self.canyon_breached.count_over)),
            ("ring24_breached_over_pct", pct(self.canyon_breached.count_over, n)),
            ("erosion_p50_m", m(self.erosion.p50_m)),
            ("erosion_p99_m", m(self.erosion.p99_m)),
            ("erosion_over_cells", c(self.erosion.count_over)),
            ("erosion_over_pct", pct(self.erosion.count_over, n)),
            ("lake_cells", c(self.lake_river_cells)),
            ("lake_pct", pct(self.lake_river_cells, n)),
            ("relief2_m", m(self.relief_2_m)),
            ("relief2_start_m", m(self.relief_2_start_m)),
            ("relief10_m", m(self.relief_10_m)),
            ("relief10_start_m", m(self.relief_10_start_m)),
            // lead #172: the relief lost against the start (positive: lost), and the spectrum slope's change
            ("relief2_lost_m", m(self.relief_2_start_m - self.relief_2_m)),
            ("relief10_lost_m", m(self.relief_10_start_m - self.relief_10_m)),
            ("land_cells", c(self.land_cells)),
            ("lowered_cells", c(self.lowered_cells)),
            ("lowered_pct", pct(self.lowered_cells, self.land_cells)),
            ("edge_h1", c(self.edge_h1)),
            ("edge_matched", c(self.edge_matched)),
            ("edge_pct", pct(self.edge_matched, self.edge_h1)),
            ("steps", c(s.steps)),
            ("uphill_steps", c(s.uphill_steps)),
            ("uphill_pct", pct(s.uphill_steps, s.steps)),
            ("uphill_max_m", m(s.uphill_max_m)),
            ("width10_cells", c(s.width_fixed.n)),
            ("width10_p50_m", format!("{:.0}", s.width_fixed.p50)),
            ("width10_p90_m", format!("{:.0}", s.width_fixed.p90)),
            ("width10_capped_sides", c(s.width_fixed_capped)),
            ("widthhalf_cells", c(s.width_half.n)),
            ("widthhalf_p50_m", format!("{:.0}", s.width_half.p50)),
            ("widthhalf_p90_m", format!("{:.0}", s.width_half.p90)),
            ("widthhalf_capped_sides", c(s.width_half_capped)),
            ("widthhalf_flat_cells", c(s.width_half_flat)),
            ("asym_cells", c(s.asymmetry.n)),
            ("asym_p50", r(s.asymmetry.p50)),
            ("asym_p90", r(s.asymmetry.p90)),
            ("density_km_per_km2", format!("{:.4}", s.density_km_per_km2)),
            ("river_km", format!("{:.1}", s.river_km)),
            ("land_km2", format!("{:.1}", s.land_km2)),
            ("catchment_pairs", c(s.conservation_pairs)),
            ("catchment_runoff_m2_per_m3s", format!("{:.4e}", s.implied_runoff)),
            ("catchment_p10", r(s.conservation_p10)),
            ("catchment_p90", r(s.conservation_p90)),
            ("seam_pairs", c(s.seam_pairs)),
            ("seam_p50_m", m(s.seam_p50_m)),
            ("seam_p99_m", m(s.seam_p99_m)),
            ("seam_max_m", m(s.seam_max_m)),
            ("interior_p50_m", m(s.interior_p50_m)),
            ("interior_p99_m", m(s.interior_p99_m)),
            ("narrow_cells", c(s.narrowness.n)),
            ("narrow_p50", r(s.narrowness.p50)),
            ("narrow_p90", r(s.narrowness.p90)),
            ("narrow_of_p50", r(s.narrowness_of_p50)),
            ("slope_area_bins_fitted", c(s.slope_area.iter().filter(|b| b.cells >= crate::river_suite::SA_BIN_MIN_CELLS).count())),
            ("theta", r(s.theta)),
            ("spectrum_n", c(s.spectrum.n)),
            ("beta", r(s.spectrum.beta)),
            ("beta_start", r(s.spectrum_start.beta)),
            ("beta_change", r(s.spectrum.beta - s.spectrum_start.beta)),
            ("trunk_reaches", c(self.trunk.reaches)),
            ("trunk_reaches_over", c(self.trunk.reaches_over)),
            ("trunk_reaches_over_pct", pct(self.trunk.reaches_over, self.trunk.reaches)),
            ("trunk_vertices", c(self.trunk.vertices)),
            ("trunk_vertices_over", c(self.trunk.vertices_over)),
            ("trunk_vertices_over_pct", pct(self.trunk.vertices_over, self.trunk.vertices)),
            ("trunk_deepest_m", m(self.trunk.deepest_m)),
            ("trunk_deepest_reach", c(self.trunk.deepest_reach)),
        ];
        f.push(("corridor_bar_m", m(self.corridor_bar_m)));
        for (names, t) in CORRIDOR_FIELDS.iter().zip(self.trunk_corridors.iter().chain(&self.trunk_corridors_snapped)) {
            let values = [
                c(t.reaches),
                c(t.reaches_over),
                pct(t.reaches - t.reaches_over, t.reaches),
                c(t.vertices),
                c(t.vertices_over),
                pct(t.vertices_over, t.vertices),
                m(t.deepest_m),
                c(t.deepest_reach),
            ];
            f.extend(names.iter().copied().zip(values));
        }
        for (names, t) in NETWORK_FIELDS.iter().zip(&self.networks) {
            let orders: Vec<String> = t.streams.iter().map(|k| k.to_string()).collect();
            let values = [
                format!("{}", t.threshold_km2),
                format!("{:.1}", t.river_km),
                format!("{:.1}", t.land_km2),
                format!("{:.4}", t.density_km_per_km2),
                if orders.is_empty() { "-".to_string() } else { orders.join("/") },
                c(t.rb_orders),
                r(t.rb),
            ];
            f.extend(names.iter().copied().zip(values));
        }
        let (rk, r0) = (&self.ridge, &self.ridge_s0);
        f.extend([
            ("ridge_ref", if self.ridge_vs_kh { "s_kh" } else { "s0" }.to_string()),
            ("ridge_cells", c(rk.ridge_cells)),
            ("ridge_lowered", c(rk.lowered)),
            ("ridge_lowered_pct", pct(rk.lowered, rk.ridge_cells)),
            ("ridge_p95_m", m(rk.lowering_p95_m)),
            ("ridge_max_m", m(rk.lowering_max_m)),
            ("cut1_cells", c(rk.cut_through[0])),
            ("saddles1", c(rk.saddles[0])),
            ("cut5_cells", c(rk.cut_through[1])),
            ("saddles5", c(rk.saddles[1])),
            ("ridge_s0_lowered_pct", pct(r0.lowered, r0.ridge_cells)),
            ("ridge_s0_p95_m", m(r0.lowering_p95_m)),
            ("saddles1_s0", c(r0.saddles[0])),
            ("saddles5_s0", c(r0.saddles[1])),
        ]);
        f
    }

    /// The ridge line (BUILD-060, lead #172).
    pub fn ridge_line(&self) -> String {
        let part = |r: &RidgeReport| {
            format!(
                "{} lowered over {} m ({:.2} %), p95 lowering {:.2} m, largest {:.2} m; cut-through saddles at {} km2 {} ({} cells), at {} km2 {} ({} cells)",
                r.lowered,
                crate::river_suite::RIDGE_LOWERED_M,
                share(r.lowered, r.ridge_cells),
                r.lowering_p95_m,
                r.lowering_max_m,
                SADDLE_KM2[0],
                r.saddles[0],
                r.cut_through[0],
                SADDLE_KM2[1],
                r.saddles[1],
                r.cut_through[1]
            )
        };
        format!(
            "RIVER-GATE ridges: {} ridge cells of the start; against {} {}; against S0 {}",
            self.ridge.ridge_cells,
            if self.ridge_vs_kh { "S_KH" } else { "S0 (no KH field)" },
            part(&self.ridge),
            part(&self.ridge_s0)
        )
    }

    /// The network lines (BUILD-059): one per channel threshold.
    pub fn network_lines(&self) -> Vec<String> {
        self.networks
            .iter()
            .map(|t| {
                let orders: Vec<String> = t.streams.iter().map(|k| k.to_string()).collect();
                let rb = if t.rb_orders >= 2 { format!("{:.3} over orders 1 to {}", t.rb, t.rb_orders) } else { "none (fewer than 2 orders below the highest)".to_string() };
                format!(
                    "RIVER-GATE network at {} km2: drainage density {:.4} km/km2 ({:.1} km of channel over {:.1} km2 of land); Strahler streams starting inside by order {}; bifurcation ratio R_b {rb}",
                    t.threshold_km2,
                    t.density_km_per_km2,
                    t.river_km,
                    t.land_km2,
                    if orders.is_empty() { "none".to_string() } else { orders.join(" / ") }
                )
            })
            .collect()
    }

    /// The fields as one printed line: `RIVER-GATE fields: name=value ...` (values hold no space).
    pub fn fields_line(&self) -> String {
        let mut f: Vec<String> = self.fields().into_iter().map(|(k, v)| format!("{k}={v}")).collect();
        f.extend(self.g2.iter().flat_map(|g| g.columns()).map(|(k, v, _)| format!("{k}={v}")));
        f.extend(self.bars().iter().map(|b| format!("{}={}", b.name, b.cell())));
        format!("RIVER-GATE fields: {}", f.join(" "))
    }

    /// The gate class of every G2 column (BUILD-060): `RIVER-GATE classes: name=class ...`, or `None` without G2 columns.
    pub fn classes_line(&self) -> Option<String> {
        let mut c: Vec<String> = self.g2.iter().flat_map(|g| g.columns()).map(|(k, _, c)| format!("{k}={}", c.name())).collect();
        c.extend(self.bars().iter().map(|b| format!("{}={}", b.name, b.class.name())));
        (!c.is_empty()).then(|| format!("RIVER-GATE classes: {}", c.join(" ")))
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
        let (n0, n1) = NEAR_RING;
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
            match (self.ponds_river_cells, self.kept_river_cells, self.kept_lake_cells, self.spill_mismatch) {
                (Some(pd), Some(k), Some(a), Some(sm)) => format!(
                    "RIVER-GATE kept lakes (BUILD-060, option C): ponds {:.2} % of the river length outside the kept lakes ({pd} cells; bar <= {PONDS_MAX_PCT} %); kept lakes {:.2} % of river length ({k} cells; bar <= {} %), {a} cells inside ({:.2} km2); {sm} kept basins with a spill mismatch over {SPILL_MISMATCH_M} m (depression level, test b), {} with a rim vertex bed under the spill by more (test a)",
                    self.ponds_pct().unwrap_or(0.0),
                    share(k, self.river_cells),
                    self.limits().2,
                    a as f64 * self.cell_area_m2 / 1.0e6,
                    self.spill_rim_mismatch.unwrap_or(0)
                ),
                _ => "RIVER-GATE kept lakes: no graph, not measured".to_string(),
            },
            format!(
                "RIVER-GATE valley depth: {:.2} % of river cells over {} m below the p50 of the surface {n0} to {n1} cells out (the canyon a viewer sees; {} cells, p50 {:.2} m, p99 {:.2} m, max {:.2} m); bar: at most the share of a GLO-30 tile with 1 to 2 x this window's relief at 10 x 10 cells ({:.2} m; the review table gives the reference)",
                self.valley_share(),
                p.cut_m,
                self.valley.count_over,
                self.valley.p50_m,
                self.valley.p99_m,
                self.valley.max_m,
                self.relief_10_m
            ),
            format!(
                "RIVER-GATE burn: {:.2} % of river cells with the river level over {} m below their surface, every basin breached ({} cells, p99 {:.2} m, max {:.2} m; K7's cut); with the lakes of D {} m kept {:.2} % ({} cells)",
                self.burn_share(),
                p.cut_m,
                self.burn_breached.count_over,
                self.burn_breached.p99_m,
                self.burn_breached.max_m,
                p.lake_depth_m,
                share(self.burn.count_over, self.river_cells),
                self.burn.count_over
            ),
            format!(
                "RIVER-GATE ring 2-4: {:.2} % of river cells with the river level over {} m below the p50 of the surface {r0} to {r1} cells out at D {} m ({} cells, p99 {:.2} m, max {:.2} m; the valley depth at 2 to 4 cells plus the burn); every basin breached {:.2} % ({} cells, max {:.2} m)",
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
            format!(
                "RIVER-GATE lowered: {} of {} land cells inside ({:.2} %) lie more than {} m below the start surface",
                self.lowered_cells,
                self.land_cells,
                share(self.lowered_cells, self.land_cells),
                p.cut_m
            ),
            edge,
        ]
        .into_iter()
        .chain(self.suite.lines())
        .chain(std::iter::once(self.trunk_line()))
        .chain(std::iter::once(self.corridor_line()))
        .chain(self.network_lines())
        .chain(std::iter::once(self.ridge_line()))
        .chain(self.g2.iter().flat_map(|g| g.lines()))
        .chain(bars_line(&self.bars()))
        .collect()
    }

    /// The relief index of this row's relief-indexed limits: the window's shared index when given, else the row's start relief.
    pub fn relief_index(&self) -> f64 {
        self.params.relief_index_m.unwrap_or(self.relief_10_start_m)
    }

    /// A count per 10,000 km2 of the counted land (the suite's land area).
    pub fn per_10k_km2(&self, n: f64) -> f64 {
        if self.suite.land_km2 > 0.0 {
            1.0e4 * n / self.suite.land_km2
        } else {
            0.0
        }
    }

    /// The G3-1 relief-indexed limits of the corridor, crest and kept-river bars at this row's start relief: (corridor percent, crest metres, kept river percent).
    pub fn limits(&self) -> (f64, f64, f64) {
        let p = &self.params;
        let r = self.relief_index();
        let ix = |given: f64, mountain: f64| if p.relief_indexed { relief_indexed(given, mountain, r) } else { given };
        let corridor = if p.corridor_fixed { p.corridor_min_pct } else { ix(p.corridor_min_pct, CORRIDOR_MIN_PCT_MOUNTAIN) };
        (corridor, ix(p.crest_max_m, CREST_P95_MAX_M_MOUNTAIN), ix(p.kept_river_max_pct, KEPT_RIVER_MAX_PCT_MOUNTAIN))
    }

    /// The G2 bars of a row with a graph (lead #168; none without): the snapped k_c 2 corridor share against `corridor_min_pct`, the uphill river steps, the graph/raster
    /// agreement, the graph's violations and missing spill nodes.
    pub fn bars(&self) -> Vec<Bar> {
        let Some(g2) = &self.g2 else { return Vec::new() };
        let k2 = CORRIDOR_K.iter().position(|&k| k == 2.0).expect("k_c 2 is a corridor form");
        let corridor = self.trunk_corridors_snapped.get(k2).map_or(0.0, |t| share(t.reaches - t.reaches_over, t.reaches));
        // lead #166: the river steps in kept-basin interiors are counted apart from the uphill bar
        let k = g2.kept.clone().unwrap_or_default();
        let mut out = g2_bars(g2, share(self.suite.uphill_steps - k.uphill, self.suite.steps - k.steps), corridor, self.limits().0);
        // owner 2026-11-20 (option C): a kept basin's spill mismatch is a graph violation; the ponds and the kept river length each have a bar
        let sm = self.spill_mismatch.unwrap_or(0) + self.spill_rim_mismatch.unwrap_or(0);
        if sm > 0 {
            if let Some(v) = out.iter_mut().find(|b| b.name == "bar_violations") {
                v.value += sm as f64;
            }
        }
        if let (Some(ponds), Some(k)) = (self.ponds_pct(), self.kept_river_cells) {
            use crate::river_gate_graph::GateClass::{Correctness, Geomorphology};
            out.push(Bar { name: "bar_ponds".into(), value: ponds, limit: PONDS_MAX_PCT, at_most: true, class: Geomorphology });
            // G3-1 (A4): the same two on the 1 km2 network
            let f = self.fine;
            out.push(Bar {
                name: "bar_uphill_1km2".into(),
                value: share(f.uphill_steps, f.steps),
                limit: crate::river_gate_graph::UPHILL_MAX_PCT,
                at_most: true,
                class: Correctness,
            });
            out.push(Bar {
                name: "bar_ponds_1km2".into(),
                value: share(f.ponds_cells, f.outside_cells),
                limit: PONDS_FINE_MAX_PCT,
                at_most: true,
                class: Geomorphology,
            });
            // G3-1 (lead #186): hump cuts and the corner-only links the surface adds, both against S0
            out.push(Bar {
                name: "bar_hump_cuts".into(),
                value: self.per_10k_km2(self.hump_cuts as f64),
                limit: HUMP_CUTS_MAX_PER_10K_KM2,
                at_most: true,
                class: Geomorphology,
            });
            out.push(Bar {
                name: "bar_corner_links".into(),
                value: self.per_10k_km2(self.corner_links as f64 - self.corner_links_s0 as f64),
                limit: CORNER_LINKS_MAX_PER_10K_KM2,
                at_most: true,
                class: Geomorphology,
            });
            out.push(Bar {
                name: "bar_kept_river".into(),
                value: share(k, self.river_cells),
                limit: self.limits().2,
                at_most: true,
                class: Geomorphology,
            });
        }
        // lead #172: the spectrum slope within the benchmark's range
        out.extend(crate::river_gate_graph::spectrum_bars(self.suite.spectrum.beta));
        // lead #172: crest rounding, the p95 of the ridge lowering against S_KH
        out.push(Bar {
            name: "bar_crest".into(),
            value: self.ridge.lowering_p95_m,
            limit: self.limits().1,
            at_most: true,
            class: crate::river_gate_graph::GateClass::Preservation,
        });
        out
    }

    /// The corridor trunk line (BUILD-057, lead #158): per `k_c`, the reaches whose best route inside the corridor still rises more than the bar above its running
    /// minimum.
    pub fn corridor_line(&self) -> String {
        if !self.trunks_given {
            return "RIVER-GATE trunks in corridors: no H1 trunk reach in the input".to_string();
        }
        let part = |form: &str, rows: &[TrunkReport]| -> String {
            let p: Vec<String> = CORRIDOR_K
                .iter()
                .zip(rows)
                .map(|(k, t)| {
                    format!(
                        "k_c {k} ({:.0} m) {:.2} % of {} reaches pass ({} over; {} of {} route vertices over), deepest {:.2} m on reach {}",
                        k * self.params.h1_cell_m,
                        share(t.reaches - t.reaches_over, t.reaches),
                        t.reaches,
                        t.reaches_over,
                        t.vertices_over,
                        t.vertices,
                        t.deepest_m,
                        t.deepest_reach
                    )
                })
                .collect();
            format!("{form}: {}", p.join("; "))
        };
        format!(
            "RIVER-GATE trunks in corridors (a reach passes when its minimax route inside its corridor keeps within {:.1} m of its running minimum, the window's bar): {} | {}",
            self.corridor_bar_m,
            part("raw H1 ends", &self.trunk_corridors),
            part("snapped", &self.trunk_corridors_snapped)
        )
    }

    /// The trunk line (BUILD-057; the E4 note's "KR does not cut").
    pub fn trunk_line(&self) -> String {
        let t = &self.trunk;
        if !self.trunks_given {
            return "RIVER-GATE trunks: no H1 trunk reach in the input".to_string();
        }
        format!(
            "RIVER-GATE trunks: {} of {} H1 trunk reaches inside ({:.2} %) and {} of {} vertices ({:.2} %) stand more than {} m above their reach's running minimum (the burn KR would need); deepest {:.2} m on reach {} at cell ({:.0}, {:.0})",
            t.reaches_over,
            t.reaches,
            share(t.reaches_over, t.reaches),
            t.vertices_over,
            t.vertices,
            share(t.vertices_over, t.vertices),
            self.params.cut_m,
            t.deepest_m,
            t.deepest_reach,
            t.deepest_at.0,
            t.deepest_at.1
        )
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

/// The p50 of the surface over a ring of cell `c`: the cells of the grid at a Chebyshev distance from `r0` to `r1` ([`NEAR_RING`], [`VALLEY_RING`]).
fn valley_floor(grid: &Grid, z: &[f64], c: usize, (r0, r1): (i64, i64)) -> f64 {
    let (w, h) = (grid.w() as i64, grid.h() as i64);
    let (cx, cy) = ((c % grid.w()) as i64, (c / grid.w()) as i64);
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
/// The relief index of a window (G3-1): the p50 of the height range of the land blocks of 10 x 10 cells of `start` (main's S0 of the window) inside `region` (the region's box).
pub fn relief_index(grid: &Grid, start: &[f64], region: &[bool], sea: f64) -> f64 {
    relief(grid, start, region, 10, sea)
}

fn relief(grid: &Grid, z: &[f64], inside: &[bool], k: usize, sea: f64) -> f64 {
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
            if all_inside && lo > sea {
                r.push(hi - lo);
            }
        }
    }
    r.sort_by(f64::total_cmp);
    r.get(r.len() / 2).copied().unwrap_or(0.0)
}

/// What a store's commit must share with the window it is measured on: its tiles are placed on the window's L grid and laid on its B.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowIdentity {
    pub cells: u32,
    pub radius_m: f64,
    pub seed: u64,
    pub base_id: u64,
    /// The window's inputs identity (`RegionSpec::effective_inputs_id`: the window, margin, rounds and carve).
    pub inputs_id: u64,
}

/// Where `commit` differs from the window, one phrase each (empty when it may be measured there). The inputs identity is checked only with the keys: a spike store's carve differs by design
/// (`--no-key-check`).
pub fn commit_differences(commit: &crate::l_writer::CommitSpec, window: &WindowIdentity, key_check: bool) -> Vec<String> {
    let mut differ = Vec::new();
    if commit.cells != window.cells {
        differ.push(format!("cells {} (the window's {})", commit.cells, window.cells));
    }
    if commit.radius_m != window.radius_m {
        differ.push(format!("radius {} m (the window's {} m)", commit.radius_m, window.radius_m));
    }
    if commit.seed != window.seed {
        differ.push(format!("seed {} (the window's {})", commit.seed, window.seed));
    }
    if commit.base_id != window.base_id {
        differ.push(format!("base {:016x} (the window's {:016x})", commit.base_id, window.base_id));
    }
    if key_check && commit.inputs_id != window.inputs_id {
        differ.push(format!(
            "inputs {:016x} (the window's {:016x}: another window, margin, rounds or carve)",
            commit.inputs_id, window.inputs_id
        ));
    }
    differ
}

/// The L window a store is laid on: its corner in the face's L cells and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LWindow {
    pub face: planet_core::Face,
    pub x0: i64,
    pub y0: i64,
    pub w: usize,
    pub h: usize,
}

/// Lays a commit's stored ground on the window (BUILD-055's store mode): every tile of the commit on the window's face that lies wholly inside it is read through the reader (`read_l_tile`, or
/// under the commit's own key without `key_check`), and its cells get B (`base` at the face's L cell) plus the tile's change and count as inside. Returns (tiles read, tiles outside); no tile read
/// is an error.
#[allow(clippy::too_many_arguments)]
pub fn lay_store_ground(
    tiles: &crate::pack::TileStore,
    world: &crate::global_store::GlobalWorld,
    commit: &crate::l_writer::Commit,
    key_check: bool,
    win: LWindow,
    base: &(dyn Fn(i64, i64) -> f64 + Sync),
    threads: usize,
    surface: &mut [f64],
    inside: &mut [bool],
) -> Result<(usize, usize), String> {
    let LWindow { face, x0, y0, w, h } = win;
    assert!(surface.len() == w * h && inside.len() == w * h, "one surface height and inside flag per window cell");
    let l_spec = commit.spec.l_spec();
    let t = l_spec.grid.tile;
    let (mut read, mut outside) = (0usize, 0usize);
    for (&(f, ti, tj), &key) in &commit.tiles {
        let (tx0, ty0) = (i64::from(ti) * t as i64 - x0, i64::from(tj) * t as i64 - y0);
        if f != face.0 || tx0 < 0 || ty0 < 0 || tx0 as usize + t > w || ty0 as usize + t > h {
            outside += 1;
            continue;
        }
        let id = planet_generators::bake::tile::TileId { face, ti, tj };
        let tile = if key_check {
            crate::l_writer::read_l_tile(tiles, world, commit.spec.res, &l_spec, commit, id)
        } else {
            crate::layer_l::load_l_tile_keyed(tiles, &l_spec, id, key)
        }
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("the commit names tile {f}/{ti}/{tj} but the store does not hold it"))?;
        // The stored ground: B plus the tile's change (K7's ground minus B, which holds the regional change already). Rows over scoped threads: the same values on any thread count.
        let mut ground = vec![0f64; t * t];
        let rows = t.div_ceil(threads.max(1)).max(1);
        std::thread::scope(|s| {
            for (k, chunk) in ground.chunks_mut(rows * t).enumerate() {
                let core = &tile.core;
                s.spawn(move || {
                    for (q, v) in chunk.iter_mut().enumerate() {
                        let p = k * rows * t + q;
                        *v = base(x0 + tx0 + (p % t) as i64, y0 + ty0 + (p / t) as i64) + f64::from(core[p]);
                    }
                });
            }
        });
        for (p, g) in ground.iter().enumerate() {
            let c = (ty0 as usize + p / t) * w + tx0 as usize + p % t;
            surface[c] = *g;
            inside[c] = true;
        }
        read += 1;
    }
    if read == 0 {
        return Err(format!("none of the commit's {} tiles lies inside the window", commit.tiles.len()));
    }
    Ok((read, outside))
}

/// The key note of a store run's input line.
pub fn key_note(key_check: bool) -> &'static str {
    if key_check {
        "keys checked"
    } else {
        "keys not checked: spike store"
    }
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

/// The matched H1 crossings of at least the discharge limit as (the nearest matching L crossing's drainage area in m2, the H1 discharge in m3/s); `l_area` is one area per L crossing.
pub fn edge_pairs(l: &[Crossing], l_area: &[f64], h1: &[Crossing], dx_m: f64, params: &GateParams) -> Vec<(f64, f64)> {
    h1.iter()
        .filter(|c| c.q_m3s >= params.edge_q_m3s)
        .filter_map(|hc| {
            l.iter()
                .zip(l_area)
                .filter(|(c, _)| c.inside == hc.inside)
                .map(|(c, &a)| (libm::hypot(c.x - hc.x, c.y - hc.y) * dx_m, a))
                .filter(|&(d, _)| d <= params.edge_m)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, a)| (a, hc.q_m3s))
        })
        .collect()
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
    // With `outside_drains`, the cells outside are outlets below the sea for the routing and the floods (an outlet keeps its height in a flood, so it must stand below every cell it drains);
    // the valley ring and the relief read the surface itself.
    let drained = |c: usize| input.outside_drains && !input.inside[c];
    let lowered: Vec<f64>;
    let z: &[f64] = if input.outside_drains {
        lowered = (0..n).map(|c| if drained(c) { input.surface[c].min(input.sea_level_m - 1.0) } else { input.surface[c] }).collect();
        &lowered
    } else {
        input.surface
    };
    let outlet: Vec<bool> = (0..n).map(|c| input.outlet[c] || drained(c)).collect();
    let (routing, route_filled) = route_rules_filled(g, z, &outlet, ROUTE_EPSILON_M, input.seed, 0.0, None, ROUTE_RULES);
    let area = areas(g, &routing);
    let river: Vec<bool> = (0..n).map(|c| area[c] >= params.channel_area_m2 && z[c] > input.sea_level_m).collect();
    let (reference, _) = pits(g, z, &route_filled, REFERENCE_WET_M);
    drop(route_filled);
    let filled = priority_flood_with(g, z, &outlet, BASIN_EPSILON_M, false);
    let (label, pits) = pits(g, z, &filled, BASIN_WET_M);
    // BUILD-060 (owner's option C, 2026-11-20): with a graph, the kept-lake interiors (the depressions of this flood that hold a kept basin's floor) are lakes, not closed
    // basins; the closed-basin bar counts river cells outside them
    let kept_lakes: Option<Vec<bool>> = input.g2.as_ref().and_then(|x| x.graph.map(|gr| kept_interiors(gr, &x.window, z, &label).0));
    let in_kept = |c: usize| kept_lakes.as_ref().is_some_and(|k| k[c]);
    let is_basin = |&(d, k): &(f64, usize)| d >= params.basin_depth_m && k >= params.basin_cells;
    let basin = |c: usize| label[c] != u32::MAX && is_basin(&pits[label[c] as usize]);
    let lake = |c: usize| basin(c) && pits[label[c] as usize].0 > params.lake_depth_m;
    let breached = levels(&routing, &river, |c| z[c]);
    let with_lakes = levels(&routing, &river, |c| if lake(c) { filled[c] } else { z[c] });
    let (mut river_cells, mut basin_cells, mut reference_cells, mut lake_cells) = (0usize, 0usize, 0usize, 0usize);
    let (mut ponds_cells, mut kept_river) = (0usize, 0usize);
    let (mut canyon, mut canyon_b, mut erosion) = (Vec::new(), Vec::new(), Vec::new());
    let (mut valley, mut burn, mut burn_b) = (Vec::new(), Vec::new(), Vec::new());
    let (mut near_depth, mut ring_depth) = (Vec::new(), Vec::new());
    for c in 0..n {
        if !river[c] || !input.inside[c] {
            continue;
        }
        river_cells += 1;
        basin_cells += usize::from(basin(c));
        ponds_cells += usize::from(basin(c) && !in_kept(c));
        kept_river += usize::from(in_kept(c));
        reference_cells += usize::from(reference[c] != u32::MAX);
        lake_cells += usize::from(lake(c));
        let here = input.surface[c];
        let near = valley_floor(g, input.surface, c, NEAR_RING) - here;
        valley.push(near);
        near_depth.push(near);
        burn.push(here - with_lakes[c]);
        burn_b.push(here - breached[c]);
        let floor = valley_floor(g, input.surface, c, VALLEY_RING);
        ring_depth.push(floor - here);
        canyon.push(floor - with_lakes[c]);
        canyon_b.push(floor - breached[c]);
        erosion.push(input.start[c] - with_lakes[c]);
    }
    // G3-1 (A4): uphill and ponds on the 1 km2 network, kept-basin interiors apart
    let fine_river: Vec<bool> = (0..n).map(|c| area[c] >= FINE_NETWORK_KM2 * 1.0e6 && z[c] > input.sea_level_m).collect();
    let mut fine = FineReport::default();
    for c in (0..n).filter(|&c| fine_river[c] && input.inside[c]) {
        fine.river_cells += 1;
        if !in_kept(c) {
            fine.outside_cells += 1;
            fine.ponds_cells += usize::from(basin(c));
        }
        let r = routing.receiver[c];
        if r == NONE || !fine_river[r as usize] || in_kept(c) || in_kept(r as usize) {
            continue;
        }
        fine.steps += 1;
        fine.uphill_steps += usize::from(input.surface[r as usize] - input.surface[c] > crate::river_suite::UPHILL_M);
    }
    // G3-1 (Carbon M4.7): the margin outside the region's box, reported only
    let mut margin = MarginReport::default();
    for c in (0..n).filter(|&c| !input.region[c]) {
        margin.cells += 1;
        margin.kept_lake_cells += usize::from(in_kept(c));
        if !river[c] {
            continue;
        }
        margin.river_cells += 1;
        margin.basin_cells += usize::from(basin(c));
        margin.ponds_cells += usize::from(basin(c) && !in_kept(c));
        let r = routing.receiver[c];
        if r == NONE || !river[r as usize] || in_kept(c) || in_kept(r as usize) {
            continue;
        }
        margin.steps += 1;
        margin.uphill_steps += usize::from(input.surface[r as usize] - input.surface[c] > crate::river_suite::UPHILL_M);
    }
    let w = g.w();
    let mut l_cross: Vec<Crossing> = Vec::new();
    let mut l_area: Vec<f64> = Vec::new();
    for (c, (&r, &is_river)) in routing.receiver.iter().zip(&river).enumerate() {
        if is_river && r != NONE && input.region[c] != input.region[r as usize] {
            l_cross.push(Crossing { inside: input.region[c], x: (c % w) as f64, y: (c / w) as f64, q_m3s: 0.0 });
            l_area.push(area[c]);
        }
    }
    let (edge_matched, edge_h1) = edge_agreement(&l_cross, input.h1, g.dx(), params);
    let pairs = edge_pairs(&l_cross, &l_area, input.h1, g.dx(), params);
    let suite = measure_suite(&SuiteInput {
        grid: g,
        start: input.start,
        surface: input.surface,
        receiver: &routing.receiver,
        area: &area,
        river: &river,
        inside: input.inside,
        region: input.region,
        sea_level_m: input.sea_level_m,
        near_depth: &near_depth,
        ring_depth: &ring_depth,
        pairs: &pairs,
    });
    // The corridor measure (#159): the bar scales with the start's relief, the corridors and the snap disc with the H1 cell.
    let relief_10_start_m = relief(g, input.start, input.inside, 10, input.sea_level_m);
    // The bar is the window's, set before the results (P3; lead, 2026-11-16): the start's relief over the region's box, so a store's tiles do not move it.
    let bar = corridor_bar_m(relief(g, input.start, input.region, 10, input.sea_level_m));
    let cell_h1 = params.h1_cell_m / g.dx();
    let snap = Snap { node_radius: cell_h1, region: input.region };
    // The corridors route and snap on the water surface (DEC explorer-5): a cell under the sea counts at the sea level, as a riverbed at the coast sits there.
    let water: Vec<f64> = input.surface.iter().map(|&z| z.max(input.sea_level_m)).collect();
    // lead #172: the ridge measures on the start's ridge mask, against S_KH (reading 21: the start less KH's stage field, 0.1 m steps) and against S0; each reference is
    // routed as the surface is, for the channels it already has
    let ridge = ridge_cells(g, input.start, input.inside, input.sea_level_m);
    let ref_area = |r: &[f64]| {
        let rz: Vec<f64> = (0..n).map(|c| if drained(c) { r[c].min(input.sea_level_m - 1.0) } else { r[c] }).collect();
        areas(g, &route_rules_filled(g, &rz, &outlet, ROUTE_EPSILON_M, input.seed, 0.0, None, ROUTE_RULES).0)
    };
    let area_s0 = ref_area(input.start);
    let ridge_s0 = ridge_report(g, input.start, input.surface, &ridge, &area, &area_s0);
    let kh = input.g2.as_ref().and_then(|x| x.stages).and_then(|st| st.iter().find(|s| s.name == "kh"));
    let s_kh: Option<Vec<f64>> = kh.map(|kh| {
        let step = crate::river_gate_graph::STAGE_STEP_M;
        (0..n).map(|c| input.start[c] - f64::from(kh.lower[c]) * step + kh.fill.as_ref().map_or(0.0, |f| f64::from(f[c]) * step)).collect()
    });
    let (ridge_kh, ridge_vs_kh) = match &s_kh {
        Some(s_kh) => (ridge_report(g, s_kh, input.surface, &ridge, &area, &ref_area(s_kh)), true),
        None => (ridge_s0.clone(), false),
    };
    // G3-1 (lead #186): hump cuts and corner-only links, S0 the one reference of B and RG; RG's count against S_KH beside
    // counted over the land inside (the per-10,000 km2 rates divide by the land area)
    let land_inside: Vec<bool> = (0..n).map(|c| input.inside[c] && input.surface[c] > input.sea_level_m).collect();
    let hump_cuts = crate::river_suite::hump_cuts(g, input.start, input.surface, &land_inside);
    let humps = crate::river_suite::humps_removed(g, input.start, input.surface, &land_inside);
    let hump_cuts_s_kh = s_kh.as_ref().map(|s_kh| crate::river_suite::hump_cuts(g, s_kh, input.surface, &land_inside));
    drop(s_kh);
    let corner_links = crate::river_suite::corner_links(g, input.surface, &land_inside);
    let corner_links_s0 = crate::river_suite::corner_links(g, input.start, &land_inside);
    GateReport {
        ridge: ridge_kh,
        ridge_s0,
        ridge_vs_kh,
        params: *params,
        river_cells,
        basins: pits.iter().filter(|p| is_basin(p)).count(),
        basin_river_cells: basin_cells,
        ponds_river_cells: kept_lakes.as_ref().map(|_| ponds_cells),
        kept_river_cells: kept_lakes.as_ref().map(|_| kept_river),
        kept_lake_cells: kept_lakes.as_ref().map(|k| (0..n).filter(|&c| k[c] && input.inside[c]).count()),
        fine,
        margin,
        hump_cuts,
        hump_cuts_s_kh,
        humps,
        corner_links,
        corner_links_s0,
        // a kept basin with its floor in the window whose depression level there (the basin flood) lies more than SPILL_MISMATCH_M under its recorded spill
        spill_mismatch: input.g2.as_ref().and_then(|x| {
            x.graph.map(|gr| {
                gr.basins
                    .iter()
                    .filter(|b| b.represented == planet_generators::bake::river_graph::Represented::ProtectedSurface)
                    .filter_map(|b| x.window.cell(b.floor_cell).map(|f| (f, b)))
                    .filter(|&(f, b)| filled[f] < b.spill_m - SPILL_MISMATCH_M)
                    .count()
            })
        }),
        spill_rim_mismatch: input.g2.as_ref().and_then(|x| x.graph.map(|gr| rim_mismatch(gr, &x.window, &label, SPILL_MISMATCH_M))),
        // lead #183 (moved from the spike, lead #192): the kept basins' notch, recorded spill against the basin flood's level at the floor
        spill_notch: input.g2.as_ref().and_then(|x| {
            x.graph.map(|gr| {
                let (mut kept, mut over, mut worst, mut dry) = (0usize, [0usize; 3], 0.0f64, 0usize);
                for b in gr.basins.iter().filter(|b| b.represented == planet_generators::bake::river_graph::Represented::ProtectedSurface) {
                    let Some(f) = x.window.cell(b.floor_cell) else { continue };
                    if !b.spill_m.is_finite() {
                        continue;
                    }
                    kept += 1;
                    let gap = b.spill_m - filled[f];
                    for (k, t) in [0.01, 0.1, 0.5].iter().enumerate() {
                        over[k] += usize::from(gap > *t);
                    }
                    worst = worst.max(gap);
                    dry += usize::from(filled[f] - z[f] <= BASIN_WET_M);
                }
                (kept, over, worst, dry)
            })
        }),
        inlets: input.g2.as_ref().and_then(|x| x.graph.map(|gr| crate::river_gate_graph::inlets_without_basin(gr, &x.window, z, &label))),
        cell_area_m2: g.dx() * g.dx(),
        reference_river_cells: reference_cells,
        lake_river_cells: lake_cells,
        valley: tail(valley, params.cut_m),
        burn_breached: tail(burn_b, params.cut_m),
        burn: tail(burn, params.cut_m),
        canyon: tail(canyon, params.cut_m),
        canyon_breached: tail(canyon_b, params.cut_m),
        erosion: tail(erosion, params.cut_m),
        relief_2_start_m: relief(g, input.start, input.inside, 2, input.sea_level_m),
        relief_2_m: relief(g, input.surface, input.inside, 2, input.sea_level_m),
        relief_10_start_m,
        relief_10_m: relief(g, input.surface, input.inside, 10, input.sea_level_m),
        land_cells: (0..n).filter(|&c| input.inside[c] && input.start[c] > input.sea_level_m).count(),
        lowered_cells: (0..n)
            .filter(|&c| input.inside[c] && input.start[c] > input.sea_level_m && input.start[c] - input.surface[c] > params.cut_m)
            .count(),
        edge_h1,
        edge_matched,
        suite,
        trunk: trunk_burn(g, input.surface, input.inside, input.trunks, params.cut_m),
        trunks_given: !input.trunks.is_empty(),
        trunk_corridors: CORRIDOR_K
            .iter()
            .map(|k| trunk_corridor(g, &water, input.inside, input.trunks, k * cell_h1, bar, input.sea_level_m, None))
            .collect(),
        trunk_corridors_snapped: CORRIDOR_K
            .iter()
            .map(|k| trunk_corridor(g, &water, input.inside, input.trunks, k * cell_h1, bar, input.sea_level_m, Some(&snap)))
            .collect(),
        corridor_bar_m: bar,
        networks: params
            .network_km2
            .iter()
            .map(|&a| network(g, z, &routing.receiver, &area, input.inside, input.sea_level_m, a * 1.0e6))
            .collect(),
        g2: input.g2.map(|x| {
            let land: Vec<bool> = z.iter().map(|&v| v > input.sea_level_m).collect();
            // reading 28 (lead #170): the budget's relief field on the start surface S0; the stage fields carry the lowering against S_KH (reading 21)
            let budget =
                x.stages.map(|_| budget_field(input.start, g.w(), g.h(), (params.h1_cell_m / g.dx()).round() as usize, g.dx(), R_RELIEF_M));
            // lead #166: kept-basin interiors on the measured surface, counted apart from the uphill and agreement bars
            let kept = x.graph.map(|gr| kept_interiors(gr, &x.window, z, &label));
            G2Report {
                graph: x.graph.map(|gr| graph_report(gr, &x.window, input.inside, &land, g.dx())),
                agreement: x
                    .graph
                    .zip(kept.as_ref())
                    .map(|(gr, (k, _))| {
                        params
                            .network_km2
                            .iter()
                            .map(|&a| agreement(gr, &x.window, &area, &land, input.inside, k, &routing.receiver, a * 1.0e6))
                            .collect()
                    })
                    .unwrap_or_default(),
                kept: kept.as_ref().map(|(k, b)| kept_steps(k, *b, &routing.receiver, &river, input.inside, input.surface)),
                budget: x.stages.zip(budget.as_ref()).map(|(st, b)| BudgetReport {
                    kc_patches: x.kc_patches,
                    validator_overrun: x.validator,
                    ..budget_report(st, b, &land, input.inside, x.residual_records, x.graph)
                }),
                partition: x.partition.map(|(a, b)| partition_report(a, b)),
            }
        }),
    }
}

/// `R_relief` of the budget field (G0 contracts section 6), metres.
pub const R_RELIEF_M: f64 = 25_000.0;
