//! Hydrology H1 on a routed grid (GEN-145, A3 section 5.2 to 5.5): the channel graph and the basins, kept apart from the diffuse field.
//!
//! - **Channel cells.** An initiated cell (`A_d >= T`, the diffuse area of the clipped MFD, runoff weighted) starts a channel; the channel continues downstream along the D8 receivers to its outlet, even where
//!   `A_d` falls below `T` (no restart). The outlet cell where it ends (the mouth) is a channel cell.
//! - **Channel discharge `Q`.** Accumulated along the D8 tree from the local term `a r` (true cell area times the runoff; zero in an outlet cell, which is the sea): `Q(c) = a(c) r(c) + sum of Q over the D8 donors`.
//!   It is conserved at confluences by construction, and the sum of `Q` at the outlets is the sum of `a r` over the land. `Q` is the potential discharge before the losses of basins: a basin's `q_out` is the
//!   lake-adjusted discharge that continues below its spill point, and an endorheic basin passes none (A3 section 5.5).
//! - **Segments.** A segment runs from a head (an initiated cell with no channel donor) or a confluence (two or more channel donors) downstream to the next confluence or the mouth. Its end cell is the start of
//!   the next segment (a confluence) or the mouth, which belongs to no segment. `segment_id = hash(body, downstream end cell, upstream end cell)`.
//! - **Basins.** A basin is a connected set (8 neighbours) of cells whose filled surface stands more than `pit_min_m` above the ground. Floor, spill point and level come from the filled surface of the routing
//!   (the minimax spill level plus the flood's epsilon ramp), the area curve has 16 level steps between the floor and the spill, and the class and the water level follow section 5.5 from the inflow and the net
//!   evaporation. `basin_id = hash(body, canonical cell)`, the canonical cell the lowest cell of the basin, ties by the global cell id.
//!
//! All sums run in fixed orders (CON-14); nothing here reads a thread count.

#![allow(clippy::needless_range_loop)]

use super::route::{Routing, NONE};
use super::Grid;
use std::collections::HashMap;

/// The number of level steps of a basin's area curve.
pub const CURVE_STEPS: usize = 16;

/// The parameters of H1 (presets in the sense of ADR 0108; the values are the prototype's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HydroParams {
    /// River initiation: `A_d >= T` in square metres of runoff-weighted true area (A3 section 5.2: 50 to 100 km^2; the default 75 km^2).
    pub initiation_m2: f64,
    /// A basin is the cells whose filled surface stands more than this many metres above the ground (A3 section 5.3: 0.5 m).
    pub pit_min_m: f64,
    /// A basin of fewer cells than this and shallower than `min_depth_m` is an artefact: no object. In cells, so that the limit scales with the grid (a square-kilometre limit is never met by a 3 km cell).
    pub min_cells: u32,
    pub min_depth_m: f64,
    /// The body's word in every identity.
    pub body: u64,
}

impl Default for HydroParams {
    fn default() -> HydroParams {
        HydroParams { initiation_m2: 75.0e6, pit_min_m: 0.5, min_cells: 4, min_depth_m: 2.0, body: 0 }
    }
}

/// A river segment.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub id: u64,
    /// The first cell (a head or a confluence) and the end cell (the next confluence or the mouth).
    pub up: u32,
    pub down: u32,
    /// The cells from `up` to `down`, `down` excluded.
    pub cells: u32,
    pub q_up: f64,
    pub q_down: f64,
    /// The index of the segment that starts at `down`, `NONE` at a mouth.
    pub downstream: u32,
}

/// The class of a kept basin (A3 section 5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasinClass {
    /// It overflows: the spill point is the active outlet.
    Lake,
    /// No active outlet: a sink for water and sediment.
    Endorheic,
}

/// A kept basin.
#[derive(Clone, Debug, PartialEq)]
pub struct Basin {
    pub id: u64,
    pub floor_cell: u32,
    pub floor_m: f64,
    pub spill_cell: u32,
    pub spill_m: f64,
    pub cells: u32,
    /// Surface area and volume at the 16 level steps `floor + (spill - floor) k / 16`, `k = 1..=16`.
    pub area_m2: [f64; CURVE_STEPS],
    pub volume_m3: [f64; CURVE_STEPS],
    /// The discharge reaching the basin.
    pub q_in: f64,
    pub class: BasinClass,
    pub water_level_m: f64,
    /// The discharge the downstream channel continues with (zero for an endorheic basin).
    pub q_out: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HydroStats {
    pub channel_cells: usize,
    pub heads: usize,
    pub confluences: usize,
    pub mouths: usize,
    pub segments: usize,
    pub basin_cells: usize,
    /// Components of pit cells shallower than the pit limit from floor to rim: the epsilon ramp of a flat, not a depression.
    pub ramps: usize,
    pub basins: usize,
    pub artefacts: usize,
}

/// H1's result.
pub struct Hydro {
    /// `Q` per cell (zero off the grid).
    pub q: Vec<f64>,
    pub channel: Vec<bool>,
    pub segments: Vec<Segment>,
    pub basins: Vec<Basin>,
    pub stats: HydroStats,
}

/// The kind words of the identities: a segment id and a basin id never collide.
const SEGMENT_KIND: u64 = 0x5E61;
const BASIN_KIND: u64 = 0xBA51;

/// The id of a segment from its body, downstream end cell and upstream end cell (global cell ids).
pub fn segment_id(body: u64, down: u64, up: u64) -> u64 {
    id_hash(body ^ SEGMENT_KIND, down, up)
}

/// The id of a basin from its body and its lowest cell (global cell id).
pub fn basin_id(body: u64, floor: u64) -> u64 {
    id_hash(body ^ BASIN_KIND, floor, 0)
}

/// A 64-bit identity of three words (SplitMix64 finaliser steps).
pub fn id_hash(body: u64, a: u64, b: u64) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    for w in [body, a, b] {
        h = h.wrapping_add(w).wrapping_add(0x9E37_79B9_7F4A_7C15);
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^= h >> 31;
    }
    h
}

/// H1 on a routing. `filled` is the filled surface it came from; `runoff` the runoff of each cell (m/s, the local term is `a r`; `None`: 1, so `Q` is a true area); `net_evap` the net evaporation
/// `max(PET - P, 0)` of each cell in m/s (`None`: 0, so every kept basin overflows).
#[allow(clippy::too_many_arguments)]
pub fn hydrology(
    grid: &Grid,
    z: &[f64],
    filled: &[f64],
    routing: &Routing,
    outlet: &[bool],
    runoff: Option<&[f32]>,
    net_evap: Option<&[f32]>,
    params: &HydroParams,
) -> Hydro {
    let n = grid.len();
    assert!(z.len() == n && filled.len() == n && outlet.len() == n && routing.receiver.len() == n, "one value per cell");
    assert!(params.initiation_m2 > 0.0 && params.pit_min_m >= 0.0, "initiation is positive, the pit limit not negative");
    assert!(routing.area.len() == n, "one area per cell");
    assert!(runoff.is_none_or(|r| r.len() == n) && net_evap.is_none_or(|e| e.len() == n), "one runoff and one net evaporation per cell");
    assert!(net_evap.is_none() || runoff.is_some(), "a net evaporation (m/s) needs the runoff in m/s: without it Q is an area");
    let (receiver, area) = (&routing.receiver, &routing.area);

    // ---- Q: the local term, then the donors, leaves first
    let mut q = vec![0.0f64; n];
    for p in 0..n {
        if grid.is_active(p) && !outlet[p] {
            q[p] = grid.cell_area(p) * runoff.map_or(1.0, |r| f64::from(r[p]));
        }
    }
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        let r = receiver[c];
        if r != NONE {
            q[r as usize] += q[c];
        }
    }

    // ---- channel cells: initiated, then downstream
    let mut channel = vec![false; n];
    for p in 0..n {
        channel[p] = grid.is_active(p) && !outlet[p] && area[p] >= params.initiation_m2;
    }
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        if channel[c] && receiver[c] != NONE {
            channel[receiver[c] as usize] = true;
        }
    }
    let mut donors = vec![0u8; n];
    for p in 0..n {
        if channel[p] && receiver[p] != NONE {
            donors[receiver[p] as usize] += 1;
        }
    }
    let mut stats = HydroStats::default();
    let mut starts: Vec<u32> = Vec::new();
    for p in 0..n {
        if !channel[p] {
            continue;
        }
        stats.channel_cells += 1;
        if donors[p] >= 2 {
            stats.confluences += 1;
        }
        if receiver[p] == NONE {
            stats.mouths += 1;
        } else if donors[p] == 0 {
            stats.heads += 1;
            starts.push(p as u32);
        } else if donors[p] >= 2 {
            starts.push(p as u32);
        }
    }
    // A confluence at the mouth cell itself starts no segment (the mouth belongs to none).
    let mut index: HashMap<u32, u32> = HashMap::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        index.insert(s, k as u32);
    }
    let mut segments: Vec<Segment> = Vec::with_capacity(starts.len());
    for &s in &starts {
        let (mut cur, mut cells) = (s as usize, 0u32);
        loop {
            cells += 1;
            let next = receiver[cur] as usize;
            let end = receiver[next] == NONE || donors[next] >= 2;
            cur = next;
            if end {
                break;
            }
        }
        segments.push(Segment {
            id: segment_id(params.body, grid.global_id(cur), grid.global_id(s as usize)),
            up: s,
            down: cur as u32,
            cells,
            q_up: q[s as usize],
            q_down: q[cur],
            downstream: NONE,
        });
    }
    for sg in &mut segments {
        sg.downstream = index.get(&sg.down).copied().unwrap_or(NONE);
    }
    stats.segments = segments.len();

    // ---- basins: the components of the cells under the filled surface
    let mut seen = vec![false; n];
    let pit = |p: usize| grid.is_active(p) && !outlet[p] && filled[p] - z[p] > params.pit_min_m;
    let mut basins: Vec<Basin> = Vec::new();
    let mut stack: Vec<u32> = Vec::new();
    let mut members: Vec<u32> = Vec::new();
    for p0 in 0..n {
        if seen[p0] || !pit(p0) {
            continue;
        }
        members.clear();
        stack.clear();
        seen[p0] = true;
        stack.push(p0 as u32);
        while let Some(c) = stack.pop() {
            members.push(c);
            for d in 0..8 {
                if let Some(q2) = grid.neighbour(c as usize, d) {
                    if !seen[q2] && pit(q2) {
                        seen[q2] = true;
                        stack.push(q2 as u32);
                    }
                }
            }
        }
        members.sort_unstable();
        stats.basin_cells += members.len();
        // The floor: the lowest cell, ties by the global cell id (the members are in id order).
        let mut floor = members[0] as usize;
        for &c in &members {
            if z[c as usize] < z[floor] {
                floor = c as usize;
            }
        }
        // The spill point: the rim cell of the lowest filled level, ties by the cell id.
        let mut spill: Option<usize> = None;
        let mut exits: Vec<usize> = Vec::new();
        let mut evap = 0.0;
        for &c in &members {
            let c = c as usize;
            evap += net_evap.map_or(0.0, |e| f64::from(e[c]));
            if !pit(receiver[c] as usize) {
                exits.push(c);
            }
            for d in 0..8 {
                if let Some(r) = grid.neighbour(c, d) {
                    if !pit(r) && spill.is_none_or(|s| (filled[r], r) < (filled[s], s)) {
                        spill = Some(r);
                    }
                }
            }
        }
        let spill = spill.expect("a basin has a rim");
        // The inflow: `Q` of the exits, but an exit whose flow comes back into the basin a few cells on (over a shallow margin or an island under the water) is inside the next exit's `Q`.
        let mut q_in = 0.0;
        'exits: for &e in &exits {
            let mut r = receiver[e];
            for _ in 0..256 {
                if r == NONE {
                    break;
                }
                if members.binary_search(&r).is_ok() {
                    continue 'exits;
                }
                r = receiver[r as usize];
            }
            q_in += q[e];
        }
        let (floor_m, spill_m) = (z[floor], filled[spill].max(z[floor]));
        if spill_m - floor_m < params.pit_min_m {
            stats.ramps += 1;
            continue;
        }
        if (members.len() as u32) < params.min_cells && spill_m - floor_m < params.min_depth_m {
            stats.artefacts += 1;
            continue;
        }
        let mut curve_a = [0.0f64; CURVE_STEPS];
        let mut curve_v = [0.0f64; CURVE_STEPS];
        for (k, (ca, cv)) in curve_a.iter_mut().zip(curve_v.iter_mut()).enumerate() {
            let h = floor_m + (spill_m - floor_m) * (k + 1) as f64 / CURVE_STEPS as f64;
            for &c in &members {
                let zc = z[c as usize];
                if zc < h {
                    let a = grid.cell_area(c as usize);
                    *ca += a;
                    *cv += a * (h - zc);
                }
            }
        }
        // Section 5.5: the steady balance A(h) E = Q_in on the curve.
        let e_net = evap / members.len() as f64;
        let (class, level, q_out) = if curve_a[CURVE_STEPS - 1] * e_net < q_in {
            (BasinClass::Lake, spill_m, q_in - curve_a[CURVE_STEPS - 1] * e_net)
        } else {
            let mut level = floor_m;
            let mut prev = (floor_m, 0.0);
            for k in 0..CURVE_STEPS {
                if q_in <= 0.0 {
                    break;
                }
                let h = floor_m + (spill_m - floor_m) * (k + 1) as f64 / CURVE_STEPS as f64;
                let w = curve_a[k] * e_net;
                if w >= q_in {
                    let t = if w > prev.1 { (q_in - prev.1) / (w - prev.1) } else { 1.0 };
                    level = prev.0 + t * (h - prev.0);
                    break;
                }
                prev = (h, w);
            }
            (BasinClass::Endorheic, level, 0.0)
        };
        basins.push(Basin {
            id: basin_id(params.body, grid.global_id(floor)),
            floor_cell: floor as u32,
            floor_m,
            spill_cell: spill as u32,
            spill_m,
            cells: members.len() as u32,
            area_m2: curve_a,
            volume_m3: curve_v,
            q_in,
            class,
            water_level_m: level,
            q_out,
        });
    }
    stats.basins = basins.len();
    Hydro { q, channel, segments, basins, stats }
}

/// The numbers of the gate G-CON and of the conservation check (A3 section 12.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verify {
    /// Segments whose chain of segments does not reach a mouth.
    pub disconnected_segments: usize,
    /// Channel cells whose receiver is neither a channel cell nor an outlet, or does not stand lower on the filled surface.
    pub broken_channel_cells: usize,
    /// Channel cells covered by no segment or by more than one (a mouth is covered by none and is not counted).
    pub miscovered_cells: usize,
    /// The largest relative error of `Q(c)` against its local term plus its donors, over the channel cells.
    pub max_confluence_error: f64,
    /// Relative error of the sum of `Q` at the outlets against the sum of the local terms.
    pub q_outlets_error: f64,
    /// Relative error of the sum of `A_d` at the outlets against the sum of the (weighted) true cell areas of the cells that drain to them (every active cell).
    pub area_outlets_error: f64,
}

/// Checks H1's result against the routing (G-CON and the conservation of the area). Linear in the cells.
/// `weights` are the runoff weights the routing was made with (`None`: unweighted, `A_d` is a true area).
pub fn verify(
    grid: &Grid,
    h: &Hydro,
    routing: &Routing,
    filled: &[f64],
    outlet: &[bool],
    runoff: Option<&[f32]>,
    weights: Option<&[f32]>,
) -> Verify {
    let n = grid.len();
    let receiver = &routing.receiver;
    let mut v = Verify::default();
    for p in 0..n {
        if h.channel[p] && receiver[p] != NONE {
            let r = receiver[p] as usize;
            if !(h.channel[r] || outlet[r]) || filled[r] >= filled[p] {
                v.broken_channel_cells += 1;
            }
        }
    }
    for s in &h.segments {
        let (mut cur, mut steps) = (s.downstream, 0usize);
        let mut at_mouth = receiver[s.down as usize] == NONE;
        while !at_mouth && cur != NONE && steps <= h.segments.len() {
            let t = &h.segments[cur as usize];
            at_mouth = receiver[t.down as usize] == NONE;
            cur = t.downstream;
            steps += 1;
        }
        if !at_mouth {
            v.disconnected_segments += 1;
        }
    }
    let mut cover = vec![0u8; n];
    for s in &h.segments {
        let mut c = s.up as usize;
        while c != s.down as usize {
            cover[c] = cover[c].saturating_add(1);
            c = receiver[c] as usize;
        }
    }
    v.miscovered_cells = (0..n).filter(|&p| h.channel[p] && usize::from(cover[p]) + usize::from(receiver[p] == NONE) != 1).count();
    // Q at every channel cell against a recomputation from its donors, summed in the cell order of the neighbours.
    let local = |p: usize| if grid.is_active(p) && !outlet[p] { grid.cell_area(p) * runoff.map_or(1.0, |r| f64::from(r[p])) } else { 0.0 };
    for c in 0..n {
        if !h.channel[c] {
            continue;
        }
        let mut sum = local(c);
        for d in 0..8 {
            if let Some(p) = grid.neighbour(c, d) {
                if receiver[p] == c as u32 {
                    sum += h.q[p];
                }
            }
        }
        let err = (sum - h.q[c]).abs() / h.q[c].abs().max(1e-300);
        v.max_confluence_error = v.max_confluence_error.max(err);
    }
    let (mut q_out, mut a_local, mut ad_out, mut ad_cells) = (0.0, 0.0, 0.0, 0.0);
    for p in 0..n {
        if !grid.is_active(p) {
            continue;
        }
        a_local += local(p);
        ad_cells += grid.cell_area(p) * weights.map_or(1.0, |w| f64::from(w[p]));
        if receiver[p] == NONE {
            q_out += h.q[p];
            ad_out += routing.area[p];
        }
    }
    v.q_outlets_error = (q_out - a_local).abs() / a_local.max(1e-300);
    v.area_outlets_error = (ad_out - ad_cells).abs() / ad_cells.max(1e-300);
    v
}
