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
    let adapter = adapter();
    let out = Command::new(env!("CARGO_BIN_EXE_planet"))
        .args(["editor", "--smoke"])
        .args(extra)
        .env("PLANET_ADAPTER", &adapter)
        .output()
        .expect("the planet binary starts");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
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
        panic!("planet editor --smoke {extra:?} failed ({}):\n{stdout}\n{stderr}", out.status);
    }
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
        let second = s.spawn(|| smoke(&["--look", look.to_str().unwrap()]));
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
    if adapter() != "software" {
        assert!(stdout.contains("DiscreteGpu"), "the real-GPU smoke runs on a discrete GPU: {stdout}");
    }
}

fn the_look_file_is_drawn_by_the_window(stdout: &str) {
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

// spec: VIEW-003, TEST-008
#[test]
fn the_f9_key_writes_a_repro_bundle_of_the_frame_that_names_its_world_and_its_lit_plan() {
    let dir = std::env::temp_dir().join(format!("planet-viewer-capture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let world = dir.join("earth.toml");
    std::fs::write(&world, "schema_version = 2\n[body.0]\nradius_m = 6371000.0\n[world]\nseed = 5\n").unwrap();
    let out = smoke(&["--scene", "terrain", "--world", world.to_str().unwrap(), "--global-res", "64", "--size", "320x200"]);
    let _ = std::fs::remove_dir_all(&dir);
    let Some(stdout) = out else { return };
    assert!(stdout.contains("1 bundle(s) written"), "the smoke run pressed F9 once: {stdout}");
    let path =
        stdout.split("last bundle: ").nth(1).and_then(|t| t.lines().next()).expect("the summary names the bundle").trim().to_string();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let bundle = planet_frame::TerrainBundle::from_text(&text).expect("the bundle reads back (its plan hash holds)");
    assert!(
        bundle.frame.is_some() && bundle.world.is_some(),
        "a frame bundle of a named world: {}",
        text.lines().take(12).collect::<Vec<_>>().join("\n")
    );
    assert!(bundle.plan.lit.is_some(), "the viewer was on the lit path");
    let _ = std::fs::remove_file(&path);
}
