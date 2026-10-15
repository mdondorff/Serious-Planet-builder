//! The orchestration of `perf stream all` (TEST-019): one child process per scene, so each scene's RAM figure is its own peak
//! working set and each scene starts cold. Pure over a [`ChildRunner`], so the order, the pass-through, the failure rule and the
//! summary are tested on fakes; the real runner (a process spawn) lives in `xtask`.

use crate::streaming::RAM_BUDGET_BYTES;

/// The label of the RAM figure everywhere it is printed.
pub const RAM_LABEL: &str = "scene's own peak working set (child process)";

/// What a child scene reports about memory, as one machine-readable line at the end of its run (`SCENE-MEMORY ...`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneMemory {
    pub scene: String,
    /// Peak working set of the child process over its life; `None` where it could not be read.
    pub ram_peak_bytes: Option<u64>,
    /// Driver-reported VRAM in use on the whole GPU after the case (an upper bound for the scene); `None` when not measured.
    pub vram_used_bytes: Option<u64>,
    /// Slots of the terrain pool the renderer was built with (its ceiling) and the most slots resident in any frame.
    pub pool_slots: u64,
    pub max_resident: u64,
    /// Slots the tier's terrain share holds at the pool's slot size.
    pub share_slots: u64,
}

const TAG: &str = "SCENE-MEMORY";

fn opt(v: Option<u64>) -> String {
    v.map_or("na".to_string(), |v| v.to_string())
}

impl SceneMemory {
    pub fn line(&self) -> String {
        format!(
            "{TAG} scene={} ram_peak_bytes={} vram_used_bytes={} pool_slots={} max_resident={} share_slots={}",
            self.scene,
            opt(self.ram_peak_bytes),
            opt(self.vram_used_bytes),
            self.pool_slots,
            self.max_resident,
            self.share_slots
        )
    }

    /// Parse a `SCENE-MEMORY` line; any other line, or a malformed one, is `None`.
    pub fn parse(line: &str) -> Option<SceneMemory> {
        let rest = line.trim().strip_prefix(TAG)?;
        let field = |key: &str| -> Option<&str> { rest.split_whitespace().find_map(|w| w.strip_prefix(key)?.strip_prefix('=')) };
        let num = |key: &str| -> Option<u64> { field(key)?.parse().ok() };
        let maybe = |key: &str| -> Option<Option<u64>> {
            let v = field(key)?;
            if v == "na" {
                Some(None)
            } else {
                v.parse().ok().map(Some)
            }
        };
        Some(SceneMemory {
            scene: field("scene")?.to_string(),
            ram_peak_bytes: maybe("ram_peak_bytes")?,
            vram_used_bytes: maybe("vram_used_bytes")?,
            pool_slots: num("pool_slots")?,
            max_resident: num("max_resident")?,
            share_slots: num("share_slots")?,
        })
    }
}

/// How a child ended and what it printed.
#[derive(Clone, Debug, Default)]
pub struct ChildResult {
    /// The exit code; `None` when the process was killed by a signal.
    pub code: Option<i32>,
    /// Everything the child printed on stdout.
    pub output: String,
}

/// Runs one child with the given arguments (the real one spawns `cargo`; tests use a fake).
pub trait ChildRunner {
    fn run(&mut self, args: &[String]) -> Result<ChildResult, String>;
}

/// One scene's outcome.
#[derive(Clone, Debug)]
pub struct SceneOutcome {
    pub scene: String,
    pub code: Option<i32>,
    /// A failure to start the child at all.
    pub start_error: Option<String>,
    pub memory: Option<SceneMemory>,
    /// Verdict lines of the child that end in `OVER` (gated measures over budget; they do not fail the child).
    pub over: usize,
}

impl SceneOutcome {
    pub fn success(&self) -> bool {
        self.start_error.is_none() && self.code == Some(0)
    }
}

/// Run every scene in order in its own child; a failing child does not stop the rest. `args_of` builds one child's arguments.
pub fn run_scenes(
    runner: &mut dyn ChildRunner,
    scenes: &[&str],
    args_of: &dyn Fn(&str) -> Result<Vec<String>, String>,
) -> Result<Vec<SceneOutcome>, String> {
    let mut outcomes = Vec::new();
    for scene in scenes {
        let args = args_of(scene)?;
        match runner.run(&args) {
            Ok(r) => outcomes.push(SceneOutcome {
                scene: (*scene).to_string(),
                code: r.code,
                start_error: None,
                memory: r.output.lines().filter_map(SceneMemory::parse).find(|m| m.scene == *scene),
                over: r.output.lines().filter(|l| l.trim_end().ends_with(" OVER")).count(),
            }),
            Err(e) => outcomes.push(SceneOutcome { scene: (*scene).to_string(), code: None, start_error: Some(e), memory: None, over: 0 }),
        }
    }
    Ok(outcomes)
}

fn gib(b: u64) -> f64 {
    b as f64 / (1u64 << 30) as f64
}

/// The combined summary of a session: one line per scene (status, RAM against the 8 GiB budget, whole-GPU VRAM beside the slots
/// against the share's), and the failure count. The bool is whether every child succeeded.
pub fn summary(outcomes: &[SceneOutcome]) -> (String, bool) {
    let mut s = format!("perf stream: {} scene(s), one child process per scene\n", outcomes.len());
    for o in outcomes {
        let status = match (&o.start_error, o.code) {
            (Some(e), _) => format!("FAILED to start ({e})"),
            (None, Some(0)) => "ok".to_string(),
            (None, Some(c)) => format!("FAILED (exit {c})"),
            (None, None) => "FAILED (killed)".to_string(),
        };
        s.push_str(&format!("  {:<13} {status}", o.scene));
        match &o.memory {
            Some(m) => {
                let ram = match m.ram_peak_bytes {
                    Some(b) => format!(
                        "{:.2} GiB {} (budget {:.0} GiB, CON-21)",
                        gib(b),
                        if b <= RAM_BUDGET_BYTES { "ok" } else { "OVER" },
                        gib(RAM_BUDGET_BYTES)
                    ),
                    None => "not measured".to_string(),
                };
                let vram = m.vram_used_bytes.map_or("not measured".to_string(), |b| format!("{:.2} GiB, whole GPU, upper bound", gib(b)));
                s.push_str(&format!(
                    "; RAM {ram} [{RAM_LABEL}]; VRAM {vram}; terrain slots {} of {} share slots (max {} resident)",
                    m.pool_slots, m.share_slots, m.max_resident
                ));
            }
            None => s.push_str("; no memory line from the child"),
        }
        if o.over > 0 {
            s.push_str(&format!("; {} gated measure(s) OVER", o.over));
        }
        s.push('\n');
    }
    let failed: Vec<&str> = outcomes.iter().filter(|o| !o.success()).map(|o| o.scene.as_str()).collect();
    if failed.is_empty() {
        s.push_str("all scenes ran to the end\n");
    } else {
        s.push_str(&format!("{} of {} scene(s) failed: {}\n", failed.len(), outcomes.len(), failed.join(", ")));
    }
    (s, failed.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::SCENES;

    struct Fake {
        calls: Vec<Vec<String>>,
        fail: Vec<&'static str>,
    }

    impl ChildRunner for Fake {
        fn run(&mut self, args: &[String]) -> Result<ChildResult, String> {
            self.calls.push(args.to_vec());
            let scene = args[args.iter().position(|a| a == "--scene").unwrap() + 1].clone();
            let failing = self.fail.contains(&scene.as_str());
            let mem = SceneMemory {
                scene: scene.clone(),
                ram_peak_bytes: Some((2 << 30) + self.calls.len() as u64),
                vram_used_bytes: if scene == "look-2km" { None } else { Some(3 << 30) },
                pool_slots: 230_640,
                max_resident: 1000 * self.calls.len() as u64,
                share_slots: 238_000,
            };
            Ok(ChildResult {
                code: Some(if failing { 1 } else { 0 }),
                output: format!("noise\n  {scene}: frame time: x OVER\n{}\nmore\n", mem.line()),
            })
        }
    }

    fn args_of(scene: &str) -> Result<Vec<String>, String> {
        Ok(["--scene", scene, "--store", "D:/bake", "--machine-ready"].iter().map(|s| s.to_string()).collect())
    }

    // spec: TEST-019
    #[test]
    fn one_child_per_scene_in_order_with_the_arguments_passed_through() {
        let mut f = Fake { calls: vec![], fail: vec![] };
        let out = run_scenes(&mut f, &SCENES, &args_of).unwrap();
        assert_eq!(f.calls.len(), 6);
        for (call, scene) in f.calls.iter().zip(SCENES) {
            assert_eq!(call, &args_of(scene).unwrap());
        }
        assert!(out.iter().all(SceneOutcome::success));
        let (text, ok) = summary(&out);
        assert!(ok && text.contains("all scenes ran to the end"), "{text}");
    }

    // spec: TEST-019
    #[test]
    fn a_failing_child_does_not_stop_the_others_and_fails_the_combined_result() {
        let mut f = Fake { calls: vec![], fail: vec!["teleport-1m", "look-1m"] };
        let out = run_scenes(&mut f, &SCENES, &args_of).unwrap();
        assert_eq!(f.calls.len(), 6, "every scene runs");
        assert_eq!(out.iter().filter(|o| !o.success()).map(|o| o.scene.as_str()).collect::<Vec<_>>(), ["teleport-1m", "look-1m"]);
        let (text, ok) = summary(&out);
        assert!(!ok);
        assert!(text.contains("2 of 6 scene(s) failed: teleport-1m, look-1m") && text.contains("FAILED (exit 1)"), "{text}");
    }

    // spec: TEST-019
    #[test]
    fn a_child_that_cannot_start_is_a_failure_and_the_rest_still_run() {
        struct Broken(usize);
        impl ChildRunner for Broken {
            fn run(&mut self, _: &[String]) -> Result<ChildResult, String> {
                self.0 += 1;
                if self.0 == 1 {
                    Err("no cargo".into())
                } else {
                    Ok(ChildResult { code: Some(0), output: String::new() })
                }
            }
        }
        let mut b = Broken(0);
        let out = run_scenes(&mut b, &SCENES, &args_of).unwrap();
        assert_eq!(b.0, 6);
        let (text, ok) = summary(&out);
        assert!(!ok && text.contains("FAILED to start (no cargo)") && text.contains("no memory line from the child"), "{text}");
    }

    // spec: TEST-019
    #[test]
    fn the_summary_lists_all_six_scenes_with_each_childs_own_ram_and_the_slots_against_the_share() {
        let mut f = Fake { calls: vec![], fail: vec![] };
        let out = run_scenes(&mut f, &SCENES, &args_of).unwrap();
        let (text, _) = summary(&out);
        for scene in SCENES {
            assert!(text.lines().any(|l| l.trim_start().starts_with(scene)), "{scene} missing in\n{text}");
        }
        assert!(text.contains(RAM_LABEL) && text.contains("budget 8 GiB, CON-21"), "{text}");
        assert!(text.contains("terrain slots 230640 of 238000 share slots (max 1000 resident)"), "{text}");
        assert!(text.contains("(max 6000 resident)"), "each scene carries its own child's figures: {text}");
        assert!(text.contains("3.00 GiB, whole GPU, upper bound") && text.contains("VRAM not measured"), "{text}");
        assert!(text.contains("1 gated measure(s) OVER"), "{text}");
    }

    // spec: TEST-019
    #[test]
    fn a_ram_peak_over_eight_gib_is_marked_over_and_the_memory_line_round_trips() {
        let m = SceneMemory {
            scene: "dive".into(),
            ram_peak_bytes: Some(RAM_BUDGET_BYTES + 1),
            vram_used_bytes: None,
            pool_slots: 5,
            max_resident: 4,
            share_slots: 6,
        };
        assert_eq!(SceneMemory::parse(&m.line()), Some(m.clone()));
        let o = SceneOutcome { scene: "dive".into(), code: Some(0), start_error: None, memory: Some(m), over: 0 };
        assert!(summary(&[o]).0.contains("OVER (budget 8 GiB"));
        for bad in [
            "",
            "SCENE-MEMORY",
            "SCENE-MEMORY scene=x ram_peak_bytes=zz vram_used_bytes=na pool_slots=1 max_resident=1 share_slots=1",
            "dive: ok",
        ] {
            assert!(SceneMemory::parse(bad).is_none(), "{bad:?}");
        }
    }
}
