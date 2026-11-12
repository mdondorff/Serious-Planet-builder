//! `cargo xtask k7-layout` (BUILD-052): the layout report of K7. Runs the `k7_layout` example of `planet-cache` (the fixture's tile layout against an alternative), prints its `LAYOUT` lines and compares the p99 and the
//! maximum of the all-cell and line-cell rows with `docs/gates/k7-layout-baseline.txt`. A figure above twice the baseline prints `REVIEW`. The exit status is 0 either way: the report informs the change's review.
//! `--write-baseline` writes the current lines as the baseline. Prepush runs it when the diff touches K7.

use crate::util::{capture, Res};
use std::path::Path;

pub const BASELINE: &str = "docs/gates/k7-layout-baseline.txt";

/// Whether a changed path is one of the K7 files whose change the report covers.
pub fn touches_k7(path: &str) -> bool {
    let p = path.replace(char::from(92), "/");
    p.starts_with("crates/generators/src/bake/k7")
        || ["crates/generators/src/bake/carve.rs", "crates/generators/src/bake/round.rs", "crates/cache/src/region_bake.rs"]
            .contains(&p.as_str())
}

/// `(p99, max)` in metres of the report line of `set` (`all cells`, `line cells`) with the band `whole`.
pub fn figures(text: &str, set: &str) -> Option<(f64, f64)> {
    let line =
        text.lines().find(|l| l.starts_with("LAYOUT |") && l.split(" | ").nth(1) == Some(set) && l.split(" | ").nth(2) == Some("whole"))?;
    let after = |key: &str| -> Option<f64> { line.split(key).nth(1)?.split_whitespace().next()?.parse().ok() };
    Some((after("p99 ")?, after("max ")?))
}

/// The comparison lines of a run against the baseline: one per figure, `REVIEW` where the figure is above twice the baseline's.
pub fn compare(baseline: &str, current: &str) -> Vec<String> {
    let mut out = Vec::new();
    for set in ["all cells", "line cells"] {
        let (Some(b), Some(c)) = (figures(baseline, set), figures(current, set)) else {
            out.push(format!("k7-layout: REVIEW the {set} row is missing from the baseline or the run"));
            continue;
        };
        for (name, bv, cv) in [("p99", b.0, c.0), ("max", b.1, c.1)] {
            let verdict = if cv > 2.0 * bv { "REVIEW" } else { "ok" };
            out.push(format!("k7-layout: {verdict} {set} {name} {cv:.4} m (baseline {bv:.4} m)"));
        }
    }
    out
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let write = args.iter().any(|a| a == "--write-baseline");
    if let Some(bad) = args.iter().find(|a| a.as_str() != "--write-baseline") {
        return Err(format!("unknown k7-layout option '{bad}' (known: --write-baseline)"));
    }
    let current = capture(root, "cargo", &["run", "-q", "-p", "planet-cache", "--example", "k7_layout"])?;
    print!("{current}");
    let path = root.join(BASELINE);
    if write {
        std::fs::write(&path, &current).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("k7-layout: baseline written to {BASELINE}");
        return Ok(());
    }
    match std::fs::read_to_string(&path) {
        Ok(baseline) => compare(&baseline, &current).iter().for_each(|l| eprintln!("{l}")),
        Err(_) => eprintln!("k7-layout: no baseline at {BASELINE} (run with --write-baseline)"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: &str = "LAYOUTS | canonical tile 64 halo 24 (9 land tiles) | alternative tile 32 halo 12 (36 land tiles)\n\
LAYOUT | all cells | whole | cells 100 | differ in bits 5 (5.00 %) | over 0.25 m 0 | over 1 m 0 | p50 0.0000 p99 0.0010 max 0.0300 m\n\
LAYOUT | line cells | whole | cells 10 | differ in bits 0 (0.00 %) | over 0.25 m 0 | over 1 m 0 | p50 0.0000 p99 0.0000 max 0.0000 m\n";

    // spec: BUILD-052
    #[test]
    fn the_figures_are_read_from_the_whole_rows() {
        assert_eq!(figures(ROWS, "all cells"), Some((0.001, 0.03)));
        assert_eq!(figures(ROWS, "line cells"), Some((0.0, 0.0)));
        assert_eq!(figures(ROWS, "other"), None);
    }

    // spec: BUILD-052
    #[test]
    fn a_figure_above_twice_the_baseline_is_marked_review_and_the_rest_ok() {
        let worse = ROWS.replace("max 0.0300", "max 0.0700");
        let lines = compare(ROWS, &worse);
        assert!(lines.iter().any(|l| l.contains("REVIEW all cells max")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("ok all cells p99")), "{lines:?}");
        assert!(compare(ROWS, ROWS).iter().all(|l| l.contains(": ok ")));
    }

    // spec: BUILD-052
    #[test]
    fn the_k7_files_are_recognised_by_path() {
        for p in [
            "crates/generators/src/bake/k7.rs",
            "crates/generators/src/bake/carve.rs",
            "crates/cache/src/region_bake.rs",
            r"crates\generators\src\bake\round.rs",
        ] {
            assert!(touches_k7(p), "{p}");
        }
        assert!(!touches_k7("crates/render/src/terrain.rs"));
    }
}
