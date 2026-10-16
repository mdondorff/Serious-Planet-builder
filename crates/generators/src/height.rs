//! CPU reference height generator: 3D value-noise fBm evaluated on the unit direction.
//!
//! The field is a pure function of `(seed, direction)`. A tile is a regular grid of such samples whose
//! directions come from `TileId::sample_direction`, so shared borders (also across cube faces) and
//! parent/child samples are the same directions bit for bit, hence the same heights.

use planet_core::cube::FaceMapping;
use planet_core::hash::{hash_u64s, unit_f64, HASH_VERSION};
use planet_core::{TileId, Vec3};
pub use planet_world_def::BodyId;
use planet_world_def::{
    records_hash64, single_planet_seed_read_hash, single_planet_seed_records, GenContext, MissingNode, ReadRecord, SPHERE_SURFACE_ID,
};

/// Bump whenever any output of this module changes (ADR 0006).
pub const GENERATOR_VERSION: u32 = 2;
/// Names this implementation in cache keys: CPU reference, hash version, and the face-mapping id is added by callers.
pub const IMPLEMENTATION_ID: &str = "cpu-ref-height-v2";

/// Domain tag so lattices of different layers never alias for the same seed.
const LAYER_HEIGHT: u64 = 0x4845_4947_4854;
const OCTAVES: u32 = 5;
const BASE_FREQUENCY: f64 = 4.0;
/// Peak amplitude of the summed noise in metres.
const AMPLITUDE_M: f64 = 4000.0;

fn lattice_value(seed: u64, layer: u64, octave: u32, ix: i64, iy: i64, iz: i64) -> f64 {
    let h = hash_u64s(seed, &[layer, u64::from(octave), ix as u64, iy as u64, iz as u64]);
    unit_f64(h) * 2.0 - 1.0
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Single-octave value noise of the height layer at `p`, in [-1, 1].
fn value_noise(seed: u64, octave: u32, p: Vec3) -> f64 {
    layer_noise(seed, LAYER_HEIGHT, octave, p)
}

/// Single-octave value noise of lattice layer `layer` at `p`, in [-1, 1]; layers never share lattice values.
#[cfg_attr(not(feature = "stress"), allow(dead_code))]
pub(crate) fn layer_noise(seed: u64, layer: u64, octave: u32, p: Vec3) -> f64 {
    let (fx, fy, fz) = (libm::floor(p.x), libm::floor(p.y), libm::floor(p.z));
    let (tx, ty, tz) = (smooth(p.x - fx), smooth(p.y - fy), smooth(p.z - fz));
    let (ix, iy, iz) = (fx as i64, fy as i64, fz as i64);
    let c = |dx: i64, dy: i64, dz: i64| lattice_value(seed, layer, octave, ix + dx, iy + dy, iz + dz);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), tx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), tx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), tx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), tx);
    lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz)
}

/// Largest gradient magnitude of [`layer_noise`] per unit of its argument: each partial derivative is a lattice
/// difference (at most 2) times the derivative of `smooth` (at most 1.5), so at most 3, and three axes give 3 sqrt(3).
const NOISE_GRADIENT_BOUND: f64 = 5.196_152_422_706_632;

/// Lipschitz constant of the gradient of [`layer_noise`] (the gradient is continuous across cell borders because the
/// derivative of `smooth` vanishes there): the Frobenius norm of the Hessian is at most sqrt(3 * 12^2 + 6 * 9^2) = 30.3 (rounded up), with
/// second differences of the lattice values (at most 2 along an axis, 4 across two) times `smooth''` (at most 6) or the
/// product of two `smooth'` (at most 1.5 each).
const NOISE_HESSIAN_BOUND: f64 = 30.3;

/// Value and gradient of [`layer_noise`] at `p`. The value is computed with the same operations as `layer_noise`.
fn layer_noise_with_gradient(seed: u64, layer: u64, octave: u32, p: Vec3) -> (f64, [f64; 3]) {
    let (fx, fy, fz) = (libm::floor(p.x), libm::floor(p.y), libm::floor(p.z));
    let (ux, uy, uz) = (p.x - fx, p.y - fy, p.z - fz);
    let (tx, ty, tz) = (smooth(ux), smooth(uy), smooth(uz));
    let (ix, iy, iz) = (fx as i64, fy as i64, fz as i64);
    let c = |dx: i64, dy: i64, dz: i64| lattice_value(seed, layer, octave, ix + dx, iy + dy, iz + dz);
    let (c000, c100, c010, c110) = (c(0, 0, 0), c(1, 0, 0), c(0, 1, 0), c(1, 1, 0));
    let (c001, c101, c011, c111) = (c(0, 0, 1), c(1, 0, 1), c(0, 1, 1), c(1, 1, 1));
    let x00 = lerp(c000, c100, tx);
    let x10 = lerp(c010, c110, tx);
    let x01 = lerp(c001, c101, tx);
    let x11 = lerp(c011, c111, tx);
    let value = lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz);
    // `smooth'(t) = 6 t (1 - t)`.
    let (dx, dy, dz) = (6.0 * ux * (1.0 - ux), 6.0 * uy * (1.0 - uy), 6.0 * uz * (1.0 - uz));
    let gx = dx * lerp(lerp(c100 - c000, c110 - c010, ty), lerp(c101 - c001, c111 - c011, ty), tz);
    let gy = dy * lerp(lerp(c010 - c000, c110 - c100, tx), lerp(c011 - c001, c111 - c101, tx), tz);
    let gz = dz * lerp(lerp(c001 - c000, c101 - c100, tx), lerp(c011 - c010, c111 - c110, tx), ty);
    (value, [gx, gy, gz])
}

/// Exact range of [`layer_noise`] over the axis-aligned box `centre +- u` (all in the argument space of the noise), or
/// `None` when the box spans too many lattice cells for the small fixed-size evaluation below.
///
/// Inside one lattice cell the noise is multilinear in the smoothed coordinates, and `smooth` is monotone, so its
/// extremes over a sub-box lie at the sub-box's corners. Cutting the box at the integer coordinates it contains gives
/// sub-boxes inside single cells, and every corner of every sub-box is a point of the grid of the box's lower and upper
/// coordinates plus those integers: the range is the range of the noise over that grid.
fn noise_box_range(seed: u64, layer: u64, octave: u32, centre: Vec3, u: f64) -> Option<(f64, f64)> {
    const MAX_CELLS: usize = 3;
    let c = [centre.x, centre.y, centre.z];
    let mut cells = [0i64; 3];
    let mut count = [0usize; 3];
    let mut coords = [[0.0f64; 4]; 3];
    for k in 0..3 {
        let (lo, hi) = (c[k] - u, c[k] + u);
        let (f0, f1) = (libm::floor(lo) as i64, libm::floor(hi) as i64);
        if f1 - f0 + 1 > MAX_CELLS as i64 {
            return None;
        }
        cells[k] = f0;
        let mut n = 0;
        coords[k][n] = lo;
        n += 1;
        for int in f0 + 1..=f1 {
            if (int as f64) > lo && (int as f64) < hi {
                coords[k][n] = int as f64;
                n += 1;
            }
        }
        coords[k][n] = hi;
        count[k] = n + 1;
    }
    // Lattice corners of the cells the box touches: (cells + 1) per axis, at most 4 each.
    let size = [
        (libm::floor(c[0] + u) as i64 - cells[0] + 2) as usize,
        (libm::floor(c[1] + u) as i64 - cells[1] + 2) as usize,
        (libm::floor(c[2] + u) as i64 - cells[2] + 2) as usize,
    ];
    let mut corner = [[[0.0f64; 4]; 4]; 4];
    for (i, plane) in corner.iter_mut().enumerate().take(size[0]) {
        for (j, row) in plane.iter_mut().enumerate().take(size[1]) {
            for (k, v) in row.iter_mut().enumerate().take(size[2]) {
                *v = lattice_value(seed, layer, octave, cells[0] + i as i64, cells[1] + j as i64, cells[2] + k as i64);
            }
        }
    }
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for &x in &coords[0][..count[0]] {
        for &y in &coords[1][..count[1]] {
            for &z in &coords[2][..count[2]] {
                let (fx, fy, fz) = (libm::floor(x), libm::floor(y), libm::floor(z));
                let (tx, ty, tz) = (smooth(x - fx), smooth(y - fy), smooth(z - fz));
                let (i, j, k) = ((fx as i64 - cells[0]) as usize, (fy as i64 - cells[1]) as usize, (fz as i64 - cells[2]) as usize);
                let q = |a: usize, b: usize, c: usize| corner[i + a][j + b][k + c];
                let x00 = lerp(q(0, 0, 0), q(1, 0, 0), tx);
                let x10 = lerp(q(0, 1, 0), q(1, 1, 0), tx);
                let x01 = lerp(q(0, 0, 1), q(1, 0, 1), tx);
                let x11 = lerp(q(0, 1, 1), q(1, 1, 1), tx);
                let v = lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    // Rounding margin: the evaluation above follows `layer_noise` but the argument differs in the last bits.
    Some((lo - 1e-9, hi + 1e-9))
}

/// Bounds of [`layer_noise`] over all points `freq * u` with `u` a unit direction within the angle `theta` of `centre`.
///
/// Sound bounds, intersected. (1) The exact range over the axis-aligned box that contains the cap ([`noise_box_range`]),
/// when the box is small against the lattice. (2) The value at the centre plus or minus the gradient bound times the
/// largest distance in the argument. (3) The first-order Taylor expansion at the centre with the Lipschitz constant of
/// the gradient for the remainder. (4) The hull of the lattice corners of the cells the cap touches (trilinear
/// interpolation with monotone weights stays inside it), when those are at most 64.
pub(crate) fn layer_noise_bounds(seed: u64, layer: u64, octave: u32, centre: Vec3, theta: f64, freq: f64) -> (f64, f64) {
    // The chord is shorter than the arc; the margin absorbs rounding in the direction maths and in `floor`.
    let reach = freq * (theta * (1.0 + 1e-9) + 1e-12);
    if let Some((lo, hi)) = noise_box_range(seed, layer, octave, centre * freq, reach) {
        let (n0, g) = layer_noise_with_gradient(seed, layer, octave, centre * freq);
        let slack = NOISE_GRADIENT_BOUND * reach * (1.0 + 1e-9) + 1e-12;
        let taylor = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt() * reach + 0.5 * NOISE_HESSIAN_BOUND * reach * reach;
        let slack = slack.min(taylor * (1.0 + 1e-9) + 1e-12);
        return (lo.max(n0 - slack).max(-1.0), hi.min(n0 + slack).min(1.0));
    }
    let range = |c: f64| (libm::floor(freq * c - reach) as i64, libm::floor(freq * c + reach) as i64);
    let ((x0, x1), (y0, y1), (z0, z1)) = (range(centre.x), range(centre.y), range(centre.z));
    let (n0, g) = layer_noise_with_gradient(seed, layer, octave, centre * freq);
    let slack = NOISE_GRADIENT_BOUND * reach * (1.0 + 1e-9) + 1e-12;
    let taylor = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt() * reach + 0.5 * NOISE_HESSIAN_BOUND * reach * reach;
    let slack = slack.min(taylor * (1.0 + 1e-9) + 1e-12);
    let (lo, hi) = ((n0 - slack).max(-1.0), (n0 + slack).min(1.0));
    let corners = (x1 - x0 + 2).saturating_mul(y1 - y0 + 2).saturating_mul(z1 - z0 + 2);
    if corners > 64 {
        return (lo, hi);
    }
    let (mut hlo, mut hhi) = (f64::MAX, f64::MIN);
    for ix in x0..=x1 + 1 {
        for iy in y0..=y1 + 1 {
            for iz in z0..=z1 + 1 {
                let v = lattice_value(seed, layer, octave, ix, iy, iz);
                hlo = hlo.min(v);
                hhi = hhi.max(v);
            }
        }
    }
    (lo.max(hlo), hi.min(hhi))
}

/// `eval` on every direction of `dirs`, in the caller's order, split over up to `threads` scoped threads in contiguous chunks
/// (a batch of fewer than 8 directions per thread uses fewer threads; a single chunk runs on the calling thread, several on scoped threads). `stop` is polled
/// every 64 samples of each chunk; once it answers true the unfinished results are 0.0. The values do not depend on `threads`.
pub fn eval_chunked(dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize, eval: &(dyn Fn(Vec3) -> f64 + Sync)) -> Vec<f64> {
    let mut out = vec![0.0; dirs.len()];
    let workers = threads.clamp(1, (dirs.len() / 8).max(1));
    let chunk = dirs.len().div_ceil(workers).max(1);
    let run = |dirs: &[Vec3], out: &mut [f64]| {
        for (k, (&d, o)) in dirs.iter().zip(out.iter_mut()).enumerate() {
            if k % 64 == 0 && stop() {
                break;
            }
            *o = eval(d);
        }
    };
    if workers == 1 {
        run(dirs, &mut out);
    } else {
        std::thread::scope(|s| {
            for (d, o) in dirs.chunks(chunk).zip(out.chunks_mut(chunk)) {
                s.spawn(move || run(d, o));
            }
        });
    }
    out
}

/// A height field over unit directions. The reference generator is [`Reference`]; the test-only stress field
/// `stress::Steep` exists behind the `stress` feature. Mesh and plan code take a field so stress tests can swap it
/// without touching production paths. Generators (`HeightGenerator`) build a field through a `GenContext`; production paths still
/// build `Reference(seed)` directly (the planet's seed), which keys identically (`inputs_hash`). Plans for other bodies take
/// their field from the generator (multi-body seams).
pub trait HeightField: Sync {
    /// Height in metres at a unit direction.
    fn height(&self, dir: Vec3) -> f64;
    /// The heights of `dirs`, in the caller's order: element `k` is bit for bit `self.height(dirs[k])` (GEN-020). A field is free to
    /// evaluate the directions in any other order internally (one that reads a block store groups them by block) because every
    /// height is a pure function of its direction. `stop` is polled during the batch (at least every 64 samples); once it
    /// answers true (the flag must be monotonic: once true, always true) no further sample is evaluated and the remaining elements are 0.0.
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        dirs.iter().map(|&d| if stop() { 0.0 } else { self.height(d) }).collect()
    }
    /// [`HeightField::heights_until`] without a stop flag.
    fn heights(&self, dirs: &[Vec3]) -> Vec<f64> {
        self.heights_until(dirs, &|| false)
    }
    /// [`HeightField::heights_until`] on up to `threads` scoped threads (the plan builder's origin heights, LOD-009): the same
    /// values in the same order for every `threads` (CON-14), because each height is a pure function of its direction and the
    /// threads only split the work. The default splits the directions into chunks and evaluates each with `height`; a field
    /// that reads a block store overrides it to read the distinct blocks of the whole batch on the threads (GEN-020).
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        eval_chunked(dirs, stop, threads, &|d| self.height(d))
    }
    /// [`HeightField::heights_threaded_until`] without a stop flag.
    fn heights_threaded(&self, dirs: &[Vec3], threads: usize) -> Vec<f64> {
        self.heights_threaded_until(dirs, &|| false, threads)
    }
    /// The seed (part of every key).
    fn seed(&self) -> u64;
    /// Names the implementation in the height key ([`cache_key_field`]).
    fn implementation_id(&self) -> &'static str;
    /// Bumped whenever this field's output changes (ADR 0006); part of the height key.
    fn version(&self) -> u32;
    /// Bound on `|height|` in metres.
    fn max_abs_height_m(&self) -> f64;
    /// Hash of the definition inputs this field was built from (the recorded read set of its [`GenContext`], WDEF-004).
    /// It enters every key in place of the seed. The default is the read set of a generator that reads only the seed of
    /// the single planet, so a field built directly from a seed keys exactly like one built through a context.
    fn inputs_hash(&self) -> u64 {
        single_planet_seed_read_hash(self.seed())
    }
    /// The typed reads behind [`inputs_hash`](HeightField::inputs_hash), for the 128-bit persistent keys (ADR 0018).
    /// The default is the read set of a generator that reads only the seed of the single planet, matching the default
    /// `inputs_hash`; a field built through a context returns that context's records.
    fn read_records(&self) -> Vec<ReadRecord> {
        single_planet_seed_records(self.seed())
    }
    /// [`read_records`](HeightField::read_records) for one tile: the field-wide reads plus what the tile's own spatial
    /// queries read (the cells of its footprint and their margin, empty ones included; WDEF-005). A field that
    /// [`reads_cells`](HeightField::reads_cells) must override this: it is the single source of both the 128-bit
    /// persistent key and the 64-bit [`inputs_hash_for`](HeightField::inputs_hash_for).
    fn read_records_for(&self, _tile: TileId, _map: &dyn FaceMapping) -> Vec<ReadRecord> {
        self.read_records()
    }
    /// Whether this field reads authored items through cell queries. Only then does [`inputs_hash_for`] differ from tile
    /// to tile; every other field has one hash for all tiles, which the batch keyer computes once.
    ///
    /// [`inputs_hash_for`]: HeightField::inputs_hash_for
    fn reads_cells(&self) -> bool {
        false
    }
    /// [`inputs_hash`](HeightField::inputs_hash) for one tile (WDEF-005): the field-wide hash, or for a field that
    /// [`reads_cells`](HeightField::reads_cells) the hash of its per-tile [`read_records_for`], so an edit in one cell
    /// re-keys the tiles that read it in both key paths. The cell grid is the tangent-warp grid: `map` is the mapping the
    /// tile id was made with, and a field that queries cells must refuse any other. The reads are declared here, at plan
    /// time, not recorded while `height` runs: a generator that reads more than it declares would key wrongly, which only
    /// a contract test over the generator can catch (learning 2026-10-11).
    ///
    /// [`read_records_for`]: HeightField::read_records_for
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        if self.reads_cells() {
            records_hash64(&self.read_records_for(tile, map))
        } else {
            self.inputs_hash()
        }
    }
    /// The body this field describes; part of every height and mesh key (ADR 0001). A field built directly from a seed
    /// stands for the single planet.
    fn body(&self) -> BodyId {
        BodyId::PLANET
    }
    /// The 128-bit stage key of the baked input this field reads (STRM-011, ADR 0028 point 5), if it reads one: the source
    /// of its persistent height key, so a change of the bake re-keys the tiles made from it. Fields that read only
    /// definition nodes have none.
    fn source_key(&self) -> Option<[u8; 16]> {
        None
    }
    /// Id of the reference surface of this field's body (CON-15); part of every height and mesh key.
    fn surface_id(&self) -> &'static str {
        SPHERE_SURFACE_ID
    }
    /// The climate field of tile `id` (ADR 0039 point 2, GEN-021): temperature and precipitation at 9 x 9 points, stored with the
    /// tile. The default is the analytic global-coarse function of the field's seed at those directions (a pure function of
    /// `(seed, mapping, tile)`), for fields without a bake; a field over a bake reads it with the integer reader (ADR 0027).
    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> crate::climate::ClimateTexture {
        crate::climate::analytic_climate(self.seed(), map, id)
    }
    /// The word of everything the climate depends on besides the tile and the field's inputs, folded into every mesh key
    /// (the climate version, the reader version and the bake format version for a field over a bake).
    fn climate_version_word(&self) -> u64 {
        crate::climate::analytic_version_word()
    }
    /// `(min, max)` of the height in metres over all directions within the angle `angular_radius` (radians) of the unit
    /// direction `centre`. The bounds are sound (never excluded a real height) but need not be tight; the default is the
    /// global `+-max_abs_height_m`. Node selection uses them for the bounding volume of a tile (LOD-008).
    fn height_bounds(&self, _centre: Vec3, _angular_radius: f64) -> (f64, f64) {
        let m = self.max_abs_height_m();
        (-m, m)
    }
}

/// The reference generator ([`height_at`]) with a seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference(pub u64);

impl HeightField for Reference {
    fn height(&self, dir: Vec3) -> f64 {
        height_at(self.0, dir)
    }
    fn seed(&self) -> u64 {
        self.0
    }
    fn implementation_id(&self) -> &'static str {
        IMPLEMENTATION_ID
    }
    fn version(&self) -> u32 {
        GENERATOR_VERSION
    }
    fn max_abs_height_m(&self) -> f64 {
        AMPLITUDE_M
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (mut lo, mut hi, mut amp, mut norm, mut freq) = (0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
        for octave in 0..OCTAVES {
            let (a, b) = layer_noise_bounds(self.0, LAYER_HEIGHT, octave, centre, angular_radius, freq);
            lo += amp * a;
            hi += amp * b;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        // Rounding margin: the sum above is evaluated in the same order as `height_at` but with different operands.
        ((AMPLITUDE_M * lo / norm - 1e-6).max(-AMPLITUDE_M), (AMPLITUDE_M * hi / norm + 1e-6).min(AMPLITUDE_M))
    }
}

/// Height in metres at a unit direction.
pub fn height_at(seed: u64, dir: Vec3) -> f64 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, BASE_FREQUENCY);
    for octave in 0..OCTAVES {
        sum += amp * value_noise(seed, octave, dir * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    AMPLITUDE_M * sum / norm
}

/// One generated tile: `(res + 1)²` height samples, row `l` (t direction) major, column `k` (s direction).
#[derive(Clone, Debug, PartialEq)]
pub struct TileData {
    pub id: TileId,
    pub res: u32,
    pub heights: Vec<f32>,
}

impl TileData {
    pub fn at(&self, k: u32, l: u32) -> f32 {
        self.heights[(l * (self.res + 1) + k) as usize]
    }

    /// Hash of the tile contents (id, resolution, height bits), versioned with `HASH_VERSION`.
    pub fn content_hash(&self) -> u64 {
        let mut v = vec![u64::from(HASH_VERSION), u64::from(GENERATOR_VERSION), self.id.raw(), u64::from(self.res)];
        v.extend(self.heights.iter().map(|h| u64::from(h.to_bits())));
        hash_u64s(0x7469_6c65, &v)
    }
}

/// Generate a tile on a `res`×`res` cell grid (`res` a power of two).
pub fn generate_tile(seed: u64, map: &dyn FaceMapping, id: TileId, res: u32) -> TileData {
    generate_tile_field(&Reference(seed), map, id, res)
}

/// [`generate_tile`] for any height field: the field's heights at the tile's samples, rounded to `f32`. [`Reference`]
/// gives exactly [`generate_tile`].
pub fn generate_tile_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, res: u32) -> TileData {
    let mut heights = Vec::with_capacity(((res + 1) * (res + 1)) as usize);
    for l in 0..=res {
        for k in 0..=res {
            heights.push(field.height(id.sample_direction(map, res, k, l)) as f32);
        }
    }
    TileData { id, res, heights }
}

/// Generate many tiles, in the order given, using `threads` worker threads. Each tile is independent and
/// the output order is the input order, so the result does not depend on the thread count (CON-14).
pub fn generate_region(seed: u64, map: &(dyn FaceMapping + Sync), ids: &[TileId], res: u32, threads: usize) -> Vec<TileData> {
    let threads = threads.max(1).min(ids.len().max(1));
    let chunk = ids.len().div_ceil(threads).max(1);
    let mut out: Vec<TileData> = Vec::with_capacity(ids.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|&id| generate_tile(seed, map, id, res)).collect::<Vec<_>>()))
            .collect();
        for h in handles {
            out.extend(h.join().expect("generator thread panicked"));
        }
    });
    out
}

pub(crate) fn fold_str(s: &str) -> Vec<u64> {
    s.bytes().map(u64::from).collect()
}

/// Key under which a generated tile may be cached: everything its content depends on (ADR 0006). Changing the
/// generator version, hash version, implementation id, face mapping, seed, tile or resolution changes the key.
pub fn cache_key(map: &dyn FaceMapping, seed: u64, id: TileId, res: u32) -> u64 {
    cache_key_field(&Reference(seed), map, id, res)
}

/// [`cache_key`] for any height field: its version, implementation id and input hash take the place of the
/// reference generator's, so [`Reference`] gives exactly [`cache_key`].
pub fn cache_key_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, res: u32) -> u64 {
    let mut v = vec![u64::from(field.version()), u64::from(HASH_VERSION), field.inputs_hash_for(id, map), id.raw(), u64::from(res)];
    v.push(hash_u64s(1, &fold_str(field.implementation_id())));
    v.push(hash_u64s(2, &fold_str(map.id())));
    v.extend_from_slice(&body_key_words(field));
    hash_u64s(0x006b_6579, &v)
}

/// Version of the key composition (which inputs enter a height or mesh key and how). History: 1 = generator, hash and
/// field versions, implementation id, mapping, tile and inputs; 2 = plus the body id and the reference surface (ADR 0001).
pub const KEY_FORMAT_VERSION: u32 = 2;

/// The words every height key adds for the body: the key format version, the body id and the reference surface id.
/// Radius and seed enter through the read set and the mesh key's body parameter hash.
pub(crate) fn body_key_words(field: &dyn HeightField) -> [u64; 3] {
    [u64::from(KEY_FORMAT_VERSION), u64::from(field.body().0), hash_u64s(3, &fold_str(field.surface_id()))]
}

/// A field together with the read set of the context it was built from (see [`HeightGenerator`]). Its keys follow the
/// read set, so a body other than the planet keys differently from a field built directly from the same seed with
/// `Reference(seed)` (which stands for the planet): plans for other bodies must take their field from the generator
/// (multi-body seams, next change).
pub struct Bound<F: HeightField> {
    field: F,
    inputs: u64,
    records: Vec<ReadRecord>,
    body: BodyId,
}

impl<F: HeightField> Bound<F> {
    /// Only generators in this crate build one, from the same context that produced the field's inputs: `inputs` must be
    /// that context's read set, or the key would name inputs the heights do not come from.
    pub(crate) fn new(field: F, ctx: &GenContext, body: BodyId) -> Bound<F> {
        Bound { field, inputs: ctx.read_set_hash(), records: ctx.read_records(), body }
    }
}

impl<F: HeightField> HeightField for Bound<F> {
    fn height(&self, dir: Vec3) -> f64 {
        self.field.height(dir)
    }
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        self.field.heights_until(dirs, stop)
    }
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        self.field.heights_threaded_until(dirs, stop, threads)
    }
    fn seed(&self) -> u64 {
        self.field.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.field.implementation_id()
    }
    fn version(&self) -> u32 {
        self.field.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.field.max_abs_height_m()
    }
    fn inputs_hash(&self) -> u64 {
        self.inputs
    }
    fn read_records(&self) -> Vec<ReadRecord> {
        self.records.clone()
    }
    fn body(&self) -> BodyId {
        self.body
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.field.source_key()
    }
    fn surface_id(&self) -> &'static str {
        self.field.surface_id()
    }
    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> crate::climate::ClimateTexture {
        self.field.climate(map, id)
    }
    fn climate_version_word(&self) -> u64 {
        self.field.climate_version_word()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.field.height_bounds(centre, angular_radius)
    }
}

/// A height generator (CON-06): it reads the definition only through a [`GenContext`] and returns the pure field it
/// describes. Open a fresh context per field: the field's key covers everything that context recorded, so a reused
/// context would key a field by what was read before it. The inner loops
/// then run on plain values, so reads are recorded once per tile or plan, not per sample.
pub trait HeightGenerator {
    fn implementation_id(&self) -> &'static str;
    fn version(&self) -> u32;
    fn field(&self, ctx: &GenContext, body: BodyId) -> Result<Box<dyn HeightField>, MissingNode>;
}

/// [`Reference`] as a generator: reads the body's seed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReferenceGenerator;

impl HeightGenerator for ReferenceGenerator {
    fn implementation_id(&self) -> &'static str {
        IMPLEMENTATION_ID
    }
    fn version(&self) -> u32 {
        GENERATOR_VERSION
    }
    fn field(&self, ctx: &GenContext, body: BodyId) -> Result<Box<dyn HeightField>, MissingNode> {
        let seed = ctx.body_seed(body)?;
        Ok(Box::new(Bound::new(Reference(seed), ctx, body)))
    }
}
