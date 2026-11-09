//! Tier A, GEN-161 and BUILD-046: the stage probe's hooks only read. The rounds and the sediment pass on a compact landmass give the same bits with a probe installed and without one, and with one
//! every stage writes its sheets (bounded in bins) and a line of numbers. Small grids; the sheets are written under the test's temp directory.

use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::probe::{self, SheetProbe};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::RoundRules;
use planet_generators::bake::sediment::sediment_pass;
use planet_generators::bake::tile::Pass2Params;
use planet_generators::bake::Grid;
use std::sync::Arc;

const N: usize = 24;
const R: f64 = 6_371_000.0;

/// The sink of this test is installed for its own thread only (the solvers emit from their calling thread): the tests of the binary that run beside it do not reach it, and it is removed even when the test fails.
struct Installed;

impl Installed {
    fn new(p: Arc<SheetProbe>) -> Installed {
        probe::install_local(p);
        Installed
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        probe::uninstall_local();
    }
}

fn landmass() -> (Grid, Vec<f64>, Vec<bool>) {
    let (n, small) = (N, 4000.0);
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let mut active = vec![false; 6 * n * n];
    let mut bits = vec![0u64; (6 * n * n).div_ceil(64)];
    for g in 0..6 * n * n {
        if cube.direction(g as u32).z > 0.2 {
            active[g] = true;
            bits[g / 64] |= 1 << (g % 64);
        }
    }
    let compact = Grid::compact_cube(n, small, bits);
    let z0: Vec<f64> = (0..active.len())
        .filter(|&g| active[g])
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    let outlet = z0.iter().map(|&h| h <= 0.0).collect();
    (compact, z0, outlet)
}

fn run(g: &Grid, z0: &[f64], outlet: &[bool]) -> (Vec<u64>, String, String) {
    let rounds = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let mut z = z0.to_vec();
    let report = solve_rounds(g, &mut z, outlet, &rounds, 2, &3usize, 5);
    let start = z.clone();
    let mut q = Pass2Params::standard(5, 0.0);
    q.carve.breach_max_m = CarveParams::default().breach_max_m;
    let sed = sediment_pass(g, &mut z, &start, &q, 3);
    (z.iter().map(|v| v.to_bits()).collect(), format!("{report:?}"), format!("{sed:?}"))
}

// spec: GEN-161
// spec: BUILD-046
#[test]
fn the_probe_leaves_every_output_bit_alone_and_writes_a_sheet_and_numbers_for_every_stage() {
    let (g, z0, outlet) = landmass();
    let off = run(&g, &z0, &outlet);

    let dir = std::env::temp_dir().join(format!("planet-stage-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let probe = Arc::new(SheetProbe::new(&dir, 2_000));
    let on = {
        let _sink = Installed::new(probe.clone());
        assert!(probe::enabled());
        run(&g, &z0, &outlet)
    };
    assert_eq!(off.0, on.0, "every height bit is the same with the probe on");
    assert_eq!((&off.1, &off.2), (&on.1, &on.2), "and so are the round report and the sediment report");

    for stage in ["round-00", "round-01", "thermal", "deposition", "pit-spreading"] {
        let fields: &[&str] = if stage.starts_with("round") { &["z", "area"] } else { &["z", "delta"] };
        for f in fields {
            let path = dir.join(format!("{stage}-{f}.png"));
            let png = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let info = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap().info().clone();
            assert!(info.width as usize * info.height as usize <= 2_000, "{stage}-{f}: {}x{} bins", info.width, info.height);
        }
    }
    let log = std::fs::read_to_string(probe.log_path()).unwrap();
    assert_eq!(log.lines().count(), 10, "two fields for each of five stages:\n{log}");
    assert!(log.lines().all(|l| l.contains("| hash ") && l.contains("| p50 ") && l.contains("probe_bytes")), "{log}");
    assert!(
        log.contains("round-01 | area (log10 m2) |")
            && log.contains("catchments")
            && log.contains("moved_volume_first_m3")
            && log.contains("passes"),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: GEN-161
#[test]
fn the_memoised_rounds_give_the_same_bits_reports_and_work_with_the_probe_on_and_off() {
    use planet_generators::bake::round::{solve_rounds_memo, Drivers, RoundCache};
    let (g, z0, outlet) = landmass();
    let p = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let go = |on: bool| {
        let dir = std::env::temp_dir().join(format!("planet-stage-probe-memo-{}-{on}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sheet = Arc::new(SheetProbe::new(&dir, 1_000));
        let _sink = on.then(|| Installed::new(sheet.clone()));
        let mut z = z0.clone();
        let mut cache = RoundCache::default();
        let (rep, work) = solve_rounds_memo(&g, &mut z, &outlet, &p, 2, &1usize, 5, &Drivers::default(), 12, &mut cache);
        let lines = std::fs::read_to_string(sheet.log_path()).map_or(0, |l| l.lines().count());
        let _ = std::fs::remove_dir_all(&dir);
        (z.iter().map(|v| v.to_bits()).collect::<Vec<u64>>(), format!("{rep:?} {work:?}"), lines)
    };
    let (off, on) = (go(false), go(true));
    assert_eq!((&off.0, &off.1), (&on.0, &on.1), "the memoised rounds are the same with the probe on");
    assert_eq!((off.2, on.2), (0, 4), "two rounds, two fields each, only with the probe");
}
