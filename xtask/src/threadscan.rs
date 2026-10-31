//! The thread-creation scan (BUDG-004, BUDG-010): background work runs on the job system, so every call that starts a thread outside it is
//! listed here with how many there are and why. A call that is not listed fails the test; a listed count that no longer matches fails it too, so
//! the list shortens with every pool that moves onto the job system and cannot go stale.

#[cfg(test)]
mod tests {
    use crate::util::{files_with_ext, repo_root};

    /// What counts as starting a thread (the production part of a file: everything before its `mod tests`).
    const CALLS: [&str; 5] = ["thread::spawn(", "thread::Builder::new(", "thread::scope(", "spawn_named(", "spawn_scoped("];

    /// (file, calls, reason). The pools of the job system and the scoped workers still to move are marked `moves in PR n` (openspec/changes/job-system).
    const ALLOWED: [(&str, usize, &str); 26] = [
        ("crates/app/src/coupledworld.rs", 1, "driver: the coupled run's one thread; stays"),
        ("crates/app/src/horizonworld.rs", 1, "moves in PR 4: the horizon bake coordinator"),
        ("crates/app/src/hoverpick.rs", 1, "driver: the hover pick's one thread; stays"),
        ("crates/app/src/skeleton.rs", 2, "moves in PR 1: the skeleton's scoped tile workers"),
        ("crates/app/src/stills.rs", 1, "moves in PR 4: the stills' scoped parts"),
        ("crates/app/src/streamperf.rs", 1, "driver: a perf scene's flight thread; stays"),
        ("crates/app/src/worldfield.rs", 1, "moves in PR 4: the coupled run's thread"),
        ("crates/cache/src/meshpack.rs", 2, "moves in PR 4: the prefill workers"),
        ("crates/core/src/par.rs", 4, "`run_scoped`: the scoped helper threads of a `Par`; `SharePar` takes them as permits of the terrain share first (the permit rule, BUDG-009)"),
        ("crates/core/src/thread.rs", 1, "the named scoped thread helper itself"),
        ("crates/frame/src/look/atmosphere.rs", 2, "moves in PR 4: the atmosphere rows"),
        ("crates/frame/src/scenes.rs", 1, "moves in PR 4: the lag post-pass parts"),
        ("crates/frame/src/water.rs", 2, "moves in PR 4: the water heights"),
        ("crates/generators/src/coarsehorizon.rs", 2, "moves in PR 4: the coarse horizon"),
        ("crates/generators/src/coupledinput.rs", 2, "moves in PR 4: the bake reduction"),
        ("crates/generators/src/global.rs", 5, "moves in PR 4: the bake rows and blocks"),
        ("crates/generators/src/height.rs", 2, "moves in PR 4: the tile workers of `generate_region`"),
        ("crates/generators/src/horizon.rs", 2, "moves in PR 4: the horizon bake"),
        ("crates/generators/src/ocean.rs", 2, "moves in PR 4: the ocean passes"),
        ("crates/perf/src/desktop.rs", 1, "tool: the desktop probe; stays"),
        ("crates/perf/src/gate.rs", 1, "tool: the GPU load query; stays"),
        ("crates/perf/src/load.rs", 1, "tool: the load generator, which must not use the system it loads; stays"),
        ("crates/perf/src/streaming.rs", 1, "tool: the streaming scene's scoped parts; stays"),
        ("crates/streaming/src/jobs.rs", 1, "the job system itself: its workers"),
        (
            "crates/streaming/src/lib.rs",
            1,
            "`ThreadPool`: no production caller since PR 3a; the spawner of tests and examples (remove when they use the job system)",
        ),
        ("xtask/src/testlog.rs", 1, "tool: the log pump; stays"),
    ];

    /// The calls of the production part of a source file.
    fn calls_in(source: &str) -> usize {
        let production = source.split("\nmod tests {").next().unwrap_or(source);
        production
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .map(|l| CALLS.iter().map(|c| l.matches(c).count()).sum::<usize>())
            .sum()
    }

    // spec: BUDG-010
    #[test]
    fn the_calls_that_count_are_found_in_production_code_only() {
        assert_eq!(calls_in("fn a() { std::thread::spawn(f); }\n// thread::spawn(x)\nfn b() { spawn_named(s, \"n\", f); }"), 2);
        assert_eq!(calls_in("fn a() {}\n\nmod tests {\n    fn t() { std::thread::scope(f); }\n}"), 0);
        assert_eq!(calls_in("pub fn spawn_named<'a>(x: u8) {}"), 0, "a definition is not a call");
    }

    // spec: BUDG-004, BUDG-010
    #[test]
    fn every_thread_started_outside_the_job_system_is_listed_with_its_count_and_a_reason() {
        let root = repo_root();
        let mut files = Vec::new();
        files_with_ext(&root, "rs", &mut files);
        let mut found: Vec<(String, usize)> = Vec::new();
        for f in files {
            let rel = f.strip_prefix(&root).unwrap().display().to_string().replace('\\', "/");
            let in_src = (rel.starts_with("crates/") || rel.starts_with("xtask/")) && rel.contains("/src/");
            if !in_src {
                continue;
            }
            let n = calls_in(&std::fs::read_to_string(&f).unwrap_or_default());
            if n > 0 {
                found.push((rel, n));
            }
        }
        found.sort();
        let mut problems = Vec::new();
        for (file, n) in &found {
            match ALLOWED.iter().find(|(f, _, _)| f == file) {
                None => problems.push(format!("{file}: {n} thread start(s) outside the job system, not listed (BUDG-004: run the work on a Share, or list it here with a reason)")),
                Some((_, want, _)) if want != n => {
                    problems.push(format!("{file}: {n} thread start(s), the list says {want} (update the list; it shortens as pools move onto the job system)"))
                }
                Some(_) => {}
            }
        }
        for (file, _, _) in ALLOWED {
            if !found.iter().any(|(f, _)| f == file) {
                problems.push(format!("{file}: listed but starts no thread any more: remove it from the list"));
            }
        }
        assert!(problems.is_empty(), "thread-creation scan:\n  {}", problems.join("\n  "));
    }
}
