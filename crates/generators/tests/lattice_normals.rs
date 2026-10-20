//! GEN-019, GEN-020, COORD-007 (ADR 0073): the lattice normal. A tile reads 1,222 directions at 32 cells; the normal equals a converged
//! normal on a smooth field; tiles sharing a vertex, inside a face, across the twelve cube edges and at the eight cube corners, compute
//! bit-identical normals (the compact words and the CPU mesh), on one and on three threads; a normal is never flipped.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_core::cube::TangentWarp;
use planet_core::{Face, Side, TileId, Vec3};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::tile_mesh_field;
use planet_generators::normals::{lattice_direction_count, lattice_directions, lattice_normals};

const R: f64 = 6_371_000.0;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

/// A field that counts the directions it is asked for.
struct Counting {
    asked: AtomicUsize,
}

impl HeightField for Counting {
    fn height(&self, dir: Vec3) -> f64 {
        self.asked.fetch_add(1, Ordering::SeqCst);
        300.0 * dir.x * dir.y + 50.0 * dir.z
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "test-counting"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        350.0
    }
}

// spec: GEN-020
#[test]
fn a_tile_evaluates_its_lattice_its_ring_without_the_corners_and_its_origin_and_nothing_else() {
    assert_eq!(lattice_direction_count(32), 33 * 33 + 4 * 33);
    assert_eq!(lattice_direction_count(32) + 1, 1_222, "against 5,446 evaluations of the four-point stencil");
    for (cells, level) in [(16u32, 5u8), (32, 7), (64, 9)] {
        let t = id(2, level, 3, 4);
        let dirs = lattice_directions(&TangentWarp, t, cells);
        assert_eq!(dirs.len(), lattice_direction_count(cells));
        let expect = lattice_direction_count(cells) + 1;
        let field = Counting { asked: AtomicUsize::new(0) };
        let _ = compact_tile_field(&field, &TangentWarp, t, cells, R);
        assert_eq!(field.asked.load(Ordering::SeqCst), expect, "compact tile, {cells} cells");
        let field = Counting { asked: AtomicUsize::new(0) };
        let _ = tile_mesh_field(&field, &TangentWarp, t, cells, R);
        assert_eq!(field.asked.load(Ordering::SeqCst), expect, "CPU mesh, {cells} cells");
    }
}

/// A smooth field with a known gradient: `A sin(w d.a)` along a fixed unit vector `a`.
struct Wave;

const WAVE_A: f64 = 100.0;
/// 2 pi R / wavelength with a wavelength of 20 km.
const WAVE_W: f64 = 2.0 * core::f64::consts::PI * R / 20_000.0;

fn wave_axis() -> Vec3 {
    Vec3::new(0.36, 0.48, 0.8)
}

impl HeightField for Wave {
    fn height(&self, dir: Vec3) -> f64 {
        WAVE_A * libm::sin(WAVE_W * dir.dot(wave_axis()))
    }
    fn seed(&self) -> u64 {
        2
    }
    fn implementation_id(&self) -> &'static str {
        "test-wave"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        WAVE_A
    }
}

/// The normal of the surface `r = R + h(d)` at `d` by centred differences of 1 m in a tangent frame: converged far below the lattice error.
fn converged_normal(field: &dyn HeightField, d: Vec3) -> Vec3 {
    let axis = if d.x.abs() <= d.y.abs() && d.x.abs() <= d.z.abs() {
        Vec3::new(1.0, 0.0, 0.0)
    } else if d.y.abs() <= d.z.abs() {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let t1 = (axis - d * axis.dot(d)).normalized();
    let t2 = d.cross(t1);
    let eps = 1.0 / R;
    let p = |t: Vec3, s: f64| {
        let q = (d + t * (s * eps)).normalized();
        q * (R + field.height(q))
    };
    (p(t1, 1.0) - p(t1, -1.0)).cross(p(t2, 1.0) - p(t2, -1.0)).normalized()
}

fn angle_deg(a: Vec3, b: Vec3) -> f64 {
    libm::atan2(a.cross(b).length(), a.dot(b)).to_degrees()
}

// spec: GEN-019
#[test]
fn the_lattice_normal_equals_a_converged_normal_on_a_smooth_field_inside_a_face_and_at_the_edges_within_their_own_bound() {
    let m = &TangentWarp;
    let cells = 32u32;
    let mut worst_inside: f64 = 0.0;
    let mut worst_edge: f64 = 0.0;
    let mut tilted: f64 = 0.0;
    for t in [id(0, 12, 2000, 1500), id(2, 12, 100, 3000), id(4, 14, 6000, 9000), id(1, 12, 0, 2048), id(3, 12, 4095, 77), id(5, 13, 10, 0)]
    {
        let dirs = lattice_directions(m, t, cells);
        let heights = Wave.heights(&dirs);
        let normals = lattice_normals(t, cells, R, &dirs, &heights);
        let w = cells as usize + 1;
        for l in 0..w {
            for k in 0..w {
                let d = dirs[l * w + k];
                let want = converged_normal(&Wave, d);
                let got = normals[l * w + k];
                let err = angle_deg(got, want);
                let on_face_edge = {
                    let n = 1i64 << t.level();
                    let (gi, gj) = (i64::from(t.x()) * i64::from(cells) + k as i64, i64::from(t.y()) * i64::from(cells) + l as i64);
                    gi == 0 || gj == 0 || gi == n * i64::from(cells) || gj == n * i64::from(cells)
                };
                if on_face_edge {
                    worst_edge = worst_edge.max(err);
                } else {
                    worst_inside = worst_inside.max(err);
                }
                tilted = tilted.max(angle_deg(want, d));
            }
        }
    }
    assert!(tilted > 1.0, "the field must tilt the normals for this test to mean anything: {tilted} degrees");
    // Inside a face the truncation error of a two-cell baseline on a wave of 20 km at spacing 76 m is of order (k dx)^2 / 6 of the slope.
    assert!(worst_inside < 0.005, "inside a face the lattice normal is {worst_inside} degrees off the converged one");
    // At a face edge the neighbour path bends: first order. The bound is its own (measured, then kept with margin).
    assert!(worst_edge < 3.0, "at a face edge the lattice normal is {worst_edge} degrees off");
}

/// One tile's normals as the compact words, the CPU mesh bits and the f64 normals of `lattice_normals`.
#[derive(Clone, PartialEq, Debug)]
struct Normals {
    words: Vec<u32>,
    mesh: Vec<[u32; 3]>,
    f64s: Vec<[u64; 3]>,
    dirs: Vec<[u64; 3]>,
}

fn normals_of(field: &dyn HeightField, t: TileId, cells: u32) -> Normals {
    let c = compact_tile_field(field, &TangentWarp, t, cells, R);
    let m = tile_mesh_field(field, &TangentWarp, t, cells, R);
    let w = cells as usize + 1;
    let dirs = lattice_directions(&TangentWarp, t, cells);
    let heights = field.heights(&dirs);
    let n = lattice_normals(t, cells, R, &dirs, &heights);
    Normals {
        words: (0..w * w).map(|k| c.texels[k][1]).collect(),
        mesh: m.vertices.iter().take(w * w).map(|v| v.normal.map(f32::to_bits)).collect(),
        f64s: n.iter().map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]).collect(),
        dirs: dirs.iter().take(w * w).map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]).collect(),
    }
}

/// The tiles to compare at `level`: the four corner tiles of every face (the eight cube corners, three tiles each) and, on every face edge,
/// a tile with its neighbours across the edge.
fn seam_tiles(level: u8) -> Vec<TileId> {
    let m = &TangentWarp;
    let last = (1u32 << level) - 1;
    let mut set: BTreeSet<TileId> = BTreeSet::new();
    for face in 0..6u8 {
        for (x, y) in [(0, 0), (last, 0), (0, last), (last, last)] {
            set.insert(id(face, level, x, y));
        }
        for (x, y) in [(0, last / 2), (last, last / 3), (last / 2, 0), (last / 3, last)] {
            let t = id(face, level, x, y);
            set.insert(t);
            for side in Side::ALL {
                set.insert(t.neighbor(m, side));
            }
        }
    }
    set.into_iter().collect()
}

fn all_normals(field: &(dyn HeightField + Sync), tiles: &[TileId], cells: u32, threads: usize) -> Vec<Normals> {
    let mut out: Vec<Option<Normals>> = vec![None; tiles.len()];
    let chunk = tiles.len().div_ceil(threads);
    std::thread::scope(|s| {
        for (ts, os) in tiles.chunks(chunk).zip(out.chunks_mut(chunk)) {
            s.spawn(move || {
                for (t, o) in ts.iter().zip(os) {
                    *o = Some(normals_of(field, *t, cells));
                }
            });
        }
    });
    out.into_iter().map(|o| o.unwrap()).collect()
}

/// Every pair of vertices of different tiles with the same direction bits has the same normal, in all three forms.
fn assert_shared_vertices_agree(tiles: &[TileId], normals: &[Normals], cells: u32, what: &str) -> usize {
    let w = cells as usize + 1;
    let mut by_dir: BTreeMap<[u64; 3], Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, n) in normals.iter().enumerate() {
        for l in 0..w {
            for k in 0..w {
                if k == 0 || l == 0 || k == w - 1 || l == w - 1 {
                    by_dir.entry(n.dirs[l * w + k]).or_default().push((ti, l * w + k));
                }
            }
        }
    }
    let mut pairs = 0;
    let mut corners = 0;
    for (dir, owners) in &by_dir {
        if owners.len() < 2 {
            continue;
        }
        let (t0, v0) = owners[0];
        for &(t1, v1) in &owners[1..] {
            pairs += 1;
            let (a, b) = (&normals[t0], &normals[t1]);
            assert_eq!(a.words[v0], b.words[v1], "{what}: compact words differ at {dir:x?} between {:?} and {:?}", tiles[t0], tiles[t1]);
            assert_eq!(a.mesh[v0], b.mesh[v1], "{what}: CPU mesh normals differ at {dir:x?} between {:?} and {:?}", tiles[t0], tiles[t1]);
            assert_eq!(a.f64s[v0], b.f64s[v1], "{what}: f64 normals differ at {dir:x?} between {:?} and {:?}", tiles[t0], tiles[t1]);
        }
        corners += usize::from(owners.len() == 3 && owners.iter().map(|o| tiles[o.0].face()).collect::<BTreeSet<_>>().len() == 3);
    }
    assert!(corners >= 8, "{what}: the eight cube corners were compared ({corners})");
    pairs
}

// spec: GEN-019
// spec: COORD-007
#[test]
fn tiles_sharing_a_vertex_compute_bit_identical_normals_inside_a_face_across_the_cube_edges_and_at_the_cube_corners_on_one_and_three_threads(
) {
    let cells = 16u32;
    let eroded = Reference(3);
    for level in [3u8, 12, 22] {
        let tiles = seam_tiles(level);
        let one = all_normals(&eroded, &tiles, cells, 1);
        let three = all_normals(&eroded, &tiles, cells, 3);
        assert_eq!(one, three, "level {level}: the thread count changed a normal");
        let pairs = assert_shared_vertices_agree(&tiles, &one, cells, &format!("level {level}"));
        assert!(pairs > 100, "level {level}: {pairs} shared vertices compared");
    }
}

// spec: GEN-019
#[test]
fn a_steep_field_gives_outward_normals_everywhere_and_the_same_seam_agreement() {
    use planet_generators::stress::Steep;
    let cells = 16u32;
    let steep = Steep(1);
    for level in [9u8, 12] {
        let tiles = seam_tiles(level);
        // `lattice_normals` asserts n.d > 0 at every vertex that is not a cube corner: building the tiles is the check.
        let normals = all_normals(&steep, &tiles, cells, 3);
        assert_shared_vertices_agree(&tiles, &normals, cells, &format!("steep level {level}"));
    }
}

// spec: GEN-019
#[test]
fn same_face_neighbours_read_the_same_four_points_and_a_flat_field_has_the_radial_normal() {
    let cells = 16u32;
    struct Flat;
    impl HeightField for Flat {
        fn height(&self, _: Vec3) -> f64 {
            10.0
        }
        fn seed(&self) -> u64 {
            0
        }
        fn implementation_id(&self) -> &'static str {
            "flat-test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            10.0
        }
    }
    for t in [id(0, 6, 20, 33), id(5, 3, 0, 0), id(2, 22, 400_000, 700_000)] {
        let dirs = lattice_directions(&TangentWarp, t, cells);
        let heights = Flat.heights(&dirs);
        let n = lattice_normals(t, cells, R, &dirs, &heights);
        let w = cells as usize + 1;
        for k in 0..w * w {
            // The tangent warp makes the lattice spacing uneven around a vertex, so the centred differences are radial only to a few
            // thousandths of a degree on a sphere (the figure of the old stencil, exactly radial, is not kept).
            assert!(angle_deg(n[k], dirs[k]) < 0.01, "a sphere's normal is radial: {} degrees", angle_deg(n[k], dirs[k]));
        }
    }
    // Two tiles of one face share the vertices of their common column with the same normal.
    let a = id(2, 8, 100, 40);
    let b = id(2, 8, 101, 40);
    let field = Reference(5);
    let (na, nb) = (normals_of(&field, a, cells), normals_of(&field, b, cells));
    let w = cells as usize + 1;
    for l in 0..w {
        assert_eq!(na.f64s[l * w + cells as usize], nb.f64s[l * w], "row {l}");
        assert_eq!(na.words[l * w + cells as usize], nb.words[l * w]);
    }
}
