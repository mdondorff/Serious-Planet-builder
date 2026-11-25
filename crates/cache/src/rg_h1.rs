//! Spike only (crusader #208 step 3): today's K6, H1's routed hydrology, channel graph and trunks (`--h1 k6`), moved verbatim from `examples/rg_chain.rs` so the wave runner's hook calls
//! the chain's code. The switch `HYDRO_TWICE` is a per-call parameter (R16). rg_chain calls this with its switch, so its bits do not change.

use planet_generators::bake::channel_graph::{q_value, ChannelGraph};
use planet_generators::bake::hydro::{hydrology, Hydro, HydroParams};
use planet_generators::bake::route::{Routing, NONE};
use planet_generators::bake::Grid;

/// A trunk reach carries at least this discharge (m3/s).
pub const EDGE_Q: f64 = 1.0;

/// A routed H1: routing, filled levels, hydrology and graph, with the trunk reaches (q >= 1 m3/s, 2 cells or more) as segment indices.
pub struct H1 {
    pub routing: Routing,
    pub hydro: Hydro,
    pub graph: ChannelGraph,
    pub trunk_seg: Vec<usize>,
    pub trunk_cells: Vec<Vec<u32>>,
    /// The D8 drainage area (true area, unweighted) of each cell along the receivers, m2.
    pub d8: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
pub fn h1_from(
    grid: &Grid,
    z: &[f64],
    filled: &[f64],
    routing: Routing,
    outlet: &[bool],
    runoff: &[f32],
    evap: &[f32],
    budget: Option<&[f64]>,
    hydro_twice: bool,
) -> H1 {
    // lead #163 (B3, result-preserving): with `budget` every basin is crossed (reading 24) and the second call only zeroed the open-water loss of the basins' cells,
    // the one input `hydrology` reads it through (each basin's e_open); one call with a zero evaporation gives the same Hydro
    let zeros: Vec<f32> = if budget.is_some() { vec![0.0; evap.len()] } else { Vec::new() };
    let mut hydro = hydrology(
        grid,
        z,
        filled,
        &routing,
        outlet,
        Some(runoff),
        Some(if budget.is_some() { &zeros } else { evap }),
        &HydroParams::default(),
    );
    // spikes #142, reading 16: a kept basin deeper than the budget at its floor is crossed (reading 13), not a lake: no open-water loss there, so q_w = q through it.
    // The basins are the same (the loss enters only the balance); the rerun changes q_w and the balance only.
    // reading 24 (lead, 2026-11-17; owner decision 6 c): no H1 basin is a lake in the prototype, every one is crossed (q_w = q); `budget` only switches the rule on
    if budget.is_some() {
        let crossed: Vec<bool> = vec![true; hydro.basins.len()];
        if hydro_twice && crossed.iter().any(|&x| x) {
            let ev2: Vec<f32> =
                (0..evap.len()).map(|c| if hydro.label[c] != NONE && crossed[hydro.label[c] as usize] { 0.0 } else { evap[c] }).collect();
            let again = hydrology(grid, z, filled, &routing, outlet, Some(runoff), Some(&ev2), &HydroParams::default());
            assert_eq!(again.basins.len(), hydro.basins.len(), "the loss does not change the basins");
            hydro = again;
        }
        // q_w as a running sum along the routing: q less the losses of the basins upstream (each basin's q_in - q_out, entered at its spill cell; zero for a crossed
        // basin). The cascade's q_w inside a basin is no channel discharge (a basin's cells are taken in index order on its flat), so a trunk through a crossed basin reads this one.
        let mut loss = vec![0f64; hydro.q.len()];
        // entered where the routing leaves the basin (the first cell outside it on the floor's receiver chain), which need not be its spill cell
        for (k, b) in hydro.basins.iter().enumerate() {
            let mut c = b.floor_cell;
            while hydro.label[c as usize] == k as u32 && routing.receiver[c as usize] != NONE {
                c = routing.receiver[c as usize];
            }
            loss[c as usize] += b.q_in - b.q_out;
        }
        for &c in routing.order.iter().rev() {
            let r = routing.receiver[c as usize];
            if r != NONE {
                loss[r as usize] += loss[c as usize];
            }
        }
        hydro.qw = (0..hydro.q.len()).map(|c| (hydro.q[c] - loss[c]).max(0.0)).collect();
    }
    let graph = ChannelGraph::from_hydro(grid, &routing, &hydro, z);
    let (mut trunk_seg, mut trunk_cells) = (Vec::new(), Vec::new());
    graph.for_each(|s| {
        let cells = s.cells(grid);
        let q_max = s.q.iter().map(|&q| q_value(q)).fold(0.0, f64::max);
        if cells.len() >= 2 && q_max >= EDGE_Q {
            trunk_seg.push(s.index);
            trunk_cells.push(cells);
        }
    });
    let mut d8: Vec<f64> = (0..grid.len()).map(|p| grid.cell_area(p)).collect();
    for &c in routing.order.iter().rev() {
        let r = routing.receiver[c as usize];
        if r != NONE {
            d8[r as usize] += d8[c as usize];
        }
    }
    H1 { routing, hydro, graph, trunk_seg, trunk_cells, d8 }
}
