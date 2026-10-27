//! `cargo xtask`: the only command surface (CON-25). std only, so it builds in about a second.

mod accept;
mod blesscmd;
mod cistats;
mod detdiff;
mod heavy;
mod layering;
mod logcmd;
mod prepush;
mod repocheck;
mod speclint;
mod storecmd;
mod testcmd;
mod testlog;
mod util;

use std::process::ExitCode;

const USAGE: &str = "cargo xtask <command>

  test fast            tier A for changed crates (and the crates above them)
  test                 tiers A + B on the software adapter
  test gpu             tier C goldens and debug views (software adapter; --real for the GPU, which also runs the tier-B oracles)
  check                rustfmt --check, clippy -D warnings, layering
  prepush              before every push to a PR branch: check, test fast, and test gpu when the diff against origin/main touches a golden-producing path (render, frame, shaders, generators, ...); prints each step's time and one verdict line (BUILD-016)
  layering             crate layering and GPU-free dependency check (CON-02)
  spec-lint            trace IDs, tiers, requirement coverage (CON-16)
  log [--since YYYY-MM-DD] [--limit N]   the project log: fragments of docs/log.d/ and the entries of docs/log.md, newest first (BUILD-010)
  accept <milestone>   run milestone acceptance, e.g. M0; M4 also judges the perf logs of a streaming window (--logs DIR, default perf/logs) and refuses without a valid one
  store [--res N] [--relief-scale S] [--link DIR]   build the production store (default 16384 cells, into target/stores/global<N>) and point the editor at it, or only link an existing one (VIEW-052); with --relief-scale S (1 to 4) an exaggerated world into its own store, and the editor command line for it (WDEF-017)
  repro <bundle>       replay a repro bundle (format arrives in M1)
  capture stills --poses FILE [--store DIR | --allow-memory-bake --global-res N] [--out-dir DIR] [--size WxH] [--views A,B] [--smoke]   one PNG per pose of the poses file over a world with its A-net chunk stage, every tile with its chunk landed (TEST-048; default directory target/review/m8-stills)
  perf [--machine-ready]   performance harness on Rasierklinge; the harness measures readiness and refuses (ADR 0020);
                           --machine-ready is the owner's override. Only the architect session runs it
  perf stream [SCENE|all] --store DIR   the streaming scenes dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km (ADR 0032) and dive-cold (the dive over a reader that ignores the pre-filled coarse tiles, report-only) over a stored 16384-cell bake; --allow-memory-bake runs over an in-memory bake labelled NOT the production source; `all` runs one child process per scene (the scene's own RAM peak, gate per child);
                           --smoke proves the scenes on the software adapter (never a measurement) and, with --store, first checks the store read-only (ocean channel, pre-filled compact tiles of levels F-4 (F the render floor, default 3), TEST-024; --prefill-levels N changes N and --floor-level F the floor, both passed through test-render); also --global-res N, --out PATH, --machine-ready, --gen-workers N (tile generation threads, default the machine's), --gen-ahead N, --gen-reserve N, --gen-priority below-normal|normal (STRM-020)
  capture flights [--store DIR | --allow-memory-bake --global-res N] [--out-dir DIR] [--size WxH] [--flight-frames N] [--smoke]   the two coastal flights as PNG sequences at 1440p with shore_filter 1 and 0 in target/review/m5-flights (and mp4 videos when ffmpeg is on the PATH) for the owner's look review (TEST-027); never a measurement, never during a perf window; --smoke uses the software adapter
  perf lit [--store DIR] [--out PATH] [--frames N] [--flight-frames N] [--views a,b,c] [--smoke] [--machine-ready]   the M5 lit perf scenes (TEST-025): per-pass GPU time over the viewer's views and the coastal flights
  perf m5 --store DIR [--out DIR] [--machine-ready]   the first M5 window in order: lit and the seven streaming scenes with --lit (one child per streaming scene); `cargo xtask accept M5` judges the logs
  perf preflight [--out DIR] [--smoke] [--machine-ready]   the check before a perf window (about 70 s): the readiness gate, three CPU calibrations, the top 5 CPU processes, AC state and power plan; refuses with the causes (TEST-040)
  perf envelope [--store DIR | --allow-memory-bake --global-res N] [--share-bound F] [--steady-window-s S] [--altitudes-m M,M] [--only-steady] [--places K] [--out FILE] [--smoke] [--machine-ready]   the M8 speed envelope (TEST-047, TEST-054): the highest level-flight speed per altitude (30 m, 100 m, 300 m, 1 km) at which the coarse share of the screen (and, over a `--store` world, the chunk stand-in share, with the chunk stage and its horizon jobs cold per flight) stays within the bound (default 0.01) over every 1 s window, plus 300 m with the horizon jobs off, written to perf/logs/envelope.json; the architect authorizes the window
  perf anet-chunk [--store DIR --global-res 16384] [--global-res N] [--machine-ready]   the GEN-052 A-net chunk time at 8, 4 and 10 m on the stage's production thread setting, and at 8 m on one thread (recorded), under the same gate; the default bake of 256 cells has coarse segments 8 times longer than the production run's: measure on the store
  perf coupled [--global-res N] [--machine-ready]   the GEN-042 coupled run (default 2048 cells per face edge, 100 steps) under the same gate
  perf global-bake [--global-res N] [--threads N] [--machine-ready]   the GEN-014 global bake (default 16384 cells per face edge) under the same gate
  cache-verify <dir> [--world <file>|--seed N] [--sample N] [--allow-stale]   regenerate a sample of a tile store from the definition and compare (STRM-004)
  cache-compact <dir> [--world <file>|--seed N] [--cap-bytes N]   rewrite a tile store keeping the records the definition still produces, evicting the oldest packs first under a cap (STRM-013)
  ci-stats [--runs N] | --record --since YYYY-MM-DD [--until YYYY-MM-DD]   median and p90 of each CI job over the last N (default 20) completed main runs, read with gh; a median over 16 min is flagged (BUILD-014)
  heavy <command> [args]   run any command (a diagnostic example, a replay) under the machine-wide heavy-run lock
  quiet on|off|status   the owner's flag file: while it is up every heavy command (test, prepush, perf, capture, accept, store, heavy) waits (BUILD-026)
  det-diff <a> <b>    compare two determinism manifests (GEN-003; CI diffs Windows against Linux)
  golden-review [--proposal <file>] [--extra <dir>]... [--tests <junit.xml>]... [--apply]   classify golden candidates under ADR 0017, and ADR 0037 for first goldens of M5 classes (auto-bless or escalate)
  bless [--extra <dir>]... [--apply <name>[,<name>]... --approved-by <who>]   list candidate goldens of every adapter (agents promote under ADR 0017 via golden-review; the rest is the owner's /bless, which applies what the owner approved with --apply; --apply runs only after the owner's /bless approval in the same session and --approved-by names the owner, never on an agent's own judgement)

options for test: --strict (a missing golden fails), --real (use the discrete GPU), extra args after `--` go to nextest";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = util::repo_root();
    let result = match args.first().map(String::as_str) {
        Some("test") => heavy::with(&format!("test {}", args[1..].join(" ")), || testcmd::test(&root, &args[1..])),
        Some("check") => testcmd::check(&root),
        Some("prepush") => heavy::with("prepush", || prepush::run(&root)),
        Some("layering") => layering::run(&root),
        Some("spec-lint") => speclint::run(&root),
        Some("log") => logcmd::run(&root, &args[1..]),
        Some("accept") => heavy::with(&format!("accept {}", args[1..].join(" ")), || accept::run(&root, &args[1..])),
        Some("repro") => testcmd::repro(&root, &args[1..]),
        Some("store") => heavy::with("store", || storecmd::run_store(&root, &args[1..])),
        Some("perf") => heavy::with(&format!("perf {}", args[1..].join(" ")), || testcmd::perf(&root, &args[1..])),
        Some("capture") => heavy::with(&format!("capture {}", args[1..].join(" ")), || testcmd::capture_cmd(&root, &args[1..])),
        Some("heavy") => heavy::run_wrapped(&root, &args[1..]),
        Some("quiet") => heavy::quiet(&args[1..]),
        Some("cache-verify") => testcmd::cache_verify(&root, &args[1..]),
        Some("cache-compact") => testcmd::cache_compact(&root, &args[1..]),
        Some("ci-stats") => cistats::run(&root, &args[1..]),
        Some("det-diff") => detdiff::run(&root, &args[1..]),
        Some("golden-review") => {
            let mut a = vec!["run", "-q", "-p", "planet-testkit", "--bin", "golden-review", "--"];
            a.extend(args[1..].iter().map(String::as_str));
            util::run(&root, "cargo", &a, &[])
        }
        Some("bless") => blesscmd::run(&root, &args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}
