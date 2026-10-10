//! Terrain mesh invariants: tile-local f32 positions, normals, geomorph targets, skirts, seams (COORD-010, GEN-004, LOD-004).

use planet_core::cube::TangentWarp;
use planet_core::{Face, Side, TileId, Vec3};
use planet_generators::mesh::{grid_indices, grid_vertex_count, tile_mesh, vertex_count, TileMesh};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

fn world(m: &TileMesh, i: u32, j: u32) -> Vec3 {
    let v = m.vertices[(j * (m.cells + 1) + i) as usize];
    m.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))
}

fn coarse_world(m: &TileMesh, i: u32, j: u32) -> Vec3 {
    let v = m.vertices[(j * (m.cells + 1) + i) as usize];
    m.origin.0 + Vec3::new(f64::from(v.coarse[0]), f64::from(v.coarse[1]), f64::from(v.coarse[2]))
}

// spec: COORD-010, REND-009
#[test]
fn vertices_are_tile_local_and_sized_as_documented() {
    for t in [id(3, 3, 5, 2), id(0, 10, 100, 200), id(5, 20, 1, 7)] {
        let m = tile_mesh(1, &TangentWarp, t, CELLS, R);
        assert_eq!(m.vertices.len(), vertex_count(CELLS));
        let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << t.level());
        for v in &m.vertices[..grid_vertex_count(CELLS)] {
            let len = Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2])).length();
            // Within the tile (about one edge length) plus the terrain height range, never planet-radius sized.
            assert!(len < edge * 1.5 + 5000.0, "tile {t:?}: vertex offset {len} m is not tile-local");
            assert!(v.height.abs() <= 4000.0);
        }
    }
}

// spec: COORD-010
#[test]
fn f32_quantisation_of_deep_tiles_stays_below_a_millimetre() {
    let t = id(2, 26, 123_456, 654_321);
    let m = tile_mesh(1, &TangentWarp, t, CELLS, R);
    let (mut worst, mut sample) = (0.0f64, 0);
    for j in 0..=CELLS {
        for i in 0..=CELLS {
            let dir = t.sample_direction(&TangentWarp, CELLS, i, j);
            let exact = dir * (R + planet_generators::height::height_at(1, dir));
            worst = worst.max(exact.distance(world(&m, i, j)));
            sample += 1;
        }
    }
    assert!(sample > 200 && worst < 1e-3, "worst vertex error {worst} m");
}

// spec: LOD-004, REND-009
#[test]
fn coarse_positions_are_the_parent_grid_points() {
    // A child's even-even vertices are parent vertices; its other vertices morph to edge/diagonal midpoints of the parent cell.
    let parent = id(4, 5, 7, 9);
    let pm = tile_mesh(1, &TangentWarp, parent, CELLS, R);
    for child in parent.children().unwrap() {
        let cm = tile_mesh(1, &TangentWarp, child, CELLS, R);
        let (cx, cy) = ((child.x() - 2 * parent.x()) * (CELLS / 2), (child.y() - 2 * parent.y()) * (CELLS / 2));
        let mut worst = 0.0f64;
        for j in 0..=CELLS {
            for i in 0..=CELLS {
                let (pi, pj) = (cx + i / 2, cy + j / 2);
                let expected = match (i % 2, j % 2) {
                    (0, 0) => world(&pm, pi, pj),
                    (1, 0) => (world(&pm, pi, pj) + world(&pm, pi + 1, pj)) * 0.5,
                    (0, 1) => (world(&pm, pi, pj) + world(&pm, pi, pj + 1)) * 0.5,
                    _ => (world(&pm, pi, pj) + world(&pm, pi + 1, pj + 1)) * 0.5,
                };
                worst = worst.max(expected.distance(coarse_world(&cm, i, j)));
            }
        }
        assert!(worst < 0.05, "child {child:?}: coarse positions differ from the parent grid by {worst} m");
    }
}

// spec: GEN-004, REND-009
#[test]
fn neighbouring_meshes_share_border_positions_across_faces() {
    let m = TangentWarp;
    for (f, l, x, y, side) in
        [(3u8, 4u8, 15u32, 7u32, Side::East), (0, 3, 7, 7, Side::East), (4, 3, 3, 7, Side::North), (2, 5, 0, 12, Side::West)]
    {
        let t = id(f, l, x, y);
        let nb = t.neighbor(&m, side);
        let (a, b) = (tile_mesh(1, &m, t, CELLS, R), tile_mesh(1, &m, nb, CELLS, R));
        let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << l);
        let tol = edge * 1e-6 + 1e-3; // f32 rounding of tile-local offsets
        let border: Vec<Vec3> = (0..=CELLS)
            .map(|k| match side {
                Side::East => world(&a, CELLS, k),
                Side::West => world(&a, 0, k),
                Side::North => world(&a, k, CELLS),
                Side::South => world(&a, k, 0),
            })
            .collect();
        for p in border {
            let best = (0..=CELLS)
                .flat_map(|k| [world(&b, 0, k), world(&b, CELLS, k), world(&b, k, 0), world(&b, k, CELLS)])
                .map(|q| q.distance(p))
                .fold(f64::MAX, f64::min);
            assert!(best <= tol, "{t:?} {side:?}: border vertex has no partner within {tol} m (nearest {best} m)");
        }
    }
}

// spec: REND-006, REND-009
#[test]
fn normals_are_unit_outward_and_skirts_hang_below_the_border() {
    let t = id(1, 6, 20, 30);
    let m = tile_mesh(1, &TangentWarp, t, CELLS, R);
    for (k, v) in m.vertices[..grid_vertex_count(CELLS)].iter().enumerate() {
        let n = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
        assert!((n.length() - 1.0).abs() < 1e-5, "vertex {k}: normal length {}", n.length());
        let radial = (m.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))).normalized();
        assert!(n.dot(radial) > 0.9, "vertex {k}: normal points {} away from radial", n.dot(radial));
    }
    let base = grid_vertex_count(CELLS);
    let border_of = |s: usize, i: usize| match s {
        0 => i,
        1 => CELLS as usize * (CELLS as usize + 1) + i,
        2 => i * (CELLS as usize + 1),
        _ => i * (CELLS as usize + 1) + CELLS as usize,
    };
    let edge = R * core::f64::consts::FRAC_PI_2 / 64.0;
    for s in 0..4 {
        for i in 0..=CELLS as usize {
            let (top, skirt) = (m.vertices[border_of(s, i)], m.vertices[base + s * (CELLS as usize + 1) + i]);
            let d =
                Vec3::new(f64::from(top.pos[0] - skirt.pos[0]), f64::from(top.pos[1] - skirt.pos[1]), f64::from(top.pos[2] - skirt.pos[2]))
                    .length();
            assert!((d - edge * 0.08).abs() < edge * 0.001, "skirt depth {d} m, expected {}", edge * 0.08);
        }
    }
}

// spec: REND-001
#[test]
fn indices_are_in_range_and_cover_the_grid_and_skirts() {
    let idx = grid_indices(CELLS);
    let triangles = idx.len() / 3;
    assert_eq!(triangles as u32, 2 * CELLS * CELLS + 4 * 2 * CELLS);
    assert!(idx.iter().all(|&i| (i as usize) < vertex_count(CELLS)));
    // Every grid vertex is used.
    let used: std::collections::BTreeSet<u32> = idx.iter().copied().collect();
    assert_eq!(used.len(), vertex_count(CELLS));
}

// spec: REND-009, LOD-004
#[test]
fn a_fully_morphed_border_meets_the_coarser_neighbours_border_across_faces() {
    // For a tile T and its same-level neighbour N across `side`, whose parent P is a coarser leaf: T's morph targets on
    // that border must lie on P's border polyline (vertices of P's grid, or midpoints of two adjacent ones).
    let m = TangentWarp;
    let mut checked = 0;
    for (f, l, x, y) in [(0u8, 1u8, 0u32, 0u32), (0, 1, 1, 0), (0, 1, 0, 1), (0, 2, 3, 0), (2, 3, 7, 7), (4, 1, 1, 1), (3, 2, 0, 3)] {
        let t = id(f, l, x, y);
        let tm = tile_mesh(1, &m, t, CELLS, R);
        for side in Side::ALL {
            let nb = t.neighbor(&m, side);
            let Some(p) = nb.parent() else { continue };
            if Some(p) == t.parent() {
                continue; // same parent: the neighbour is a sibling, not a coarser leaf
            }
            let pm = tile_mesh(1, &m, p, CELLS, R);
            // Border polyline of P: all border vertices of the parent mesh (any side).
            let n = CELLS;
            let border: Vec<Vec3> =
                (0..=n).flat_map(|k| [world(&pm, k, 0), world(&pm, k, n), world(&pm, 0, k), world(&pm, n, k)]).collect();
            let polyline_dist = |q: Vec3| {
                let mut best = f64::MAX;
                for a in &border {
                    for b in &border {
                        // Only segments of adjacent lattice points count: lattice spacing bound.
                        if a.distance(*b) < 1.0e-6
                            || a.distance(*b) > 1.2 * R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << p.level()) / f64::from(n)
                        {
                            continue;
                        }
                        let ab = *b - *a;
                        let tt = ((q - *a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
                        best = best.min((q - (*a + ab * tt)).length());
                    }
                }
                best
            };
            let ks: Vec<(u32, u32)> = (0..=n)
                .map(|k| match side {
                    Side::East => (n, k),
                    Side::West => (0, k),
                    Side::North => (k, n),
                    Side::South => (k, 0),
                })
                .collect();
            let worst = ks.iter().map(|&(i, j)| polyline_dist(coarse_world(&tm, i, j))).fold(0.0f64, f64::max);
            assert!(worst < 0.5, "{t:?} {side:?}: morphed border vertex is {worst} m off the coarser neighbour's border");
            checked += 1;
        }
    }
    assert!(checked >= 6, "only {checked} borders checked");
}

// spec: REND-009
#[test]
fn vertex_serialisation_matches_the_documented_gpu_layout() {
    let m = tile_mesh(1, &TangentWarp, id(2, 4, 3, 5), CELLS, R);
    let v = m.vertices[40];
    let f = v.to_floats();
    assert_eq!(f.len(), planet_generators::mesh::MeshVertex::FLOATS);
    let o = planet_generators::mesh::MeshVertex::OFFSETS;
    assert_eq!(&f[o[0]..o[0] + 3], &v.pos);
    assert_eq!(&f[o[1]..o[1] + 3], &v.coarse);
    assert_eq!(&f[o[2]..o[2] + 3], &v.normal);
    assert_eq!(f[o[3]], v.height);
    assert_eq!(&f[o[4]..o[4] + 3], &v.ref_pos);
    assert_eq!(planet_generators::mesh::MeshVertex::STRIDE, 52);
}

/// Worst angle (degrees) between the normals of coincident border vertices of two neighbouring tiles.
fn worst_border_normal_angle(a: &TileMesh, b: &TileMesh, side: Side) -> f64 {
    let n = CELLS;
    let nrm = |m: &TileMesh, i: u32, j: u32| {
        let v = m.vertices[(j * (n + 1) + i) as usize];
        Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]))
    };
    let mut worst = 0.0f64;
    for k in 0..=n {
        let (i, j) = match side {
            Side::East => (n, k),
            Side::West => (0, k),
            Side::North => (k, n),
            Side::South => (k, 0),
        };
        let p = world(a, i, j);
        // The partner: the border vertex of `b` at the same position.
        let mut best = (f64::MAX, Vec3::ZERO);
        for m in 0..=n {
            for (bi, bj) in [(0, m), (n, m), (m, 0), (m, n)] {
                let d = world(b, bi, bj).distance(p);
                if d < best.0 {
                    best = (d, nrm(b, bi, bj));
                }
            }
        }
        assert!(best.0 < 0.5, "no partner vertex found for {i},{j}: nearest {} m", best.0);
        let (x, y) = (nrm(a, i, j), best.1);
        worst = worst.max(libm::atan2(x.cross(y).length(), x.dot(y)).to_degrees());
    }
    worst
}

// spec: REND-009, GEN-004
#[test]
fn border_normals_match_across_every_cube_edge_and_within_faces() {
    // Every tile of level 2 on every face, every side: normals of coincident border vertices agree (bit-level).
    let m = TangentWarp;
    let (mut cross_pairs, mut same_pairs) = (0, 0);
    let mut worst = 0.0f64;
    for f in 0..6u8 {
        for x in 0..4u32 {
            for y in 0..4u32 {
                let t = id(f, 2, x, y);
                let a = tile_mesh(1, &m, t, CELLS, R);
                for side in Side::ALL {
                    let nb = t.neighbor(&m, side);
                    let angle = worst_border_normal_angle(&a, &tile_mesh(1, &m, nb, CELLS, R), side);
                    worst = worst.max(angle);
                    if nb.face() != t.face() {
                        cross_pairs += 1;
                    } else {
                        same_pairs += 1;
                    }
                }
            }
        }
    }
    // 12 cube edges x 4 tiles x 2 directions = 96 cross-face pairs.
    assert_eq!(cross_pairs, 96, "the test must cross every cube edge");
    assert!(same_pairs > 100);
    assert!(worst < 1e-6, "border normals of neighbouring tiles differ by up to {worst} degrees (ADR 0003)");
}

// spec: REND-009
#[test]
fn normals_follow_the_terrain_and_agree_with_the_lattice_slope() {
    // The seam fix must not turn normals into something that ignores the terrain or is smoothed by a wrong step.
    let m = TangentWarp;
    let mut max_tilt = 0.0f64;
    for (f, l, x, y) in [(0u8, 6u8, 20u32, 33u32), (3, 9, 100, 200), (5, 12, 1000, 2000), (2, 20, 400_000, 700_000)] {
        let t = id(f, l, x, y);
        let mesh = tile_mesh(1, &m, t, CELLS, R);
        for (i, j) in [(3u32, 3u32), (8, 8), (12, 5), (5, 12)] {
            let v = mesh.vertices[(j * (CELLS + 1) + i) as usize];
            let normal = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
            // Reference: the plain lattice central difference of the mesh itself.
            let lattice =
                (world(&mesh, i + 1, j) - world(&mesh, i - 1, j)).cross(world(&mesh, i, j + 1) - world(&mesh, i, j - 1)).normalized();
            let angle = |a: Vec3, b: Vec3| libm::atan2(a.cross(b).length(), a.dot(b)).to_degrees();
            let radial = world(&mesh, i, j).normalized();
            max_tilt = max_tilt.max(angle(lattice, radial));
            let off = angle(normal, lattice);
            assert!(off < 0.02, "tile {t:?} vertex ({i},{j}): normal is {off} degrees from the lattice slope");
        }
    }
    assert!(max_tilt > 0.05, "the terrain must tilt the normals measurably for this test to mean anything, got {max_tilt} degrees");
}
