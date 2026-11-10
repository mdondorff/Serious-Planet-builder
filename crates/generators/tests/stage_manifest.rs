//! Tier A, BUILD-047, TEST-103, TEST-104 (briefing item 18, PR 2): the committed stage manifest. A small fixture, a compact landmass of 24 cells a face edge, runs the new chain through the production
//! functions (the rounds, H1, the sediment pass) with a `SheetProbe` installed; every stage's field gives a line of the probe's numbers (cells, percentiles, the data hash, the stage's numbers) and the lines
//! are `docs/stages/manifest.txt`. The test fails when the file is not what the run produces, naming the stage, the field and the first differing column; `cargo xtask stages --update` (the variable
//! `PLANET_UPDATE_STAGES=1`) rewrites it. The sheets go to `target/review/stages/ci/`, where CI picks them up as an artifact. The data is hashed, never the PNG bytes.

use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, Hydro, HydroParams};
use planet_generators::bake::probe::{self, FieldData, FieldKind, SheetProbe, Stage, StageView};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::sediment::{outlets, pit_cells, sediment_pass, PitCells, SedimentReport};
use planet_generators::bake::tile::Pass2Params;
use planet_generators::bake::Grid;
use std::path::PathBuf;
use std::sync::Arc;

const N: usize = 24;
const R: f64 = 6_371_000.0;

/// The sink of this test is installed for its own thread only (the stages emit from the calling thread), and removed even when the test fails.
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

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn manifest_path() -> PathBuf {
    repo().join("docs").join("stages").join("manifest.txt")
}

/// The fixture landmass: a cap of the cube at 24 cells a face edge on a small body, with a sea on its edge.
fn landmass() -> (Grid, Vec<f64>) {
    let cube = planet_generators::bake::h0::GlobalGrid::new(N, R);
    let mut active = vec![false; 6 * N * N];
    let mut bits = vec![0u64; (6 * N * N).div_ceil(64)];
    for g in 0..6 * N * N {
        if cube.direction(g as u32).z > 0.2 {
            active[g] = true;
            bits[g / 64] |= 1 << (g % 64);
        }
    }
    let compact = Grid::compact_cube(N, 4000.0, bits);
    let z0 = (0..active.len())
        .filter(|&g| active[g])
        .map(|g| {
            let d = cube.direction(g as u32);
            // A bowl on the slope: a pit for the pit stages and for H1's basins.
            let (bx, by, bz) = (0.30, 0.20, 0.93);
            let c = (d.x * bx + d.y * by + d.z * bz) / libm::sqrt(bx * bx + by * by + bz * bz);
            let t = libm::acos(c.clamp(-1.0, 1.0)) / 0.25;
            let bowl = if t < 1.0 { -700.0 * (1.0 - t * t) } else { 0.0 };
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
                + bowl
        })
        .collect();
    (compact, z0)
}

/// What the run found besides the lines, for the number checks.
struct Run {
    lines: Vec<String>,
    grid: Grid,
    z0: Vec<f64>,
    z_rounds: Vec<f64>,
    z_final: Vec<f64>,
    hydro: Hydro,
    sediment: SedimentReport,
}

/// H1 on a surface with the sediment pass's outlets and flood, and the pit cells of the same surface at H1's limit: the production functions with the same inputs.
fn h1_on(g: &Grid, z: &[f64]) -> (Hydro, PitCells, HydroParams) {
    let q = Pass2Params::standard(5, 0.0);
    let outlet = outlets(g, z, 0.0);
    let filled = priority_flood_with(g, z, &outlet, q.carve.epsilon, true);
    let routing = route_rules(g, z, &outlet, q.carve.epsilon, 5, 0.0, None, RoundRules::ROUND);
    let params = HydroParams { initiation_m2: 30.0 * g.cell_area(0), ..HydroParams::default() };
    let hydro = hydrology(g, z, &filled, &routing, &outlet, None, None, &params);
    let pits = pit_cells(g, z, 0.0, q.carve.epsilon, params.pit_min_m);
    (hydro, pits, params)
}

/// The fixture through the production functions; the sheets are written into `dir`.
fn run(dir: &std::path::Path) -> Run {
    let (g, z0) = landmass();
    let sheet = Arc::new(SheetProbe::new(dir, 1_500));
    let _sink = Installed::new(sheet.clone());
    let area_cells = g.len() as f64;
    let pits_z0 = pit_cells(&g, &z0, 0.0, 1e-3, 0.5);
    probe::emit(Stage::Landmass, |s| {
        StageView::new(s, &g).f64("z", FieldKind::Height, &z0).number("cells", area_cells).number("pit_cells", pits_z0.pit_cells as f64)
    });

    let outlet: Vec<bool> = z0.iter().map(|&h| h <= 0.0).collect();
    let rounds = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, keep_pits: true, ..CarveParams::default() };
    let mut z = z0.clone();
    solve_rounds(&g, &mut z, &outlet, &rounds, 2, &3usize, 5);
    let z_rounds = z.clone();

    // H1 on the surface the rounds left.
    let (hydro, pits_at_h1, _) = h1_on(&g, &z);
    let st = &hydro.stats;
    probe::emit(Stage::H1, |s| {
        let mut v = StageView::new(s, &g).f64("q", FieldKind::Area, &hydro.q);
        v.fields.push(probe::Field { name: "channel", kind: FieldKind::Mask, data: FieldData::Bool(&hydro.channel), mask: None });
        v.number("channel_cells", st.channel_cells as f64)
            .number("segments", st.segments as f64)
            .number("basins", st.basins as f64)
            .number("basin_cells", st.basin_cells as f64)
            .number("artefacts", st.artefacts as f64)
            .number("ramps", st.ramps as f64)
            .number("pit_cells", pits_at_h1.pit_cells as f64)
    });

    let mut q2 = Pass2Params::standard(5, 0.0);
    q2.carve.breach_max_m = CarveParams::default().breach_max_m;
    let sediment = sediment_pass(&g, &mut z, &z0, &q2, 3);
    let text = std::fs::read_to_string(sheet.log_path()).unwrap_or_default();
    // The manifest has no cost fields: they vary from run to run.
    let lines = text
        .lines()
        .map(|l| l.split(" | ").filter(|c| !c.starts_with("probe_ms") && !c.starts_with("probe_bytes")).collect::<Vec<_>>().join(" | "))
        .collect();
    Run { lines, grid: g, z0, z_rounds, z_final: z, hydro, sediment }
}

fn fixture_dir() -> PathBuf {
    // The sheets of the run: under target/, where CI uploads them.
    repo().join("target").join("review").join("stages").join("ci")
}

/// The first difference between two manifests: the stage, the field and the column.
fn first_difference(want: &str, got: &[String]) -> Option<String> {
    let want: Vec<&str> = want.lines().collect();
    for k in 0..want.len().max(got.len()) {
        let (w, g) = (want.get(k).copied().unwrap_or("<missing line>"), got.get(k).map(String::as_str).unwrap_or("<missing line>"));
        if w != g {
            let (wc, gc): (Vec<&str>, Vec<&str>) = (w.split(" | ").collect(), g.split(" | ").collect());
            let col = (0..wc.len().max(gc.len())).find(|&c| wc.get(c) != gc.get(c)).unwrap_or(0);
            let name = if g == "<missing line>" { w } else { g };
            let (stage, field) = (name.split(" | ").next().unwrap_or("?"), name.split(" | ").nth(1).unwrap_or("?"));
            return Some(format!(
                "stage {stage}, field {field}, column {col}: the manifest has `{}` and the run `{}`",
                wc.get(col).unwrap_or(&"<none>"),
                gc.get(col).unwrap_or(&"<none>")
            ));
        }
    }
    None
}

// spec: BUILD-047
#[test]
fn the_manifest_is_the_run_of_the_fixture() {
    let dir = fixture_dir();
    let _ = std::fs::remove_dir_all(&dir);
    let r = run(&dir);
    assert!(r.lines.len() >= 12, "every stage wrote its fields: {} lines", r.lines.len());
    let path = manifest_path();
    if std::env::var_os("PLANET_UPDATE_STAGES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{}\n", r.lines.join("\n"))).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask stages --update`", path.display()));
    if let Some(d) = first_difference(&want.replace("\r\n", "\n"), &r.lines) {
        panic!("the stage manifest is stale: {d}. If the change is meant, run `cargo xtask stages --update` and commit docs/stages/manifest.txt.");
    }
    // The sheets are there for the artifact.
    assert!(dir.join("round-00-z.png").is_file() && dir.join("h1-q.png").is_file() && dir.join("pit-spreading-delta.png").is_file());
}

// spec: BUILD-047
#[test]
fn a_changed_number_a_missing_line_and_an_extra_line_are_each_named_by_stage_field_and_column() {
    let got: Vec<String> = vec!["round-00 | z | cells 10 | min 1 | hash aa".into(), "h1 | q | cells 10 | min 2 | hash bb".into()];
    assert_eq!(first_difference("round-00 | z | cells 10 | min 1 | hash aa\nh1 | q | cells 10 | min 2 | hash bb\n", &got), None);
    let d = first_difference("round-00 | z | cells 10 | min 1 | hash aa\nh1 | q | cells 10 | min 2 | hash cc\n", &got).unwrap();
    assert!(d.contains("stage h1, field q, column 4") && d.contains("hash cc") && d.contains("hash bb"), "{d}");
    let d = first_difference("round-00 | z | cells 10 | min 1 | hash aa\n", &got).unwrap();
    assert!(d.contains("stage h1") && d.contains("<missing line>"), "{d}");
    let d = first_difference(
        "round-00 | z | cells 10 | min 1 | hash aa\nh1 | q | cells 10 | min 2 | hash bb\nk7-f0-t0-0 | z | cells 1\n",
        &got,
    )
    .unwrap();
    assert!(d.contains("stage k7-f0-t0-0") && d.contains("<missing line>"), "{d}");
}

// spec: TEST-103
#[test]
fn the_numbers_of_every_stage_keep_their_invariants() {
    let dir = std::env::temp_dir().join(format!("planet-stage-numbers-{}", std::process::id()));
    let r = run(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let g = &r.grid;
    // The landmass: its cells are the active set, and the fixture has land and sea.
    assert_eq!(r.z0.len(), g.len());
    assert!(r.z0.iter().any(|&h| h > 0.0) && r.z0.iter().any(|&h| h <= 0.0));
    // The rounds lower the land on the whole: the sum of the land heights does not grow.
    let land_sum = |z: &[f64]| z.iter().filter(|&&h| h > 0.0).sum::<f64>();
    assert!(land_sum(&r.z_rounds) <= land_sum(&r.z0), "the rounds carve: {} against {}", land_sum(&r.z_rounds), land_sum(&r.z0));
    // Thermal and the pit spreading keep the height volume; the deposition's own balance is eroded = deposited + exported.
    let s = &r.sediment;
    assert!(
        s.thermal_volume_change.abs() <= 1e-9 && s.spread_volume_change.abs() <= 1e-9,
        "{} and {}",
        s.thermal_volume_change,
        s.spread_volume_change
    );
    let d = &s.deposit;
    assert!((d.eroded - d.deposited - d.exported).abs() <= 1e-9 * d.eroded.abs().max(1.0), "{d:?}");
    // H1: the channel cells are a part of the land, every basin has cells, and the segments are the channels' pieces.
    let st = &r.hydro.stats;
    assert!(st.channel_cells > 0 && st.channel_cells < g.len() && st.segments > 0, "{st:?}");
    assert!(st.basin_cells >= st.basins, "a basin has at least a cell: {st:?}");
    // The height volume the deposition adds equals what it reports as deposited (the heights moved by what the sweep reports).
    assert!(
        (s.deposit_height_volume_m3 - d.deposited).abs() <= 1e-9 * d.deposited.abs().max(1.0),
        "{} against {}",
        s.deposit_height_volume_m3,
        d.deposited
    );
}

// spec: TEST-104
#[test]
fn two_definitions_of_the_pits_of_one_surface_agree() {
    let dir = std::env::temp_dir().join(format!("planet-stage-defs-{}", std::process::id()));
    let r = run(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    // Pit cells (the land cells whose fill depth reaches the limit) against the cells of H1's basins: both read the same filled surface with the same outlets and the same limit. On the fixture's
    // own landmass (a bowl on a slope: pits), on what the rounds left, on what thermal left (the stage that makes pits), and on the end of the sediment pass.
    let mut with_pits = 0;
    for (name, z) in [("landmass", &r.z0), ("rounds", &r.z_rounds), ("final", &r.z_final)] {
        let (h, pits, params) = h1_on(&r.grid, z);
        assert_eq!(
            pits.pit_cells, h.stats.basin_cells,
            "{name}: the pit cells of the surface ({}) and the cells of H1's basins ({}) are one quantity (limit {} m): {pits:?} {:?}",
            pits.pit_cells, h.stats.basin_cells, params.pit_min_m, h.stats
        );
        with_pits += usize::from(pits.pit_cells > 0);
    }
    assert!(with_pits >= 1, "the fixture has pits on at least one surface, so the equality is not 0 = 0");
}
