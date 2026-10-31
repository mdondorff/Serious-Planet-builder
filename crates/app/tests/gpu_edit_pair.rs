//! Tier C, TEST-076: the edit shown as the REND-033 preview and the same edit at rest (the pair of the owner's drag report). The software adapter and a small
//! memory bake (the scene's smoke world): a 50 m hill at 200 m, the stale frame, the first frame after the edit and the exact frame once the refresh settled,
//! in the relief and normals debug views, judged by `planet_testkit::pair` (TEST-077). The verdict of each view is reported, not asserted: the pair is a
//! measurement of how far the preview is from the exact frame (REND-033), and what it finds is a finding for the planner. The test asserts what makes the
//! pair trustworthy: the control (the world after taking the edit back) is the stale frame bit for bit, and the edit is seen. The pictures go to
//! `target/review/edit-pair/`. The real-store run with the lit frame and the horizon is the scene `edit-pair`.

use planet::editperf::pair_frames;
use planet_frame::TerrainView;
use planet_render::{AdapterPolicy, GpuContext};
use planet_testkit::pair::{compare_pair, PairVerdict};
use planet_testkit::{repo_root, write_png, Image};

const VIEWS: [TerrainView; 2] = [TerrainView::Relief, TerrainView::Normals];
const GREY: u8 = 3;

// spec: TEST-076
#[test]
fn the_pair_of_an_edit_is_trustworthy_and_its_verdict_per_view_is_reported() {
    let args: Vec<String> = [
        "test-render",
        "--perf-smoke",
        "--scene",
        "edit-latency",
        "--adapter",
        "software",
        "--gen-priority",
        "normal",
        "--global-res",
        "128",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let o = planet::parse_args(&args).expect("options");
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let dir = repo_root().join("target").join("review").join("edit-pair");
    std::fs::create_dir_all(&dir).unwrap();
    let pairs = pair_frames(&o, &ctx, &VIEWS, 200.0, 50.0, None).expect("the pair frames");
    assert_eq!(pairs.len(), VIEWS.len());
    let mut seen = 0;
    for p in &pairs {
        let img = |f: &planet::editperf::StillFrame| Image::new(f.width, f.height, f.rgba.clone());
        let (stale, preview, exact) = (img(&p.stale), img(&p.preview), img(&p.exact));
        let name = format!("{:?}", p.view).to_lowercase();
        for (tag, i) in [("stale", &stale), ("preview", &preview), ("exact", &exact)] {
            write_png(&dir.join(format!("{name}-{tag}.png")), i).unwrap();
        }
        let differing = p.stale.rgba.iter().zip(&p.restored.rgba).filter(|(a, b)| a != b).count();
        println!("{name}: control, the world after taking the edit back against the stale frame: {differing} bytes differ");
        assert_eq!(
            p.stale.rgba, p.restored.rgba,
            "{name}: the world after taking the edit back differs from the stale frame ({differing} bytes)"
        );
        let r = compare_pair(&stale, &preview, &exact, GREY).expect("frames of one size");
        {
            use std::collections::BTreeSet;
            let set = |f: &planet::editperf::StillFrame| f.nodes.iter().map(|(t, l, _)| (*t, *l)).collect::<BTreeSet<_>>();
            let (s, p2, e) = (set(&p.stale), set(&p.preview), set(&p.exact));
            println!(
                "{name}: nodes drawn: stale {}, preview {}, exact {}; preview vs exact differ in {}, stale vs exact in {}",
                s.len(),
                p2.len(),
                e.len(),
                p2.symmetric_difference(&e).count(),
                s.symmetric_difference(&e).count()
            );
            let states: BTreeSet<&str> = p.preview.nodes.iter().map(|(_, _, st)| st.as_str()).collect();
            println!("{name}: preview node states {states:?}");
        }
        println!("{name}: {}", r.line());
        seen += usize::from(r.verdict != PairVerdict::TooSmall);
    }
    assert!(seen > 0, "no view shows the edit (every pair is TooSmall): the pair proves nothing");
}

/// The same pair over a real store on the discrete GPU, for the owner's drag report (run by hand: `PLANET_PAIR_STORE=C:/DEV/SeriousPlanetData/global16384
/// PLANET_ADAPTER=hardware cargo nextest run -p planet -E 'test(/gpu_edit_pair::the_pair_over_a_real_store/)' --run-ignored only --no-capture`; the machine must be
/// unlocked, the display on). Prints the pair of each case; the pictures go to `target/review/edit-pair/<altitude>-<radius>/`.
// spec: TEST-076
#[test]
#[ignore = "needs a real store (PLANET_PAIR_STORE) and an unlocked desktop"]
fn the_pair_over_a_real_store() {
    let store = std::env::var("PLANET_PAIR_STORE").expect("PLANET_PAIR_STORE names the store");
    let adapter = std::env::var("PLANET_ADAPTER").unwrap_or_else(|_| "hardware".into());
    let args: Vec<String> = [
        "test-render",
        "--perf-smoke",
        "--scene",
        "edit-latency",
        "--adapter",
        adapter.as_str(),
        "--store",
        store.as_str(),
        "--tau",
        "2.4",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let o = planet::parse_args(&args).expect("options");
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // PLANET_PAIR_SITE="lat,lon" moves the case from the scene's centre (latitude 20, longitude 30) to another place.
    let site = std::env::var("PLANET_PAIR_SITE").ok().map(|s| {
        let (a, b) = s.split_once(',').expect("PLANET_PAIR_SITE is lat,lon");
        (a.trim().parse::<f64>().expect("lat"), b.trim().parse::<f64>().expect("lon"))
    });
    for (altitude, radius) in [(200.0, 50.0), (200.0, 500.0), (2000.0, 50.0), (2000.0, 500.0)] {
        let dir = repo_root().join("target").join("review").join("edit-pair").join(format!("{altitude}m-{radius}m"));
        std::fs::create_dir_all(&dir).unwrap();
        let pairs = match pair_frames(&o, &ctx, &VIEWS, altitude, radius, site) {
            Ok(p) => p,
            Err(e) => {
                println!("{altitude} m, stamp {radius} m: NOT RUN: {e}");
                continue;
            }
        };
        for p in &pairs {
            let img = |f: &planet::editperf::StillFrame| Image::new(f.width, f.height, f.rgba.clone());
            let (stale, preview, exact) = (img(&p.stale), img(&p.preview), img(&p.exact));
            let name = format!("{:?}", p.view).to_lowercase();
            for (tag, i) in [("stale", &stale), ("preview", &preview), ("exact", &exact)] {
                write_png(&dir.join(format!("{name}-{tag}.png")), i).unwrap();
            }
            let control = p.stale.rgba == p.restored.rgba;
            let r = compare_pair(&stale, &preview, &exact, GREY).expect("frames of one size");
            let nodes =
                |f: &planet::editperf::StillFrame| f.nodes.iter().map(|(t, l, _)| (*t, *l)).collect::<std::collections::BTreeSet<_>>();
            let differ = nodes(&p.preview).symmetric_difference(&nodes(&p.exact)).count();
            let mut states = std::collections::BTreeMap::<String, usize>::new();
            for (_, l, s) in &p.stale.nodes {
                *states.entry(format!("L{l} {s}")).or_default() += 1;
            }
            println!("  drawn nodes of the stale frame: {states:?}");
            println!(
                "{altitude} m, stamp {radius} m, {name}: control {} | tiles differing preview/exact {differ} | {}",
                if control { "exact" } else { "DIFFERS" },
                r.line()
            );
        }
    }
}

/// The CPU plans of the case view over a real store (no GPU): how deep the plan goes and how much it holds back, with the pair's parameters and with plan-check's.
/// `PLANET_PAIR_STORE=... [PLANET_PAIR_SITE=lat,lon] cargo nextest run -p planet -E 'test(/gpu_edit_pair::the_plan_over_a_real_store/)' --run-ignored only --no-capture`.
// spec: TEST-076
#[test]
#[ignore = "needs a real store (PLANET_PAIR_STORE)"]
fn the_plan_over_a_real_store_is_reported_per_parameter_set() {
    let store = std::env::var("PLANET_PAIR_STORE").expect("PLANET_PAIR_STORE names the store");
    let args: Vec<String> = ["test-render", "--perf-smoke", "--scene", "edit-latency", "--adapter", "software", "--store", store.as_str()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let o = planet::parse_args(&args).expect("options");
    let site = std::env::var("PLANET_PAIR_SITE").ok().map(|s| {
        let (a, b) = s.split_once(',').expect("PLANET_PAIR_SITE is lat,lon");
        (a.trim().parse::<f64>().expect("lat"), b.trim().parse::<f64>().expect("lon"))
    });
    for altitude in [200.0, 2000.0] {
        println!("at {altitude} m:\n{}", planet::editperf::plan_probe(&o, altitude, site).unwrap_or_else(|e| e));
    }
}
