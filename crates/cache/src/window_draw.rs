//! The G3-8 out-of-sample windows (BUILD-066, lead #186a): a seeded stratified draw over the seed-14 world's coarse field. The candidates are the five windows' size on a fixed
//! lattice of each face; each is classed from samples of the coarse field only (base elevation, runoff, latitude) by the rules below; the draw takes, per stratum in a fixed order,
//! the candidates in the order of a seeded hash, skipping any that overlap one of the five windows or a window already drawn. Nobody looks at a window's terrain before the
//! draw is committed: the rules and the seed decide it.

/// The regional cells of a face edge.
pub const REGIONAL: usize = 12_288;
/// A window's side in regional cells: the five windows' (620).
pub const SIZE: usize = 620;
/// A window is on a face edge when its box lies within this many regional cells of it: the gate's margin (200) and the 4 cells of the C1 read, so the window's solve
/// crosses the edge.
pub const EDGE_BAND: usize = 204;
/// The world the windows are drawn from (seed 14) and the seed of the draw (recorded in the data file).
pub const WORLD_SEED: u64 = 14;
pub const DRAW_SEED: u64 = 0x2026_1123;

/// The five windows of G1 to G3 (BUILD-055), `face, i0, i1, j0, j1`: R1h, R3a, R3b, M1, F1.
pub const FIVE: [Box; 5] = [
    Box { face: 1, i0: 8056, i1: 8675, j0: 760, j1: 1379 },
    Box { face: 4, i0: 368, i1: 987, j0: 4720, j1: 5339 },
    Box { face: 2, i0: 9968, i1: 10587, j0: 8304, j1: 8923 },
    Box { face: 4, i0: 6600, i1: 7219, j0: 10200, j1: 10819 },
    Box { face: 4, i0: 4800, i1: 5419, j0: 3300, j1: 3919 },
];

/// A window's box in regional cells, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box {
    pub face: u8,
    pub i0: usize,
    pub i1: usize,
    pub j0: usize,
    pub j1: usize,
}

impl Box {
    pub fn overlaps(&self, o: &Box) -> bool {
        self.face == o.face && self.i0 <= o.i1 && o.i0 <= self.i1 && self.j0 <= o.j1 && o.j0 <= self.j1
    }

    /// The face edges whose band holds the box (0 to 4).
    pub fn edges_near(&self) -> usize {
        usize::from(self.i0 < EDGE_BAND)
            + usize::from(self.j0 < EDGE_BAND)
            + usize::from(self.i1 + EDGE_BAND >= REGIONAL)
            + usize::from(self.j1 + EDGE_BAND >= REGIONAL)
    }
}

/// The candidates of one face: boxes of `SIZE` at `i0, j0` in `0, SIZE, 2 SIZE, ...` while they fit, and one more at `REGIONAL - SIZE` on each axis, so both far edges have
/// windows on them.
pub fn candidates(face: u8) -> Vec<Box> {
    let mut at: Vec<usize> = (0..).map(|k| k * SIZE).take_while(|&p| p + SIZE <= REGIONAL).collect();
    at.push(REGIONAL - SIZE);
    at.dedup();
    let mut out = Vec::new();
    for &j0 in &at {
        for &i0 in &at {
            out.push(Box { face, i0, i1: i0 + SIZE - 1, j0, j1: j0 + SIZE - 1 });
        }
    }
    out
}

/// A window's coarse statistics: the land share of its samples (base elevation above 0 m), the p50 and p90 of the land samples' elevation (m), their mean runoff (mm a year),
/// and the absolute latitude of the box's centre (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub land: f64,
    pub land_p50_m: f64,
    pub land_p90_m: f64,
    pub runoff_mm: f64,
    pub abs_lat_deg: f64,
}

/// The strata of lead #186a, with their quota, in the draw's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stratum {
    CubeCorner,
    CubeEdge,
    HighMountain,
    Arid,
    FlatCoast,
    MixedMidLatitude,
}

impl Stratum {
    pub const ALL: [Stratum; 6] =
        [Stratum::CubeCorner, Stratum::CubeEdge, Stratum::HighMountain, Stratum::Arid, Stratum::FlatCoast, Stratum::MixedMidLatitude];

    pub fn quota(self) -> usize {
        match self {
            Stratum::CubeCorner | Stratum::MixedMidLatitude => 1,
            _ => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Stratum::CubeCorner => "cube-corner",
            Stratum::CubeEdge => "cube-edge",
            Stratum::HighMountain => "high-mountain",
            Stratum::Arid => "arid-endorheic",
            Stratum::FlatCoast => "flat-coast",
            Stratum::MixedMidLatitude => "mixed-mid-latitude",
        }
    }
}

/// The class of a window by its box and statistics, the first rule that holds:
/// - cube corner: in the band of two face edges, land at least 20 %;
/// - cube edge: in the band of one face edge, land at least 30 %;
/// - (the rest are interior: in no band)
/// - high mountain: land at least 80 %, land p50 at least 1,500 m;
/// - arid or endorheic: land at least 80 %, land runoff under 50 mm a year;
/// - flat coast: land 20 to 80 %, land p90 under 300 m;
/// - mixed mid-latitude: land 40 to 90 %, centre latitude 30 to 60 degrees;
/// - none otherwise.
pub fn classify(b: &Box, s: &Stats) -> Option<Stratum> {
    match b.edges_near() {
        2.. => return (s.land >= 0.2).then_some(Stratum::CubeCorner),
        1 => return (s.land >= 0.3).then_some(Stratum::CubeEdge),
        _ => {}
    }
    if s.land >= 0.8 && s.land_p50_m >= 1500.0 {
        Some(Stratum::HighMountain)
    } else if s.land >= 0.8 && s.runoff_mm < 50.0 {
        Some(Stratum::Arid)
    } else if (0.2..=0.8).contains(&s.land) && s.land_p90_m < 300.0 {
        Some(Stratum::FlatCoast)
    } else if (0.4..=0.9).contains(&s.land) && (30.0..=60.0).contains(&s.abs_lat_deg) {
        Some(Stratum::MixedMidLatitude)
    } else {
        None
    }
}

/// The draw's order of a candidate: a seeded hash of its face and corner (splitmix64).
pub fn draw_key(seed: u64, b: &Box) -> u64 {
    let mut z = seed ^ (u64::from(b.face) << 56) ^ ((b.i0 as u64) << 28) ^ (b.j0 as u64);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A drawn window: its stratum, its box, its statistics and its index in the draw (0 first).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub index: usize,
    pub stratum: Stratum,
    pub window: Box,
    pub stats: Stats,
}

/// The draw over classed candidates: per stratum in `Stratum::ALL`'s order, its candidates by `draw_key`, each taken unless it overlaps one of `FIVE` or a window already
/// drawn, until the stratum's quota is met. A stratum short of candidates is refused, naming it and the count found.
pub fn draw(classed: &[(Box, Stats, Stratum)], seed: u64) -> Result<Vec<Drawn>, String> {
    let mut out: Vec<Drawn> = Vec::new();
    for st in Stratum::ALL {
        let mut pool: Vec<&(Box, Stats, Stratum)> = classed.iter().filter(|c| c.2 == st).collect();
        pool.sort_by_key(|c| (draw_key(seed, &c.0), c.0.face, c.0.i0, c.0.j0));
        let mut taken = 0usize;
        for c in pool {
            if taken == st.quota() {
                break;
            }
            if FIVE.iter().any(|f| f.overlaps(&c.0)) || out.iter().any(|d| d.window.overlaps(&c.0)) {
                continue;
            }
            out.push(Drawn { index: out.len(), stratum: st, window: c.0, stats: c.1 });
            taken += 1;
        }
        if taken < st.quota() {
            return Err(format!("stratum {} has {taken} free candidates, needs {}", st.name(), st.quota()));
        }
    }
    Ok(out)
}

/// The data file of a draw (TOML): the seeds, the rules' source, and one `[[window]]` per drawn window.
pub fn to_toml(drawn: &[Drawn], seed: u64) -> String {
    let mut s = format!(
        "# G3-8's out-of-sample windows (BUILD-066, lead #186a): a seeded stratified draw over the coarse field of the seed-{WORLD_SEED} world, by `cargo xtask windows-draw`.\n\
         # Pre-registered: no RG or B run happens on them before G3-8. A window is `face,i0,i1,j0,j1,seed` in regional cells.\n\
         world_seed = {WORLD_SEED}\ndraw_seed = {seed}\nsize = {SIZE}\n"
    );
    for d in drawn {
        let w = d.window;
        s += &format!(
            "\n[[window]]\nindex = {}\nclass = \"{}\"\nwindow = \"{},{},{},{},{},{WORLD_SEED}\"\nland = {:.3}\nland_p50_m = {:.1}\nland_p90_m = {:.1}\nrunoff_mm = {:.1}\nabs_lat_deg = {:.2}\n",
            d.index,
            d.stratum.name(),
            w.face,
            w.i0,
            w.i1,
            w.j0,
            w.j1,
            d.stats.land,
            d.stats.land_p50_m,
            d.stats.land_p90_m,
            d.stats.runoff_mm,
            d.stats.abs_lat_deg
        );
    }
    s
}
