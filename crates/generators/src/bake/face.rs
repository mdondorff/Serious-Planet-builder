//! Pass 1 of the bake on one cube face (GEN-100): the base field sampled once per cell, carved on the face's own grid, and the change (carved minus base) kept.
//!
//! The grid has `cells x cells` cells over the face in the tangent-warped face coordinates (cell `(i, j)` at `s = 2 (i + 0.5) / cells - 1`), `dx` metres apart at the face's
//! centre line (`radius * pi / 2 / cells`); the face does not route across its edges, its border cells are outlets like the sea.

use super::carve::{carve_rock, CarveParams, Rock};
use super::Grid;
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};

/// What pass 1 needs to know about a face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceSpec {
    pub face: Face,
    /// Cells per face edge (the pass's resolution: 12,288 for 814 m at Earth's size, fewer for a coarse planet pass).
    pub cells: usize,
    pub radius_m: f64,
    pub sea_level: f64,
    /// The world's stream: a face's jitter is `seed` mixed with the face, so two faces never share it.
    pub seed: u64,
    /// Cells carved beyond each edge of the face, on the plane that continues the face over the neighbouring faces' ground (a halo): the face's own border is then an ordinary
    /// cell, not an outlet, and the two faces that meet at an edge agree up to what the halo's width lets the water see. Only the face's own cells are kept. 0 is no halo.
    pub halo: usize,
}

/// The change of one face: carved minus base, in metres, per cell (f32 as stored), row-major, over the face's own cells and a halo of `halo` cells beyond each edge (the plane is
/// `(cells + 2 halo)` cells wide; cell `(i, j)` of the face is at `(i + halo, j + halo)` of the plane).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceChange {
    pub face: Face,
    pub cells: usize,
    pub halo: usize,
    pub change: Vec<f32>,
}

impl FaceChange {
    /// Cells per edge of the stored plane.
    pub fn plane_cells(&self) -> usize {
        self.cells + 2 * self.halo
    }

    /// The change of cell `(i, j)` of the face (`-halo <= i < cells + halo`: the halo cells have negative or large indices).
    pub fn at(&self, i: i64, j: i64) -> f64 {
        let (h, m) = (self.halo as i64, self.plane_cells());
        f64::from(self.change[(j + h) as usize * m + (i + h) as usize])
    }
}

impl FaceSpec {
    /// The direction of the centre of cell `(i, j)` of the face (the face's own cells; `i` and `j` below 0 or from `cells` on are the halo, see [`FaceSpec::halo_direction`]).
    pub fn direction(&self, i: usize, j: usize) -> Vec3 {
        self.halo_direction(i as i64, j as i64)
    }

    /// The direction of the centre of cell `(i, j)`, which may lie in the halo (`-halo <= i < cells + halo`): the plane coordinate continues past the face's edge.
    pub fn halo_direction(&self, i: i64, j: i64) -> Vec3 {
        let n = self.cells as f64;
        face_to_direction(&TangentWarp, self.face, 2.0 * (i as f64 + 0.5) / n - 1.0, 2.0 * (j as f64 + 0.5) / n - 1.0)
    }

    /// Cells per edge of the carved grid: the face and its halo.
    pub fn grid_cells(&self) -> usize {
        self.cells + 2 * self.halo
    }

    /// The cell size at the face's centre line.
    pub fn dx(&self) -> f64 {
        self.radius_m * core::f64::consts::FRAC_PI_2 / self.cells as f64
    }

    fn grid(&self) -> Grid {
        Grid::new(self.grid_cells(), self.grid_cells(), self.dx())
    }

    /// The jitter stream of this face.
    pub fn stream(&self) -> u64 {
        face_stream(self.seed, self.face)
    }
}

/// The jitter stream of a face of the world `seed`: a multiplier other than the jitter hash's own (0x9E37..), so that (face, cell) pairs of two faces do not collide in the hash's input.
pub fn face_stream(seed: u64, face: Face) -> u64 {
    seed ^ (u64::from(face.0) + 1).wrapping_mul(0xD6E8_FEB8_6659_FD93)
}

/// The rock fields of a carve (GEN-127, ADR 0102 and 0103), read at a direction: `k` the erodibility at a direction and a height (it multiplies the carve factor), `uplift` the ratio `U / K` at K 1.
/// Pass 1 reads both, pass 2 (incise only, no uplift) only `k`.
#[derive(Clone, Copy)]
pub struct RockFields<'a> {
    pub k: Option<&'a (dyn Fn(Vec3, f64) -> f64 + Sync)>,
    pub uplift: Option<&'a (dyn Fn(Vec3) -> f64 + Sync)>,
}

impl RockFields<'static> {
    /// No fields: the carve of the class factor and the uniform `u_over_k`.
    pub const fn none() -> RockFields<'static> {
        RockFields { k: None, uplift: None }
    }
}

/// A field sampled once per cell of the carved grid (the face and its halo), row-major, `threads` threads (a value per cell: the same bits for any count).
fn sample_grid(spec: &FaceSpec, field: &(dyn Fn(Vec3) -> f64 + Sync), threads: usize) -> Vec<f64> {
    let (par, h, m): (&dyn Par, i64, usize) = (&threads, spec.halo as i64, spec.grid_cells());
    par::map(par, m, &|j| (0..m).map(|i| field(spec.halo_direction(i as i64 - h, j as i64 - h))).collect::<Vec<f64>>())
        .into_iter()
        .flatten()
        .collect()
}

/// The base field sampled once per cell of the carved grid (the face and its halo).
pub fn sample_base(spec: &FaceSpec, base: &(dyn Fn(Vec3) -> f64 + Sync), threads: usize) -> Vec<f64> {
    sample_grid(spec, base, threads)
}

/// Pass 1 on a face: samples `base` once per cell, carves it ([`carve`]; `factor` the carve factor of GEN-097 at a direction, 1 when absent) and returns the change.
/// The outlets are the carved grid's border cells (the face's own, without a halo) and the cells of the base at or below the sea level.
pub fn pass1_face(
    spec: &FaceSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    params: &CarveParams,
    threads: usize,
) -> FaceChange {
    pass1_face_with_area(spec, base, factor, params, threads).0
}

/// The drainage area of a face's own cells as a byte per cell (GEN-110): `round(8 log2(area / cell area))`, so 0 for a cell's own area and 255 for 2^31.9 cells.
pub fn area_code(area_m2: f64, cell_m2: f64) -> u8 {
    (AREA_CODE_STEPS * libm::log2((area_m2 / cell_m2).max(1.0))).round().min(255.0) as u8
}

/// The area in cells of a code of [`area_code`].
pub fn area_cells(code: u8) -> f64 {
    libm::exp2(f64::from(code) / AREA_CODE_STEPS)
}

const AREA_CODE_STEPS: f64 = 8.0;

/// A pit depth in metres as a 16-bit code (GEN-120, STRM-076): `round(m * FILL_STEPS_PER_M)`, deeper than 65,535 steps stored as 65,535. The step is a parameter of the channel's key (the owner has not fixed the code; 0.25 m, planner 2026-11-05).
pub fn fill_code(m: f64) -> u16 {
    (m * FILL_STEPS_PER_M).max(0.0).round().min(f64::from(u16::MAX)) as u16
}

/// The depth in metres of a code of [`fill_code`].
pub fn fill_m(code: u16) -> f64 {
    f64::from(code) / FILL_STEPS_PER_M
}

/// Steps of the fill-depth code a metre (4: steps of 0.25 m).
pub const FILL_STEPS_PER_M: f64 = 4.0;

/// [`pass1_face`] and the codes of the final route's drainage area (by multiple flow direction, GEN-100) of the face's own cells, row-major `cells x cells` (`j` outer).
pub fn pass1_face_with_area(
    spec: &FaceSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    params: &CarveParams,
    threads: usize,
) -> (FaceChange, Vec<u8>) {
    pass1_face_rock(spec, base, factor, &RockFields::none(), params, threads)
}

/// [`pass1_face_with_area`] with the rock fields (GEN-127): the erodibility at a cell's direction and current height, and the uplift at its direction.
pub fn pass1_face_rock(
    spec: &FaceSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    rock: &RockFields,
    params: &CarveParams,
    threads: usize,
) -> (FaceChange, Vec<u8>) {
    assert!(
        spec.halo * 2 < spec.cells,
        "the halo ({} cells) must be under half the face ({} cells): beyond that the extended plane leaves the neighbouring ground",
        spec.halo,
        spec.cells
    );
    let grid = spec.grid();
    let z0 = sample_base(spec, base, threads);
    let outlet = grid.outlets(&z0, spec.sea_level);
    let factors: Option<Vec<f64>> = factor.map(|f| sample_grid(spec, f, threads));
    let mut z = z0.clone();
    let params = CarveParams { sea_level: spec.sea_level, ..*params };
    let (m, h) = (spec.grid_cells(), spec.halo as i64);
    let uplift: Option<Vec<f32>> = rock.uplift.map(|u| {
        let par: &dyn Par = &threads;
        par::map(par, m, &|j| (0..m).map(|i| u(spec.halo_direction(i as i64 - h, j as i64 - h)) as f32).collect::<Vec<f32>>())
            .into_iter()
            .flatten()
            .collect()
    });
    let kf = rock.k.map(|k| move |c: usize, zc: f64| k(spec.halo_direction((c % m) as i64 - h, (c / m) as i64 - h), zc));
    let par: &dyn Par = &threads;
    let carve_rock_in = Rock { par, k: kf.as_ref().map(|f| f as &(dyn Fn(usize, f64) -> f64 + Sync)), uplift: uplift.as_deref() };
    let routing = carve_rock(&grid, &mut z, &outlet, &params, factors.as_deref(), &carve_rock_in, spec.stream());
    drop(uplift);
    let (m, h, cell_m2) = (spec.cells + 2 * spec.halo, spec.halo, spec.dx() * spec.dx());
    let mut area = Vec::with_capacity(spec.cells * spec.cells);
    for j in 0..spec.cells {
        for i in 0..spec.cells {
            area.push(area_code(routing.area[(j + h) * m + i + h], cell_m2));
        }
    }
    (
        FaceChange {
            face: spec.face,
            cells: spec.cells,
            halo: spec.halo,
            change: z.iter().zip(&z0).map(|(a, b)| (a - b) as f32).collect(),
        },
        area,
    )
}
