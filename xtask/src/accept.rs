//! `cargo xtask accept <milestone>`: runs the automated part of milestone acceptance and writes
//! `target/review/accept-<milestone>.md`. Criteria that need CI, the owner or Rasierklinge are listed
//! as such, never as passed.

use crate::util::{run as exec, Res};
use std::path::Path;
use std::time::Instant;

struct Step {
    name: &'static str,
    outcome: Result<f64, String>,
}

fn step(name: &'static str, f: impl FnOnce() -> Res) -> Step {
    let t = Instant::now();
    let outcome = f().map(|_| t.elapsed().as_secs_f64());
    Step { name, outcome }
}

/// Criteria no local run can settle, per milestone (report §15).
fn manual_criteria(milestone: &str) -> &'static [&'static str] {
    match milestone {
        "M0" => &[
            "CI green on Windows (WARP) and Linux (lavapipe): needs a pushed branch (see docs/status.md)",
            "Candidate goldens blessed by the owner (/bless)",
            "Performance harness run on Rasierklinge: needs the owner's go (charger, power profile)",
        ],
        "M1" => &[
            "Tile hashes identical on Windows and Linux: needs the CI hash comparison job",
            "Candidate goldens blessed by the owner (/bless)",
        ],
        "M2" => &[
            "Orbit to 1 m altitude without jitter: the f32 shader-path emulation passes (REND-004); a visual flight on the real GPU needs the owner",
            "Terrain at most 8 ms at 1440p on the mid-range tier: needs a performance run on Rasierklinge (the owner confirms the machine first)",
            "No cracks in 10 scripted views: checked on the software adapters (LOD-005); a real-GPU run needs the owner (`cargo xtask test gpu --real`)",
            "Six terrain goldens blessed by the owner (/bless), and the Linux goldens",
            "GO/NO-GO gate: the owner decides after reading docs/milestones/M2-report.md",
        ],
        _ => &["No automated criteria are defined for this milestone yet"],
    }
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let Some(milestone) = args.first() else {
        return Err("usage: cargo xtask accept <milestone>, e.g. M0".into());
    };
    let m = milestone.to_uppercase();
    let steps = vec![
        step("check (fmt, clippy, layering)", || crate::testcmd::check(root)),
        step("spec-lint", || crate::speclint::run(root)),
        step("test (tiers A+B, software adapter)", || crate::testcmd::test(root, &[])),
        step("test gpu (tier C, software adapter)", || crate::testcmd::test(root, &["gpu".to_string()])),
        step("run mode editor --smoke", || exec(root, "cargo", &["run", "-q", "-p", "planet", "--", "editor", "--smoke"], &[])),
        step("run mode generate --smoke", || exec(root, "cargo", &["run", "-q", "-p", "planet", "--", "generate", "--smoke"], &[])),
        step("run mode test-render --smoke", || exec(root, "cargo", &["run", "-q", "-p", "planet", "--", "test-render", "--smoke"], &[])),
        step("test fast (timing)", || {
            let t = Instant::now();
            crate::testcmd::test(root, &["fast".to_string()])?;
            let secs = t.elapsed().as_secs_f64();
            if secs > 10.0 {
                return Err(format!("test fast took {secs:.1}s, budget is 10 s"));
            }
            Ok(())
        }),
    ];

    let mut md = format!("# Acceptance run {m}\n\n| Step | Result | Seconds |\n|---|---|---|\n");
    let mut failed = false;
    for s in &steps {
        match &s.outcome {
            Ok(secs) => md += &format!("| {} | pass | {secs:.1} |\n", s.name),
            Err(e) => {
                failed = true;
                md += &format!("| {} | FAIL: {} | |\n", s.name, e.lines().next().unwrap_or(""));
            }
        }
    }
    md += "\nNeeds CI, the owner or Rasierklinge (not passed by this command):\n\n";
    for c in manual_criteria(&m) {
        md += &format!("- {c}\n");
    }
    let out = root.join("target").join("review").join(format!("accept-{m}.md"));
    std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&out, &md).map_err(|e| e.to_string())?;
    eprintln!("\n{md}\nwrote {}", out.display());
    if failed {
        Err(format!("acceptance {m}: at least one automated step failed (see above)"))
    } else {
        Ok(())
    }
}
