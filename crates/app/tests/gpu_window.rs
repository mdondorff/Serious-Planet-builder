//! Tier C: the viewer window path end to end (VIEW-005). The real binary opens the window, runs the pipelined flight, presses the
//! keys through the command path and closes it, on the adapter the run selects: the software adapter in `cargo xtask test gpu`
//! (test mode), the discrete GPU in `cargo xtask test gpu --real` (Rasierklinge).
//!
//! Where there is no display (a headless CI runner) the window cannot open. That is a skip with a reason, and only when it is
//! declared: the runner sets `PLANET_NO_DISPLAY` to the reason. Without the declaration a missing display fails the test, so a
//! machine that should have one never passes silently.

use std::process::Command;

/// The marker the binary's error carries when there is no display (`planet::window::NO_DISPLAY`).
const NO_DISPLAY: &str = planet::window::NO_DISPLAY;

// spec: VIEW-005
#[test]
fn the_window_path_opens_runs_the_flight_and_closes() {
    let adapter = std::env::var("PLANET_ADAPTER").unwrap_or_else(|_| "software".into());
    let out = Command::new(env!("CARGO_BIN_EXE_planet"))
        .args(["editor", "--smoke"])
        .env("PLANET_ADAPTER", &adapter)
        .output()
        .expect("the planet binary starts");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        if stderr.contains(NO_DISPLAY) {
            let reason = stderr.lines().find(|l| l.contains(NO_DISPLAY)).unwrap_or("").trim().to_string();
            match std::env::var("PLANET_NO_DISPLAY") {
                Ok(declared) if !declared.is_empty() => {
                    eprintln!("SKIPPED the viewer window smoke (VIEW-005): {reason}; this runner declares PLANET_NO_DISPLAY='{declared}'");
                    return;
                }
                _ => panic!(
                    "the viewer window cannot open here ({reason}). On a runner without a display declare the skip with \
                     PLANET_NO_DISPLAY=<reason>; on a machine that has one this is a failure."
                ),
            }
        }
        panic!("planet editor --smoke failed ({}):\n{stdout}\n{stderr}", out.status);
    }
    assert!(stdout.contains("[smoke ok]"), "{stdout}");
    // The adapter line names what ran, and the run presented frames to the window.
    assert!(stdout.contains("adapter: "), "{stdout}");
    let presented: u64 = stdout
        .split(" presented")
        .next()
        .and_then(|s| s.rsplit(", ").next())
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| panic!("no presented count in: {stdout}"));
    assert!(presented > 0, "{stdout}");
    if adapter == "hardware" {
        assert!(stdout.contains("DiscreteGpu"), "the real-GPU smoke runs on a discrete GPU: {stdout}");
    }
}
