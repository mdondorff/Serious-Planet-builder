//! GEN-017, tier B: `generate --plan-check` plans the standard flight over a stored bake with both fields. Bakes a 256-cell
//! store and plans twenty views (about 4 s), so it lives outside the fast tier; the production-resolution run is recorded at
//! M3 acceptance (docs/milestones/M3-report.md).

use std::path::PathBuf;

fn run_str(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("planet-plan-check-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    fn s(&self, leaf: &str) -> String {
        self.0.join(leaf).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// spec: GEN-017
#[test]
fn plan_check_plans_the_standard_flight_over_a_stored_bake_with_both_fields() {
    let d = TempDir::new("flight");
    let store = d.s("store");
    run_str(&["generate", "--global-dir", &store, "--global-res", "256", "--seed", "3", "--threads", "2", "--prefill-levels", "0"])
        .unwrap();
    let ok = run_str(&["generate", "--plan-check", &store, "--global-res", "256", "--seed", "3"]).unwrap();
    for needle in ["terrain ground-1m:", "erosion ground-1m:", "erosion orbit-20000km:", "both fields plan every view"] {
        assert!(ok.contains(needle), "missing {needle}: {ok}");
    }
    assert!(!ok.contains("REJECTED"), "{ok}");
    // A bake of another resolution is not found under the keys of this one: an error naming the problem, not a default.
    let wrong = run_str(&["generate", "--plan-check", &store, "--global-res", "512", "--seed", "3"]).unwrap_err();
    assert!(wrong.contains("rejected or unreadable"), "{wrong}");
    // The refusals.
    assert!(run_str(&["generate", "--plan-check", &d.s("nowhere"), "--global-res", "256"])
        .unwrap_err()
        .contains("no such store directory"));
    let lone = run_str(&["generate", "--plan-check", &store]).unwrap_err();
    assert!(lone.contains("needs --global-res"), "{lone}");
    let clash = run_str(&["generate", "--plan-check", &store, "--global-res", "256", "--threads", "2"]).unwrap_err();
    assert!(clash.contains("takes only --global-res"), "{clash}");
    let e = run_str(&["test-render", "--plan-check", &store]).unwrap_err();
    assert!(e.contains("only applies to `generate`"), "{e}");
}
