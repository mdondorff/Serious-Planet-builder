//! COORD-007, GEN-019 (ADR 0073): the ring of a tile's lattice and the stencil of the eight cube corners. A ring point beyond a face edge is the
//! neighbour tile's own lattice direction one cell inside it, on the row that continues the shared edge; inside a face it is the next
//! tile's lattice point; at a cube corner the three tiles read the same three edge points in the same order.

use planet_core::cube::TangentWarp;
use planet_core::{Face, Side, TileId, Vec3};

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

fn bits(v: Vec3) -> [u64; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

const RES: u32 = 16;

/// Every lattice vertex direction of a tile with its indices.
fn lattice(t: TileId) -> Vec<((u32, u32), Vec3)> {
    let mut out = Vec::new();
    for l in 0..=RES {
        for k in 0..=RES {
            out.push(((k, l), t.sample_direction(&TangentWarp, RES, k, l)));
        }
    }
    out
}

// spec: COORD-007
#[test]
fn a_ring_point_inside_a_face_is_the_next_tiles_lattice_point_and_inside_the_tile_it_is_the_tiles_own() {
    let m = &TangentWarp;
    let t = id(2, 5, 7, 9);
    for l in 0..=RES as i64 {
        for k in 0..=RES as i64 {
            assert_eq!(bits(t.ring_direction(m, RES, k, l)), bits(t.sample_direction(m, RES, k as u32, l as u32)));
        }
    }
    // East ring column = the east neighbour's column 1.
    let east = t.neighbor(m, Side::East);
    for l in 0..=RES {
        assert_eq!(bits(t.ring_direction(m, RES, i64::from(RES) + 1, i64::from(l))), bits(east.sample_direction(m, RES, 1, l)));
    }
    let south = t.neighbor(m, Side::South);
    for k in 0..=RES {
        assert_eq!(bits(t.ring_direction(m, RES, i64::from(k), -1)), bits(south.sample_direction(m, RES, k, RES - 1)));
    }
}

// spec: COORD-007
#[test]
fn a_ring_point_beyond_a_face_edge_is_the_neighbours_lattice_point_one_cell_inside_on_the_row_that_continues_the_edge() {
    let m = &TangentWarp;
    for level in [3u8, 12] {
        let last = (1u32 << level) - 1;
        for face in 0..6u8 {
            for (x, y, side) in [
                (0, 5 % (last + 1), Side::West),
                (last, 7 % (last + 1), Side::East),
                (3 % (last + 1), 0, Side::South),
                (2 % (last + 1), last, Side::North),
            ] {
                let t = id(face, level, x, y);
                let nb = t.neighbor(m, side);
                assert_ne!(nb.face(), t.face(), "the tile lies at the face edge");
                let nb_lattice = lattice(nb);
                let mine_on_edge: Vec<[u64; 3]> = lattice(t).into_iter().map(|(_, d)| bits(d)).collect();
                for e in 0..=RES {
                    // The edge vertex and the ring point beyond it.
                    let (k, l, rk, rl) = match side {
                        Side::West => (0, e, -1, i64::from(e)),
                        Side::East => (RES, e, i64::from(RES) + 1, i64::from(e)),
                        Side::South => (e, 0, i64::from(e), -1),
                        Side::North => (e, RES, i64::from(e), i64::from(RES) + 1),
                    };
                    let edge_dir = bits(t.sample_direction(m, RES, k, l));
                    let ring = bits(t.ring_direction(m, RES, rk, rl));
                    // The same vertex in the neighbour's lattice.
                    let (ek, el) = nb_lattice.iter().find(|(_, d)| bits(*d) == edge_dir).map(|(kl, _)| *kl).expect("the shared vertex");
                    // Its neighbours in the neighbour's lattice that are inside it and not on the shared edge are the candidates.
                    let mut found = false;
                    for (dk, dl) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                        let (ck, cl) = (i64::from(ek) + dk, i64::from(el) + dl);
                        if !(0..=i64::from(RES)).contains(&ck) || !(0..=i64::from(RES)).contains(&cl) {
                            continue;
                        }
                        let cand = bits(nb.sample_direction(m, RES, ck as u32, cl as u32));
                        if cand == ring {
                            found = true;
                            assert!(!mine_on_edge.contains(&cand), "the ring point is inside the neighbour, not on the shared edge");
                        }
                    }
                    assert!(
                        found,
                        "face {face} level {level} {side:?}: ring point of edge vertex {e} is no lattice neighbour of the shared vertex"
                    );
                }
            }
        }
    }
}

// spec: COORD-007
#[test]
fn the_three_tiles_at_every_cube_corner_read_the_same_three_points_in_the_same_order() {
    let m = &TangentWarp;
    for level in [3u8, 12, 22] {
        let last = (1u32 << level) - 1;
        // The corner vertex directions of the cube, from the tiles at the face corners.
        let mut by_corner: std::collections::BTreeMap<[u64; 3], Vec<[[u64; 3]; 3]>> = Default::default();
        for face in 0..6u8 {
            for (x, y, k, l) in [(0, 0, 0u32, 0u32), (last, 0, RES, 0), (0, last, 0, RES), (last, last, RES, RES)] {
                let t = id(face, level, x, y);
                let order = t.corner_stencil(RES, k, l).expect("a cube corner");
                let corner = bits(t.sample_direction(m, RES, k, l));
                let point = |slot: u8| {
                    let (dk, dl) = match slot {
                        0 => (1i64, 0i64),
                        1 => (-1, 0),
                        2 => (0, 1),
                        _ => (0, -1),
                    };
                    bits(t.ring_direction(m, RES, i64::from(k) + dk, i64::from(l) + dl))
                };
                by_corner.entry(corner).or_default().push(order.map(point));
                // Other vertices of the tile are no corner.
                if RES >= 2 {
                    assert_eq!(t.corner_stencil(RES, 1, 1), None);
                    assert_eq!(t.corner_stencil(RES, RES / 2, 0), None);
                }
            }
        }
        assert_eq!(by_corner.len(), 8, "level {level}: eight cube corners");
        for (corner, triples) in &by_corner {
            assert_eq!(triples.len(), 3, "level {level}: three tiles at the corner {corner:x?}");
            assert!(triples[0] != [[0; 3]; 3]);
            assert_eq!(triples[0], triples[1], "level {level}: corner {corner:x?}");
            assert_eq!(triples[0], triples[2], "level {level}: corner {corner:x?}");
            // Three different points.
            let t = triples[0];
            assert!(t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
        }
    }
}
