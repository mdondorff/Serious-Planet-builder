//! Tier A, TEST-139: the G0 minimum fixture (river-graph plan, G0 contract addendum section 2.7) builds deterministically and holds its features. The fixture is terrain and expectations only: the
//! features are checked with today's routing and flood (no graph code), and the measured values must equal the recorded table in `support/g0_expectations.md`.

#![allow(clippy::needless_range_loop)]

#[path = "support/g0_fixture.rs"]
mod fixture;

use fixture::*;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::h0::GlobalGrid;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

const EPS: f64 = 1e-3;

// The two-region split lies within one H1 column (3.75 degrees on face 0's equator) east of the divide.
const _: () = assert!(REGION_SPLIT_LON > DIVIDE_LON && REGION_SPLIT_LON - DIVIDE_LON < 3.75);

/// The receiver path from `from` to its outlet (the outlet included).
fn path(receiver: &[u32], from: usize) -> Vec<usize> {
    let mut out = vec![from];
    let mut c = from;
    while receiver[c] != NONE {
        c = receiver[c] as usize;
        out.push(c);
        assert!(out.len() <= receiver.len(), "a cycle in the receivers");
    }
    out
}

/// The recorded values: the rows `| key | value |` of the table under "## Recorded values".
fn recorded() -> Vec<(String, f64)> {
    let text = include_str!("support/g0_expectations.md");
    let section = text.split("## Recorded values").nth(1).expect("a section '## Recorded values'");
    section
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Key") && !l.starts_with("|---"))
        .map(|l| {
            let cols: Vec<&str> = l.split('|').map(str::trim).collect();
            (cols[1].trim_matches('`').to_string(), cols[2].parse::<f64>().unwrap_or_else(|_| panic!("a number in row '{l}'")))
        })
        .collect()
}

/// The fixture's measured values, by key (the keys of the recorded table).
fn measure(fx: &Fixture) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = Vec::new();
    let mut put = |k: &str, x: f64| v.push((k.to_string(), x));
    let h1 = GlobalGrid::new(H1_N, RADIUS_M);
    let l = GlobalGrid::new(L_N, RADIUS_M);
    let lon_lat_h1 = |p: usize| lon_lat(h1.direction(p as u32));
    let lon_lat_l = |p: usize| lon_lat(l.direction(p as u32));

    // H1: routing on z_s
    let gh = Grid::cube(H1_N, RADIUS_M);
    let outlet_h = gh.outlets(&fx.z_s, SEA_LEVEL_M);
    let rh = route_rules(&gh, &fx.z_s, &outlet_h, EPS, 1, 0.0, None, RoundRules::ROUND);
    let trunk = path(&rh.receiver, cell_at(H1_N, -27.0, TRUNK_LAT));
    let mouth = *trunk.last().unwrap();
    put("h1.trunk.outlet_lon", lon_lat_h1(mouth).0);
    put("h1.trunk.cells", trunk.len() as f64);
    put("h1.trunk.face_changes", trunk.windows(2).filter(|w| w[0] / (H1_N * H1_N) != w[1] / (H1_N * H1_N)).count() as f64);
    put(
        "h1.trunk.crosses_ridge",
        f64::from(u8::from(trunk.windows(2).any(|w| lon_lat_h1(w[0]).0 < RIDGE_LON && lon_lat_h1(w[1]).0 >= RIDGE_LON))),
    );
    let join = |head: usize| {
        let p = path(&rh.receiver, head);
        let Some(at) = p.iter().position(|c| trunk.contains(c)) else {
            let ll =
                |q: &[usize]| q.iter().map(|&c| format!("({:.1},{:.1})", lon_lat_h1(c).0, lon_lat_h1(c).1)).collect::<Vec<_>>().join(" ");
            panic!(
                "the tributary reaches the trunk:
tributary {}
trunk {}",
                ll(&p),
                ll(&trunk)
            );
        };
        (p[at], p.len())
    };
    let (c1, _) = join(cell_at(H1_N, -4.5, 29.0));
    let (c2, _) = join(cell_at(H1_N, 39.5, -27.0));
    put("h1.T1.confluence_lon", lon_lat_h1(c1).0);
    put("h1.T2.confluence_lon", lon_lat_h1(c2).0);
    let w = path(&rh.receiver, cell_at(H1_N, -30.5, TRUNK_LAT));
    put("h1.W.outlet_lon", lon_lat_h1(*w.last().unwrap()).0);
    // the divide: the H1 cells either side of it on the trunk's row drain to the two coasts
    let west = path(&rh.receiver, cell_at(H1_N, DIVIDE_LON - 1.0, TRUNK_LAT));
    let east = path(&rh.receiver, cell_at(H1_N, DIVIDE_LON + 1.0, TRUNK_LAT));
    put("h1.divide.west_outlet_lon", lon_lat_h1(*west.last().unwrap()).0);
    put("h1.divide.east_outlet_lon", lon_lat_h1(*east.last().unwrap()).0);
    // the two-region layout: the H1 cells either side of the divide lie in different regions, and neither catchment's path leaves its region
    let lay = &layouts()[1];
    let (dw, de) = (west[0], east[0]);
    put("h1.divide.west_region", f64::from(lay.region_of_h1[dw]));
    put("h1.divide.east_region", f64::from(lay.region_of_h1[de]));
    put("h1.trunk.region_changes", trunk.windows(2).filter(|w| lay.region_of_h1[w[0]] != lay.region_of_h1[w[1]]).count() as f64);
    put("h1.W.region_changes", w.windows(2).filter(|x| lay.region_of_h1[x[0]] != lay.region_of_h1[x[1]]).count() as f64);
    let fh = priority_flood_with(&gh, &fx.z_s, &outlet_h, EPS, false);
    let bowl_h = cell_at(H1_N, BOWL.0, BOWL.1);
    put("h1.bowl.depth_m", fh[bowl_h] - fx.z_s[bowl_h]);

    // L: routing and flood on S0
    let gl = Grid::cube(L_N, RADIUS_M);
    let outlet_l = gl.outlets(&fx.s0, SEA_LEVEL_M);
    let rl = route_rules(&gl, &fx.s0, &outlet_l, EPS, 1, 0.0, None, RoundRules::ROUND);
    let tl = path(&rl.receiver, cell_at(L_N, -27.0, TRUNK_LAT));
    put("l.trunk.outlet_lon", lon_lat_l(*tl.last().unwrap()).0);
    put("l.trunk.face_changes", tl.windows(2).filter(|w| w[0] / (L_N * L_N) != w[1] / (L_N * L_N)).count() as f64);
    put("l.trunk.tile_changes", tl.windows(2).filter(|w| tile_of_l(w[0]) != tile_of_l(w[1])).count() as f64);
    put(
        "l.trunk.crosses_tile_at_15",
        f64::from(u8::from(tl.windows(2).any(|w| tile_of_l(w[0]) != tile_of_l(w[1]) && (lon_lat_l(w[1]).0 - 15.0).abs() < 0.5))),
    );
    for (name, head) in [("l.T1.confluence_lon", (-4.5, 29.0)), ("l.T2.confluence_lon", (39.5, -27.0))] {
        let p = path(&rl.receiver, cell_at(L_N, head.0, head.1));
        let at = p.iter().position(|c| tl.contains(c)).expect("the tributary reaches the trunk at L");
        put(name, lon_lat_l(p[at]).0);
    }
    let (lw, le) =
        (path(&rl.receiver, cell_at(L_N, DIVIDE_LON - 0.5, TRUNK_LAT)), path(&rl.receiver, cell_at(L_N, DIVIDE_LON + 0.5, TRUNK_LAT)));
    put("l.divide.west_outlet_lon", lon_lat_l(*lw.last().unwrap()).0);
    put("l.divide.east_outlet_lon", lon_lat_l(*le.last().unwrap()).0);
    put("l.trunk.max_lat_dev", tl.iter().map(|&c| (lon_lat_l(c).1 - TRUNK_LAT).abs()).fold(0.0, f64::max));
    // the antecedent crossing: S0 along the trunk's latitude from 20 E to 30 E (201 samples, 0.05 degrees apart), the rise over the running minimum
    let (mut bed, mut rise) = (f64::INFINITY, 0.0f64);
    for k in 0..=200 {
        let c = cell_at(L_N, 20.0 + 0.05 * f64::from(k), TRUNK_LAT);
        bed = bed.min(fx.s0[c]);
        rise = rise.max(fx.s0[c] - bed);
    }
    put("l.ridge.s0_rise_m", rise);
    let ridge_c = cell_at(L_N, RIDGE_LON, TRUNK_LAT);
    put("l.ridge.s0_at_crossing_m", fx.s0[ridge_c]);
    put("l.ridge.b_at_crossing_m", fx.b[ridge_c]);
    put("l.ridge.s0_crest_over_bed_m", fx.s0[ridge_c] - trunk_bed(RIDGE_LON));
    // the closed basin at L
    // the basin: the cells the flood raises (by more than 1 mm) connected to the bowl's centre; its spill cell the first cell of the centre's receiver path outside it
    let fl = priority_flood_with(&gl, &fx.s0, &outlet_l, EPS, false);
    let bowl = cell_at(L_N, BOWL.0, BOWL.1);
    put("l.bowl.floor_m", fx.s0[bowl]);
    let mut basin = vec![false; fl.len()];
    let mut stack = vec![bowl];
    basin[bowl] = true;
    let mut cells = 0usize;
    while let Some(c) = stack.pop() {
        cells += 1;
        for q in (0..8).filter_map(|d| gl.neighbour(c, d)) {
            if !basin[q] && fl[q] > fx.s0[q] + 0.01 {
                basin[q] = true;
                stack.push(q);
            }
        }
    }
    put("l.bowl.cells", cells as f64);
    put("l.bowl.area_km2", (0..fl.len()).filter(|&c| basin[c]).map(|c| gl.cell_area(c)).sum::<f64>() / 1e6);
    let leave = path(&rl.receiver, bowl).into_iter().find(|&c| !basin[c]).expect("the basin spills");
    put("l.bowl.spill_m", fx.s0[leave]);
    let (slon, slat) = lon_lat_l(leave);
    put("l.bowl.spill_lon", slon);
    put("l.bowl.spill_lat", slat);
    let after = path(&rl.receiver, leave);
    put("l.bowl.outlet_lon", lon_lat_l(*after.last().unwrap()).0);
    v
}

// spec: TEST-139
#[test]
fn the_g0_fixture_is_deterministic_on_any_thread_count() {
    let one = build(1);
    for t in [2, 4, 7] {
        let many = build(t);
        for (name, a, b) in [("B", &one.b, &many.b), ("z_b", &one.z_b, &many.z_b), ("z_s", &one.z_s, &many.z_s), ("S0", &one.s0, &many.s0)]
        {
            assert_eq!(a.len(), b.len());
            let differ = a.iter().zip(b.iter()).position(|(x, y)| x.to_bits() != y.to_bits());
            assert_eq!(differ, None, "{name} differs on {t} threads at cell {differ:?}");
        }
    }
    assert_eq!(one.b.len(), 6 * L_N * L_N);
    assert_eq!(one.z_b.len(), 6 * H1_N * H1_N);
}

// spec: TEST-139
#[test]
fn the_g0_fixture_holds_its_features_and_the_recorded_values() {
    let fx = build(4);
    let got = measure(&fx);
    let want = recorded();
    let mut report = String::new();
    for (k, x) in &got {
        report.push_str(&format!("| `{k}` | {x:.3} |\n"));
    }
    if std::env::var_os("G0_FIXTURE_PRINT").is_some() {
        println!("{report}");
    }
    let mut bad = Vec::new();
    for (k, x) in &got {
        match want.iter().find(|(w, _)| w == k) {
            None => bad.push(format!("{k}: measured {x:.3}, not recorded")),
            Some((_, y)) if (x - y).abs() > 0.006 => bad.push(format!("{k}: measured {x:.3}, recorded {y:.3}")),
            _ => {}
        }
    }
    for (k, _) in &want {
        if !got.iter().any(|(g, _)| g == k) {
            bad.push(format!("{k}: recorded, not measured"));
        }
    }
    assert!(bad.is_empty(), "the fixture differs from support/g0_expectations.md:\n{}\nmeasured:\n{report}", bad.join("\n"));
    // the features themselves (the design, independent of the recorded numbers)
    let m = |k: &str| got.iter().find(|(g, _)| g == k).unwrap().1;
    assert!(m("h1.trunk.outlet_lon") > 68.0 && m("l.trunk.outlet_lon") > 68.0, "the trunk reaches the east coast");
    assert!(m("h1.W.outlet_lon") < -44.0 && m("h1.divide.west_outlet_lon") < -44.0, "river W reaches the west coast");
    assert!(m("h1.divide.east_outlet_lon") > 68.0, "east of the divide drains to the trunk's mouth");
    assert_eq!(m("h1.trunk.face_changes"), 1.0, "the trunk crosses one cube-face edge");
    assert!(m("l.trunk.crosses_tile_at_15") == 1.0, "the trunk crosses the tile boundary at 15 E");
    assert_eq!(m("h1.trunk.crosses_ridge"), 1.0, "the trunk crosses the ridge at H1 (antecedent)");
    assert!(m("l.ridge.s0_rise_m") > 0.0, "S0 keeps part of the ridge on the trunk's line");
    assert!(m("h1.divide.west_region") != m("h1.divide.east_region"), "the region boundary separates the two sides of the divide");
    assert_eq!((m("h1.trunk.region_changes"), m("h1.W.region_changes")), (0.0, 0.0), "each catchment's main path stays in its region");
    assert!(
        m("l.bowl.spill_m") - m("l.bowl.floor_m") >= 2.0 && m("l.bowl.area_km2") >= 0.25,
        "the closed basin is a lake (>= 2 m, >= 0.25 km2)"
    );
    assert!(m("l.bowl.outlet_lon") > 68.0, "the basin spills into the trunk's catchment");
}

// spec: TEST-139
#[test]
fn the_two_layouts_cover_every_cell_and_split_just_east_of_the_divide() {
    let [one, two] = layouts();
    assert_eq!(one.region_of_h1.len(), 6 * H1_N * H1_N);
    assert!(one.region_of_h1.iter().all(|&r| r == 0));
    assert!(two.region_of_h1.iter().all(|&r| r < 2) && two.region_of_h1.contains(&0) && two.region_of_h1.contains(&1));
    let h1 = GlobalGrid::new(H1_N, RADIUS_M);
    // on the trunk's row the split falls between the H1 columns either side of -30, within one H1 column (3.75 degrees) east of the divide
    let (w, e) = (cell_at(H1_N, REGION_SPLIT_LON - 1.0, TRUNK_LAT), cell_at(H1_N, REGION_SPLIT_LON + 1.0, TRUNK_LAT));
    assert_ne!(two.region_of_h1[w], two.region_of_h1[e]);
    let d = cell_at(H1_N, DIVIDE_LON, TRUNK_LAT);
    assert_eq!(two.region_of_h1[d], two.region_of_h1[w], "the divide lies in the western region");
    assert!(
        (lon_lat(h1.direction(e as u32)).0 - lon_lat(h1.direction(w as u32)).0 - 3.75).abs() < 0.01,
        "the split lies between two neighbouring H1 columns"
    );
    // every L cell's H1 cell is the H1 cell holding its centre (L centres never lie on H1 boundaries), so an L cell takes its H1 cell's region
    let l = GlobalGrid::new(L_N, RADIUS_M);
    for p in 0..6 * L_N * L_N {
        let (lon, lat) = lon_lat(l.direction(p as u32));
        assert_eq!(h1_of_l(p), cell_at(H1_N, lon, lat), "L cell {p}");
    }
    // the tiles of face 0 change at longitude -15 and 15 on the trunk's latitude: columns 0, 1, 2
    let col = |lon: f64| tile_of_l(cell_at(L_N, lon, TRUNK_LAT));
    assert_eq!([col(-15.2).1, col(-14.8).1, col(14.8).1, col(15.2).1], [0, 1, 1, 2]);
    assert_eq!((col(-15.2).0, col(15.2).0), (0, 0));
}
