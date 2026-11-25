//! The calibration of geometric gate metrics (TEST-155, BUILD-069): straight synthetic valleys at five angles, with a smooth floor and three lattice defects, on three
//! backgrounds. A metric passes when every smooth case reads inside its band (it is angle-invariant) and every defect case reads outside it (it detects its defect).
//! Every function is pure: the same input gives the same bits. Spec values live in TEST-155's and BUILD-069's Detail.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use planet_generators::bake::Grid;

use crate::river_suite;

/// The cases' grid: `N` x `N` cells of R1h's L cell.
pub const N: usize = 256;
pub const CELL_M: f64 = 101.802;
/// The valleys' angles against the grid's x axis (degrees, counter-clockwise towards +y).
pub const ANGLES_DEG: [f64; 5] = [0.0, 22.5, 45.0, 67.5, 135.0];
/// The valley's axis passes this far (cells) across itself from the grid's centre, so that no floor lies on cell centres.
pub const OFFSET_CELLS: f64 = 0.37;
/// The zone: the cells within this many cells of the axis.
pub const ZONE_CELLS: f64 = 12.0;
/// The box: the grid less this many cells at each edge.
pub const BOX_MARGIN: usize = 16;
/// The valley wall's slope (m/m) and its depth (m) at the grid's centre; the depth grows downstream by `GRADE` (m/m), so the floor falls at `GRADE`.
pub const WALL: f64 = 0.12;
pub const DEPTH_M: f64 = 80.0;
pub const GRADE: f64 = 0.004;
/// The sawtooth: `SAW_M` times `(x mod SAW_PERIOD) / SAW_PERIOD` (the cell's x index) added on the cells within `SAW_HALF_WIDTH` cells of the axis.
pub const SAW_M: f64 = 5.0;
pub const SAW_PERIOD: usize = 8;
pub const SAW_HALF_WIDTH: f64 = 1.5;
/// The flat background's height (m).
pub const FLAT_M: f64 = 200.0;
/// The fBm background: integer-hashed value noise with a quintic fade, one octave per lattice spacing (cells; primes, so that no octave is locked to the 8-cell phase),
/// the first octave's amplitude `FBM_AMP_M`, each next one half, on `FLAT_M`.
pub const FBM_SPACINGS: [usize; 4] = [37, 19, 11, 7];
pub const FBM_AMP_M: f64 = 20.0;
pub const FBM_SEED: u64 = 155;
/// The straight-run measure counts runs longer than this (m).
pub const STRAIGHT_RUN_M: f64 = 2000.0;
/// M4 reads the cells within this many cells of the channel path.
pub const M4_CELLS: f64 = 4.0;

/// The valley's cross-section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// `WALL * d`, `d` the distance to the floor (m).
    V,
    /// `WALL * (sqrt(d^2 + L^2) - L)`, L one cell: a round floor.
    Round,
}

/// The valley's floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floor {
    /// The distance to the true axis.
    Smooth,
    /// The distance to the axis's 4-connected raster cells: a staircase floor.
    Stair4,
    /// The distance to the axis's 8-connected raster cells.
    Stair8,
    /// The smooth floor plus a lattice-locked sawtooth.
    Sawtooth,
}

pub const PROFILES: [Profile; 2] = [Profile::V, Profile::Round];
pub const FLOORS: [Floor; 4] = [Floor::Smooth, Floor::Stair4, Floor::Stair8, Floor::Sawtooth];

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::V => "v",
            Profile::Round => "round",
        }
    }
}

impl Floor {
    pub fn name(self) -> &'static str {
        match self {
            Floor::Smooth => "smooth",
            Floor::Stair4 => "stair4",
            Floor::Stair8 => "stair8",
            Floor::Sawtooth => "sawtooth",
        }
    }

    /// Whether this floor is a defect at `angle_deg`: a staircase along a raster that is itself a straight line (stair4 at multiples of 90 degrees, stair8 at multiples
    /// of 45) is a shifted smooth floor, not a defect.
    pub fn is_defect_at(self, angle_deg: f64) -> bool {
        let on = |step: f64| (angle_deg / step).fract() == 0.0;
        match self {
            Floor::Smooth => false,
            Floor::Stair4 => !on(90.0),
            Floor::Stair8 => !on(45.0),
            Floor::Sawtooth => true,
        }
    }
}

/// A background: the reference surface the valley is carved into (`N` x `N` heights, row-major, y down the rows).
#[derive(Clone, Debug)]
pub struct Background {
    pub name: String,
    pub z: Vec<f64>,
}

impl Background {
    /// `FLAT_M` everywhere.
    pub fn flat() -> Background {
        Background { name: "none".into(), z: vec![FLAT_M; N * N] }
    }

    /// The fBm background (`FBM_*`).
    pub fn fbm() -> Background {
        let z = (0..N * N)
            .map(|c| {
                let (x, y) = ((c % N) as f64, (c / N) as f64);
                let mut v = FLAT_M;
                let mut amp = FBM_AMP_M;
                for (k, &sp) in FBM_SPACINGS.iter().enumerate() {
                    v += amp * value_noise(x / sp as f64, y / sp as f64, k as u64);
                    amp *= 0.5;
                }
                v
            })
            .collect();
        Background { name: "fbm".into(), z }
    }

    /// A background from `N` x `N` little-endian f32 heights (the S0 fixture).
    pub fn from_f32_le(name: &str, bytes: &[u8]) -> Result<Background, String> {
        if bytes.len() != N * N * 4 {
            return Err(format!("background {name}: {} bytes, not {} ({N} x {N} f32)", bytes.len(), N * N * 4));
        }
        let z: Vec<f64> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b) as f64).collect();
        if let Some(c) = z.iter().position(|v| !v.is_finite()) {
            return Err(format!("background {name}: cell {c} is not finite"));
        }
        Ok(Background { name: name.into(), z })
    }
}

fn mix(mut h: u64) -> u64 {
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

/// The lattice value at `(ix, iy)` of octave `k`, in [-1, 1).
fn lattice(ix: i64, iy: i64, k: u64) -> f64 {
    let h = mix(FBM_SEED ^ (k << 56) ^ (ix as u32 as u64) ^ ((iy as u32 as u64) << 32));
    (h >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

fn value_noise(x: f64, y: f64, k: u64) -> f64 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i64, fy as i64);
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v) = (fade(x - fx), fade(y - fy));
    let (a, b) = (lattice(ix, iy, k), lattice(ix + 1, iy, k));
    let (c, d) = (lattice(ix, iy + 1, k), lattice(ix + 1, iy + 1, k));
    let top = a + (b - a) * u;
    let bot = c + (d - c) * u;
    top + (bot - top) * v
}

/// One straight valley's geometry: the axis through `centre` along `(ux, uy)`, its normal `(nx, ny)`.
#[derive(Clone, Copy, Debug)]
struct Axis {
    cx: f64,
    cy: f64,
    ux: f64,
    uy: f64,
    nx: f64,
    ny: f64,
}

impl Axis {
    fn new(angle_deg: f64) -> Axis {
        let t = angle_deg * std::f64::consts::PI / 180.0;
        let (ux, uy) = (libm::cos(t), libm::sin(t));
        let (nx, ny) = (-uy, ux);
        let mid = (N as f64 - 1.0) / 2.0;
        Axis { cx: mid + OFFSET_CELLS * nx, cy: mid + OFFSET_CELLS * ny, ux, uy, nx, ny }
    }

    /// (along, across) of the point `(x, y)`, in cells.
    fn coords(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = (x - self.cx, y - self.cy);
        (dx * self.ux + dy * self.uy, dx * self.nx + dy * self.ny)
    }

    /// The axis's raster cells: the cell under the axis at quarter-cell steps; for `four`, a diagonal step is split by the axis step whose cell lies nearer the axis
    /// (the first on a tie).
    fn raster(&self, four: bool) -> Vec<(i64, i64)> {
        let mut cells: Vec<(i64, i64)> = Vec::new();
        let span = 2 * N as i64 * 4;
        for q in -span..=span {
            let t = q as f64 * 0.25;
            let (x, y) = ((self.cx + t * self.ux + 0.5).floor() as i64, (self.cy + t * self.uy + 0.5).floor() as i64);
            let Some(&(lx, ly)) = cells.last() else {
                cells.push((x, y));
                continue;
            };
            if (x, y) == (lx, ly) {
                continue;
            }
            if four && x != lx && y != ly {
                let (a, b) = ((x, ly), (lx, y));
                let da = self.coords(a.0 as f64, a.1 as f64).1.abs();
                let db = self.coords(b.0 as f64, b.1 as f64).1.abs();
                cells.push(if da <= db { a } else { b });
            }
            cells.push((x, y));
        }
        let n = N as i64;
        cells.retain(|&(x, y)| x >= -1 && y >= -1 && x <= n && y <= n);
        cells
    }
}

/// One case: the reference (the background), the surface (the background with the valley carved in), the zone, the counted cells and the channel path.
#[derive(Clone, Debug)]
pub struct Case {
    pub angle_deg: f64,
    pub profile: Profile,
    pub floor: Floor,
    pub background: String,
    pub reference: Vec<f64>,
    pub surface: Vec<f64>,
    /// The cells within `ZONE_CELLS` of the axis.
    pub zone: Vec<bool>,
    /// The box's land cells (the reference above 0).
    pub inside: Vec<bool>,
    /// The channel path, upstream first: from the box's zone cell nearest the axis with the least along-axis coordinate, each step to the 8-neighbour in the box's zone
    /// with a larger along-axis coordinate and the steepest descent (drop over step length, possibly negative; ties by cell index), until it reaches the box's edge
    /// downstream of the centre or has no such neighbour.
    pub channel: Vec<usize>,
}

impl Case {
    /// Whether this case is a defect (its floor at its angle; see [`Floor::is_defect_at`]).
    pub fn is_defect(&self) -> bool {
        self.floor.is_defect_at(self.angle_deg)
    }
}

/// The grid all cases share.
pub fn grid() -> Grid {
    Grid::new(N, N, CELL_M)
}

/// The distance (cells) from each cell to the nearest raster cell; cells more than `ZONE_CELLS + 4` cells off the axis keep their distance to the true axis (their carve
/// is 0 either way: the deepest valley reaches its wall top within `ZONE_CELLS`).
fn raster_distance(axis: &Axis, raster: &[(i64, i64)]) -> Vec<f64> {
    let rs: Vec<(f64, f64, f64)> = raster.iter().map(|&(x, y)| (x as f64, y as f64, axis.coords(x as f64, y as f64).0)).collect();
    (0..N * N)
        .map(|c| {
            let (x, y) = ((c % N) as f64, (c / N) as f64);
            let (s, d) = axis.coords(x, y);
            if d.abs() > ZONE_CELLS + 4.0 {
                return d.abs();
            }
            let mut best = f64::INFINITY;
            for &(rx, ry, rsv) in &rs {
                if (rsv - s).abs() <= ZONE_CELLS + 6.0 {
                    let (ex, ey) = (rx - x, ry - y);
                    best = best.min(ex * ex + ey * ey);
                }
            }
            best.sqrt()
        })
        .collect()
}

/// The floor distances of one angle: per floor, the distance (cells) each cell's carve is read at.
struct AngleFloors {
    axis: Axis,
    smooth: Vec<f64>,
    stair4: Vec<f64>,
    stair8: Vec<f64>,
}

impl AngleFloors {
    fn new(angle_deg: f64) -> AngleFloors {
        let axis = Axis::new(angle_deg);
        let smooth = (0..N * N).map(|c| axis.coords((c % N) as f64, (c / N) as f64).1.abs()).collect();
        let stair4 = raster_distance(&axis, &axis.raster(true));
        let stair8 = raster_distance(&axis, &axis.raster(false));
        AngleFloors { axis, smooth, stair4, stair8 }
    }
}

/// The carve (m, at most 0) at a cell of along-axis coordinate `s` and floor distance `d` (cells).
pub fn carve(profile: Profile, s: f64, d: f64) -> f64 {
    let depth = DEPTH_M + GRADE * CELL_M * s;
    let dm = d * CELL_M;
    let reach = match profile {
        Profile::V => dm,
        Profile::Round => (dm * dm + CELL_M * CELL_M).sqrt() - CELL_M,
    };
    (WALL * reach).min(depth) - depth
}

fn build_case(af: &AngleFloors, angle_deg: f64, profile: Profile, floor: Floor, bg: &Background) -> Case {
    let axis = &af.axis;
    let dist = match floor {
        Floor::Smooth | Floor::Sawtooth => &af.smooth,
        Floor::Stair4 => &af.stair4,
        Floor::Stair8 => &af.stair8,
    };
    let mut surface = Vec::with_capacity(N * N);
    let mut zone = Vec::with_capacity(N * N);
    let mut inside = Vec::with_capacity(N * N);
    let mut along = Vec::with_capacity(N * N);
    for (c, (&zb, &fd)) in bg.z.iter().zip(dist.iter()).enumerate() {
        let (x, y) = (c % N, c / N);
        let (s, d) = axis.coords(x as f64, y as f64);
        let mut v = zb + carve(profile, s, fd);
        if floor == Floor::Sawtooth && d.abs() <= SAW_HALF_WIDTH {
            v += SAW_M * (x % SAW_PERIOD) as f64 / SAW_PERIOD as f64;
        }
        surface.push(v);
        zone.push(d.abs() <= ZONE_CELLS);
        let in_box = x >= BOX_MARGIN && y >= BOX_MARGIN && x < N - BOX_MARGIN && y < N - BOX_MARGIN;
        inside.push(in_box && zb > 0.0);
        along.push(s);
    }
    let channel = channel_path(&surface, &zone, &inside, &along, axis);
    Case { angle_deg, profile, floor, background: bg.name.clone(), reference: bg.z.clone(), surface, zone, inside, channel }
}

fn channel_path(z: &[f64], zone: &[bool], inside: &[bool], along: &[f64], axis: &Axis) -> Vec<usize> {
    let ok = |c: usize| zone[c] && inside[c];
    let near = |c: usize| axis.coords((c % N) as f64, (c / N) as f64).1.abs() <= 0.75;
    let Some(start) = (0..N * N).filter(|&c| ok(c) && near(c)).min_by(|&a, &b| along[a].total_cmp(&along[b]).then(a.cmp(&b))) else {
        return Vec::new();
    };
    let mut path = vec![start];
    let mut c = start;
    let edge = |c: usize| {
        let (x, y) = (c % N, c / N);
        x == BOX_MARGIN || y == BOX_MARGIN || x == N - BOX_MARGIN - 1 || y == N - BOX_MARGIN - 1
    };
    loop {
        if along[c] > 0.0 && edge(c) {
            return path;
        }
        let (x, y) = ((c % N) as i64, (c / N) as i64);
        let mut best: Option<(f64, usize)> = None;
        for &(dx, dy) in &[(-1i64, -1i64), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (qx, qy) = (x + dx, y + dy);
            if qx < 0 || qy < 0 || qx >= N as i64 || qy >= N as i64 {
                continue;
            }
            let q = (qy * N as i64 + qx) as usize;
            if !ok(q) || along[q] <= along[c] {
                continue;
            }
            let len = if dx != 0 && dy != 0 { std::f64::consts::SQRT_2 } else { 1.0 };
            let slope = (z[c] - z[q]) / len;
            if best.is_none_or(|(bs, bq)| slope > bs || (slope == bs && q < bq)) {
                best = Some((slope, q));
            }
        }
        match best {
            Some((_, q)) => {
                path.push(q);
                c = q;
            }
            None => return path,
        }
    }
}

/// Every case, in the order backgrounds, profiles, floors, angles; `visit` sees each once (the cases are built one at a time).
pub fn for_each_case(backgrounds: &[Background], mut visit: impl FnMut(&Case)) {
    let floors: Vec<AngleFloors> = ANGLES_DEG.iter().map(|&a| AngleFloors::new(a)).collect();
    for bg in backgrounds {
        for &profile in &PROFILES {
            for &floor in &FLOORS {
                for (ai, &a) in ANGLES_DEG.iter().enumerate() {
                    visit(&build_case(&floors[ai], a, profile, floor, bg));
                }
            }
        }
    }
}

/// A geometric metric under calibration.
pub trait GeometricMetric {
    /// The metric's name (its report's file name).
    fn name(&self) -> &str;
    /// The band a defect-free valley reads inside (inclusive), or `None` for a metric that is only reported.
    fn band(&self) -> Option<(f64, f64)>;
    /// The metric's reading on one case.
    fn measure(&self, grid: &Grid, case: &Case) -> f64;
}

/// One case's reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub background: String,
    pub profile: Profile,
    pub floor: Floor,
    pub angle_deg: f64,
    pub value: f64,
}

impl Reading {
    fn label(&self) -> String {
        format!("{} {} {} at {} deg: {}", self.background, self.profile.name(), self.floor.name(), self.angle_deg, self.value)
    }
}

/// A calibration's verdict.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Pass,
    /// No band: the readings are reported only.
    ReportOnly,
    /// A smooth case reads outside the band (the first such case).
    NotAngleInvariant(String),
    /// A defect case reads inside the band (the first such case).
    MissesItsDefect(String),
}

impl Verdict {
    pub fn passed(&self) -> bool {
        *self == Verdict::Pass
    }

    pub fn describe(&self) -> String {
        match self {
            Verdict::Pass => "PASS".into(),
            Verdict::ReportOnly => "REPORT ONLY (no band)".into(),
            Verdict::NotAngleInvariant(c) => format!("FAIL angle-invariant: a smooth valley reads outside the band ({c})"),
            Verdict::MissesItsDefect(c) => format!("FAIL detects its defect: a defect reads inside the band ({c})"),
        }
    }
}

/// One metric's calibration.
#[derive(Clone, Debug)]
pub struct Calibration {
    pub name: String,
    pub band: Option<(f64, f64)>,
    pub readings: Vec<Reading>,
    pub verdict: Verdict,
}

fn in_band(band: (f64, f64), v: f64) -> bool {
    band.0 <= v && v <= band.1
}

/// The verdict on a set of readings: smooth cases inside the band, then defect cases outside it (a NaN reading is outside every band).
pub fn judge(band: Option<(f64, f64)>, readings: &[Reading]) -> Verdict {
    let Some(b) = band else {
        return Verdict::ReportOnly;
    };
    if let Some(r) = readings.iter().find(|r| r.floor == Floor::Smooth && !in_band(b, r.value)) {
        return Verdict::NotAngleInvariant(r.label());
    }
    if let Some(r) = readings.iter().find(|r| r.floor.is_defect_at(r.angle_deg) && in_band(b, r.value)) {
        return Verdict::MissesItsDefect(r.label());
    }
    Verdict::Pass
}

/// Calibrates `metrics` over every case on `backgrounds`.
pub fn calibrate(metrics: &[&dyn GeometricMetric], backgrounds: &[Background]) -> Vec<Calibration> {
    let g = grid();
    let mut readings: Vec<Vec<Reading>> = vec![Vec::new(); metrics.len()];
    for_each_case(backgrounds, |case| {
        for (m, out) in metrics.iter().zip(readings.iter_mut()) {
            out.push(Reading {
                background: case.background.clone(),
                profile: case.profile,
                floor: case.floor,
                angle_deg: case.angle_deg,
                value: m.measure(&g, case),
            });
        }
    });
    metrics
        .iter()
        .zip(readings)
        .map(|(m, r)| Calibration { name: m.name().to_string(), band: m.band(), verdict: judge(m.band(), &r), readings: r })
        .collect()
}

impl Calibration {
    /// The table: one row per background, profile and floor, one column per angle; each value marked `in` or `out` of the band, a defect case's mark starred.
    pub fn table(&self) -> String {
        let mut s = String::new();
        let band = match self.band {
            Some((lo, hi)) => format!("{lo} to {hi}"),
            None => "none".into(),
        };
        let _ = writeln!(s, "metric {}  band {band}", self.name);
        let _ = writeln!(s, "verdict {}", self.verdict.describe());
        let _ = writeln!(s, "a starred mark is a defect case (it must read out); an unstarred one is smooth or a shifted smooth floor");
        let _ = write!(s, "{:<10} {:<6} {:<9}", "background", "prof", "floor");
        for a in ANGLES_DEG {
            let _ = write!(s, " {:>18}", format!("{a} deg"));
        }
        s.push('\n');
        let mut i = 0;
        while i < self.readings.len() {
            let r0 = &self.readings[i];
            let _ = write!(s, "{:<10} {:<6} {:<9}", r0.background, r0.profile.name(), r0.floor.name());
            for r in self.readings[i..].iter().take(ANGLES_DEG.len()) {
                let mark = match self.band {
                    Some(b) if in_band(b, r.value) => "in",
                    Some(_) => "out",
                    None => "",
                };
                let star = if r.floor.is_defect_at(r.angle_deg) { "*" } else { " " };
                let _ = write!(s, " {:>13.4} {mark:>3}{star}", r.value);
            }
            s.push('\n');
            i += ANGLES_DEG.len();
        }
        s
    }

    /// Writes the table to `dir/<name>.txt`.
    pub fn write(&self, dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let p = dir.join(format!("{}.txt", self.name));
        std::fs::write(&p, self.table())?;
        Ok(p)
    }
}

// --- The entries (BUILD-069's Detail holds each band). ---

/// The gate's corner links added: `river_suite::corner_links` of the surface minus the reference's, over the counted cells.
pub struct CornerLinksAdded;
/// The gate's hump cuts (`river_suite::hump_cuts`), reported only.
pub struct HumpCuts;
/// The relief spectrum's slope change (`river_suite::spectrum`: the surface's beta minus the reference's), reported only.
pub struct SpectrumSlopeChange;
/// SPIKE-143's in-zone Xdiag: the diagonal kink energy's phase excess over the zone's counted cells.
pub struct XdiagInZone;
/// M4 along the channel, added to the reference's (m).
pub struct M4Added;
/// The channel path's sinuosity: its length over its chord.
pub struct D8Sinuosity;
/// The share of the channel path's length in runs of one repeated step longer than `STRAIGHT_RUN_M`.
pub struct StraightRunShare;

impl GeometricMetric for CornerLinksAdded {
    fn name(&self) -> &str {
        "corner-links-added"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((0.0, 0.0))
    }
    fn measure(&self, g: &Grid, c: &Case) -> f64 {
        river_suite::corner_links(g, &c.surface, &c.inside) as f64 - river_suite::corner_links(g, &c.reference, &c.inside) as f64
    }
}

impl GeometricMetric for HumpCuts {
    fn name(&self) -> &str {
        "hump-cuts"
    }
    fn band(&self) -> Option<(f64, f64)> {
        None
    }
    fn measure(&self, g: &Grid, c: &Case) -> f64 {
        river_suite::hump_cuts(g, &c.reference, &c.surface, &c.inside) as f64
    }
}

impl GeometricMetric for SpectrumSlopeChange {
    fn name(&self) -> &str {
        "spectrum-slope-change"
    }
    fn band(&self) -> Option<(f64, f64)> {
        None
    }
    fn measure(&self, g: &Grid, c: &Case) -> f64 {
        river_suite::spectrum(g, &c.surface, &c.inside).beta - river_suite::spectrum(g, &c.reference, &c.inside).beta
    }
}

/// SPIKE-143's M1 X_diag over `mask` (the grid's border cells left out): d1 = (z(x+1, y+1) - 2 z + z(x-1, y-1)) / 2 against (x - y) mod 8, d2 = (z(x-1, y+1) - 2 z +
/// z(x+1, y-1)) / 2 against (x + y) mod 8; per diagonal the largest share of its squared energy held by two adjacent phases (cyclic); X_diag = the mean of the two over
/// 2 / 8, minus 1. The phases are local: a background's origin is a multiple of 8.
pub fn xdiag(z: &[f64], mask: &[bool]) -> f64 {
    let (mut h1, mut h2) = ([0.0f64; 8], [0.0f64; 8]);
    for y in 1..N - 1 {
        for x in 1..N - 1 {
            let c = y * N + x;
            if !mask[c] {
                continue;
            }
            let d1 = (z[c + N + 1] - 2.0 * z[c] + z[c - N - 1]) / 2.0;
            let d2 = (z[c + N - 1] - 2.0 * z[c] + z[c - N + 1]) / 2.0;
            h1[(x + 8 * N - y) % 8] += d1 * d1;
            h2[(x + y) % 8] += d2 * d2;
        }
    }
    let ls = |h: &[f64; 8]| {
        let t = h.iter().sum::<f64>().max(1e-30);
        (0..8).map(|i| (h[i] + h[(i + 1) % 8]) / t).fold(0.0, f64::max)
    };
    (ls(&h1) + ls(&h2)) / 2.0 / 0.25 - 1.0
}

impl GeometricMetric for XdiagInZone {
    fn name(&self) -> &str {
        "xdiag-in-zone"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((-1.0, 0.010))
    }
    fn measure(&self, _g: &Grid, c: &Case) -> f64 {
        let mask: Vec<bool> = (0..N * N).map(|k| c.zone[k] && c.inside[k]).collect();
        xdiag(&c.surface, &mask)
    }
}

fn bilinear(z: &[f64], x: f64, y: f64) -> f64 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = ((fx as usize).min(N - 2), (fy as usize).min(N - 2));
    let (u, v) = (x - ix as f64, y - iy as f64);
    let c = iy * N + ix;
    let top = z[c] + (z[c + 1] - z[c]) * u;
    let bot = z[c + N] + (z[c + N + 1] - z[c + N]) * u;
    top + (bot - top) * v
}

/// M4 along the channel: the counted cells within `M4_CELLS` of the channel path's segments (between consecutive cell centres), each taking the unit direction `t` of
/// its nearest segment (the first on a tie); k = z(p + t) - 2 z(p) + z(p - t), bilinear; the RMS of k. Returns (surface, reference) on the same cells and directions.
pub fn m4_along(c: &Case) -> (f64, f64) {
    let pts: Vec<(f64, f64)> = c.channel.iter().map(|&k| ((k % N) as f64, (k / N) as f64)).collect();
    if pts.len() < 2 {
        return (f64::NAN, f64::NAN);
    }
    let (mut ss, mut sr, mut n) = (0.0, 0.0, 0usize);
    for k in 0..N * N {
        if !c.inside[k] || !c.zone[k] {
            continue;
        }
        let (px, py) = ((k % N) as f64, (k / N) as f64);
        let mut best: Option<(f64, f64, f64)> = None;
        for w in pts.windows(2) {
            let ((ax, ay), (bx, by)) = (w[0], w[1]);
            let (vx, vy) = (bx - ax, by - ay);
            let l2 = vx * vx + vy * vy;
            let t = (((px - ax) * vx + (py - ay) * vy) / l2).clamp(0.0, 1.0);
            let (qx, qy) = (ax + t * vx - px, ay + t * vy - py);
            let d2 = qx * qx + qy * qy;
            if best.is_none_or(|(bd, _, _)| d2 < bd) {
                let l = l2.sqrt();
                best = Some((d2, vx / l, vy / l));
            }
        }
        let Some((d2, tx, ty)) = best else { continue };
        if d2 > M4_CELLS * M4_CELLS {
            continue;
        }
        let ks = bilinear(&c.surface, px + tx, py + ty) - 2.0 * c.surface[k] + bilinear(&c.surface, px - tx, py - ty);
        let kr = bilinear(&c.reference, px + tx, py + ty) - 2.0 * c.reference[k] + bilinear(&c.reference, px - tx, py - ty);
        ss += ks * ks;
        sr += kr * kr;
        n += 1;
    }
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    ((ss / n as f64).sqrt(), (sr / n as f64).sqrt())
}

impl GeometricMetric for M4Added {
    fn name(&self) -> &str {
        "m4-along-added"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((-0.5, 0.5))
    }
    fn measure(&self, _g: &Grid, c: &Case) -> f64 {
        let (s, r) = m4_along(c);
        s - r
    }
}

/// The channel path's steps (dx, dy) in cells.
fn steps(c: &Case) -> Vec<(i64, i64)> {
    c.channel.windows(2).map(|w| ((w[1] % N) as i64 - (w[0] % N) as i64, (w[1] / N) as i64 - (w[0] / N) as i64)).collect()
}

fn step_len(s: (i64, i64)) -> f64 {
    if s.0 != 0 && s.1 != 0 {
        std::f64::consts::SQRT_2
    } else {
        1.0
    }
}

impl GeometricMetric for D8Sinuosity {
    fn name(&self) -> &str {
        "d8-sinuosity"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((1.0, 1.02))
    }
    fn measure(&self, _g: &Grid, c: &Case) -> f64 {
        let (Some(&a), Some(&b)) = (c.channel.first(), c.channel.last()) else {
            return f64::NAN;
        };
        let len: f64 = steps(c).into_iter().map(step_len).sum();
        let (dx, dy) = ((b % N) as f64 - (a % N) as f64, (b / N) as f64 - (a / N) as f64);
        len / (dx * dx + dy * dy).sqrt()
    }
}

impl GeometricMetric for StraightRunShare {
    fn name(&self) -> &str {
        "straight-run-share"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((0.0, 0.5))
    }
    fn measure(&self, _g: &Grid, c: &Case) -> f64 {
        let st = steps(c);
        let total: f64 = st.iter().copied().map(step_len).sum();
        if total == 0.0 {
            return f64::NAN;
        }
        let mut straight = 0.0;
        let mut i = 0;
        while i < st.len() {
            let mut j = i;
            while j < st.len() && st[j] == st[i] {
                j += 1;
            }
            let run = (j - i) as f64 * step_len(st[i]);
            if run * CELL_M > STRAIGHT_RUN_M {
                straight += run;
            }
            i = j;
        }
        straight / total
    }
}

/// The entries registered under BUILD-069: each must pass its calibration (none yet).
pub fn registered() -> Vec<Box<dyn GeometricMetric>> {
    Vec::new()
}

/// The entries that predate BUILD-069: calibrated and reported, their verdicts printed, not asserted (CON-31: a failing one goes to the owner).
pub fn reported() -> Vec<Box<dyn GeometricMetric>> {
    vec![
        Box::new(CornerLinksAdded),
        Box::new(HumpCuts),
        Box::new(SpectrumSlopeChange),
        Box::new(XdiagInZone),
        Box::new(M4Added),
        Box::new(D8Sinuosity),
        Box::new(StraightRunShare),
    ]
}
