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
    let Some(stdout) = smoke(&[]) else { return };
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    // The adapter line names what ran, and the run presented frames to the window.
    assert!(stdout.contains("adapter: "), "{stdout}");
    assert!(count_before(&stdout, " presented") > 0, "{stdout}");
    // The lit path was the active one: the run itself judged these counts (`smoke_verdict`), and the summary shows them.
    assert!(count_before(&stdout, " lit,") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
    assert!(count_before(&stdout, " with the atmosphere") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
    assert!(count_before(&stdout, " debug)") > 0, "the L key must have drawn the debug path too: {stdout}");
    assert!(count_before(&stdout, " exposure reading") > 0, "{stdout}");
    assert!(stdout.contains("view: the lit path with the atmosphere"), "{stdout}");
    if adapter() != "software" {
        assert!(stdout.contains("DiscreteGpu"), "the real-GPU smoke runs on a discrete GPU: {stdout}");
    }
    the_look_file_is_drawn_by_the_window();
}

// The look run follows the first in the same test: two windows on the software adapter at once would starve each other.
fn the_look_file_is_drawn_by_the_window() {
    let dir = std::env::temp_dir().join(format!("planet-viewer-look-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let look = dir.join("moonlit.look.toml");
    // A sun from +z, a dimmer sun and a narrower exposure range: all of it reaches the lit plan (tier A pins the plan; this runs it).
    std::fs::write(
        &look,
        "schema_version = 3\nkind = \"look\"\n[body.0.sun]\ndirection = [0.0, 0.0, 1.0]\n[body.0.units]\nsolar_illuminance_lux = 90000.0\n\
         [body.0.exposure]\nmin_ev100 = 4.0\nmax_ev100 = 16.0\n",
    )
    .unwrap();
    let Some(stdout) = smoke(&["--look", look.to_str().unwrap()]) else { return };
    assert!(stdout.contains("look: ") && stdout.contains("moonlit.look.toml"), "{stdout}");
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    assert!(count_before(&stdout, " with the atmosphere") >= planet_frame::viewer::SMOKE_MIN_LIT_FRAMES, "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}
