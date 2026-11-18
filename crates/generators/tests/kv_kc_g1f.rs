//! SPIKE-132 (spike branch spike/kv-kc, never merged): KV and KC on the G0 fixture (TEST-139) with the fixture's expected graph as the synthetic graph input (TEST-140's hand graph: its L beds,
//! `q_w` and exceptions). KS is off: the stage input is `S0` (labelled), so the protected basins are `S0`'s kept L basins and the H1 bowl's image (reading 5 names KS1; with KS off KS1 is `S0`).
//! The budget is the fixture's (contract section 6; the 10 m floor everywhere on this tiny body). `G1F_PRINT` prints the report, `G1F_DUMP=dir` writes the cross-sections.

#![allow(clippy::needless_range_loop)]

use crate::g0_fixture::fixture::*;
use crate::g0_graph::expected;
use planet_generators::bake::basins_rg::{produce, BasinParams};
use planet_generators::bake::h0::GlobalGrid;
use planet_generators::bake::kv_kc::{flag_residuals, run, KvParams, Switch};
use planet_generators::bake::river_graph::*;
use planet_generators::bake::Grid;

const BODY: u64 = 0x0132;

struct Case {
    s0: Vec<f64>,
    outlet: Vec<bool>,
    graph: Graph,
    protected: Vec<bool>,
    budget: Vec<f64>,
    trunk: Vec<usize>,
}

fn case() -> Case {
    let fx = build(4);
    let b = expected(&fx);
    let gl = Grid::cube(L_N, RADIUS_M);
    let outlet = gl.outlets(&fx.s0, SEA_LEVEL_M);
    let tile = |c: usize| {
        let (f, ti, tj) = tile_of_l(c);
        Domain::Tile { face: f as u8, ti: ti as u32, tj: tj as u32 }
    };
    let basins = produce(&gl, &fx.s0, &outlet, &b.graph.reaches, &tile, &BasinParams::l(BODY));
    let mut protected = vec![false; fx.s0.len()];
    for bs in basins.basins.iter().filter(|x| x.represented == Represented::ProtectedSurface) {
        for &c in &basins.cells[&bs.id] {
            protected[c] = true;
        }
    }
    let budget: Vec<f64> = (0..fx.s0.len()).map(|c| b.budget_h1[h1_of_l(c)]).collect();
    Case { s0: fx.s0, outlet, graph: b.graph, protected, budget, trunk: b.trunk_l }
}

struct Report {
    out: Vec<f64>,
    v: Vec<Violation>,
    lines: Vec<String>,
}

fn stage_run(k: &Case, switch: Switch) -> Report {
    let gl = Grid::cube(L_N, RADIUS_M);
    let r = run(
        &gl,
        &k.s0,
        &k.outlet,
        &k.graph,
        &k.protected,
        &k.budget,
        &KvParams::plan(RADIUS_M * core::f64::consts::FRAC_PI_2 / L_N as f64),
        switch,
    );
    // the features of the residuals are flagged budget_residual (lead #159: the writer flags them; reading 9: beside any other reason)
    let mut g = k.graph.clone();
    let flagged = flag_residuals(&mut g, &r.residuals);
    let conflict = flagged.with_other;
    assert_eq!(flagged.no_feature, 0, "every residual names a reach");
    assert!(r.residuals.is_empty() || flagged.reaches > 0, "a residual flags its reach");
    let stages: Vec<Stage> = match switch {
        Switch::KvOnly => vec![Stage { name: "kv", out: &r.kv, wanted_lowering: Some(&r.kv_wanted), wanted_fill: None, fills: false }],
        Switch::KcOnly => vec![Stage { name: "kc", out: &r.kc, wanted_lowering: None, wanted_fill: Some(&r.kc_wanted), fills: true }],
        Switch::Both => vec![
            Stage { name: "kv", out: &r.kv, wanted_lowering: Some(&r.kv_wanted), wanted_fill: None, fills: false },
            Stage { name: "kc", out: &r.kc, wanted_lowering: None, wanted_fill: Some(&r.kc_wanted), fills: true },
        ],
    };
    let v = validate_budget(&k.s0, &k.budget, &stages, &r.residuals, &g);
    let n = k.s0.len();
    let lowered: Vec<f64> = (0..n).map(|c| (k.s0[c] - r.kv[c]).max(0.0)).filter(|&x| x > 0.0).collect();
    let mut sorted = lowered.clone();
    sorted.sort_by(f64::total_cmp);
    let at = |f: f64| sorted.get(((sorted.len() as f64 * f) as usize).min(sorted.len().saturating_sub(1))).copied().unwrap_or(0.0);
    let filled: Vec<f64> = (0..n).map(|c| (r.kc[c] - r.kv[c]).max(0.0)).filter(|&x| x > 0.0).collect();
    let kv_res = r.residuals.iter().filter(|x| x.stage == "kv").count();
    let kc_res = r.residuals.iter().filter(|x| x.stage == "kc").count();
    let mut lines = vec![
        format!(
            "{switch:?}: KV lowered {} cells, p50 {:.2} p99 {:.2} max {:.2} m, {kv_res} clamps (residuals)",
            lowered.len(),
            at(0.5),
            at(0.99),
            sorted.last().copied().unwrap_or(0.0)
        ),
        format!(
            "{switch:?}: KC filled {} cells in {} patches, {:.0} m3, max {:.2} m, {kc_res} residuals",
            filled.len(),
            r.patches.len(),
            r.patches.iter().map(|p| p.1).sum::<f64>(),
            filled.iter().copied().fold(0.0, f64::max)
        ),
        format!("{switch:?}: residual features already exceptional (kept): {conflict}; budget violations {}", v.len()),
        format!("{switch:?}: KC breaches (reading 22) {}, cut volume {:.0} m3", r.breaches, r.breach_volume),
    ];
    if switch != Switch::KvOnly {
        // the coastal pit drains: KC breaches it on the stage input; after KV under reading 10 it is lowered with its surroundings and no KC patch is left there
        if switch == Switch::KcOnly {
            assert!(r.breaches >= 1, "{switch:?}: the coastal pit drains by a breach (reading 22)");
        } else {
            assert_eq!(r.patches.len(), 1, "{switch:?}: only the reported basin is filled; the coastal pit is drained");
        }
    }
    // divides: KV touches nothing within d_buf of a divide; protected basins: nothing at all
    let div_touched = (0..n).filter(|&c| r.near_divide[c] && r.kv[c] != k.s0[c]).count();
    let prot_touched = (0..n).filter(|&c| k.protected[c] && (r.kv[c] != k.s0[c] || r.kc[c] != k.s0[c])).count();
    lines.push(format!(
        "{switch:?}: divide cells {} ({} within d_buf), KV changes there {div_touched}; protected cells {}, changed {prot_touched}",
        r.divide.iter().filter(|&&d| d).count(),
        r.near_divide.iter().filter(|&&d| d).count(),
        k.protected.iter().filter(|&&p| p).count()
    ));
    // the antecedent ridge: along the trunk's L vertices, the lowering stays within the budget and the rise over the running minimum stays (left uncut)
    let out = if switch == Switch::KvOnly { r.kv.clone() } else { r.kc.clone() };
    let (mut low, mut rise, mut cut) = (f64::INFINITY, 0.0f64, 0.0f64);
    for &c in &k.trunk {
        low = low.min(out[c]);
        rise = rise.max(out[c] - low);
        cut = cut.max(k.s0[c] - out[c]);
    }
    lines.push(format!(
        "{switch:?}: trunk: largest lowering {cut:.2} m (budget {:.1}); rise over its running minimum after the stage {rise:.1} m",
        k.budget[k.trunk[0]]
    ));
    if let Ok(dir) = std::env::var("G1F_DUMP") {
        dump(&dir, switch, k, &r.kv, &r.kc, &r.near_divide);
    }
    assert_eq!(div_touched, 0, "{switch:?}: KV changed a cell within d_buf of a divide");
    assert_eq!(prot_touched, 0, "{switch:?}: a protected basin changed");
    assert!(cut <= k.budget[k.trunk[0]] + 1e-9, "{switch:?}: the trunk was cut beyond its budget");
    Report { out, v, lines }
}

/// Cross-sections as CSV: across the trunk at 10 E, across the confluence's cell, along the trunk over the ridge (22-28 E) and across it at 25 E.
fn dump(dir: &str, switch: Switch, k: &Case, kv: &[f64], kc: &[f64], near_divide: &[bool]) {
    std::fs::create_dir_all(dir).unwrap();
    let l = GlobalGrid::new(L_N, RADIUS_M);
    let conf = k
        .graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Confluence && n.level == Level::L)
        .map(|n| lon_lat(l.direction(n.anchor as u32)))
        .unwrap();
    let lines: [(&str, Box<dyn Fn(f64) -> (f64, f64)>); 4] = [
        ("trunk-across-10E", Box::new(|t| (10.0, TRUNK_LAT - 6.0 + 12.0 * t))),
        ("confluence-across", Box::new(move |t| (conf.0, conf.1 - 6.0 + 12.0 * t))),
        ("ridge-along-trunk", Box::new(|t| (22.0 + 6.0 * t, TRUNK_LAT))),
        ("ridge-across-25E", Box::new(|t| (RIDGE_LON, TRUNK_LAT - 6.0 + 12.0 * t))),
    ];
    for (name, f) in lines {
        let mut s = String::from("t,lon,lat,s0,kv,kc,protected,near_divide,on_graph\n");
        let on_graph: std::collections::BTreeSet<u64> =
            k.graph.reaches.iter().filter(|r| r.level == Level::L).flat_map(|r| r.vertices.iter().copied()).collect();
        let mut last = usize::MAX;
        for i in 0..=600 {
            let t = i as f64 / 600.0;
            let (lo, la) = f(t);
            let c = cell_at(L_N, lo, la);
            if c == last {
                continue;
            }
            last = c;
            s.push_str(&format!(
                "{t:.4},{lo:.4},{la:.4},{:.3},{:.3},{:.3},{},{},{}\n",
                k.s0[c],
                kv[c],
                kc[c],
                u8::from(k.protected[c]),
                u8::from(near_divide[c]),
                u8::from(on_graph.contains(&(c as u64)))
            ));
        }
        std::fs::write(format!("{dir}/{switch:?}-{name}.csv"), s).unwrap();
    }
}

// spec: SPIKE-132
#[test]
fn kv_and_kc_keep_the_budget_divides_basins_and_the_ridge_on_the_fixture() {
    let k = case();
    let mut all = Vec::new();
    for switch in [Switch::KvOnly, Switch::KcOnly, Switch::Both] {
        let r = stage_run(&k, switch);
        // KC holds a fill at the budget and records a residual: a fill clamp, which GEN-189's Stage cannot state (it has a wanted lowering, no wanted fill), so the validator reads it
        // as a residual without an event; those are checked here instead (the fill is exactly the budget, the need above it) and counted
        let gl = Grid::cube(L_N, RADIUS_M);
        let rr = run(
            &gl,
            &k.s0,
            &k.outlet,
            &k.graph,
            &k.protected,
            &k.budget,
            &KvParams::plan(RADIUS_M * core::f64::consts::FRAC_PI_2 / L_N as f64),
            switch,
        );
        let fill_clamp = |c: usize| {
            rr.residuals.iter().any(|x| {
                x.cell == c
                    && x.stage == "kc"
                    && (rr.kc[c] - rr.kv[c] - k.budget[c]).abs() < 1e-9
                    && x.needed_m > x.available_m
                    && (x.available_m - k.budget[c]).abs() < 1e-9
            })
        };
        let fill_clamps =
            r.v.iter().filter(|x| matches!(x, Violation::ResidualMismatch { cell, stage: "kc", .. } if fill_clamp(*cell))).count();
        all.push(format!("{switch:?}: KC fill clamps (residual held at the budget, outside the validator's terms): {fill_clamps}"));
        let hard: Vec<&Violation> =
            r.v.iter()
                .filter(|x| !matches!(x, Violation::ResidualFeatureNotFlagged { .. }))
                .filter(|x| !matches!(x, Violation::ResidualMismatch { cell, stage: "kc", .. } if fill_clamp(*cell)))
                .collect();
        assert!(hard.is_empty(), "{switch:?}: {hard:?}");
        all.extend(r.lines);
        let _ = r.out;
    }
    if std::env::var_os("G1F_PRINT").is_some() {
        for l in &all {
            println!("{l}");
        }
    }
}

// spec: SPIKE-132
#[test]
fn kv_and_kc_are_deterministic() {
    let k = case();
    let a = stage_run(&k, Switch::Both).out;
    let b = stage_run(&k, Switch::Both).out;
    assert_eq!(a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
}

/// A digest of a KV/KC run: both surfaces' bits, the residuals and the patches.
fn kvkc_digest(r: &planet_generators::bake::kv_kc::KvKc) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    r.kv.iter().chain(&r.kc).for_each(|x| x.to_bits().hash(&mut h));
    for x in &r.residuals {
        (x.cell, x.stage, x.needed_m.to_bits(), x.available_m.to_bits(), x.feature).hash(&mut h);
    }
    for (c, v) in &r.patches {
        (c, v.to_bits()).hash(&mut h);
    }
    h.finish()
}

// spec: SPIKE-132
#[test]
fn kv_and_kc_give_the_pinned_output() {
    // order #161: the memory cut keeps KV and KC's output bit-identical (digests pinned before it); the breach (reading 22) and the S_KH budget change it on purpose (re-pinned with
    // their reason)
    let k = case();
    let gl = Grid::cube(L_N, RADIUS_M);
    let mut got = Vec::new();
    for switch in [Switch::KvOnly, Switch::KcOnly, Switch::Both] {
        let r = run(
            &gl,
            &k.s0,
            &k.outlet,
            &k.graph,
            &k.protected,
            &k.budget,
            &KvParams::plan(RADIUS_M * core::f64::consts::FRAC_PI_2 / L_N as f64),
            switch,
        );
        got.push(kvkc_digest(&r));
    }
    if std::env::var_os("G1F_DIGEST").is_some() {
        println!("kvkc digests {:#018x} {:#018x} {:#018x}", got[0], got[1], got[2]);
    }
    assert_eq!(got, KVKC_DIGESTS.to_vec());
}

// KC only: re-pinned for reading 22 (#161); KV only and both re-pinned for reading 10 (#165: 93 of 3,462 divide cells exempted, KV acts on 57 more cells)
const KVKC_DIGESTS: [u64; 3] = [0x2459_5f35_b20c_984c, 0x4c07_3ae4_4545_fb75, 0xf32d_c94a_61e4_62f5];

// spec: SPIKE-132
#[test]
fn kv_s_budget_is_what_ks_left_against_s_kh() {
    // reading 21 (owner, #161): KS used `S_KH - s0` of the budget before KV, so KV lowers at most the rest
    use planet_generators::bake::kv_kc::run_with;
    let k = case();
    let gl = Grid::cube(L_N, RADIUS_M);
    let s_kh: Vec<f64> = k.s0.iter().map(|z| z + 5.0).collect();
    let p = KvParams::plan(RADIUS_M * core::f64::consts::FRAC_PI_2 / L_N as f64);
    let r = run_with(&gl, &k.s0, &k.outlet, &k.graph, &k.protected, &k.budget, Some(&s_kh), &p, Switch::KvOnly);
    let worst = (0..k.s0.len()).map(|c| (k.s0[c] - r.kv[c]) - (k.budget[c] - 5.0).max(0.0)).fold(f64::NEG_INFINITY, f64::max);
    assert!(worst <= 1e-9, "KV lowered {worst} m past the budget KS left");
    let plain = run(&gl, &k.s0, &k.outlet, &k.graph, &k.protected, &k.budget, &p, Switch::KvOnly);
    let max_low = |kv: &[f64]| (0..kv.len()).map(|c| k.s0[c] - kv[c]).fold(0.0, f64::max);
    assert!(max_low(&r.kv) < max_low(&plain.kv), "the reference takes budget away");
}

// spec: SPIKE-132
#[test]
fn reading_10_exempts_divide_cells_near_graph_lines() {
    // lead #165: a divide cell within W_floor(q_w)/2 + d_buf of a graph line is valley floor; the buffer spares it, so KV acts at confluences and junctions
    let k = case();
    let gl = Grid::cube(L_N, RADIUS_M);
    let on = KvParams::plan(RADIUS_M * core::f64::consts::FRAC_PI_2 / L_N as f64);
    let off = KvParams { exempt_near_lines: false, ..on };
    let a = run(&gl, &k.s0, &k.outlet, &k.graph, &k.protected, &k.budget, &on, Switch::KvOnly);
    let b = run(&gl, &k.s0, &k.outlet, &k.graph, &k.protected, &k.budget, &off, Switch::KvOnly);
    let n = k.s0.len();
    let divides = a.divide.iter().filter(|&&d| d).count();
    let exempt = a.divide_exempt.iter().filter(|&&d| d).count();
    assert!(exempt > 0 && exempt < divides, "some divide cells, not all, lie near a graph line: {exempt} of {divides}");
    assert!(b.divide_exempt.iter().all(|&d| !d), "off: none exempted");
    let near = |r: &planet_generators::bake::kv_kc::KvKc| r.near_divide.iter().filter(|&&d| d).count();
    assert!(near(&a) < near(&b), "the buffer shrinks: {} against {}", near(&a), near(&b));
    // KV now changes cells the old buffer froze, and still none within d_buf of a remaining divide
    let freed = (0..n).filter(|&c| b.near_divide[c] && !a.near_divide[c] && a.kv[c] != k.s0[c]).count();
    assert!(freed > 0, "KV acts where the buffer was");
    assert_eq!((0..n).filter(|&c| a.near_divide[c] && a.kv[c] != k.s0[c]).count(), 0);
    if std::env::var_os("G1F_PRINT").is_some() {
        println!("reading 10: {exempt} of {divides} divide cells exempted ({:.1} %); cells within d_buf {} -> {}; KV changes {freed} freed cells", 100.0 * exempt as f64 / divides as f64, near(&b), near(&a));
    }
}
