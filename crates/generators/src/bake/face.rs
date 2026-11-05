//! Pass 1 of the bake on one cube face (GEN-100): the base field sampled once per cell, carved on the face's own grid, and the change (carved minus base) kept.
//!
//! The grid has `cells x cells` cells over the face in the tangent-warped face coordinates (cell `(i, j)` at `s = 2 (i + 0.5) / cells - 1`), `dx` metres apart at the face's
//! centre line (`radius * pi / 2 / cells`); the face does not route across its edges, its border cells are outlets like the sea.

use super::carve::{carve, CarveParams};
use super::flood::fill_depth;
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

/// A pit depth in metres as a byte (GEN-120, STRM-076): `round(m * FILL_STEPS_PER_M)`, deeper than 255 steps stored as 255. The step is a parameter of the channel's key (the owner has not fixed the code).
pub fn fill_code(m: f64) -> u8 {
    (m * FILL_STEPS_PER_M).max(0.0).round().min(255.0) as u8
}

/// The depth in metres of a code of [`fill_code`].
pub fn fill_m(code: u8) -> f64 {
    f64::from(code) / FILL_STEPS_PER_M
}

/// Steps of the fill-depth code a metre (1: steps of 1 m).
pub const FILL_STEPS_PER_M: f64 = 1.0;

/// The smallest pit the fill depth records, in metres: the flood's epsilon tilt over a plain and a shallower dent count as no pit (GEN-120).
pub const FILL_MIN_M: f64 = 0.5;

/// [`pass1_face`] and the codes of the final route's drainage area (by multiple flow direction, GEN-100) of the face's own cells, row-major `cells x cells` (`j` outer).
pub fn pass1_face_with_area(
    spec: &FaceSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    params: &CarveParams,
    threads: usize,
) -> (FaceChange, Vec<u8>) {
    let (change, area, _) = pass1_face_with_channels(spec, base, factor, params, threads);
    (change, area)
}

/// [`pass1_face_with_area`] and the codes ([`fill_code`]) of the pit depth of the carved surface (GEN-120): how far each of the face's own cells lies below the flood of the carved heights,
/// which the runtime adds back while the pits are kept in the change.
pub fn pass1_face_with_channels(
    spec: &FaceSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    params: &CarveParams,
    threads: usize,
) -> (FaceChange, Vec<u8>, Vec<u8>) {
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
    let routing = carve(&grid, &mut z, &outlet, &params, factors.as_deref(), spec.stream());
    let (m, h, cell_m2) = (spec.cells + 2 * spec.halo, spec.halo, spec.dx() * spec.dx());
    let mut area = Vec::with_capacity(spec.cells * spec.cells);
    for j in 0..spec.cells {
        for i in 0..spec.cells {
            area.push(area_code(routing.area[(j + h) * m + i + h], cell_m2));
        }
    }
    // The routing is dropped before the flood: two surfaces of a face would not fit its memory budget.
    drop(routing);
    let depth = fill_depth(&grid, &z, &outlet, params.epsilon, FILL_MIN_M);
    let mut fill = Vec::with_capacity(spec.cells * spec.cells);
    for j in 0..spec.cells {
        for i in 0..spec.cells {
            fill.push(fill_code(f64::from(depth[(j + h) * m + i + h])));
        }
    }
    drop(depth);
    (
        FaceChange {
            face: spec.face,
            cells: spec.cells,
            halo: spec.halo,
            change: z.iter().zip(&z0).map(|(a, b)| (a - b) as f32).collect(),
        },
        area,
        fill,
    )
}

/// The pit depth codes ([`fill_code`]) of one face's own cells, row-major `cells x cells` (`j` outer): pass 1's fill depth, which pass 2 reads for the basins that cross its windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceFill {
    pub face: Face,
    pub cells: usize,
    pub codes: Vec<u8>,
}

/// The pit depth of the six faces at a direction in metres: the code of the face cell that holds it (the nearest cell, no cross-fade: the depth is a flood's level difference, not a
/// field to be differentiated).
pub fn fill_at(faces: &[FaceFill; 6], d: Vec3) -> f64 {
    let (face, s, t) = planet_core::cube::direction_to_face(&TangentWarp, d);
    let f = &faces[face.0 as usize];
    debug_assert_eq!(f.face, Face(face.0));
    let n = f.cells as f64;
    let cell = |c: f64| ((c + 1.0) * 0.5 * n).floor().clamp(0.0, n - 1.0) as usize;
    fill_m(f.codes[cell(t) * f.cells + cell(s)])
}
