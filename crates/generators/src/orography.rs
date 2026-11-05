//! The orographic climate: a wind field and a linear orographic precipitation model run for two seasons (GEN-117; ADR 0104 points 1 and 2; SPIKE-102 F2 and F3).
//!
//! CPU-canonical: `libm` for every transcendental, integer hashing for every random number, a **fixed number of order-independent iterations** in f64, the same bits for any thread count (CON-14).
//! The field is built in memory from a [`BaseElevation`] (the class source's base elevation `e0`, before the hill band and before the class relief driver `k`, so the class weights, which read the
//! aridity, never feed back into the rain), read bilinearly by direction, never stored, and keyed by [`OROGRAPHY_VERSION`], which a consumer folds into its keys. All numbers are start values.
//!
//! The model keeps Smith and Barstad's (2004) physics (upslope forced condensation, a condensation lag and a fall-out lag of about 1,000 s, so rain falls on the windward slope shifted a little downwind and
//! leaves a shadow behind a range) but integrates it as a Jacobi advection of two stores, the water vapour column `q` and the cloud water `c` (mm), on a lattice over the cube faces: every iteration reads
//! the previous iteration's lattice only, so no cell order exists to disagree where winds converge, rotate or reverse (the ITCZ, a monsoon). A reversing wind is two seasons, each with its own steady wind.
//! Every cell has its departure point (the cell's direction moved back along its wind, read by direction, so a step crosses face edges), its forcing and its sources computed once per season.

use crate::global::{fbm, rain_bands_mm, temperature_c};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};

/// Bump whenever the field's output changes for a seed (the formulas, the constants or the base elevation it reads); a consumer folds it into its keys.
pub const OROGRAPHY_VERSION: u32 = 1;
/// Cells per face edge of the production lattice (about 20 km a cell; a belt is 125 to 560 km wide, a narrow coastal range under 3 cells).
pub const OROGRAPHY_CELLS: usize = 512;
/// Cells per face edge of the land-fraction lattice that drives the monsoon term.
pub const LAND_CELLS: usize = 64;

/// The cells a face edge of the lattice a product or a bake builds: the production [`OROGRAPHY_CELLS`], or in a build with debug assertions (tests) the value of `PLANET_OROGRAPHY_CELLS` (`.cargo/config.toml`: a
/// coarse lattice, so that a test that builds a class-source world stays fast); a release build ignores the variable, so a product bake and a preview are always the production lattice.
pub fn default_cells() -> usize {
    if cfg!(debug_assertions) {
        if let Some(n) = std::env::var("PLANET_OROGRAPHY_CELLS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| *n >= 4) {
            return n;
        }
    }
    OROGRAPHY_CELLS
}

/// A planet's base elevation as the climate reads it. The class source implements it with `e0`; a test field implements it with a synthetic ridge.
pub trait BaseElevation: Sync {
    /// The base elevation in metres at the unit direction `d` (0 at the coast, negative at sea): never a function of the class weights.
    fn e0(&self, d: Vec3) -> f64;
    /// The land depth at `d`: positive over land, not positive over the sea (the sign alone is used, and it is cheap to evaluate).
    fn land_depth(&self, d: Vec3) -> f64;
}

/// One season of the model: which way the thermal equator has shifted (`sign` 1: northern summer, -1: northern winter, 0: the annual mean wind), the part of the year it stands for, and how much of the land-sea
/// heating term it carries (the annual-mean comparison has none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Season {
    pub sign: f64,
    pub weight: f64,
    pub monsoon: f64,
}

/// The two seasons of the product: northern summer and winter, half a year each.
pub const SEASONS: [Season; 2] = [Season { sign: 1.0, weight: 0.5, monsoon: 1.0 }, Season { sign: -1.0, weight: 0.5, monsoon: 1.0 }];
/// The preview's comparison: the annual mean wind alone, no shift, no monsoon.
pub const ANNUAL_WIND: [Season; 1] = [Season { sign: 0.0, weight: 1.0, monsoon: 0.0 }];

/// Latitude shift of the wind and rain bands with the season in degrees (the summer hemisphere's bands lie poleward).
pub const SEASON_SHIFT_DEG: f64 = 12.0;
/// Latitude shift of the zonal rain bands with the season in degrees: smaller than the wind's, so that the subtropical dry belts stay dry through the year (the equatorial belt and the storm tracks move
/// with the sun more than the highs do).
pub const RAIN_SHIFT_DEG: f64 = 5.0;
/// The turn of the wind direction by the noise in degrees at most.
pub const WIND_TURN_DEG: f64 = 25.0;
/// The strength of the land-sea heating term in m/s at most, and the land-fraction gradient (per radian) at which it is full.
pub const MONSOON_MS: f64 = 6.0;
const MONSOON_GRADIENT: f64 = 3.0;
/// The fastest wind in m/s.
pub const WIND_MAX_MS: f64 = 15.0;
/// One iteration in seconds, the number of iterations (the travel of about 1,300 km at 10 m/s), and the last ones whose rain is averaged.
pub const STEP_S: f64 = 1300.0;
pub const ITERATIONS: usize = 96;
const AVERAGED: usize = 8;
/// The condensation lag and the fall-out lag of Smith and Barstad in seconds.
pub const TAU_CONDENSATION_S: f64 = 1000.0;
pub const TAU_FALLOUT_S: f64 = 1000.0;
/// The relaxation of the vapour towards saturation over the sea and towards its recycled share over land, in seconds.
const TAU_SEA_S: f64 = 4.0e4;
const TAU_LAND_S: f64 = 6.0e4;
/// The saturation column in mm at 15 degrees C and its growth per degree (Clausius-Clapeyron, about 7 % a degree), and the land's recycled share of it.
pub const COLUMN_REF_MM: f64 = 25.0;
const COLUMN_GROWTH: f64 = 0.07;
const LAND_RECYCLE: f64 = 0.55;
/// The upslope wind (m/s) below which the terrain's roughness lifts no air: the gentle gradients of a plain make no rain.
pub const UPSLOPE_THRESHOLD_MS: f64 = 0.02;
/// The depth of the layer the upslope wind lifts (m) and the share of the lifted vapour that condenses (the model's precipitation efficiency).
const LIFT_DEPTH_M: f64 = 2000.0;
pub const EFFICIENCY: f64 = 0.18;
const YEAR_S: f64 = 3.1557e7;
const EARTH_RADIUS_M: f64 = 6_371_000.0;
const LAYER_WIND: u64 = 0x4f52_0001;

/// The saturation water column in mm at temperature `t_c`.
pub fn column_saturation_mm(t_c: f64) -> f64 {
    COLUMN_REF_MM * libm::exp(COLUMN_GROWTH * (t_c.clamp(-40.0, 40.0) - 15.0))
}

fn gauss(x: f64, mean: f64, width: f64) -> f64 {
    let z = (x - mean) / width;
    libm::exp(-z * z)
}

/// The east and north unit vectors of the tangent plane at `d` (an arbitrary pair at the poles).
fn frame(d: Vec3) -> (Vec3, Vec3) {
    let e = Vec3::new(-d.y, d.x, 0.0);
    let east = if e.length() > 1e-9 { e.normalized() } else { Vec3::new(0.0, 1.0, 0.0) };
    (east, d.cross(east))
}

/// `d` moved by the angle `angle` along the unit tangent `t`.
fn step(d: Vec3, t: Vec3, angle: f64) -> Vec3 {
    (d * libm::cos(angle) + t * libm::sin(angle)).normalized()
}

/// A scalar lattice over the cube faces, `cells` cells a face edge, `(cells + 1)^2` nodes a face (the class lattice's geometry), read bilinearly by direction.
struct Grid {
    n: usize,
}

impl Grid {
    fn dir(&self, idx: usize) -> Vec3 {
        let per = (self.n + 1) * (self.n + 1);
        let (f, r) = (idx / per, idx % per);
        let (i, j) = (r % (self.n + 1), r / (self.n + 1));
        let (s, t) = (2.0 * i as f64 / self.n as f64 - 1.0, 2.0 * j as f64 / self.n as f64 - 1.0);
        face_to_direction(&TangentWarp, Face(f as u8), s, t)
    }

    /// The first node of the cell holding `d` and the fractions along its two axes.
    fn locate(&self, d: Vec3) -> (usize, f32, f32) {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.n as f64;
        let (x, y) = (((s + 1.0) * 0.5 * n).clamp(0.0, n), ((t + 1.0) * 0.5 * n).clamp(0.0, n));
        let (i, j) = ((x as usize).min(self.n - 1), (y as usize).min(self.n - 1));
        let per = (self.n + 1) * (self.n + 1);
        (face.0 as usize * per + j * (self.n + 1) + i, (x - i as f64) as f32, (y - j as f64) as f32)
    }

    fn read(&self, data: &[f32], at: (usize, f32, f32)) -> f32 {
        let (k, fx, fy) = at;
        let w = self.n + 1;
        let (a, b) = (data[k] + (data[k + 1] - data[k]) * fx, data[k + w] + (data[k + w + 1] - data[k + w]) * fx);
        a + (b - a) * fy
    }

    fn at(&self, data: &[f32], d: Vec3) -> f64 {
        f64::from(self.read(data, self.locate(d)))
    }
}

/// The land fraction around each node of a coarse lattice (the share of land within about 1,000 km, from rings of 8 points at 500 and 1,000 km), the source of the monsoon term.
struct LandField {
    grid: Grid,
    land: Vec<f32>,
}

impl LandField {
    fn build(src: &dyn BaseElevation, par: &dyn Par) -> LandField {
        let grid = Grid { n: LAND_CELLS };
        let rows = par::map(par, 6 * (grid.n + 1), &|r| {
            (0..=grid.n)
                .map(|i| {
                    let d = grid.dir(r * (grid.n + 1) + i);
                    let (east, north) = frame(d);
                    let mut sum = if src.land_depth(d) > 0.0 { 1.0 } else { 0.0 };
                    for ring in [500.0e3 / EARTH_RADIUS_M, 1000.0e3 / EARTH_RADIUS_M] {
                        for k in 0..8 {
                            let a = core::f64::consts::TAU * f64::from(k) / 8.0;
                            let t = east * libm::cos(a) + north * libm::sin(a);
                            sum += if src.land_depth(step(d, t, ring)) > 0.0 { 1.0 } else { 0.0 };
                        }
                    }
                    (sum / 17.0) as f32
                })
                .collect::<Vec<f32>>()
        });
        LandField { grid, land: rows.into_iter().flatten().collect() }
    }

    /// The gradient of the land fraction at `d` per radian, as a tangent vector.
    fn gradient(&self, d: Vec3) -> Vec3 {
        let (east, north) = frame(d);
        let h = 0.08;
        let comp = |t: Vec3| (self.grid.at(&self.land, step(d, t, h)) - self.grid.at(&self.land, step(d, t, -h))) / (2.0 * h);
        east * comp(east) + north * comp(north)
    }
}

/// The wind at `d` in a season in m/s as a tangent vector: the zonal bands (trades, westerlies, polar easterlies, shifted with the season), a meridional part, a noise turn and the monsoon term.
fn wind_at(seed: u64, land: &LandField, season: &Season, d: Vec3) -> Vec3 {
    let lat = libm::asin(d.z.clamp(-1.0, 1.0)).to_degrees();
    let lat_eff = lat - SEASON_SHIFT_DEG * season.sign;
    // The meridional part changes sign at the thermal equator over a few degrees (a step would be an infinite convergence and a line of rain).
    let (a, hem) = (libm::fabs(lat_eff), libm::tanh(lat_eff / 4.0));
    let (g1, g2, g3) = (gauss(a, 15.0, 14.0), gauss(a, 45.0, 15.0), gauss(a, 78.0, 12.0));
    let zonal = -8.0 * g1 + 12.0 * g2 - 5.0 * g3;
    let meridional = hem * (-2.0 * g1 + 2.5 * g2 - 1.5 * g3);
    let turn = WIND_TURN_DEG.to_radians() * fbm(seed, LAYER_WIND, d, 2.0, 2);
    let (c, s) = (libm::cos(turn), libm::sin(turn));
    let (east, north) = frame(d);
    let mut w = east * (zonal * c - meridional * s) + north * (zonal * s + meridional * c);
    if season.monsoon > 0.0 {
        let g = land.gradient(d);
        let len = g.length();
        if len > 1e-9 {
            let own_summer = if d.z < 0.0 { -season.sign } else { season.sign };
            let strength = MONSOON_MS * season.monsoon * (len / MONSOON_GRADIENT).min(1.0) * libm::cos(lat.to_radians());
            w = w + g * (own_summer * strength / len);
        }
    }
    let speed = w.length();
    if speed > WIND_MAX_MS {
        w = w * (WIND_MAX_MS / speed);
    }
    w
}

/// The orographic climate of a planet: per lattice node the base elevation, the annual temperature and the rain of the two seasons.
pub struct Orography {
    seed: u64,
    grid: Grid,
    land: LandField,
    elevation: Vec<f32>,
    temperature: Vec<f32>,
    /// Rain in mm a year per season (the seasons' shares of the year included: their sum is the annual total).
    rain: Vec<Vec<f32>>,
    seasons: Vec<Season>,
}

impl Orography {
    /// The production climate of `src`: [`default_cells`] cells a face edge, the two [`SEASONS`], on `threads` threads (the bits do not depend on the count).
    pub fn new(seed: u64, src: &dyn BaseElevation, threads: usize) -> Orography {
        Self::build(seed, src, default_cells(), &SEASONS, threads)
    }

    /// A climate on a lattice of `cells` cells a face edge and the given `seasons` (the preview's annual-wind comparison and its finer lattice, the tests' coarse one).
    pub fn build(seed: u64, src: &dyn BaseElevation, cells: usize, seasons: &[Season], threads: usize) -> Orography {
        Self::build_iterations(seed, src, cells, seasons, ITERATIONS, threads)
    }

    /// [`Orography::build`] with `iterations` Jacobi steps (the convergence test runs twice as many as the product).
    pub fn build_iterations(
        seed: u64,
        src: &dyn BaseElevation,
        cells: usize,
        seasons: &[Season],
        iterations: usize,
        threads: usize,
    ) -> Orography {
        assert!(iterations > AVERAGED, "more iterations than the averaged ones");
        assert!(
            cells >= 4 && seasons.iter().all(|s| s.weight > 0.0),
            "a lattice of at least 4 cells and seasons that hold part of the year"
        );
        let par: &dyn Par = &threads;
        let grid = Grid { n: cells };
        let land = LandField::build(src, par);
        let w = cells + 1;
        let rows = par::map(par, 6 * w, &|r| {
            (0..w)
                .map(|i| {
                    let d = grid.dir(r * w + i);
                    let h = src.e0(d);
                    (h as f32, temperature_c(seed, d, h.max(0.0)) as f32, src.land_depth(d) > 0.0, temperature_c(seed, d, 0.0) as f32)
                })
                .collect::<Vec<_>>()
        });
        let flat: Vec<(f32, f32, bool, f32)> = rows.into_iter().flatten().collect();
        let elevation: Vec<f32> = flat.iter().map(|x| x.0).collect();
        let temperature: Vec<f32> = flat.iter().map(|x| x.1).collect();
        let is_land: Vec<bool> = flat.iter().map(|x| x.2).collect();
        // The air that carries the vapour is the air at sea level: the column it can hold is that of the sea-level temperature, not of the cold summit under it.
        let sea_level_temperature: Vec<f32> = flat.iter().map(|x| x.3).collect();
        let rain = seasons
            .iter()
            .map(|s| run_season(seed, &grid, &land, &elevation, &sea_level_temperature, &is_land, s, iterations, par))
            .collect();
        Orography { seed, grid, land, elevation, temperature, rain, seasons: seasons.to_vec() }
    }

    /// The seasons this climate was built for.
    pub fn seasons(&self) -> &[Season] {
        &self.seasons
    }

    /// The cells a face edge of the lattice.
    pub fn cells(&self) -> usize {
        self.grid.n
    }

    /// The annual precipitation at `d` in mm: the seasons' rain summed (bilinear on the lattice, so a pass-1 cell reads it per point).
    pub fn precipitation_mm(&self, d: Vec3) -> f64 {
        let at = self.grid.locate(d);
        self.rain.iter().map(|r| f64::from(self.grid.read(r, at))).sum()
    }

    /// The rain of season `k` at `d` in mm a year of that season's wind (its share of the year included).
    pub fn season_precipitation_mm(&self, k: usize, d: Vec3) -> f64 {
        self.grid.at(&self.rain[k], d)
    }

    /// The annual mean temperature at the base elevation in degrees C.
    pub fn temperature_c(&self, d: Vec3) -> f64 {
        self.grid.at(&self.temperature, d)
    }

    /// The base elevation the climate was made from in metres.
    pub fn base_elevation_m(&self, d: Vec3) -> f64 {
        self.grid.at(&self.elevation, d)
    }

    /// The aridity `P / PET` at `d` with `PET = 300 + 45 T` (SPIKE-81), PET at least 150 mm.
    pub fn aridity(&self, d: Vec3) -> f64 {
        aridity_index(self.precipitation_mm(d), self.temperature_c(d))
    }

    /// The seasonality at `d` in `[0, 1]`: the difference of the two seasons' rain over their sum (0 where there is no rain or one season).
    pub fn seasonality(&self, d: Vec3) -> f64 {
        if self.rain.len() < 2 {
            return 0.0;
        }
        let at = self.grid.locate(d);
        let (a, b) = (f64::from(self.grid.read(&self.rain[0], at)), f64::from(self.grid.read(&self.rain[1], at)));
        if a + b > 1e-9 {
            libm::fabs(a - b) / (a + b)
        } else {
            0.0
        }
    }

    /// The wind at `d` in season `k` in m/s as a tangent vector.
    pub fn wind(&self, k: usize, d: Vec3) -> Vec3 {
        wind_at(self.seed, &self.land, &self.seasons[k], d)
    }

    /// The steadiness of the wind at `d` in `[0, 1]`: the length of the mean of the seasons' winds over the mean of their lengths.
    pub fn steadiness(&self, d: Vec3) -> f64 {
        let winds: Vec<Vec3> = (0..self.seasons.len()).map(|k| self.wind(k, d)).collect();
        let mean = winds.iter().fold(Vec3::ZERO, |a, w| a + *w) * (1.0 / winds.len() as f64);
        let speeds = winds.iter().map(|w| w.length()).sum::<f64>() / winds.len() as f64;
        if speeds > 1e-9 {
            (mean.length() / speeds).min(1.0)
        } else {
            0.0
        }
    }
}

/// The aridity `P / PET` for precipitation `p_mm` a year and temperature `t_c` (`PET = 300 + 45 T`, at least 150 mm).
pub fn aridity_index(p_mm: f64, t_c: f64) -> f64 {
    p_mm / (300.0 + 45.0 * t_c).max(150.0)
}

/// The static part of a node in one season.
struct Static {
    /// The departure point's cell.
    dep: (usize, f32, f32),
    /// The fraction of the vapour that condenses in one step by the forcing, the saturation column, the frontal rain of one step in mm, and whether the node is sea.
    force: f32,
    sat: f32,
    frontal: f32,
    sea: bool,
}

fn run_season(
    seed: u64,
    grid: &Grid,
    land: &LandField,
    elevation: &[f32],
    temperature: &[f32],
    is_land: &[bool],
    season: &Season,
    iterations: usize,
    par: &dyn Par,
) -> Vec<f32> {
    let w = grid.n + 1;
    let cell = core::f64::consts::FRAC_PI_2 / grid.n as f64;
    let delta = 1.5 * cell;
    let h_of = |d: Vec3| grid.at(elevation, d).max(0.0);
    let rows = par::map(par, 6 * w, &|r| {
        (0..w)
            .map(|i| {
                let idx = r * w + i;
                let d = grid.dir(idx);
                let u = wind_at(seed, land, season, d);
                let speed = u.length();
                let dep = if speed > 1e-9 {
                    let t = u * (1.0 / speed);
                    step(d, t, -speed * STEP_S / EARTH_RADIUS_M)
                } else {
                    d
                };
                // The upslope wind: the derivative of the base elevation along the wind (sea level 0) times the speed.
                let w_oro = if speed > 1e-9 {
                    let t = u * (1.0 / speed);
                    let slope = (h_of(step(d, t, delta)) - h_of(step(d, t, -delta))) / (2.0 * delta * EARTH_RADIUS_M);
                    (speed * slope - UPSLOPE_THRESHOLD_MS).max(0.0)
                } else {
                    0.0
                };
                // The convergence of the wind (1/s): the surface divergence from the components along the tangent frame at `d`.
                let (east, north) = frame(d);
                let comp = |t: Vec3| {
                    let (p, m) = (step(d, t, delta), step(d, t, -delta));
                    (wind_at(seed, land, season, p).dot(t) - wind_at(seed, land, season, m).dot(t)) / (2.0 * delta * EARTH_RADIUS_M)
                };
                let converge = (-(comp(east) + comp(north))).max(0.0);
                let rate = EFFICIENCY * (w_oro / LIFT_DEPTH_M + converge);
                let force = (1.0 - libm::exp(-rate * STEP_S)).min(1.0 - libm::exp(-STEP_S / TAU_CONDENSATION_S));
                let lat = libm::asin(d.z.clamp(-1.0, 1.0)).to_degrees();
                let band = rain_bands_mm(lat - RAIN_SHIFT_DEG * season.sign);
                Static {
                    dep: grid.locate(dep),
                    force: force as f32,
                    sat: column_saturation_mm(f64::from(temperature[idx])) as f32,
                    frontal: (band * STEP_S / YEAR_S) as f32,
                    sea: !is_land[idx],
                }
            })
            .collect::<Vec<_>>()
    });
    let nodes: Vec<Static> = rows.into_iter().flatten().collect();
    let (ke, kl) = (1.0 - libm::exp(-STEP_S / TAU_SEA_S), 1.0 - libm::exp(-STEP_S / TAU_LAND_S));
    let ff = (1.0 - libm::exp(-STEP_S / TAU_FALLOUT_S)) as f32;
    let (ke, kl) = (ke as f32, kl as f32);
    let mut q: Vec<f32> = nodes.iter().map(|n| if n.sea { n.sat } else { LAND_RECYCLE as f32 * n.sat }).collect();
    let mut c = vec![0f32; nodes.len()];
    let mut rain = vec![0f64; nodes.len()];
    for it in 0..iterations {
        let rows = par::map(par, 6 * w, &|r| {
            (0..w)
                .map(|i| {
                    let n = &nodes[r * w + i];
                    let (mut qa, mut ca) = (grid.read(&q, n.dep), grid.read(&c, n.dep));
                    // The vapour relaxes towards saturation over the sea and towards its recycled share over land.
                    let (target, k) = if n.sea { (n.sat, ke) } else { (LAND_RECYCLE as f32 * n.sat, kl) };
                    qa += (target - qa) * k;
                    // Frontal rain of the bands, in proportion to the vapour present.
                    let rb = (n.frontal * (qa / n.sat.max(1e-3)).min(1.5)).min(0.9 * qa);
                    qa -= rb;
                    // Forced condensation moves vapour to cloud; the cloud falls out with its lag.
                    let dq = qa * n.force;
                    qa -= dq;
                    ca += dq;
                    let rc = ca * ff;
                    ca -= rc;
                    (qa, ca, f64::from(rb + rc))
                })
                .collect::<Vec<_>>()
        });
        let mut k = 0;
        for row in rows {
            for (qa, ca, rn) in row {
                q[k] = qa;
                c[k] = ca;
                if it >= iterations - AVERAGED {
                    rain[k] += rn;
                }
                k += 1;
            }
        }
    }
    // The steady rain of one step in mm over the last iterations, as mm a year of the season's wind times the season's share of the year.
    let scale = season.weight * YEAR_S / (STEP_S * AVERAGED as f64);
    rain.into_iter().map(|r| (r * scale) as f32).collect()
}
