//! Tier A, GEN-163 (TEST-108): the lake producer. A bowl with a known sill, a river through it and a playa: the level of the balance in the three cases, the bounds, the records, the dropped channel cells
//! and the determinism. Small grids; no store read.

#![allow(clippy::needless_range_loop)]
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, BasinClass, Hydro, HydroParams};
use planet_generators::bake::lakes::{cut_segments, lakes_from_hydro, lakes_with_tiles, water_cells, Lakes, TileRef};
use planet_generators::bake::route::{route_rules, RoundRules, Routing, NONE};
use planet_generators::bake::Grid;

const W: usize = 40;
const EPS: f64 = 1e-3;

/// A slope rising to the east with a bowl at (10..16, 10..16): the floor at (12, 12), 36 cells, rim 590 m on the west.
fn bowl_land() -> Vec<f64> {
    let mut z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64).collect();
    for j in 10..16 {
        for i in 10..16 {
            z[j * W + i] = 300.0;
        }
    }
    z[12 * W + 12] = 280.0;
    z
}

struct Run {
    z: Vec<f64>,
    g: Grid,
    routing: Routing,
    hydro: Hydro,
}

fn run(z: &[f64], runoff: Option<f32>, evap: Option<f32>, p: &HydroParams) -> Run {
    let g = Grid::new(W, W, 800.0);
    let outlet = g.outlets(z, 0.0);
    let routing = route_rules(&g, z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, z, &outlet, EPS, true);
    let r = runoff.map(|v| vec![v; W * W]);
    let e = evap.map(|v| vec![v; W * W]);
    let hydro = hydrology(&g, z, &filled, &routing, &outlet, r.as_deref(), e.as_deref(), p);
    Run { z: z.to_vec(), g, routing, hydro }
}

fn small_t() -> HydroParams {
    HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() }
}

fn lakes(r: &Run) -> Lakes {
    lakes_from_hydro(&r.g, &r.routing, &r.hydro, &r.z)
}

// spec: GEN-163, TEST-108
#[test]
fn the_level_is_the_balance_in_the_three_cases() {
    let z = bowl_land();
    // Overflow: no evaporation, the level is the sill and everything continues.
    let r = run(&z, None, None, &small_t());
    let l = &lakes(&r).lakes[0];
    assert_eq!((l.class, l.overflows, l.wet), (BasinClass::Lake, true, true));
    assert_eq!(l.level_m, l.spill_m, "full to the sill");
    assert!((l.q_out - l.q_in).abs() <= 1e-9 * l.q_in);
    assert_eq!(l.area_m2, r.hydro.basins[0].area_m2[15], "the surface at the sill is the curve's last step");
    assert!((l.area_m2 - 36.0 * r.g.cell_area(0)).abs() < 1.0, "the surface at the sill is the bowl");
    // Below the sill: evaporation takes all of it; the level is where A(h) E meets the inflow, hand-computed between the 1st and 2nd step.
    let r = run(&z, Some(1.0e-8), Some(1.0e-6), &small_t());
    let (b, l) = (&r.hydro.basins[0], &lakes(&r).lakes[0]);
    let (h1, h2) = (b.floor_m + (b.spill_m - b.floor_m) / 16.0, b.floor_m + 2.0 * (b.spill_m - b.floor_m) / 16.0);
    let (w1, w2) = (b.area_m2[0] * 1.0e-6, b.area_m2[1] * 1.0e-6);
    let want = h1 + (b.q_in - w1) / (w2 - w1) * (h2 - h1);
    assert!(w1 < b.q_in && b.q_in <= w2, "between the 1st and 2nd step");
    assert_eq!((l.class, l.overflows, l.wet, l.q_out), (BasinClass::Endorheic, false, true, 0.0));
    assert!((l.level_m - want).abs() <= 1e-9 * want.abs(), "level {} against {want}", l.level_m);
    let a = l.area_m2;
    let e_open = f64::from(1.0e-6f32); // the evaporation field is f32
    assert!((a * e_open - b.q_in).abs() <= 1e-9 * b.q_in, "A(h) E meets the inflow: {} against {}", a * e_open, b.q_in);
    assert!(b.area_m2[0] < a && a <= b.area_m2[1], "the area at the level lies on the curve between the steps: {a}");
    // Q_in = 0: a playa, the floor and no water surface.
    let r = run(&z, Some(0.0), Some(1.0e-6), &small_t());
    let l = &lakes(&r).lakes[0];
    assert_eq!((l.q_in, l.wet, l.level_m, l.area_m2), (0.0, false, l.floor_m, 0.0));
}

// spec: GEN-163, TEST-108
#[test]
fn the_level_stays_between_the_floor_and_the_sill_and_the_outflow_follows_the_overflow() {
    let z = bowl_land();
    let cases = [
        (None, None),
        (Some(1.0e-8f32), Some(1.0e-6f32)),
        (Some(1.0e-8), Some(1.0e-9)),
        (Some(0.0), Some(1.0e-6)),
        (Some(1.0e-8), Some(3.0e-7)),
    ];
    for (runoff, evap) in cases {
        let r = run(&z, runoff, evap, &small_t());
        for l in &lakes(&r).lakes {
            assert!(
                l.floor_m <= l.level_m && l.level_m <= l.spill_m,
                "level {} outside [{}, {}] for {runoff:?} {evap:?}",
                l.level_m,
                l.floor_m,
                l.spill_m
            );
            assert_eq!(l.q_out > 0.0, l.overflows, "q_out is positive iff the basin overflows ({runoff:?} {evap:?})");
            assert!(l.q_out <= l.q_in + 1e-12 * l.q_in);
            assert!(l.area_m2 >= 0.0 && l.area_m2 <= 36.0 * r.g.cell_area(0) * (1.0 + 1e-9));
        }
    }
}

// spec: GEN-163, TEST-108
#[test]
fn one_lake_per_kept_basin_and_none_for_an_artefact_or_a_ramp() {
    let mut dent = bowl_land();
    dent[30 * W + 30] = 789.0; // a one-cell dent a metre deep: an artefact
    let r = run(&dent, None, None, &small_t());
    assert!(r.hydro.stats.artefacts >= 1);
    let ls = lakes(&r);
    let mut ids: Vec<u64> = r.hydro.basins.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    assert_eq!(ls.lakes.iter().map(|l| l.id).collect::<Vec<_>>(), ids, "the lakes are the kept basins, by id");
    assert_eq!(ls.lakes.len(), 1);
    // A flat plain ramps by the flood's epsilon: no basin, no lake.
    let g = Grid::new(1200, 1200, 800.0);
    let z = vec![10.0; 1200 * 1200];
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, None, None, &HydroParams::default());
    assert!(hydro.stats.ramps >= 1);
    assert!(lakes_from_hydro(&g, &routing, &hydro, &z).lakes.is_empty());
}

// spec: GEN-163, TEST-108
#[test]
fn no_channel_cell_stays_inside_a_kept_lake_and_the_river_continues_at_the_spill() {
    let r = run(&bowl_land(), None, None, &small_t());
    let ls = lakes(&r);
    assert!(ls.dropped_cells > 0, "a channel passes through the bowl: {:?}", r.hydro.stats);
    // Only a segment that loses a cell has pieces (none when all its cells are inside); every other segment is whole.
    let inside_any = |s: &planet_generators::bake::hydro::Segment| {
        let mut c = s.up as usize;
        (0..s.cells).any(|_| {
            let hit = r.hydro.label[c] != NONE;
            c = r.routing.receiver[c] as usize;
            hit
        })
    };
    let whole = r.hydro.segments.iter().filter(|s| !inside_any(s)).count();
    let touched: usize = r.hydro.segments.iter().filter(|s| inside_any(s)).map(|s| s.cells as usize).sum();
    let kept: usize = ls.pieces.iter().map(|p| p.cells as usize).sum();
    assert!(whole > 0 && touched > 0, "some segments are whole and some touch the lake");
    assert_eq!(kept + ls.dropped_cells, touched, "every cell of a touched segment is in a piece or dropped");
    let listed: std::collections::HashSet<u64> = ls.pieces.iter().map(|p| p.segment).collect();
    for s in r.hydro.segments.iter().filter(|s| listed.contains(&s.id)) {
        assert!(inside_any(s), "a listed segment loses a cell");
    }
    let cells = |p: &planet_generators::bake::lakes::RiverPiece| -> Vec<u32> {
        let mut v = vec![p.first];
        while v.len() < p.cells as usize {
            v.push(r.routing.receiver[*v.last().unwrap() as usize]);
        }
        v
    };
    for p in &ls.pieces {
        for &c in &cells(p) {
            assert_eq!(r.hydro.label[c as usize], NONE, "piece cell {c} of segment {} is inside a lake", p.segment);
            assert!(r.hydro.channel[c as usize]);
        }
    }
    // The river leaves the lake at its spill cell with the outflow added.
    let b = &r.hydro.basins[0];
    let sp = b.spill_cell;
    let continues = ls.pieces.iter().any(|p| p.first == sp) || r.hydro.segments.iter().any(|s| s.up == sp && !inside_any(s));
    assert!(continues, "a piece or a whole segment starts at the spill cell");
    assert!(
        r.hydro.qw[sp as usize] >= b.q_out * (1.0 - 1e-9),
        "Q_w at the spill cell {} carries q_out {}",
        r.hydro.qw[sp as usize],
        b.q_out
    );
    // The piece before ends at the last cell outside the basin: its receiver is a member.
    let last = |p: &&planet_generators::bake::lakes::RiverPiece| *cells(p).last().unwrap() as usize;
    let before: Vec<_> = ls.pieces.iter().filter(|p| r.hydro.label[r.routing.receiver[last(p)] as usize] != NONE).collect();
    assert!(!before.is_empty(), "a piece ends where the river enters the lake");
    for p in before {
        assert_eq!(p.q_last, r.hydro.qw[last(&p)]);
    }
    // Parts of a segment are numbered from zero without gaps.
    for s in &r.hydro.segments {
        let parts: Vec<u32> = ls.pieces.iter().filter(|p| p.segment == s.id).map(|p| p.part).collect();
        assert!(parts.iter().enumerate().all(|(k, &q)| q as usize == k));
    }
}

// spec: GEN-163, CON-14
#[test]
fn the_result_does_not_depend_on_the_order_of_the_basins() {
    // Two bowls; the basin list of the hydrology is reversed (labels remapped): the lakes and the pieces are the same.
    let mut z = bowl_land();
    for j in 26..31 {
        for i in 24..30 {
            z[j * W + i] = 380.0;
        }
    }
    z[28 * W + 26] = 360.0;
    let r = run(&z, None, None, &small_t());
    assert_eq!(r.hydro.basins.len(), 2);
    let want = lakes(&r);
    let mut swapped = Run { z: r.z.clone(), g: Grid::new(W, W, 800.0), routing: r.routing.clone(), hydro: clone_hydro(&r.hydro) };
    let n = swapped.hydro.basins.len();
    swapped.hydro.basins.reverse();
    for l in &mut swapped.hydro.label {
        if *l != NONE {
            *l = (n as u32 - 1) - *l;
        }
    }
    assert_eq!(lakes(&swapped), want);
    assert!(want.lakes.windows(2).all(|p| p[0].id < p[1].id), "the lakes are in the order of their id");
}

fn clone_hydro(h: &Hydro) -> Hydro {
    Hydro {
        q: h.q.clone(),
        qw: h.qw.clone(),
        channel: h.channel.clone(),
        segments: h.segments.clone(),
        basins: h.basins.clone(),
        label: h.label.clone(),
        stats: h.stats.clone(),
    }
}

// spec: GEN-163
#[test]
#[should_panic(expected = "the producer needs Q_w")]
fn a_released_discharge_is_refused_with_a_clear_message() {
    let mut r = run(&bowl_land(), None, None, &small_t());
    r.hydro.qw = Vec::new();
    lakes(&r);
}

// spec: GEN-163
#[test]
fn a_segment_cut_twice_has_numbered_runs_and_one_inside_a_lake_has_none() {
    // The labels of a copy of the hydrology are set by hand on the cells of the longest segment: two lakes on its chain, then the whole chain.
    let r = run(&bowl_land(), None, None, &small_t());
    let s = r.hydro.segments.iter().max_by_key(|s| s.cells).unwrap().clone();
    assert!(s.cells >= 8, "a long segment to cut: {}", s.cells);
    let chain: Vec<u32> = {
        let mut v = vec![s.up];
        while v.len() < s.cells as usize {
            v.push(r.routing.receiver[*v.last().unwrap() as usize]);
        }
        v
    };
    // The cells of the chain that are under water, by hand: the cut is `cut_segments` with that predicate.
    let with_labels = |inside: &[usize]| {
        let set: std::collections::HashSet<u32> = inside.iter().map(|&k| chain[k]).collect();
        let (pieces, dropped_cells) = cut_segments(&r.hydro, &r.routing, &|c| set.contains(&(c as u32)));
        Lakes { lakes: Vec::new(), pieces, dropped_cells, water: Vec::new(), ring: Vec::new() }
    };
    let mine = |l: &Lakes| l.pieces.iter().filter(|p| p.segment == s.id).map(|p| (p.part, p.first, p.cells)).collect::<Vec<_>>();
    // Cells 0, 3 and 4 inside: a run at the start is dropped, then a run of 2 (1..=2), then the rest (5..).
    let n = s.cells;
    let l = with_labels(&[0, 3, 4]);
    assert_eq!(mine(&l), vec![(0, chain[1], 2), (1, chain[5], n - 5)]);
    // A lake at the end: the last cell inside leaves one run from the start.
    let l = with_labels(&[n as usize - 1]);
    assert_eq!(mine(&l), vec![(0, chain[0], n - 1)]);
    // The whole chain inside: no piece, every cell dropped.
    let all: Vec<usize> = (0..n as usize).collect();
    let l = with_labels(&all);
    assert!(mine(&l).is_empty());
    assert!(l.dropped_cells >= n as usize);
}

/// The cells of a segment, from its first cell to the one before its end cell.
fn seg_cells(r: &Run, s: &planet_generators::bake::hydro::Segment) -> Vec<u32> {
    let mut v = vec![s.up];
    while v.len() < s.cells as usize {
        v.push(r.routing.receiver[*v.last().unwrap() as usize]);
    }
    v
}

fn piece_cells(r: &Run, ls: &Lakes) -> std::collections::HashSet<u32> {
    let mut set = std::collections::HashSet::new();
    for p in &ls.pieces {
        let mut c = p.first;
        for k in 0..p.cells {
            set.insert(c);
            if k + 1 < p.cells {
                c = r.routing.receiver[c as usize];
            }
        }
    }
    set
}

// spec: GEN-163, TEST-108
#[test]
fn a_gentle_shore_under_the_level_is_water_and_a_river_over_it_is_not_drawn() {
    // The bowl of the other tests with a shelf on its east side 0.2 m under the sill: H1 does not label it (under the 0.5 m pit limit), the lake covers it.
    let mut z = bowl_land();
    for j in 10..16 {
        for i in 16..19 {
            z[j * W + i] = 589.8;
        }
    }
    let r = run(&z, None, None, &small_t());
    let ls = lakes(&r);
    let level = ls.lakes[0].level_m;
    assert_eq!(level, ls.lakes[0].spill_m, "the lake is full to the sill");
    let water = water_cells(&r.g, &r.routing, &r.hydro, &r.z);
    let shelf: Vec<usize> =
        (0..W * W).filter(|&c| r.hydro.channel[c] && r.hydro.label[c] == NONE && z[c] < level && water.contains(c)).collect();
    assert!(!shelf.is_empty(), "channel cells on the shelf are water though H1 did not label them");
    assert!(shelf.iter().all(|&c| (16..19).contains(&(c % W)) && (10..16).contains(&(c / W))), "they are the shelf's: {shelf:?}");
    let kept = piece_cells(&r, &ls);
    for &c in &shelf {
        assert!(!kept.contains(&(c as u32)), "shelf cell {c} is in a piece");
    }
    // No cell of any piece is under the level of the lake it drains into, apart from the ones below the sill that drain away (west of the rim).
    assert!(kept.iter().all(|&c| z[c as usize] >= level || (c as usize % W) < 10), "a kept cell under the level and not below the sill");
    // The water does not climb out over the sill: nothing west of the rim, nothing on the slope above the lake.
    assert!(water.cells().iter().all(|&c| (10..19).contains(&(c as usize % W))), "the water stays in the basin and its shore");
}

/// A cone-shaped basin (radius 8 cells round (13, 13), floor 290 m, 25 m a cell) in the slope rising east: a lake of a few cells at the bottom when evaporation limits it.
fn cone_land() -> Vec<f64> {
    let mut z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64).collect();
    for j in 0..W {
        for i in 0..W {
            let (dx, dy) = (i as f64 - 13.0, j as f64 - 13.0);
            let r = libm::sqrt(dx * dx + dy * dy);
            if r < 8.0 {
                z[j * W + i] = 290.0 + 25.0 * r;
            }
        }
    }
    z
}

// spec: GEN-163, TEST-108
#[test]
fn an_endorheic_lake_below_its_sill_takes_the_river_down_to_the_waters_edge() {
    let z = cone_land();
    let r = run(&z, Some(1.0e-8), Some(1.0e-6), &HydroParams { initiation_m2: 2.0 * 800.0 * 800.0, ..HydroParams::default() });
    let ls = lakes(&r);
    let l = &ls.lakes[0];
    assert_eq!((l.class, l.wet, l.overflows), (BasinClass::Endorheic, true, false));
    assert!(
        l.level_m < l.spill_m - 100.0 && l.level_m > l.floor_m,
        "a small lake low in the cone: level {} of [{}, {}]",
        l.level_m,
        l.floor_m,
        l.spill_m
    );
    let water = water_cells(&r.g, &r.routing, &r.hydro, &r.z);
    let under: Vec<u32> = water.cells().into_iter().filter(|&c| z[c as usize] < l.level_m).collect();
    assert!(!under.is_empty(), "there is water");
    assert!(
        under.len() < r.hydro.basins[0].cells as usize / 4,
        "the lake covers a fraction of the basin: {} of {}",
        under.len(),
        r.hydro.basins[0].cells
    );
    // The drawn river: the cells of every segment out of the water and the dead ground below it (the whole segments and the pieces of the cut ones).
    let kept: std::collections::HashSet<u32> =
        r.hydro.segments.iter().flat_map(|s| seg_cells(&r, s)).filter(|&c| !water.contains(c as usize)).collect();
    assert!(piece_cells(&r, &ls).iter().all(|c| kept.contains(c)));
    // The river stays drawn on the dry slope above the water (labelled cells above the level), down to the water: the label rule dropped all of it.
    let dry_basin: Vec<u32> = kept.iter().copied().filter(|&c| r.hydro.label[c as usize] != NONE && z[c as usize] >= l.level_m).collect();
    assert!(dry_basin.len() > 5, "channel cells inside the basin and above the level are kept: {}", dry_basin.len());
    // The river reaches the water: a drawn cell drains into water (the segment goes under it or ends in it).
    assert!(kept
        .iter()
        .any(|&c| z[r.routing.receiver[c as usize] as usize] < l.level_m && water.contains(r.routing.receiver[c as usize] as usize)));
    // Nothing is drawn below the water inside the basin: after a segment's first water cell, every drawn cell is out of the basin (the routing climbs to the spill on the filled surface, no river runs there).
    for s in &r.hydro.segments {
        let mut under_water = false;
        for c in seg_cells(&r, s) {
            if z[c as usize] < l.level_m && water.contains(c as usize) {
                under_water = true;
            } else if under_water && r.hydro.label[c as usize] != NONE {
                assert!(!kept.contains(&c), "cell {c} of segment {} is drawn inside the basin below the water", s.id);
            }
        }
    }
    assert!(kept.iter().all(|&c| !water.contains(c as usize)));
}

// spec: GEN-163, TEST-108
#[test]
fn a_dry_lake_has_no_water_and_cuts_nothing() {
    let r = run(&cone_land(), Some(0.0), Some(1.0e-6), &small_t());
    let ls = lakes(&r);
    assert!(!ls.lakes[0].wet);
    assert!(water_cells(&r.g, &r.routing, &r.hydro, &r.z).cells().is_empty());
    assert_eq!((ls.pieces.len(), ls.dropped_cells), (0, 0));
}

// spec: GEN-163, STRM-080
#[test]
fn each_lake_lists_the_tiles_that_hold_its_water() {
    // Tiles of 8 cells: the bowl (columns 10..16) and the shelf of the gentle-shore fixture (columns 16..19) lie over two tile columns, rows 10..16 in one tile row.
    let mut z = bowl_land();
    for j in 10..16 {
        for i in 16..19 {
            z[j * W + i] = 589.8;
        }
    }
    let r = run(&z, None, None, &small_t());
    let tile_of = |c: usize| TileRef { face: 0, ti: (c % W / 8) as u32, tj: (c / W / 8) as u32 };
    let with = lakes_with_tiles(&r.g, &r.routing, &r.hydro, &r.z, &tile_of);
    let without = lakes(&r);
    assert!(without.lakes[0].tiles.is_empty(), "no tile map, no tiles");
    let water = water_cells(&r.g, &r.routing, &r.hydro, &r.z);
    let mut want: Vec<TileRef> =
        water.cells().iter().filter(|&&c| z[c as usize] < with.lakes[0].level_m).map(|&c| tile_of(c as usize)).collect();
    want.sort_unstable();
    want.dedup();
    assert_eq!(with.lakes[0].tiles, want);
    assert_eq!(
        want,
        vec![TileRef { face: 0, ti: 1, tj: 1 }, TileRef { face: 0, ti: 2, tj: 1 }],
        "the bowl is in tile column 1 and the shelf crosses into column 2"
    );
    let mut stripped = with.clone();
    stripped.lakes[0].tiles.clear();
    assert_eq!(stripped, without, "the tile map changes nothing else");
}

// spec: GEN-163, TEST-108
#[test]
fn the_producer_in_two_steps_around_the_release_gives_the_same_lakes() {
    use planet_generators::bake::lakes::{lake_records, river_pieces};
    let z = bowl_land();
    for (runoff, evap) in [(None, None), (Some(1.0e-8), Some(1.0e-6)), (Some(0.0), Some(1.0e-6))] {
        let mut r = run(&z, runoff, evap, &small_t());
        let whole = lakes(&r);
        // The plan's order: the pieces while `Q_w` is there, the records after `q` and `qw` are released.
        let pre = river_pieces(&r.g, &r.routing, &r.hydro, &r.z);
        r.hydro.q = Vec::new();
        r.hydro.qw = Vec::new();
        let split = Lakes {
            lakes: lake_records(&r.g, &r.hydro),
            pieces: pre.pieces,
            dropped_cells: pre.dropped_cells,
            water: pre.water,
            ring: pre.ring,
        };
        assert_eq!(split, whole, "the same lakes, pieces and dropped cells, bit for bit");
        assert_eq!(format!("{split:?}"), format!("{whole:?}"));
    }
}

// spec: GEN-167, TEST-109
#[test]
fn the_water_and_ring_lists_are_the_lakes_cells_and_the_cells_next_to_them_that_drain_in() {
    let z = bowl_land();
    let r = run(&z, None, None, &small_t());
    let l = lakes(&r);
    let id = r.hydro.basins[0].id;
    assert!(l.water.windows(2).all(|w| w[0].0 < w[1].0), "the water list is sorted by cell, one entry a cell");
    assert!(l.ring.windows(2).all(|w| w[0] < w[1]), "the ring list is sorted without repeats");
    assert!(l.water.iter().all(|e| e.1 == id) && l.ring.iter().all(|e| e.1 == id));
    // Every labelled cell under the level is water and every water cell is a cell of the water rule.
    let level = r.hydro.basins[0].water_level_m;
    for (c, &lab) in r.hydro.label.iter().enumerate() {
        if lab != NONE && r.z[c] < level {
            assert!(
                l.water.binary_search_by_key(&(c as u32), |e| e.0).is_ok(),
                "cell {c} of the basin under the level is in the water list"
            );
        }
    }
    let w = water_cells(&r.g, &r.routing, &r.hydro, &r.z);
    assert!(l.water.iter().all(|e| w.contains(e.0 as usize)));
    // The ring: next to the water, not water, drains into the lake: the east side of the bowl is in it, and a ring cell's receiver chain reaches the water.
    assert!(!l.ring.is_empty());
    assert!(l.ring.iter().any(|e| e.0 as usize == 12 * W + 16), "the cell east of the bowl drains into it");
    assert!(
        !l.ring.iter().any(|e| e.0 as usize == 12 * W + 9),
        "the west side is the spill side: the cell there drains away from the lake"
    );
    for &(c, _) in &l.ring {
        assert!(l.water.binary_search_by_key(&c, |e| e.0).is_err(), "a ring cell is not water");
        let mut cur = c as usize;
        let mut hit = false;
        for _ in 0..16 {
            let nxt = r.routing.receiver[cur];
            if nxt == NONE {
                break;
            }
            cur = nxt as usize;
            if l.water.binary_search_by_key(&(cur as u32), |e| e.0).is_ok() {
                hit = true;
                break;
            }
        }
        assert!(hit, "ring cell {c} drains into the lake's water");
    }
}

// spec: GEN-167, TEST-109
#[test]
fn the_channels_of_a_hydrology_follow_its_segments_with_q_w_and_a_bed_that_does_not_rise() {
    use planet_generators::bake::k7::channels_from_hydro;
    let z = bowl_land();
    let r = run(&z, Some(1.0e-8), None, &small_t());
    let ch = channels_from_hydro(&r.hydro, &r.routing, &r.z, &|c| ((c % W) as f64 * 8.0 + 3.5, (c / W) as f64 * 8.0 + 3.5));
    assert_eq!(ch.len(), r.hydro.segments.len());
    for (c, s) in ch.iter().zip(&r.hydro.segments) {
        assert_eq!(c.points.len(), s.cells as usize + 1, "the cells from up to down, both included");
        assert_eq!(c.q.len(), c.points.len());
        assert!(c.levels.windows(2).all(|w| w[1] <= w[0]), "the bed does not rise downstream");
        assert_eq!(c.q[0], r.hydro.qw[s.up as usize]);
        assert_eq!(*c.q.last().unwrap(), r.hydro.qw[s.down as usize]);
    }
}
