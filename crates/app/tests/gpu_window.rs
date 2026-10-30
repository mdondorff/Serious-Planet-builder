//! Tier C: the viewer window path end to end (VIEW-005, VIEW-007). The real binary opens the window on the lit path with the
//! atmosphere, runs the pipelined flight, presses the keys through the command path (L to the debug path and back, V on the debug
//! path), checks its own frame counts and closes it, on the adapter the run selects: the software adapter in `cargo xtask test gpu`
//! (test mode), the discrete GPU in `cargo xtask test gpu --real` (Rasierklinge).
//!
//! Where there is no display (a headless CI runner) the window cannot open. That is a skip with a reason, and only when it is
//! declared: the runner sets `PLANET_NO_DISPLAY` to the reason. Without the declaration a missing display fails the test, so a
//! machine that should have one never passes silently.

use std::process::Command;

/// The marker the binary's error carries when there is no display (`planet::window::NO_DISPLAY`).
const NO_DISPLAY: &str = planet::window::NO_DISPLAY;

fn adapter() -> String {
    std::env::var("PLANET_ADAPTER").unwrap_or_else(|_| "software".into())
}

/// Run `planet editor --smoke <extra>`; `None` is the declared skip, a failure panics.
fn smoke(extra: &[&str]) -> Option<String> {
    smoke_in(extra, &[])
}

/// [`smoke`] with more environment variables for the run.
fn smoke_in(extra: &[&str], envs: &[(&str, &str)]) -> Option<String> {
    run_editor(&["--smoke"], extra, envs)
}

/// Run `planet editor <flags> <extra>` with the run's adapter; `None` is the declared skip or a NOT RUN, a failure panics. On the discrete GPU a
/// locked desktop is a NOT RUN (TEST-073): the binary says so itself, and a failure with the desktop locked by the end of the run is the lock's.
fn run_editor(flags: &[&str], extra: &[&str], envs: &[(&str, &str)]) -> Option<String> {
    let name = format!("gpu_window editor {flags:?} {extra:?}");
    let hardware = adapter() != "software";
    // A nextest timeout kills the test process and nothing can look at the desktop afterwards: the watchdog looks while it runs and ends the process as
    // a NOT RUN before the shortest timeout (300 s) when the desktop was locked.
    planet_perf::desktop::start_watchdog(&name, hardware, std::time::Duration::from_secs(240));
    planet_perf::desktop::guarded(&name, hardware, &planet_perf::desktop::detect, planet_perf::desktop::marker_dir().as_deref(), || {
        run_editor_unguarded(flags, extra, envs)
    })
    .flatten()
}

fn run_editor_unguarded(flags: &[&str], extra: &[&str], envs: &[(&str, &str)]) -> Option<String> {
    let started = std::time::Instant::now();
    let adapter = adapter();
    let out = Command::new(env!("CARGO_BIN_EXE_planet"))
        .arg("editor")
        .args(flags)
        .args(extra)
        .env("PLANET_ADAPTER", &adapter)
        .env("PLANET_TIMING", "1")
        .envs(envs.iter().copied())
        .output()
        .expect("the planet binary starts");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() && stdout.lines().any(|l| l.starts_with("NOT RUN ")) {
        println!("{}", stdout.lines().find(|l| l.starts_with("NOT RUN ")).unwrap_or(""));
        return None;
    }
    // The phases of the run (process start to the last frame), for the CI log: where a window test spends its time (shown for passing tests by the nextest override).
    if let Some(line) = stdout.lines().find(|l| l.contains("[timing]")) {
        println!("TIMING editor {flags:?} {extra:?}: wall {:.0} ms; {line}", started.elapsed().as_secs_f64() * 1e3);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        if stderr.contains(NO_DISPLAY) {
            let reason = stderr.lines().find(|l| l.contains(NO_DISPLAY)).unwrap_or("").trim().to_string();
            match std::env::var("PLANET_NO_DISPLAY") {
                Ok(declared) if !declared.is_empty() => {
                    eprintln!("SKIPPED the viewer window smoke (VIEW-005): {reason}; this runner declares PLANET_NO_DISPLAY='{declared}'");
                    return None;
                }
                _ => panic!(
                    "the viewer window cannot open here ({reason}). On a runner without a display declare the skip with \
                     PLANET_NO_DISPLAY=<reason>; on a machine that has one this is a failure."
                ),
            }
        }
        panic!("planet editor {flags:?} {extra:?} failed ({}):\n{stdout}\n{stderr}", out.status);
    }
    // A panic on a pool worker is caught and reported, and the window goes on: a run with one is not a run that worked.
    assert!(!stderr.contains("panicked at"), "planet editor {flags:?} {extra:?} panicked on a thread and went on:\n{stdout}\n{stderr}");
    Some(stdout)
}

/// The number in front of `after` in the smoke summary, e.g. `44` in `(44 lit` for `" lit"`.
fn count_before(stdout: &str, after: &str) -> u64 {
    let summary = stdout.split("[smoke ok]:").nth(1).unwrap_or(stdout);
    summary
        .split(after)
        .next()
        .and_then(|s| s.rsplit(|c: char| !c.is_ascii_digit()).next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no count before '{after}' in: {stdout}"))
}

// spec: VIEW-005, VIEW-007, LOOK-005
#[test]
fn the_window_path_opens_runs_the_flight_lit_with_the_atmosphere_and_closes() {
    // The two runs are separate processes (two WARP devices in one process crash it, docs/status.md) and judge frame counts, never
    // wall-clock, so they run at the same time: on the software adapter each run is dominated by the adapter's own frame time and
    // together they take little more than one (measured on a 20-core machine only: 2 x 30 s alone, 32 s together; 110 s to 90 s inside the whole `test gpu` run).
    let dir = std::env::temp_dir().join(format!("planet-viewer-look-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let look = dir.join("moonlit.look.toml");
    // A sun from +z, a dimmer sun and a narrower exposure range: all of it reaches the lit plan (tier A pins the plan; this runs it).
    std::fs::write(
        &look,
        "schema_version = 3
kind = \"look\"
[body.0.sun]
direction = [0.0, 0.0, 1.0]
[body.0.units]
solar_illuminance_lux = 90000.0
         [body.0.exposure]
min_ev100 = 4.0
max_ev100 = 16.0
",
    )
    .unwrap();
    let (first, second) = std::thread::scope(|s| {
        let first = s.spawn(|| smoke(&[]));
        let second = s.spawn(|| smoke(&["--look", look.to_str().unwrap(), "--no-ui"]));
        (first.join(), second.join())
    });
    let _ = std::fs::remove_dir_all(&dir);
    // A panic in either run (a failed smoke) fails the test with its own message.
    let first = first.unwrap_or_else(|e| std::panic::resume_unwind(e));
    let second = second.unwrap_or_else(|e| std::panic::resume_unwind(e));
    let (Some(first), Some(second)) = (first, second) else { return };
    the_window_runs_lit_with_the_atmosphere(&first);
    the_look_file_is_drawn_by_the_window(&second);
}

fn the_window_runs_lit_with_the_atmosphere(stdout: &str) {
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    // The adapter line names what ran, and the run presented frames to the window.
    assert!(stdout.contains("adapter: "), "{stdout}");
    assert!(count_before(stdout, " presented") > 0, "{stdout}");
    // The lit path was the active one: the run itself judged these counts (`smoke_verdict`), and the summary shows them.
    assert!(count_before(stdout, " lit,") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
    assert!(count_before(stdout, " with the atmosphere") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
    assert!(count_before(stdout, " debug)") > 0, "the L key must have drawn the debug path too: {stdout}");
    assert!(count_before(stdout, " exposure reading") > 0, "{stdout}");
    assert!(stdout.contains("view: the lit path with the atmosphere"), "{stdout}");
    // VIEW-014: the smoke script cycles the overlay (tiles, mesh, regions, off) and turns the coarse LOD on while the mesh shows, so some lit frames
    // were drawn with an overlay, some with the mesh, some planned with the coarse LOD (and the shader that has them ran with the atmosphere).
    assert!(count_before(stdout, " with an overlay") > 0, "F3 must have turned an overlay on for some frames: {stdout}");
    assert!(count_before(stdout, " the mesh)") > 0, "the second F3 shows the mesh: {stdout}");
    assert!(count_before(stdout, " coarse LOD,") > 0, "F4 must have planned some frames with the coarse LOD: {stdout}");
    // VIEW-044: the fourth F3 press showed the stale-tile overlay for some lit frames (the shader that tints by state ran with the atmosphere).
    assert!(count_before(stdout, " stale-tile overlay,") > 0, "F3 must have drawn some lit frames with the stale-tile overlay: {stdout}");
    // VIEW-042: H switched the editing light on for a few lit frames, and the renderer drew them (the run's verdict wants them too).
    assert!(count_before(stdout, " editing light)") > 0, "H must have drawn some lit frames with the editing light: {stdout}");
    // VIEW-013: the smoke script shows the globe overview with a marker under the camera, drags and zooms it and clicks the marker: some
    // frames were the overview, it showed the marker, and a click started a flight.
    assert!(count_before(stdout, " frames of the overview") > 0, "G must have shown the globe overview: {stdout}");
    assert!(count_before(stdout, " markers shown") >= 1, "the overview showed the marker: {stdout}");
    assert!(count_before(stdout, " flights started") >= 1, "the click on the marker started a flight: {stdout}");
    // VIEW-011: the editor UI ran a pass for every frame it presented (the window draws it by default).
    assert!(count_before(stdout, " frames with the editor UI") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
    if adapter() != "software" {
        assert!(stdout.contains("DiscreteGpu"), "the real-GPU smoke runs on a discrete GPU: {stdout}");
    }
}

fn the_look_file_is_drawn_by_the_window(stdout: &str) {
    // VIEW-011: this run was given `--no-ui`: no UI pass was run.
    assert!(!stdout.contains("with the editor UI"), "{stdout}");
    assert!(stdout.contains("look: ") && stdout.contains("moonlit.look.toml"), "{stdout}");
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    assert!(count_before(stdout, " with the atmosphere") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
}

// spec: VIEW-008
#[test]
fn a_look_whose_atmosphere_does_not_fit_the_world_gives_the_lit_path_without_the_atmosphere_and_says_why() {
    // A body of 60 km radius under the default look: the atmosphere's top (100 km) is above the surface it should wrap, so the plan
    // builder refuses it. The window falls back to the lit path without the atmosphere and prints the reason; the smoke verdict then
    // wants no atmosphere frames at all and the run succeeds.
    let dir = std::env::temp_dir().join(format!("planet-viewer-fit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let world = dir.join("small.toml");
    std::fs::write(&world, "schema_version = 2\n[body.0]\nradius_m = 60000.0\n[world]\nheight_field = \"cpu-ref-height-v2\"\nseed = 5\n")
        .unwrap();
    let out = smoke(&["--scene", "terrain", "--world", world.to_str().unwrap(), "--global-res", "64"]);
    let _ = std::fs::remove_dir_all(&dir);
    let Some(stdout) = out else { return };
    assert!(stdout.contains("atmosphere off: "), "the run says why the atmosphere is off: {stdout}");
    assert!(stdout.contains("view: the lit path with no atmosphere"), "{stdout}");
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    assert_eq!(count_before(&stdout, " with the atmosphere"), 0, "no frame has the atmosphere: {stdout}");
    assert!(count_before(&stdout, " lit,") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
}

// spec: VIEW-009, ATMO-016
#[test]
fn a_world_with_a_bake_has_an_ocean_and_the_window_draws_its_water_through_the_water_pass() {
    // A world with the default (eroded) field over an in-memory bake has the sea of its definition (the ocean channel is computed with
    // the bake; the viewer's default world is such a world). The flight starts in orbit over the planet: the water pass runs on the
    // lit frames, and the summary's count of frames with water is the plans' own.
    let dir = std::env::temp_dir().join(format!("planet-viewer-sea-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let world = dir.join("earth.toml");
    std::fs::write(&world, "schema_version = 2\n[body.0]\nradius_m = 6371000.0\n[world]\nseed = 5\n").unwrap();
    let out = smoke(&["--scene", "terrain", "--world", world.to_str().unwrap(), "--global-res", "128", "--size", "320x200"]);
    let _ = std::fs::remove_dir_all(&dir);
    let Some(stdout) = out else { return };
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    // How many of the flight's frames see the sea depends on the flight clock (frames per second of the adapter): some do.
    assert!(count_before(&stdout, " with water") > 0, "the lit frames carry water: {stdout}");
    assert!(count_before(&stdout, " with the atmosphere") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
}

// spec: VIEW-022
#[test]
fn the_editor_mode_places_drags_types_undoes_and_deletes_in_the_real_window_and_writes_the_item_file() {
    let dir = std::env::temp_dir().join(format!("planet-editor-smoke-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // A small window: after about 14 s of start-up the run is bound by the software adapter drawing 90 lit frames (0.6 s each at the default
    // 1280x720: 67 s alone, 100 to 220 s beside the other tier-C processes, against the 300 s limit); at 480x270 a frame costs half. The
    // counts asserted below are those of the script, which does not depend on the size.
    // F9 is pressed by the script where a directory for its files is named (VIEW-024).
    let repro = std::env::temp_dir().join(format!("planet-editor-repro-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repro);
    let Some(stdout) =
        smoke_in(&["--edit", "--size", "480x270", "--items", dir.to_str().unwrap()], &[("PLANET_REPRO_DIR", repro.to_str().unwrap())])
    else {
        return;
    };
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    // The script of the run (placing with the stamp tool, dragging the move handle, typing a radius, undo, redo, delete, undo) is judged by
    // the run itself (`EditRuntime::smoke_verdict`); the summary shows the counts.
    let line = stdout.lines().find(|l| l.trim_start().starts_with("edit:")).unwrap_or_else(|| panic!("no edit line in: {stdout}"));
    let count = |after: &str| -> u64 {
        line.split(after)
            .next()
            .and_then(|s| s.rsplit(|c: char| !c.is_ascii_digit()).next())
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("no count before '{after}' in: {line}"))
    };
    assert_eq!(count(" placed"), 1, "{line}");
    assert_eq!(count(" dragged"), 1, "{line}");
    assert_eq!(count(" typed"), 1, "{line}");
    assert_eq!(count(" deleted"), 1, "{line}");
    assert_eq!(count(" undone"), 2, "{line}");
    assert_eq!(count(" redone"), 1, "{line}");
    assert_eq!(count(" refused"), 0, "{line}");
    assert!(count(" frames in the editor mode") > 0 && count(" tiles sent to refresh") > 0, "{line}");
    assert!(count(" definitions handed to the generator") >= 1, "the edits reached the generator: {line}");
    // spec: VIEW-023
    assert!(count(" previews set") >= 1, "the window fed the preview of the edit in flight: {line}");
    // spec: VIEW-024
    let repro_line = stdout.lines().find(|l| l.trim_start().starts_with("repro:")).unwrap_or_else(|| panic!("no repro line in: {stdout}"));
    assert!(repro_line.contains(" 1 F9 write(s)"), "the script pressed F9 once: {repro_line}");
    let names: Vec<String> =
        std::fs::read_dir(&repro).expect("the F9 directory").map(|f| f.unwrap().file_name().to_string_lossy().into_owned()).collect();
    let path_name = names.iter().find(|n| n.ends_with("-path.txt")).unwrap_or_else(|| panic!("no path file in {names:?}"));
    let bundle_name = path_name.replace("-path.txt", ".bundle");
    assert!(names.contains(&bundle_name), "the bundle next to the path: {names:?}");
    let path =
        planet_frame::pathlog::PathLog::from_text(&std::fs::read_to_string(repro.join(path_name)).unwrap()).expect("a valid path file");
    assert!(path.len() >= 2, "the path holds poses: {}", path.summary());
    let bundle = std::fs::read_to_string(repro.join(&bundle_name)).unwrap();
    assert!(bundle.starts_with("planet-terrain-repro "), "{}", &bundle[..bundle.len().min(60)]);
    assert!(bundle.contains("world_toml "), "the bundle records the world, edits included");
    // spec: VIEW-026
    let replay_dir = repro.join("replay");
    let replay = Command::new(env!("CARGO_BIN_EXE_planet"))
        .args(["test-render", "--adapter", &adapter()])
        .arg("--repro")
        .arg(repro.join(&bundle_name))
        .arg("--replay-path")
        .arg(repro.join(path_name))
        .arg("--out-dir")
        .arg(&replay_dir)
        .args(["--replay-every", "1000"])
        .output()
        .expect("the planet binary starts");
    let replayed = String::from_utf8_lossy(&replay.stdout).into_owned();
    assert!(replay.status.success(), "the replay failed: {replayed}{}", String::from_utf8_lossy(&replay.stderr));
    assert!(replayed.contains(&format!("replay path: {} poses", path.len())), "{replayed}");
    let rows = std::fs::read_to_string(replay_dir.join("replay.txt")).expect("the replay index");
    assert_eq!(rows.lines().filter(|l| !l.starts_with('#')).count(), path.len(), "one line per pose: {rows}");
    // A pose the plan refuses is a REFUSED line and is not drawn; the others are, pose 0 among them.
    if !rows.lines().find(|l| !l.starts_with('#')).is_some_and(|l| l.contains(" REFUSED ")) {
        assert!(replay_dir.join("pose_0000.png").is_file(), "pose 0 is rendered");
    }
    let _ = std::fs::remove_dir_all(&repro);
    // The run ends with the edit back (the last command undid the delete): one file, written through by every command.
    let files: Vec<_> =
        std::fs::read_dir(dir.join("0")).expect("the items directory of the planet").map(|f| f.unwrap().file_name()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(files[0].to_string_lossy().ends_with(".toml"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// One synthetic item file (the editor's own format, `schema_version = 1`): a plateau or a crater with a large reach.
fn synthetic_item(n: usize, kind: &str, lat: f64, lon: f64) -> String {
    let (params, max_delta) = if kind == "plateau" { ("target_m = 300.0", 1000.0) } else { ("depth_m = 800.0", 800.0) };
    format!(
        "schema_version = 1
id = \"01M5D8EZ581MFKT87R9R1M{n:04}\"
body = 0
surface = \"sphere\"
layer = \"stamps\"
priority = 0
kind = \"{kind}\"
         reach_m = 15000.0
max_delta_m = {max_delta}
base_hash = \"58100a60bf321ffe\"
generator_version = 2
stage_version = 3
         name = \"{kind} {n}\"

[anchor]
latitude = {lat}
longitude = {lon}
height = 0.0
heading = 0.0

[params]
{params}
"
    )
}

/// The item sweep at one altitude (VIEW-040): ten synthetic plateaus and craters, one stop each, no refused plan. One altitude per test keeps each
/// run (a window opened on the software adapter, ten still frames per stop) well inside the limit of a `gpu_` test; the three tests together
/// are the 30 stops of the sweep.
fn item_sweep_at(alt: &str) {
    let dir = std::env::temp_dir().join(format!("planet-sweep-items-{alt}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("0")).unwrap();
    let report = std::env::temp_dir().join(format!("planet-sweep-report-{alt}-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&report);
    for n in 1..=10usize {
        let kind = if n % 2 == 1 { "plateau" } else { "crater" };
        let (lat, lon) = (-60.0 + 13.0 * n as f64, -170.0 + 33.0 * n as f64);
        std::fs::write(dir.join("0").join(format!("01M5D8EZ581MFKT87R9R1M{n:04}.toml")), synthetic_item(n, kind, lat, lon)).unwrap();
    }
    let Some(stdout) = run_editor(
        &[],
        &["--edit", "--size", "480x270", "--items", dir.to_str().unwrap()],
        &[("PLANET_SWEEP", report.to_str().unwrap()), ("PLANET_SWEEP_ALTS", alt), ("PLANET_SWEEP_HOLD", "4")],
    ) else {
        return;
    };
    assert!(stdout.contains("[sweep ok]: 10 stops, 10 items x 1 altitudes, 0 stops with a refused plan"), "{stdout}");
    let text = std::fs::read_to_string(&report).expect("the sweep wrote its report");
    for n in 1..=10 {
        let kind = if n % 2 == 1 { "plateau" } else { "crater" };
        assert!(text.lines().any(|l| l.contains(&format!(" {kind}_{n} {kind} {alt} 4 "))), "no stop for {kind} {n} at {alt} m: {text}");
    }
}

// spec: VIEW-040
#[test]
fn the_item_sweep_visits_ten_large_plateaus_and_craters_at_2000_m_without_a_refused_plan() {
    item_sweep_at("2000");
}

// spec: VIEW-040
#[test]
fn the_item_sweep_visits_ten_large_plateaus_and_craters_at_200_m_without_a_refused_plan() {
    item_sweep_at("200");
}

// spec: VIEW-040
#[test]
fn the_item_sweep_visits_ten_large_plateaus_and_craters_at_30_m_without_a_refused_plan() {
    item_sweep_at("30");
}

/// A tiny deterministic generator (SplitMix64) so the synthetic strokes are the same on every run.
struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A stroke as (kind, radius m, strength m) and with its points (latitude, longitude in degrees).
type StrokeSpec = (&'static str, f64, f64);
type Stroke = (&'static str, f64, f64, Vec<(f64, f64)>);

/// One brush item of the owner's shape: strokes of 30 points a quarter of the radius apart, each starting within 1.5 km of the centre of
/// the cluster and wandering, `reach_m` the farthest point from the anchor plus the radius plus a metre, `schema_version = 3`.
fn synthetic_brush(n: usize, strokes: &[StrokeSpec], max_delta: f64, centre: (f64, f64), rng: &mut Rng) -> String {
    const M_PER_DEG: f64 = 111_195.0;
    let mut all: Vec<Stroke> = Vec::new();
    for &(kind, radius, strength) in strokes {
        let mut at = (centre.0 + (rng.unit() - 0.5) * 3000.0 / M_PER_DEG, centre.1 + (rng.unit() - 0.5) * 3000.0 / M_PER_DEG);
        let mut heading = rng.unit() * std::f64::consts::TAU;
        let step = radius / 4.0;
        let mut points = Vec::new();
        for _ in 0..30 {
            points.push(at);
            heading += (rng.unit() - 0.5) * 0.6;
            at = (
                at.0 + step * libm::cos(heading) / M_PER_DEG,
                at.1 + step * libm::sin(heading) / (M_PER_DEG * libm::cos(at.0.to_radians())),
            );
        }
        all.push((kind, radius, strength, points));
    }
    let anchor = all[0].3[0];
    let far = all
        .iter()
        .flat_map(|(_, r, _, pts)| {
            pts.iter().map(move |p| {
                let (dy, dx) = ((p.0 - anchor.0) * M_PER_DEG, (p.1 - anchor.1) * M_PER_DEG * libm::cos(anchor.0.to_radians()));
                (dy * dy + dx * dx).sqrt() + r + 1.0
            })
        })
        .fold(1001.0f64, f64::max)
        * 1.01;
    let mut text = format!(
        "schema_version = 3\nid = \"01M5DB1K8WC4NJMJXX835F{n:04}\"\nbody = 0\nsurface = \"sphere\"\nlayer = \"brush\"\npriority = 0\nkind = \"brush\"\n\
         reach_m = {far}\nmax_delta_m = {max_delta}\nbase_hash = \"58100a60bf321ffe\"\ngenerator_version = 2\nstage_version = 3\nname = \"Brush {n}\"\n\n\
         [anchor]\nlatitude = {}\nlongitude = {}\nheight = 0.0\nheading = 0.0\n\n[params]\n",
        anchor.0, anchor.1
    );
    for (kind, radius, strength, points) in &all {
        text += &format!("\n[[strokes]]\nkind = \"{kind}\"\nradius_m = {radius}\nstrength_m = {strength}\n");
        if *kind == "flatten" {
            // The owner's flatten target (an absolute height above the reference sphere, metres).
            text += "target_m = 3106.363650538702\n";
        }
        for p in points {
            text += &format!("\n[[strokes.points]]\nlatitude = {}\nlongitude = {}\n", p.0, p.1);
        }
    }
    text
}

/// A one-point stamp file (`schema_version = 1`) of the owner's format.
#[allow(clippy::too_many_arguments)]
fn stamp_file(id: &str, kind: &str, reach: f64, max_delta: f64, lat: f64, lon: f64, params: &str, name: &str) -> String {
    format!(
        "schema_version = 1\nid = \"{id}\"\nbody = 0\nsurface = \"sphere\"\nlayer = \"stamps\"\npriority = 0\nkind = \"{kind}\"\nreach_m = {reach}\n\
         max_delta_m = {max_delta}\nbase_hash = \"58100a60bf321ffe\"\ngenerator_version = 2\nstage_version = 4\nname = \"{name}\"\n\n[anchor]\n\
         latitude = {lat}\nlongitude = {lon}\nheight = 0.0\nheading = 0.0\n\n[params]\n{params}\n"
    )
}

// spec: VIEW-040
// The owner's cluster of brushes 10 to 23 (many overlapping raise, flatten and smooth strokes at about 27.9 S 12.5 W), his ridge 3, hill 7
// and crater 6 in their sizes, swept at 200 m and 30 m: the sweep of 2026-10-21 refused plans at these stops (SPIKE-26, "morph interval is
// empty" at levels 16 to 22). It failed then and was ignored; with ADR 0074's capping (#505) it passes (181 s on the RTX, 2026-10-21).
#[test]
fn the_item_sweep_over_a_dense_brush_cluster_a_ridge_a_hill_and_a_crater_has_no_refused_plan() {
    // 34 stops of 40 frames: 181 s on the RTX, about seven minutes on the software adapter, over the tier-C job's limit. It runs in
    // `cargo xtask test gpu --real`; the software run of the same sweep is the ten-item test above.
    if adapter() == "software" {
        eprintln!("SKIPPED on the software adapter (too slow); it runs with PLANET_ADAPTER=hardware (cargo xtask test gpu --real)");
        return;
    }
    let dir = std::env::temp_dir().join(format!("planet-sweep-cluster-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("0")).unwrap();
    let report = std::env::temp_dir().join(format!("planet-sweep-cluster-report-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&report);
    let (centre, mut rng) = ((-27.906, -12.49), Rng(0x5EED));
    // (kind, radius m, strength m) per stroke and the item's bound, as the owner's brushes 10 to 23.
    let raise = |n: usize| vec![("raise", 100.0, 5.0); n];
    let flat_smooth = |flats: usize, smooths: usize| {
        let mut v = vec![("flatten", 300.0, 5.0); flats];
        v.extend(vec![("smooth", 300.0, 5.0); smooths]);
        v
    };
    let spec: Vec<(Vec<StrokeSpec>, f64)> = vec![
        (raise(3), 15.0),
        (raise(1), 5.0),
        (raise(5), 25.0),
        (flat_smooth(4, 1), 25.0),
        (flat_smooth(1, 10), 100.0),
        (flat_smooth(3, 0), 15.0),
        (flat_smooth(1, 2), 15.0),
        (flat_smooth(2, 0), 10.0),
        (flat_smooth(0, 1), 5.0),
        (flat_smooth(0, 4), 20.0),
        (vec![("smooth", 300.0, 50.0); 2], 100.0),
        (vec![("smooth", 1500.0, 50.0); 2], 100.0),
        (vec![("smooth", 1500.0, 50.0); 2], 100.0),
        (vec![("smooth", 1500.0, 50.0)], 50.0),
    ];
    for (i, (strokes, max_delta)) in spec.iter().enumerate() {
        let n = 10 + i;
        let file = dir.join("0").join(format!("01M5DB1K8WC4NJMJXX835F{n:04}.toml"));
        std::fs::write(file, synthetic_brush(n, strokes, *max_delta, centre, &mut rng)).unwrap();
    }
    // The owner's ridge 3 (30 m high, 200 m wide, three points), hill 7 (400 m, reach 1.5 km) and crater 6 (800 m deep, reach 465 m).
    let ridge = "schema_version = 2\nid = \"01M5DAMJQ0C4WKS3C9WM5M0003\"\nbody = 0\nsurface = \"sphere\"\nlayer = \"stamps\"\npriority = 0\nkind = \"ridge\"\n\
        reach_m = 1500.1857704052918\nmax_delta_m = 60.0\nbase_hash = \"58100a60bf321ffe\"\ngenerator_version = 2\nstage_version = 4\nname = \"Ridge 3\"\n\n\
        [anchor]\nlatitude = 7.29698279195186\nlongitude = -18.07508354174058\nheight = 0.0\nheading = 0.0\n\n[params]\namount_m = 30.0\nwidth_m = 200.0\n\n\
        [[points]]\nlatitude = 7.29698279195186\nlongitude = -18.07508354174058\n\n[[points]]\nlatitude = 7.291602889965999\nlongitude = -18.06950703817636\n\n\
        [[points]]\nlatitude = 7.301005745267928\nlongitude = -18.064024496784953\n";
    std::fs::write(dir.join("0").join("01M5DAMJQ0C4WKS3C9WM5M0003.toml"), ridge).unwrap();
    let hill = stamp_file(
        "01M5DANN8P2VS70C58QJHMBF07",
        "hill",
        1521.5549097287067,
        400.0,
        7.233915851986685,
        -18.10671838076562,
        "height_m = 400.0",
        "Hill 7",
    );
    std::fs::write(dir.join("0").join("01M5DANN8P2VS70C58QJHMBF07.toml"), hill).unwrap();
    let crater = stamp_file(
        "01M5DD1QQTS23K7QWHSSMZAM06",
        "crater",
        464.6188315875319,
        800.0,
        23.771852832388916,
        -13.864149021381316,
        "depth_m = 800.0",
        "Crater 6",
    );
    std::fs::write(dir.join("0").join("01M5DD1QQTS23K7QWHSSMZAM06.toml"), crater).unwrap();
    let Some(stdout) = run_editor(
        &[],
        &["--edit", "--size", "480x270", "--items", dir.to_str().unwrap()],
        &[("PLANET_SWEEP", report.to_str().unwrap()), ("PLANET_SWEEP_ALTS", "200,30"), ("PLANET_SWEEP_HOLD", "40")],
    ) else {
        return;
    };
    assert!(stdout.contains("[sweep ok]: 34 stops, 17 items x 2 altitudes, 0 stops with a refused plan"), "{stdout}");
}

/// Run `planet generate <args>` and return its stdout; a failure panics with both streams.
fn generate(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_planet")).arg("generate").args(args).output().expect("the planet binary starts");
    assert!(
        out.status.success(),
        "planet generate {args:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A store of 32 cells with its coupled run and coarse horizon, and the world file the window needs to take that bake size (the default world,
/// written out): the chunk stage of the window is removed, so the world is the base. Returns the store, the world file and the arguments that name them.
fn stage_store(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, Vec<String>) {
    let store = std::env::temp_dir().join(format!("planet-window-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    let world = std::env::temp_dir().join(format!("planet-window-{tag}-{}.toml", std::process::id()));
    let def = planet_world_def::WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 0);
    std::fs::write(&world, planet_world_def::toml_io::to_toml(&def).unwrap()).unwrap();
    let (dir, w) = (store.to_str().unwrap().to_string(), world.to_str().unwrap().to_string());
    generate(&["--global-dir", &dir, "--world", &w, "--global-res", "32", "--threads", "2"]);
    generate(&["--coupled-run", &dir, "--world", &w, "--global-res", "32"]);
    generate(&["--coarse-horizon", &dir, "--world", &w, "--global-res", "32", "--threads", "2"]);
    let args = ["--store", &dir, "--world", &w, "--global-res", "32", "--tau", "0.4"].map(String::from).to_vec();
    (store, world, args)
}

// spec: TEST-061, TEST-062, TEST-064
#[test]
fn the_gate_passes_on_a_small_world_without_the_chunk_stage_which_is_the_default() {
    // The default since the owner's fallback of 2026-10-28: no chunk stage, so no chunk job is computed.
    let (store, world, args) = stage_store("nochunks");
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    args.extend(["--edit", "--no-vsync", "--size", "480x270"]);
    let out = smoke_in(&args, &[("PLANET_GATE", "1"), ("PLANET_GATE_SETTLE", "10"), ("PLANET_GATE_SOFTWARE", "1")]);
    let _ = std::fs::remove_file(&world);
    let _ = std::fs::remove_dir_all(&store);
    let Some(stdout) = out else { return };
    let line = stdout.lines().find(|l| l.starts_with("gate: ")).unwrap_or_else(|| panic!("no gate line: {stdout}"));
    assert!(line.contains("horizon jobs: Some(") && !line.contains("chunk jobs"), "the stage was on: {line}");
}
