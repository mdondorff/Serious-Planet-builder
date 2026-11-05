//! Tier A, STRM-077 (m9 keep-pits-seams PR 2): which cells of other tiles touch a tile's border cell, inside a face, across a cube edge and at a cube corner. A tiny layer: 96 cells a face edge,
//! tiles of 24, so 4 x 4 tiles a face.

use planet_core::Face;
use planet_generators::bake::fillgeom::border_neighbours;
use planet_generators::bake::tile::{TileGrid, TileId};

fn grid() -> TileGrid {
    TileGrid { cells: 96, tile: 24, halo: 6, radius_m: 100.0 * 96.0 / core::f64::consts::FRAC_PI_2 }
}

fn tile(f: u8, ti: u32, tj: u32) -> TileId {
    TileId { face: Face(f), ti, tj }
}

// spec: STRM-077
#[test]
fn inside_a_face_a_border_cell_touches_the_cells_of_the_next_tile_and_the_relation_is_symmetric() {
    let g = grid();
    // The east edge of tile (1, 1): three cells of tile (2, 1); the corner cell: five cells in three tiles.
    let edge = border_neighbours(&g, tile(0, 1, 1), 23, 10);
    assert_eq!(edge, vec![(tile(0, 2, 1), 0, 9), (tile(0, 2, 1), 0, 10), (tile(0, 2, 1), 0, 11)]);
    let corner = border_neighbours(&g, tile(0, 1, 1), 23, 23);
    assert_eq!(corner.len(), 5);
    assert_eq!(corner.iter().map(|c| c.0).collect::<std::collections::BTreeSet<_>>().len(), 3);
    // An inner cell touches no other tile.
    assert!(border_neighbours(&g, tile(0, 1, 1), 10, 10).is_empty());
    // Symmetry: every neighbour of a cell has that cell among its own neighbours, for every border cell of every tile of face 0 that is not on the face's edge.
    for ti in 0..4u32 {
        for tj in 0..4u32 {
            for k in 0..24usize {
                for (i, j) in [(0, k), (23, k), (k, 0), (k, 23)] {
                    let me = tile(0, ti, tj);
                    for (other, oi, oj) in border_neighbours(&g, me, i, j) {
                        if other.face != me.face {
                            continue;
                        }
                        let back = border_neighbours(&g, other, oi, oj);
                        assert!(back.contains(&(me, i, j)), "{me:?} ({i}, {j}) -> {other:?} ({oi}, {oj}) is not returned");
                    }
                }
            }
        }
    }
}

// spec: STRM-077
#[test]
fn across_a_cube_edge_the_neighbours_are_border_cells_of_the_next_face_and_mostly_mutual_and_a_corner_reaches_two_other_faces() {
    let g = grid();
    let (mut pairs, mut mutual) = (0usize, 0usize);
    for ti in 0..4u32 {
        for k in 0..24usize {
            // The tiles along the +i edge of face 0 (ti 3), cells in the last column.
            let me = tile(0, 3, ti);
            for (other, oi, oj) in border_neighbours(&g, me, 23, k) {
                if other.face == me.face {
                    continue;
                }
                assert!(oi == 0 || oi == 23 || oj == 0 || oj == 23, "{other:?} ({oi}, {oj}) is not on its tile's border");
                pairs += 1;
                mutual += usize::from(border_neighbours(&g, other, oi, oj).iter().any(|c| c.0 == me));
            }
        }
    }
    assert!(pairs >= 24 * 3, "only {pairs} pairs across the edge");
    assert!(mutual * 10 >= pairs * 9, "only {mutual} of {pairs} pairs across the cube edge are mutual");
    // The corner cell of face 0 touches cells of two other faces.
    let faces: std::collections::BTreeSet<u8> = border_neighbours(&g, tile(0, 3, 3), 23, 23).iter().map(|c| c.0.face.0).collect();
    assert!(faces.len() >= 2, "a cube corner reaches {faces:?}");
}
