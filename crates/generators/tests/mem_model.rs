//! Spike only (crusader, G3-6 / R13, lead 2026-11-25 19:20): the planet-scale memory gate's byte model (tier A). Every component of wave 1 and wave 2 and the per-tile L buffers, in bytes
//! per cell from the arrays the code holds, at the largest landmass's cell count (8.4e7 km², terrain-target §6.6: 8.10e9 L cells, 1.27e8 H1 cells) with 16 worker `CubePatch` buffers; the
//! components over the 10.8 GB gate are named. Its measurement at 1/16 is the example `mem_stress` (`cargo run --release -p planet-generators --example mem_stress -- full|compact`), a binary of
//! its own for the counting allocator (BUILD-015 keeps one test binary per tier).
#![allow(dead_code)]

// ---------------------------------------------------------------- the model

/// The gate (G3-6): 10.8 GB, 10 % headroom under the 12 GB world-build budget.
pub const GATE_B: f64 = 10.8e9;
/// The largest landmass (terrain-target §6.6) and the nominal cells (6,371 km x π/2 / 98,304 = 101.8 m at L; 8 x 8 L cells an H1 cell).
const LAND_KM2: f64 = 8.4e7;
const L_CELL_KM2: f64 = 0.010364;
const H1_PER_L: f64 = 64.0;
/// GEN-182's production tile and halo (L cells), and the workers in flight.
const TILE_L: f64 = 3072.0;
const HALO_L: f64 = 128.0;
const WORKERS: f64 = 16.0;
/// The H1 tile (GEN-143's partitioned route at H1: 3,072 / 8).
const TILE_H1: f64 = 384.0;
const HALO_H1: f64 = 16.0;

/// How a component's bytes grow.
#[derive(Clone, Copy, Debug)]
pub enum Basis {
    /// per H1 land cell, resident over the landmass
    H1Land,
    /// per cell of one worker's buffer, times the workers
    Patch { l: bool },
    /// a fixed total (bytes)
    Total,
}

/// A component: its wave, what it holds (bytes per cell, from the code's arrays), its basis and where the number comes from.
pub struct Component {
    pub wave: &'static str,
    pub name: &'static str,
    pub bytes_per_cell: f64,
    pub basis: Basis,
    pub source: &'static str,
}

pub fn cells() -> (f64, f64) {
    let l = LAND_KM2 / L_CELL_KM2;
    (l, l / H1_PER_L)
}

pub fn bytes(c: &Component) -> f64 {
    let (_, h1) = cells();
    match c.basis {
        Basis::H1Land => c.bytes_per_cell * h1,
        Basis::Patch { l: true } => c.bytes_per_cell * (TILE_L + 2.0 * HALO_L) * (TILE_L + 2.0 * HALO_L) * WORKERS,
        Basis::Patch { l: false } => c.bytes_per_cell * (TILE_H1 + 2.0 * HALO_H1) * (TILE_H1 + 2.0 * HALO_H1) * WORKERS,
        Basis::Total => c.bytes_per_cell,
    }
}

/// The components as the code holds them today (counted from the arrays; the 1/16 run measures the wave-1 rows).
pub fn components() -> Vec<Component> {
    let f64b = core::mem::size_of::<f64>() as f64;
    let u32b = core::mem::size_of::<u32>() as f64;
    let boolb = core::mem::size_of::<bool>() as f64;
    vec![
        Component { wave: "1", name: "z_b, z_s and the outlets, resident", bytes_per_cell: 2.0 * f64b + boolb, basis: Basis::H1Land, source: "the surfaces wave 1 reads and writes" },
        Component {
            wave: "1",
            name: "base1 / sel-post, global round input (level, label, depression cells, output copy)",
            bytes_per_cell: f64b + u32b + u32b + f64b,
            basis: Basis::H1Land,
            source: "drain::sel_round + sel_apply's copy",
        },
        Component {
            wave: "1",
            name: "K5, GEN-143's partitioned route (filled, receiver, sum, pending, ready, area, order)",
            bytes_per_cell: f64b + u32b + f64b + 1.0 + u32b + f64b + u32b,
            basis: Basis::H1Land,
            source: "partition.rs: resident arrays (\"this change gates the equality, not the memory\")",
        },
        Component {
            wave: "1",
            name: "K5, a round's catchment members",
            bytes_per_cell: u32b,
            basis: Basis::H1Land,
            source: "round::cut_in",
        },
        Component {
            wave: "1",
            name: "K5, gathered catchment inputs in flight (z, receiver, area, dist, taper, outlet flag, scale, up unit)",
            bytes_per_cell: 6.0 * f64b - f64b + u32b + 4.0 + boolb,
            basis: Basis::H1Land,
            source: "round::CatchmentInputs; the largest-first queue puts the 16 largest catchments in flight, which on one continent hold about all its cells (the 1/16 run measures the share)",
        },
        Component {
            wave: "1",
            name: "K6, routing (16), filled (8), hydrology (24), d8 (8)",
            bytes_per_cell: 2.0 * u32b + f64b + f64b + (2.0 * f64b + u32b + 4.0 * boolb) + f64b,
            basis: Basis::H1Land,
            source: "route::Routing, flood, hydro::hydrology's arrays, rg_h1::H1.d8",
        },
        Component {
            wave: "2",
            name: "landmass trunks, portal-only (explorer #195, exact; measured 11.6-19.2 GB)",
            bytes_per_cell: 8.8e9,
            basis: Basis::Total,
            source: "explorer #195 (R1h 5.2, R3a 8.8, M1 6.6 GB exact): the worst window",
        },
        Component { wave: "2", name: "portal records (80 B)", bytes_per_cell: 0.08e9, basis: Basis::Total, source: "explorer #195" },
        Component {
            wave: "L",
            name: "per-tile L stages, 16 worker buffers (3,328² cells each)",
            bytes_per_cell: 177.0,
            basis: Basis::Patch { l: true },
            source: "rg_chain's window peak, 10.3-11.7 GiB over 8,160² L cells (seam design §3): 177 B a cell at 11.0 GiB, an upper bound (it holds the window's H1 too)",
        },
        Component {
            wave: "L",
            name: "base1's tiled form instead of the global one (#207), 16 H1 buffers",
            bytes_per_cell: f64b + u32b + u32b + f64b,
            basis: Basis::Patch { l: false },
            source: "the same arrays per (384 + 32)² patch",
        },
    ]
}

// spike only (crusader, G3-6 / R13): the byte model
#[test]
fn the_byte_model_at_the_largest_landmass() {
    let (l, h1) = cells();
    eprintln!("G3-6 model: the largest landmass {LAND_KM2:.1e} km² = {:.3e} L cells, {:.3e} H1 cells; gate {:.1} GB", l, h1, GATE_B / 1e9);
    let comps = components();
    let mut over = Vec::new();
    let (mut w1, mut w2) = (0.0, 0.0);
    for c in &comps {
        let b = bytes(c);
        eprintln!(
            "G3-6 model | {} | {} | {:.1} B a cell, {:?} | {:.2} GB | {}",
            c.wave,
            c.name,
            c.bytes_per_cell,
            c.basis,
            b / 1e9,
            c.source
        );
        if b > GATE_B {
            over.push(c.name);
        }
        match c.wave {
            "1" => w1 += b,
            "2" => w2 += b,
            _ => {}
        }
    }
    // wave 1 resident at its peak (K6, with the surfaces); K5's route and gathers are freed before K6
    let peak = |names: &[&str]| comps.iter().filter(|c| names.iter().any(|n| c.name.starts_with(n))).map(bytes).sum::<f64>();
    let w1_k5 = peak(&["z_b, z_s", "K5, GEN-143", "K5, a round", "K5, gathered"]);
    let w1_k6 = peak(&["z_b, z_s", "K6"]);
    let l_tiles = peak(&["per-tile L stages"]);
    eprintln!(
        "G3-6 model: wave 1 at K5 {:.2} GB, at K6 {:.2} GB (all wave-1 rows summed {:.2} GB); wave 2 {:.2} GB; L buffers {:.2} GB; components over the gate alone: {over:?}",
        w1_k5 / 1e9,
        w1_k6 / 1e9,
        w1 / 1e9,
        w2 / 1e9,
        l_tiles / 1e9
    );
    // the verdicts the report names (a change to a layout changes them, and this test with them)
    assert!(w1_k5 > GATE_B, "wave 1's K5 with GEN-143's resident arrays does not fit");
    assert!(l_tiles > GATE_B, "16 L buffers of 3,328² at today's bytes a cell do not fit");
    assert!(w2 < GATE_B, "wave 2 portal-only fits alone");
    // what would fit: the L buffers' workers at today's bytes, and the H1 bytes a cell a resident wave 1 may hold
    let per_buffer = l_tiles / WORKERS;
    eprintln!(
        "G3-6 model: one L buffer {:.2} GB, so {:.0} workers fit the gate alone; a resident wave 1 must hold under {:.0} B an H1 land cell",
        per_buffer / 1e9,
        (GATE_B / per_buffer).floor(),
        GATE_B / h1
    );
}
