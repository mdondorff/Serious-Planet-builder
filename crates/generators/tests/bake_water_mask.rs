//! Tier A, GEN-167 (TEST-109): the lake water mask of a tile of L (design section 13, M1-M6). One analytic surface and one lake on a small H1 grid; the mask of every tile of a partition against one
//! whole-grid window, in any order and on several threads.

use planet_generators::bake::water_mask::{
    decode, disconnected_wet, encode, lake_mask, lakes_of, Status, Window, MAX_LAKES_PER_TILE, NO_LAKE,
};

const LEVEL: f64 = 100.0;
const LAKE: u64 = 7;
const RATIO: i64 = 8;
/// The local grid of the fixture: 40 H1 cells of 8 local cells.
const N: usize = 320;
const HALO: usize = 16;
const DRY: u64 = NO_LAKE;

/// The H1 cells (i, j): the lake water is the block 10..14 x 10..14; its ring is the cells around the block except the east column (the outflow side: they drain away from the lake).
fn status(i: i64, j: i64) -> Status {
    if (10..14).contains(&i) && (10..14).contains(&j) {
        Status::Water(LAKE)
    } else if (9..=14).contains(&i) && (9..=14).contains(&j) && i != 14 {
        Status::Ring(LAKE)
    } else {
        Status::Dry
    }
}

fn level(_: u64) -> f64 {
    LEVEL
}

fn sq(a: i64, b: i64) -> i64 {
    a * a + b * b
}

/// The surface at a local cell: a bowl (95 m) of radius 24 around (96, 96) that reaches 8 cells beyond the block, an island above the level inside it, a valley (90 m) leaving to the east and an unrelated
/// hollow (90 m) around (200, 200); 130 m elsewhere.
fn zl(x: i64, y: i64) -> f64 {
    if (96..=100).contains(&x) && (94..=98).contains(&y) {
        105.0
    } else if sq(x - 96, y - 96) <= 24 * 24 {
        95.0
    } else if ((112..=240).contains(&x) && (92..=100).contains(&y)) || sq(x - 200, y - 200) <= 20 * 20 {
        90.0
    } else {
        130.0
    }
}

/// The mask of the core at `(ci, cj)` of side `core`, from a window cut at the halo.
fn tile_mask(ci: i64, cj: i64, core: usize, halo: usize) -> Vec<u64> {
    let w = core + 2 * halo;
    let origin = (ci - halo as i64, cj - halo as i64);
    let mut z = Vec::with_capacity(w * w);
    for y in 0..w {
        for x in 0..w {
            z.push(zl(origin.0 + x as i64, origin.1 + y as i64));
        }
    }
    lake_mask(&Window { zl: &z, w, origin, ratio: RATIO }, halo, core, &status, &level)
}

fn whole() -> Vec<u64> {
    tile_mask(0, 0, N, HALO)
}

fn at(m: &[u64], x: usize, y: usize) -> u64 {
    m[y * N + x]
}

// spec: GEN-167, TEST-109
#[test]
fn m1_the_valley_below_the_sill_and_the_valley_sides_are_dry() {
    let m = whole();
    assert_eq!(at(&m, 96, 90), LAKE, "the lake's own water is wet");
    assert_eq!(at(&m, 74, 96), LAKE, "a ring cell on the west side under the level is wet");
    // The valley below the sill (x from 112) is under the level and joins the lake: dry, the outflow side is no ring.
    for y in 92..=100 {
        for x in 112..=240 {
            assert_eq!(at(&m, x, y), DRY, "valley cell ({x}, {y}) below the sill is dry");
        }
    }
    // The valley sides: the bowl's cells next to the spill (x 112..120, any y) are under the level and their H1 cell drains elsewhere: dry.
    for y in 72..=120 {
        for x in 112..120 {
            assert_eq!(at(&m, x, y), DRY, "valley side ({x}, {y}) is dry");
        }
    }
}

// spec: GEN-167, TEST-109
#[test]
fn m2_another_hollow_under_the_level_is_dry() {
    let m = whole();
    assert!(zl(200, 200) < LEVEL);
    for y in 180..=220 {
        for x in 180..=220 {
            assert_eq!(at(&m, x, y), DRY, "the hollow cell ({x}, {y}) is not the lake");
        }
    }
}

// spec: GEN-167, TEST-109
#[test]
fn m3_every_wet_cell_is_under_the_level_and_the_island_stays_dry() {
    let m = whole();
    let mut wet = 0;
    for y in 0..N {
        for x in 0..N {
            if at(&m, x, y) != DRY {
                wet += 1;
                assert!(zl(x as i64, y as i64) < LEVEL, "wet cell ({x}, {y}) is at or over the level");
            }
        }
    }
    assert!(wet > 1200, "the lake has water: {wet} cells");
    for y in 94..=98 {
        for x in 96..=100 {
            assert_eq!(at(&m, x, y), DRY, "the island cell ({x}, {y}) is dry");
        }
    }
    // The ring reaches at most 8 cells beyond the block: nothing wet west of x = 72.
    assert_eq!(at(&m, 71, 96), DRY);
}

// spec: GEN-167, TEST-109
#[test]
fn m4_every_partition_into_tiles_gives_the_whole_grids_mask_in_any_order() {
    let reference = whole();
    // 64 and 40 and 160 divide the grid; 80 puts the block's corner on a tile corner.
    for core in [64usize, 40, 80, 160] {
        let per = N / core;
        let mut tiles: Vec<(usize, usize)> = (0..per).flat_map(|tj| (0..per).map(move |ti| (ti, tj))).collect();
        for order in 0..2 {
            if order == 1 {
                tiles.reverse();
            }
            for &(ti, tj) in &tiles {
                let m = tile_mask((ti * core) as i64, (tj * core) as i64, core, HALO);
                for y in 0..core {
                    for x in 0..core {
                        assert_eq!(
                            m[y * core + x],
                            at(&reference, ti * core + x, tj * core + y),
                            "tile ({ti}, {tj}) of {core} cells: cell ({x}, {y}) differs from the whole grid"
                        );
                    }
                }
            }
        }
    }
}

// spec: GEN-167, TEST-109, CON-14
#[test]
fn m5_one_thread_and_eight_give_the_same_masks() {
    let core = 64usize;
    let per = N / core;
    let all: Vec<(usize, usize)> = (0..per).flat_map(|tj| (0..per).map(move |ti| (ti, tj))).collect();
    let serial: Vec<Vec<u64>> = all.iter().map(|&(ti, tj)| tile_mask((ti * core) as i64, (tj * core) as i64, core, HALO)).collect();
    let mut threaded: Vec<Vec<u64>> = vec![Vec::new(); all.len()];
    let per_thread = all.len().div_ceil(8);
    std::thread::scope(|s| {
        for (chunk, out) in all.chunks(per_thread).zip(threaded.chunks_mut(per_thread)) {
            s.spawn(move || {
                for (t, o) in chunk.iter().zip(out.iter_mut()) {
                    *o = tile_mask((t.0 * core) as i64, (t.1 * core) as i64, core, HALO);
                }
            });
        }
    });
    assert_eq!(serial, threaded);
}

// spec: GEN-167, TEST-109
#[test]
fn m5b_the_diagnostic_counts_wet_cells_cut_off_from_the_largest_component() {
    // Two components of lake 1 (four cells and one cell) and one of lake 2: one cut-off cell of lake 1.
    #[rustfmt::skip]
    let m: Vec<u64> = vec![
        1, 1, DRY, DRY, 2,
        1, 1, DRY, DRY, 2,
        DRY, DRY, DRY, DRY, DRY,
        DRY, DRY, DRY, 1, DRY,
        DRY, DRY, DRY, DRY, DRY,
    ];
    assert_eq!(disconnected_wet(&m, 5), vec![(1, 1)]);
    // Diagonal neighbours are connected (8-neighbours).
    assert!(disconnected_wet(&[1, DRY, DRY, 1], 2).is_empty());
    // On the fixture, reported and not gated.
    println!("m5b fixture: cut-off wet cells {:?}", disconnected_wet(&whole(), N));
}

// spec: GEN-167, TEST-109
#[test]
fn m6_the_encoding_round_trips_refuses_what_it_cannot_hold_and_states_its_size() {
    let core = 64usize;
    let per = N / core;
    let mut bytes_total = 0usize;
    let mut wet_tiles = 0;
    for tj in 0..per {
        for ti in 0..per {
            let m = tile_mask((ti * core) as i64, (tj * core) as i64, core, HALO);
            let b = encode(&m, core).expect("encodes");
            assert_eq!(b.is_empty(), lakes_of(&m).is_empty(), "a dry tile has an empty section, a wet one has runs");
            assert_eq!(decode(&b, core).expect("decodes"), m, "tile ({ti}, {tj}) round trips");
            if !b.is_empty() {
                wet_tiles += 1;
                bytes_total += b.len();
            }
        }
    }
    println!("m6 fixture: {wet_tiles} wet tiles of {}, {bytes_total} bytes of water sections", per * per);
    assert!(wet_tiles >= 1 && wet_tiles < per * per);
    // Refusals name what was found.
    let m = tile_mask(64, 64, core, HALO);
    let b = encode(&m, core).unwrap();
    assert!(decode(&b[..b.len() - 1], core).unwrap_err().0.contains("row"), "a cut section is refused naming the row");
    assert!(decode(&b[..5], core).unwrap_err().0.contains("lakes"));
    let mut trailing = b.clone();
    trailing.push(0);
    assert!(decode(&trailing, core).unwrap_err().0.contains("follow"));
    // A run outside the tile: the first row of the tile is dry up to the lake, so rewrite the lake index of the first run found to one past the list.
    let mut bad = b.clone();
    let list_end = 1 + 8;
    let mut at_row = list_end;
    for _ in 0..core {
        let runs = usize::from(u16::from_le_bytes([bad[at_row], bad[at_row + 1]]));
        if runs > 0 {
            bad[at_row + 2 + 4] = 5;
            break;
        }
        at_row += 2;
    }
    assert!(decode(&bad, core).unwrap_err().0.contains("outside"));
    // More lakes than a run label holds.
    let many: Vec<u64> = (0..=MAX_LAKES_PER_TILE as u64).collect();
    let mut labels = vec![DRY; 16 * 16];
    labels[..many.len()].copy_from_slice(&many);
    assert!(encode(&labels, 16).unwrap_err().0.contains("lists 256 lakes"));
}

/// The mask with any surface and status (for the fixtures below).
fn mask_with(zlf: &dyn Fn(i64, i64) -> f64, st: &dyn Fn(i64, i64) -> Status, ci: i64, cj: i64, core: usize) -> Vec<u64> {
    let w = core + 2 * HALO;
    let origin = (ci - HALO as i64, cj - HALO as i64);
    let z: Vec<f64> = (0..w * w).map(|k| zlf(origin.0 + (k % w) as i64, origin.1 + (k / w) as i64)).collect();
    lake_mask(&Window { zl: &z, w, origin, ratio: RATIO }, HALO, core, st, &level)
}

// spec: GEN-167, TEST-109
#[test]
fn m4b_every_tile_offset_gives_the_whole_grids_mask() {
    // One-cell cores at every cell of a box around the lake's ring strips: a reach under 8 or a source outside the region would change a cell.
    let reference = whole();
    for cj in 64..128i64 {
        for ci in 64..128i64 {
            let m = tile_mask(ci, cj, 1, HALO);
            assert_eq!(m[0], at(&reference, ci as usize, cj as usize), "the 1 x 1 tile at ({ci}, {cj}) differs from the whole grid");
        }
    }
}

// spec: GEN-167, TEST-109
#[test]
fn m4c_the_ring_stops_after_eight_steps_and_tiles_agree_on_it() {
    // A ring two H1 cells deep around a one-cell block, everything under the level: the wet set is the cells within 8 steps (8-neighbours) of the block, not the whole ring.
    let st = |i: i64, j: i64| {
        if (i, j) == (10, 10) {
            Status::Water(LAKE)
        } else if (8..=12).contains(&i) && (8..=12).contains(&j) {
            Status::Ring(LAKE)
        } else {
            Status::Dry
        }
    };
    let flat = |_: i64, _: i64| 90.0;
    let m = mask_with(&flat, &st, 0, 0, N);
    let wet = |x: i64, y: i64| m[y as usize * N + x as usize] == LAKE;
    for (x, y, want) in [
        (72, 84, true),
        (71, 84, false),
        (95, 84, true),
        (96, 84, false),
        (84, 72, true),
        (84, 71, false),
        (72, 72, true),
        (71, 71, false),
        (95, 95, true),
        (96, 96, false),
    ] {
        assert_eq!(wet(x, y), want, "cell ({x}, {y})");
    }
    for core in [40usize, 64, 160] {
        for tj in 0..N / core {
            for ti in 0..N / core {
                let t = mask_with(&flat, &st, (ti * core) as i64, (tj * core) as i64, core);
                for y in 0..core {
                    for x in 0..core {
                        assert_eq!(t[y * core + x], m[(tj * core + y) * N + ti * core + x], "tile ({ti}, {tj}) of {core}: ({x}, {y})");
                    }
                }
            }
        }
    }
}

// spec: GEN-167, TEST-109
#[test]
fn m4d_a_lake_across_the_low_edge_of_the_grid_agrees_in_negative_coordinates() {
    // The block straddles local x = 0 (H1 cells -2..2): the parents of negative local cells are negative H1 cells (div_euclid), the surface and the status are defined across the edge.
    let st = |i: i64, j: i64| if (-2..2).contains(&i) && (10..14).contains(&j) { Status::Water(LAKE) } else { Status::Dry };
    let bowl = |x: i64, y: i64| if sq(x, y - 96) <= 24 * 24 { 95.0 } else { 130.0 };
    let (lo, side) = (-64i64, 192usize);
    let m = mask_with(&bowl, &st, lo, 0, side);
    let cell = |x: i64, y: i64| m[y as usize * side + (x - lo) as usize];
    assert_eq!(cell(-9, 96), LAKE, "a cell at x = -9 has parent H1 cell -2 (water), not 0 or -1 only");
    assert_eq!(cell(-17, 96), DRY, "the parent of x = -17 is -3: dry");
    for core in [64usize, 48] {
        for tj in 0..(side / core) as i64 {
            for ti in 0..(side / core) as i64 {
                let t = mask_with(&bowl, &st, lo + ti * core as i64, tj * core as i64, core);
                for y in 0..core {
                    for x in 0..core {
                        assert_eq!(
                            t[y * core + x],
                            cell(lo + ti * core as i64 + x as i64, tj * core as i64 + y as i64),
                            "tile ({ti}, {tj}) of {core}: ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}

// spec: GEN-167, TEST-109
#[test]
fn m6b_a_section_that_is_not_canonical_or_not_in_the_tile_is_refused() {
    let core = 8usize;
    let mut labels = vec![DRY; core * core];
    labels[8 + 2..8 + 5].fill(LAKE);
    labels[8 + 6] = LAKE;
    let b = encode(&labels, core).unwrap();
    assert_eq!(decode(&b, core).unwrap(), labels);
    // Layout: 1 count, 8 id, then row 0 (count 0), row 1: count 2, runs of 5 bytes.
    let row1 = 1 + 8 + 2;
    let mut t = b.clone();
    assert_eq!(u16::from_le_bytes([t[row1], t[row1 + 1]]), 2);
    t[row1 + 2 + 2] = 0;
    t[row1 + 2 + 3] = 0; // length 0
    assert!(decode(&t, core).unwrap_err().0.contains("outside"));
    let mut t = b.clone();
    t[row1 + 2] = 7; // start 7 with length 3: past the row
    assert!(decode(&t, core).unwrap_err().0.contains("outside"));
    let mut t = b.clone();
    t[row1 + 2 + 5] = 3; // the second run starts at 3, inside the first (2..5)
    assert!(decode(&t, core).unwrap_err().0.contains("overlap"));
    // Two adjacent runs of one lake are not canonical (they would be one run).
    let mut adj = vec![DRY; core * core];
    adj[..4].fill(LAKE);
    let mut e = encode(&adj, core).unwrap();
    // Split the single run (0, 4) into (0, 2) and (2, 2) by hand: row 0 count 2.
    let row0 = 1 + 8;
    e.truncate(row0);
    e.extend([2, 0, 0, 0, 2, 0, 0, 2, 0, 2, 0, 0]);
    for _ in 1..core {
        e.extend([0, 0]);
    }
    assert!(decode(&e, core).unwrap_err().0.contains("one run"));
    // A duplicated lake id.
    let mut dup = vec![2u8];
    dup.extend(LAKE.to_le_bytes());
    dup.extend(LAKE.to_le_bytes());
    for _ in 0..core {
        dup.extend([0, 0]);
    }
    assert!(decode(&dup, core).unwrap_err().0.contains("increasing"));
}
