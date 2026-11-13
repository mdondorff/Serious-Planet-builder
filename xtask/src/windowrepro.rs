//! `cargo xtask repro <frame-n-path.txt | frame-n.bundle> --window [--real] [-- PLANET OPTIONS]` (BUILD-054): the camera path F9 wrote, replayed in a
//! window over the world the bundle records (its seed, `--commit` and `--site`, VIEW-071), in real time, with the stalls counted.
//!
//! The window is `planet editor --smoke` with the gate's path replay (`PLANET_GATE_PATH`, TEST-061); its report line carries the frames of a
//! second or more with the GPU phases counted (what the owner calls a stall). The options after `--` go to `planet editor` as they are
//! (for example `--resident-cap N`, VIEW-070).

use crate::util::Res;
use std::path::Path;
use std::process::Command;

/// The `planet editor` options the head of a bundle records: `seed N`, `open_commit DIR` and `open_site LAT,LON` (VIEW-071).
pub fn open_args(bundle_text: &str) -> Vec<String> {
    let head = bundle_text.split("\n---\n").next().unwrap_or("");
    let mut out = Vec::new();
    for line in head.lines() {
        if let Some(v) = line.strip_prefix("seed ") {
            out.extend(["--seed".to_string(), v.trim().to_string()]);
        } else if let Some(v) = line.strip_prefix("open_commit ") {
            out.extend(["--commit".to_string(), unescape(v)]);
        } else if let Some(v) = line.strip_prefix("open_site ") {
            out.extend(["--site".to_string(), v.trim().to_string()]);
        }
    }
    out
}

/// A bundle line's escapes: backslash, newline and carriage return.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match (c, if c == '\\' { it.next() } else { None }) {
            ('\\', Some('n')) => out.push('\n'),
            ('\\', Some('r')) => out.push('\r'),
            ('\\', Some(other)) => out.push(other),
            ('\\', None) => out.push('\\'),
            (c, _) => out.push(c),
        }
    }
    out
}

/// The `gate: ...` line of the window's output.
pub fn gate_line(stdout: &str) -> Option<&str> {
    stdout.lines().find(|l| l.starts_with("gate: "))
}

/// The stalls of a gate line: frames of the stall limit or more with the GPU phases counted.
pub fn stalls_of(line: &str) -> Option<u64> {
    let rest = line.split("; stalls ").nth(1)?;
    rest.split_whitespace().next()?.parse().ok()
}

/// The verdict line of a replay: the stalls and the world it ran over.
pub fn verdict(line: &str, args: &[String]) -> String {
    match stalls_of(line) {
        Some(n) => {
            format!("window replay: {n} stall(s); world options: {}", if args.is_empty() { "(none)".to_string() } else { args.join(" ") })
        }
        None => "window replay: the gate line has no stall count (an older build?)".to_string(),
    }
}

/// Replays `path` (a `frame-<n>-path.txt`) over the world of `bundle` in a window; prints the gate line and the verdict.
pub fn run(root: &Path, bundle: &str, path: &str, real: bool, extra: &[String]) -> Res {
    let text = std::fs::read_to_string(bundle).map_err(|e| format!("reading {bundle}: {e}"))?;
    if !text.starts_with("planet-terrain-repro") {
        return Err(format!("{bundle}: --window replays a terrain repro bundle (planet-terrain-repro)"));
    }
    let world = open_args(&text);
    if real {
        // A locked desktop makes the window stall and lose its device: NOT RUN, never a pass (TEST-073).
        if let planet_perf::desktop::Desktop::Locked(reason) = planet_perf::desktop::detect() {
            return Err(format!("{} the window replay was NOT RUN (desktop locked: {reason})", crate::util::INCOMPLETE));
        }
    }
    crate::util::run(root, "cargo", &["build", "--release", "-p", "planet"], &[])?;
    let exe = root.join("target").join("release").join(if cfg!(windows) { "planet.exe" } else { "planet" });
    let out_dir = root.join("target").join("repro-window");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let report = out_dir.join("report.txt");
    let adapter = if real { "hardware" } else { "software" };
    let mut args: Vec<String> = ["editor", "--smoke", "--no-vsync", "--adapter", adapter].iter().map(|s| s.to_string()).collect();
    args.extend(world.iter().cloned());
    args.extend(extra.iter().cloned());
    eprintln!("$ planet {}", args.join(" "));
    let mut cmd = Command::new(&exe);
    cmd.args(&args)
        .current_dir(root)
        .env("PLANET_GATE", "1")
        .env("PLANET_GATE_PATH", path)
        .env("PLANET_GATE_REPORT", &report)
        // A replay measures: the gate's failures are printed, not turned into an error that ends the run before its closing lines.
        .env("PLANET_GATE_MEASURE", "1")
        .env("PLANET_CRASH_DIR", out_dir.join("crashes"));
    if !real {
        // The software adapter's frame times are not judged (the gate's rule).
        cmd.env("PLANET_GATE_SOFTWARE", "1");
    }
    let out = cmd.output().map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The whole output stays beside the report: the closing lines of the window are not all printed.
    let _ = std::fs::write(out_dir.join("stdout.txt"), stdout.as_bytes());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let _ = std::fs::write(out_dir.join("stderr.txt"), stderr.as_bytes());
    let Some(line) = gate_line(&stdout) else {
        return Err(format!("the window replay printed no gate line ({}): {}", out.status, stderr.lines().last().unwrap_or("")));
    };
    println!("{line}");
    for r in stdout.lines().filter(|l| l.starts_with("planet editor: resident tiles") || l.starts_with("planet editor: gpu memory")) {
        println!("{r}");
    }
    // The failures the gate would have judged (held-back tiles, slow frames), printed as they are (stdout, after the gate line).
    if let Some(k) = stdout.lines().position(|l| l.starts_with("gate (measure, not judged):")) {
        for l in stdout.lines().skip(k).take(4) {
            println!("{l}");
        }
    }
    println!("{}", verdict(line, &world));
    println!("report: {}; stall reports of the window: {}", report.display(), out_dir.join("crashes").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-054
    #[test]
    fn the_bundle_head_gives_the_seed_the_commit_and_the_site_of_the_window() {
        let text = "planet-terrain-repro 2\ngenerator_version 9\nseed 14\nopen_commit C:\\\\data\\\\r3a e1\nopen_site -36.7498,-162.2827\nplan_hash 00\n---\nopen_site 1,2\n";
        assert_eq!(
            open_args(text),
            ["--seed", "14", "--commit", "C:\\data\\r3a e1", "--site", "-36.7498,-162.2827"],
            "the plan text after --- is not read"
        );
        let plain = "planet-terrain-repro 2\nseed 3\nplan_hash 00\n---\n";
        assert_eq!(open_args(plain), ["--seed", "3"], "a run without --commit and --site records neither");
    }

    // spec: BUILD-054
    #[test]
    fn the_stalls_are_read_from_the_gate_line() {
        let line = "gate: 900 frames, longest 40 ms, 0 over 100 ms; main-thread block decodes 0 in 0 frames; held back: 0 frames, 0 with no edit in reach, up to 0 under an edit; horizon maps baked 0; panics 0; stalls 7 (frames of 1000 ms or more with the GPU phases, longest 6400 ms)";
        assert_eq!(gate_line(&format!("noise\n{line}\n")), Some(line));
        assert_eq!(stalls_of(line), Some(7));
        assert_eq!(stalls_of("gate: 10 frames, longest 4 ms"), None);
        assert!(verdict(line, &["--seed".to_string(), "14".to_string()]).starts_with("window replay: 7 stall(s); world options: --seed 14"));
    }
}
