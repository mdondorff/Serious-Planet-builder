//! Tier A, GEN-148 (A3 sections 3.1 to 3.3, gate G-SURF): K7, the local pass in the new chain: the base read at every cell, the regional change read C1, the channels burned in, the breach of pass 1 at the
//! local grid, then pass 2's carve, thermal, deposition and pit spreading; the stored change against `B` and the runtime's `B + L` against the bake at the lattice nodes. Small tiles; no store read.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::deposit::{DepositParams, Kc};
use planet_generators::bake::k7::{burn_channels, k7_tile, pit_stats, surface_gate, Channel, K7Params, RegionField};
use planet_generators::bake::tile::{TileGrid, TileId};

/// A tiny planet: 384 cells a face edge of about 100 m.
fn tile_grid() -> TileGrid {
    TileGrid { cells: 384, tile: 48, halo: 8, radius_m: 100.0 * 384.0 * 2.0 / core::f64::consts::PI }
}

fn id() -> TileId {
    TileId { face: Face(1), ti: 3, tj: 4 }
}

/// The face cell-index coordinates of a direction on the tile's face.
fn face_xy(g: &TileGrid, d: Vec3) -> (f64, f64) {
    let n = g.cells as f64;
    let face = id().face;
    let w = d.dot(face.normal());
    let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
    let (s, t) = (plane(d.dot(face.u_axis()) / w), plane(d.dot(face.v_axis()) / w));
    ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5)
}

/// A valley falling 0.8 m a cell to the east, its axis the face cell row 212, with walls rising 0.05 m a cell squared and a long swell.
fn base(d: Vec3) -> f64 {
    let (x, y) = face_xy(&tile_grid(), d);
    400.0 - 0.8 * (x - 150.0) + 0.05 * (y - 212.0) * (y - 212.0) + 3.0 * libm::sin(0.03 * x)
}

/// The face cell-index coordinates of the window's cell `(a, b)`.
fn cell(g: &TileGrid, a: i64, b: i64) -> (f64, f64) {
    let (t, h) = (g.tile as i64, g.halo as i64);
    ((i64::from(id().ti) * t - h + a) as f64, (i64::from(id().tj) * t - h + b) as f64)
}

/// A dam across the channel's course: the regional change as a function of the direction, 40 m over a ridge of a few cells at the face cell column `ix`.
fn dam_at(g: &TileGrid, ix: f64, j_lo: f64, j_hi: f64) -> impl Fn(Vec3) -> f64 + Sync {
    let g = *g;
    move |d: Vec3| {
        let (x, y) = face_xy(&g, d);
        if y >= j_lo && y <= j_hi && libm::fabs(x - ix) <= 2.5 {
            40.0
        } else {
            0.0
        }
    }
}

/// A channel along a row of the tile from its west to its east end, the bed 60 m under the base and falling by 0.05 m a cell.
fn channel(g: &TileGrid, row: i64) -> Channel {
    let (h, t) = (g.halo as i64, g.tile as i64);
    // The whole window, to its border (the next tile's channel continues it): a trench that ends inside the window would be a sink.
    let w = t + 2 * h;
    let mut points: Vec<(f64, f64)> = (0..w).step_by(4).map(|a| cell(g, a, row)).collect();
    points.push(cell(g, w - 1, row));
    let levels: Vec<f64> = points
        .iter()
        .enumerate()
        .map(|(k, &(x, y))| {
            let d =
                face_to_direction(&TangentWarp, id().face, 2.0 * (x + 0.5) / g.cells as f64 - 1.0, 2.0 * (y + 0.5) / g.cells as f64 - 1.0);
            base(d) - 60.0 - 0.2 * k as f64
        })
        .collect();
    Channel { points, levels, q: Vec::new() }
}

// spec: GEN-148
#[test]
fn the_burn_in_only_lowers_and_reaches_the_profile_along_the_segment() {
    let w = 40;
    let mut z: Vec<f64> = (0..w * w).map(|p| 100.0 + ((p * 37) % 11) as f64).collect();
    let before = z.clone();
    let ch = Channel { points: vec![(3.0, 5.0), (12.0, 9.0), (30.0, 9.0)], levels: vec![90.0, 80.0, 70.0], q: Vec::new() };
    let lowered = burn_channels(&mut z, w, (0, 0), std::slice::from_ref(&ch));
    assert!(lowered > 20);
    assert!(z.iter().zip(&before).all(|(a, b)| a <= b), "only lowers");
    // The cells at the points are at their levels, and the line is 8-connected: no gap between consecutive cells of the line.
    for (&(x, y), &l) in ch.points.iter().zip(&ch.levels) {
        assert!(z[y as usize * w + x as usize] <= l + 1e-9, "the point at ({x}, {y}) is at its level");
    }
    let burnt: Vec<usize> = (0..w * w).filter(|&c| z[c] != before[c]).collect();
    for &c in &burnt {
        let (cx, cy) = ((c % w) as i64, (c / w) as i64);
        let near = burnt.iter().any(|&o| o != c && ((o % w) as i64 - cx).abs() <= 1 && ((o / w) as i64 - cy).abs() <= 1);
        assert!(near, "a burnt cell has a burnt neighbour");
    }
    // A cell already lower than the profile is not raised.
    let mut low = vec![10.0; w * w];
    assert_eq!(burn_channels(&mut low, w, (0, 0), &[ch]), 0);
    assert!(low.iter().all(|&v| v == 10.0));
}

// spec: GEN-148
#[test]
fn a_dam_across_a_channel_is_a_pit_before_the_burn_in_and_none_after() {
    let g = tile_grid();
    let row = g.halo as i64 + 20;
    let (_, y) = cell(&g, 0, row);
    let (x_dam, _) = cell(&g, g.halo as i64 + 24, 0);
    let regional = dam_at(&g, x_dam, y - 100.0, y + 100.0);
    let ch = channel(&g, row);
    let p = K7Params::standard(5, 0.0);
    let with = k7_tile(&g, id(), &base, &regional, std::slice::from_ref(&ch), &p, 2);
    let without = k7_tile(&g, id(), &base, &regional, &[], &p, 2);
    let start = with.pits[0].1;
    assert_eq!(with.pits[0].0, "base + regional change");
    assert!(start.pit_cells > 0 && start.max_m > 10.0, "the dam holds a lake before the burn-in: {start:?}");
    assert!(with.burned > 20, "the channel is burned in");
    assert!(with.pits[1].1.max_m < start.max_m / 2.0, "the burn-in drains the lake: {:?} against {start:?}", with.pits[1].1);
    assert!(
        with.pits[4].1.pit_cells * 10 < start.pit_cells,
        "after the whole pass the dam holds at most a tenth of its pit cells: {:?} against {start:?}",
        with.pits[4].1
    );
    assert!(with.pits[4].1.max_m <= with.pits[1].1.max_m + 1.0, "the whole pass leaves no deeper pit than the burn-in: {:?}", with.pits);
    assert_eq!(without.burned, 0);
    // Without the burn-in the pits stay (the breach of pass 1 at the local grid is reported, not asserted to drain a dam of 40 m: its first pass can deepen the floor it cuts from).
    assert!(without.pits[4].1.pit_cells > 0, "without the channel the dam still holds water after the whole pass: {:?}", without.pits);
    // The result is the same bits on any thread count.
    let again = k7_tile(&g, id(), &base, &regional, std::slice::from_ref(&ch), &p, 1);
    assert_eq!(with.z.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), again.z.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    assert_eq!(with.change, again.change);
}

// spec: GEN-148
// spec: GEN-154
#[test]
fn the_runtime_surface_is_the_bake_surface_at_the_centres_and_within_a_step_at_the_lattice_nodes() {
    let g = tile_grid();
    let p = K7Params::standard(5, 0.0);
    // A smooth regional change and no channel: C1 of the bake is B plus C1 of the stored change, so the runtime reads the bake's surface at the nodes (before quantisation) to rounding.
    let smooth = |d: Vec3| 6.0 * libm::sin(0.5 * d.x) - 4.0 * libm::cos(0.7 * d.y);
    let r = k7_tile(&g, id(), &base, &smooth, &[], &p, 2);
    let gate = surface_gate(&g, id(), &base, &r, 6);
    assert!(gate.nodes > 20, "lattice nodes read: {gate:?}");
    assert!(gate.max_centres_unquantised_m < 1e-9, "at the cells' centres the C1 read returns the stored value: {gate:?}");
    assert!(gate.max_centres_quantised_m <= 0.125 + 1e-9, "quantisation at the centres is at most half a step: {gate:?}");
    // The clamp of the C1 read (to the four cells around the point) is not linear: where the surface has an extremum (the valley's axis) the runtime's B + C1(L) differs from C1 of the bake by a few centimetres, not 0.
    assert!(gate.max_unquantised_m < 0.1, "G-SURF before quantisation on a smooth tile: {gate:?}");
    assert!(gate.max_quantised_m <= 0.25, "G-SURF after quantisation on a smooth tile, A3's bound of 0.25 m: {gate:?}");
    // With a burnt channel the clamp of the C1 read (to the four cells around the point) is not linear, so the runtime's read of B plus L can differ from the read of the bake near the channel: reported, bounded.
    let with = k7_tile(&g, id(), &base, &smooth, &[channel(&g, g.halo as i64 + 20)], &p, 2);
    let gate_c = surface_gate(&g, id(), &base, &with, 6);
    assert!(gate_c.max_centres_unquantised_m < 1e-9, "{gate_c:?}");
    assert!(gate_c.max_quantised_m < 1.0, "{gate_c:?}");
    // Reading the composed heights (B at the four cells plus L, clamped once) agrees with the bake at the nodes to the quantisation and the read of B: the reading that passes G-SURF.
    assert!(gate_c.max_compose_quantised_m <= 0.25, "{gate_c:?}");
    // G-SURF-I, the error against the local base slope and the overshoot of the runtime read: finite, not negative, and counted over the nodes with a rise.
    for g in [&gate, &gate_c] {
        assert!(
            g.max_error_over_rise.is_finite() && g.p99_error_over_rise >= 0.0 && g.p99_error_over_rise <= g.max_error_over_rise,
            "{g:?}"
        );
        assert!(
            g.max_runtime_overshoot_m.is_finite()
                && g.max_runtime_overshoot_m >= 0.0
                && g.runtime_overshoot_nodes <= g.nodes
                && g.nodes_with_rise <= g.nodes,
            "{g:?}"
        );
    }
    // The new numbers against the existing ones: the overshoot is the clamp of L plus the re-read of B (bounded by `max_base_resample_m` and half a step); the error over the rise is at most the difference itself, the rise being at least 1 m.
    for g in [&gate, &gate_c] {
        assert!(g.max_runtime_overshoot_m <= g.max_base_resample_m + 0.125 + 1e-9, "{g:?}");
        assert!(g.max_error_over_rise <= g.max_quantised_m + 1e-9, "{g:?}");
    }
    assert!(gate.nodes_with_rise > 0, "the smooth tile has a base rise of at least 1 m across a cell somewhere: {gate:?}");
}

// spec: GEN-148
#[test]
fn a_regional_field_is_read_c1_and_clamped() {
    let (w, h) = (12, 10);
    let data: Vec<f32> = (0..w * h).map(|p| (p % w) as f32 * 2.0 + (p / w) as f32 * 3.0).collect();
    let n = 384;
    let f = RegionField::new(Face(1), n, (100, 200), w, h, data.clone());
    // At a cell's centre the read returns the cell's value.
    for (a, b) in [(3usize, 4usize), (5, 5), (8, 2)] {
        let d = face_to_direction(
            &TangentWarp,
            Face(1),
            2.0 * ((100 + a) as f64 + 0.5) / n as f64 - 1.0,
            2.0 * ((200 + b) as f64 + 0.5) / n as f64 - 1.0,
        );
        assert!((f.at(d) - f64::from(data[b * w + a])).abs() < 1e-3, "cell ({a}, {b})");
    }
    // Far outside the window the read is clamped to the window's edge, within the range of its values.
    let far = face_to_direction(&TangentWarp, Face(1), 0.9, 0.9);
    let v = f.at(far);
    assert!((0.0..=f64::from(*data.iter().max_by(|a, b| a.total_cmp(b)).unwrap())).contains(&v));
    // The pits of a plane are none.
    let grid = planet_generators::bake::Grid::new(16, 16, 100.0);
    let z: Vec<f64> = (0..256).map(|p| 100.0 + (p % 16) as f64).collect();
    assert_eq!(pit_stats(&grid, &z, 0.0, 1e-3, 0.5, 0).pit_cells, 0);
}

// spec: GEN-154
#[test]
fn the_vertex_report_is_a_consistent_partition_of_the_nodes() {
    let g = tile_grid();
    let p = K7Params::standard(5, 0.0);
    let smooth = |d: Vec3| 6.0 * libm::sin(0.5 * d.x) - 4.0 * libm::cos(0.7 * d.y);
    for channels in [vec![], vec![channel(&g, g.halo as i64 + 20)]] {
        let r = k7_tile(&g, id(), &base, &smooth, &channels, &p, 2);
        let gate = surface_gate(&g, id(), &base, &r, 6);
        // The gate is at the centres; the vertices are reported.
        assert!(gate.max_centres_unquantised_m < 1e-9 && gate.max_centres_quantised_m <= 0.125 + 1e-9, "{gate:?}");
        // Every node is in one step class, every node over the bound in one too and in one bin.
        assert_eq!(gate.nodes_by_step.iter().sum::<usize>(), gate.nodes, "{gate:?}");
        assert_eq!(gate.over_by_step.iter().sum::<usize>(), gate.over_bound, "{gate:?}");
        for c in 0..3 {
            assert!(gate.over_by_step[c] <= gate.nodes_by_step[c], "{gate:?}");
        }
        assert!(gate.bins >= gate.bins_over && gate.busiest_bin_over <= gate.over_bound, "{gate:?}");
        assert_eq!(gate.bins_over > 0, gate.over_bound > 0, "{gate:?}");
        assert!(
            gate.p50_quantised_m <= gate.p95_quantised_m
                && gate.p95_quantised_m <= gate.p99_quantised_m
                && gate.p99_quantised_m <= gate.max_quantised_m,
            "{gate:?}"
        );
        assert!(gate.max_error_over_step.is_finite() && gate.max_error_over_step >= 0.0, "{gate:?}");
    }
}

/// FNV-1a over the bits of the window's heights and the stored change: the digest the plain-flood K7 must keep (GEN-160).
fn k7_digest(r: &planet_generators::bake::k7::K7Result) -> u64 {
    let words = r.z.iter().map(|v| v.to_bits()).chain(r.change.iter().map(|v| u64::from(v.to_bits())));
    words.fold(0xcbf2_9ce4_8422_2325u64, |h, w| (h ^ w).wrapping_mul(0x0100_0000_01b3))
}

// spec: GEN-160
#[test]
fn k7_with_the_plain_flood_is_the_k7_of_before_and_the_canonical_k7_is_thread_independent() {
    let g = tile_grid();
    let smooth = |d: Vec3| 6.0 * libm::sin(0.5 * d.x) - 4.0 * libm::cos(0.7 * d.y);
    let mut plain = K7Params::standard(5, 0.0);
    plain.canonical_flood = false;
    let with = k7_tile(&g, id(), &base, &smooth, &[channel(&g, g.halo as i64 + 20)], &plain, 2);
    let without = k7_tile(&g, id(), &base, &smooth, &[], &plain, 2);
    // The digests were taken on the code before GEN-160 (origin/main at 3d3ad99a).
    assert_eq!(k7_digest(&with), 0x68e2e76ffed7d2b7, "the plain-flood K7 with a channel is what it was");
    assert_eq!(k7_digest(&without), 0x7700012a670c5e69, "the plain-flood K7 without a channel is what it was");
    // The canonical flood is the default of the new chain. The valley above has no flat, so both floods agree on it; a surface cut flat at 380 m (a plain of the base) tells them apart.
    let canonical = K7Params::standard(5, 0.0);
    assert!(canonical.canonical_flood);
    let flat = |d: Vec3| (380.0 - base(d)).min(0.0);
    let ch = [channel(&g, g.halo as i64 + 20)];
    let on1 = k7_tile(&g, id(), &base, &flat, &ch, &canonical, 1);
    let on2 = k7_tile(&g, id(), &base, &flat, &ch, &canonical, 2);
    let off = k7_tile(&g, id(), &base, &flat, &ch, &plain, 2);
    assert_eq!(k7_digest(&on1), k7_digest(&on2), "the canonical K7 is the same bits on 1 and 2 threads");
    assert_ne!(k7_digest(&on2), k7_digest(&off), "on a flat the canonical flood and the plain one give another L");
}

// spec: GEN-160
#[test]
fn the_pit_statistics_and_the_sweep_follow_the_flood_parameter_and_the_plain_sweep_is_unchanged() {
    use planet_generators::bake::deposit::{deposit_on_own_route, deposit_on_own_route_flood};
    use planet_generators::bake::k7::pit_stats_with;
    // A flat chain between two sea cells, walls rising inwards (the fixture of bake_round's GEN-159 tests, on a square window).
    let g = planet_generators::bake::Grid::new(16, 16, 100.0);
    let mut z: Vec<f64> = (0..16 * 16).map(|p| 100.0 + (p % 16).min(p / 16).min(15 - p % 16).min(15 - p / 16) as f64).collect();
    let at = |x: usize| 8 * 16 + x;
    z[at(2)] = 0.0;
    z[at(11)] = 0.0;
    for x in 3..=10 {
        z[at(x)] = 0.01;
    }
    let plain = pit_stats_with(&g, &z, 0.0, 0.02, 0.1, 0, false);
    let canonical = pit_stats_with(&g, &z, 0.0, 0.02, 0.1, 0, true);
    assert_eq!(plain.pit_cells, 3, "the plain flood lifts the chain's last cells: {plain:?}");
    assert_eq!(canonical.pit_cells, 0, "{canonical:?}");
    assert_eq!(pit_stats(&g, &z, 0.0, 0.02, 0.1, 0).pit_cells, plain.pit_cells, "pit_stats is the plain case");
    // The sweep: the plain case is bit for bit `deposit_on_own_route` (pass 2's call).
    let original: Vec<f64> = z.iter().map(|v| v + 0.5).collect();
    let params = DepositParams { kc: Kc::Uniform(1e5), zeta: 0.5, sea_level: 0.0 };
    let (mut a, mut b, mut old) = (z.clone(), z.clone(), z.clone());
    let ra = deposit_on_own_route(&g, &mut a, &original, 0.02, 7, &params);
    let rb = deposit_on_own_route_flood(&g, &mut b, &original, 0.02, 7, &params, false);
    // The old body of `deposit_on_own_route`, written out: the plain flood and the plain route.
    let outlet = g.outlets(&z, 0.0);
    let ro = planet_generators::bake::deposit::deposit(
        &g,
        &mut old,
        &original,
        &planet_generators::bake::flood::priority_flood(&g, &z, &outlet, 0.02),
        &planet_generators::bake::route::route(&g, &z, &outlet, 0.02, 7, 0.0),
        &params,
    );
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&old), "pass 2's call is the old body");
    assert_eq!(bits(&a), bits(&b));
    assert_eq!(ra.deposited.to_bits(), ro.deposited.to_bits());
    assert_eq!(rb.deposited.to_bits(), ro.deposited.to_bits());
    // The canonical sweep fills the chain from both sea cells: symmetric about the middle of columns 3 to 10, which the plain sweep's one-sided ramp is not.
    let mut c = z.clone();
    deposit_on_own_route_flood(&g, &mut c, &original, 0.02, 7, &params, true);
    assert!((3..=10).all(|x| c[at(x)].to_bits() == c[at(13 - x)].to_bits()), "the canonical sweep fills the chain from both sea cells");
    assert!(!(3..=10).all(|x| a[at(x)].to_bits() == a[at(13 - x)].to_bits()), "the plain sweep fills it from one end");
}

// spec: GEN-165, TEST-108
#[test]
fn the_width_law_is_a_q_to_the_b_and_a_trench_is_max_of_one_cell_and_the_width() {
    use planet_generators::bake::k7::{burn_channels_wide, WidthLaw};
    let law = WidthLaw::STANDARD;
    assert_eq!((law.a, law.b), (7.2, 0.5));
    assert!((law.width_m(100.0) - 72.0).abs() < 1e-12 && (law.width_m(1.0e4) - 720.0).abs() < 1e-9);
    assert_eq!((law.width_m(0.0), law.width_m(-3.0)), (0.0, 0.0), "no discharge, no width");
    let w = 64;
    let line = |q: f64| Channel { points: vec![(4.0, 32.0), (60.0, 32.0)], levels: vec![50.0, 50.0], q: vec![q, q] };
    let burn = |q: f64, cell_m: f64| {
        let mut z = vec![100.0; w * w];
        let n = burn_channels_wide(&mut z, w, (0, 0), &[line(q)], Some((&law, cell_m)));
        (z, n)
    };
    // A width under one cell burns exactly the one-cell line, and so does a channel without discharges and a burn without a law.
    let mut one = vec![100.0; w * w];
    let n1 = burn_channels(&mut one, w, (0, 0), &[Channel { q: Vec::new(), ..line(0.0) }]);
    let (narrow, nn) = burn(0.25, 100.0); // 7.2 * 0.5 = 3.6 m of a 100 m cell
    assert_eq!((narrow == one, nn), (true, n1), "narrower than a cell: the one-cell line");
    let mut no_law = vec![100.0; w * w];
    burn_channels_wide(&mut no_law, w, (0, 0), &[line(1.0e6)], None);
    assert!(no_law == one, "without a law every channel is one cell wide");
    // A wide trench: q = 100 m3/s is 72 m = 7.2 cells of 10 m; the cells whose centres are within 3.6 cells of the line are at the bed level, and nothing farther.
    let (wide, _) = burn(100.0, 10.0);
    for y in 0..w {
        for x in 0..w {
            let d = (y as f64 - 32.0).abs();
            let inside = (4.0..=60.0).contains(&(x as f64)) && d <= 3.6;
            if inside && x > 8 && x < 56 {
                assert_eq!(wide[y * w + x], 50.0, "({x}, {y}) is inside the trench");
            }
            if d > 3.6 + 1e-9 {
                assert_eq!(wide[y * w + x], 100.0, "({x}, {y}) is outside the trench and is not touched");
            }
        }
    }
    let across = (0..w).filter(|&y| wide[y * w + 30] == 50.0).count();
    assert_eq!(across, 7, "a trench of 7.2 cells is 7 cell centres across (those within 3.6 of the line)");
    // More discharge, a wider trench: the burnt cells grow as a set; and a bed higher than the ground raises nothing.
    let (bigger, _) = burn(400.0, 10.0);
    assert!((0..w * w).all(|c| wide[c] == 100.0 || bigger[c] == 50.0), "the trench of 400 m3/s contains the one of 100");
    assert!((0..w * w).filter(|&c| bigger[c] == 50.0).count() > (0..w * w).filter(|&c| wide[c] == 50.0).count());
    let mut low = vec![10.0; w * w];
    assert_eq!(burn_channels_wide(&mut low, w, (0, 0), &[line(100.0)], Some((&law, 10.0))), 0);
    assert!(low.iter().all(|&v| v == 10.0));
}

// spec: GEN-165, TEST-108
#[test]
fn a_tile_burns_a_trench_of_the_width_law_and_the_law_is_in_the_parameters() {
    let g = tile_grid();
    let mut p = K7Params::standard(5, 0.0);
    assert_eq!(p.width, planet_generators::bake::k7::WidthLaw::STANDARD);
    let thin = channel(&g, g.halo as i64 + 20);
    let mut wide = thin.clone();
    wide.q = vec![50.0; thin.points.len()]; // 7.2 * sqrt(50) = 51 m, half a 102 m cell: narrower than a cell
    let none = |_d: Vec3| 0.0;
    let run = |c: &Channel, p: &K7Params| k7_tile(&g, id(), &base, &none, std::slice::from_ref(c), p, 1);
    let a = run(&thin, &p);
    let b = run(&wide, &p);
    assert_eq!(a.burned, b.burned, "a channel narrower than a cell burns the one-cell line");
    let mut big = wide.clone();
    big.q = vec![1.0e5; thin.points.len()]; // 2.3 km: 22 cells
    let c = run(&big, &p);
    let mask = |r: &planet_generators::bake::k7::K7Result| r.burn_mask.iter().filter(|&&b| b).count();
    assert!(mask(&c) > 10 * mask(&a), "a wide river lowers many more cells: {} against {}", mask(&c), mask(&a));
    // The law is a parameter: another `a` is another burn, `b = 0` makes every width the same.
    p.width = planet_generators::bake::k7::WidthLaw { a: 0.5, b: 0.5 };
    assert!(run(&big, &p).burned < c.burned, "a smaller a narrows the trench");
}

// spec: GEN-165, TEST-108
#[test]
fn a_trench_just_over_one_cell_wide_still_burns_the_line_on_half_integer_points_and_far_rivers_cost_nothing() {
    use planet_generators::bake::k7::{burn_channels_wide, WidthLaw};
    let law = WidthLaw::STANDARD;
    let w = 64;
    // Points on cell corners (x.5): the disc of a width of 1.2 cells has a radius of 0.6, under the 0.707 of the four nearest centres, so only the always-burnt line keeps the channel.
    let q_for = |cells: f64| (cells * 10.0 / law.a) * (cells * 10.0 / law.a);
    let mk = |q: f64, (x0, y0): (f64, f64), (x1, y1): (f64, f64)| Channel {
        points: vec![(x0, y0), (x1, y1)],
        levels: vec![50.0, 50.0],
        q: vec![q, q],
    };
    for (a, b) in [((4.5, 32.5), (60.5, 32.5)), ((4.5, 8.5), (50.5, 54.5)), ((4.5, 10.5), (60.5, 40.5))] {
        let burn = |q: f64| {
            let mut z = vec![100.0; w * w];
            burn_channels_wide(&mut z, w, (0, 0), &[mk(q, a, b)], Some((&law, 10.0)));
            z
        };
        let narrow = burn(q_for(0.99));
        let just_over = burn(q_for(1.2));
        let wide = burn(q_for(3.0));
        assert!(narrow.contains(&50.0), "the narrow channel burns its line");
        assert!(
            (0..w * w).all(|c| narrow[c] != 50.0 || just_over[c] == 50.0),
            "a trench of 1.2 cells contains the line of 0.99 ({a:?} to {b:?})"
        );
        assert!((0..w * w).all(|c| just_over[c] != 50.0 || wide[c] == 50.0), "the trench grows with the discharge ({a:?} to {b:?})");
    }
    // A river far outside the window lowers nothing and costs no more than the steps along it.
    let far = Channel { points: (0..2000).map(|k| (5000.0 + k as f64, 5000.0)).collect(), levels: vec![50.0; 2000], q: vec![1.0e7; 2000] };
    let mut z = vec![100.0; w * w];
    let t = std::time::Instant::now();
    assert_eq!(burn_channels_wide(&mut z, w, (0, 0), &[far], Some((&law, 102.0))), 0);
    assert!(t.elapsed().as_millis() < 500, "a river outside the window is skipped, not scanned: {:?}", t.elapsed());
}

// spec: GEN-165, TEST-108
#[test]
#[should_panic(expected = "the discharges of a channel are finite")]
fn a_discharge_that_is_not_finite_is_refused() {
    use planet_generators::bake::k7::{burn_channels_wide, WidthLaw};
    let ch = Channel { points: vec![(4.0, 4.0), (20.0, 4.0)], levels: vec![50.0, 50.0], q: vec![f64::INFINITY, 5.0] };
    burn_channels_wide(&mut vec![100.0; 32 * 32], 32, (0, 0), &[ch], Some((&WidthLaw::STANDARD, 10.0)));
}
