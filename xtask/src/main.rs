//! `cargo xtask`: the only command surface (CON-25). std only, so it builds in about a second.

mod accept;
mod detdiff;
mod layering;
mod repocheck;
mod speclint;
mod testcmd;
mod util;

use std::process::ExitCode;

const USAGE: &str = "cargo xtask <command>

  test fast            tier A for changed crates (and the crates above them)
  test                 tiers A + B on the software adapter
  test gpu             tier C goldens and debug views (software adapter; --real for the GPU, which also runs the tier-B oracles)
  check                rustfmt --check, clippy -D warnings, layering
  layering             crate layering and GPU-free dependency check (CON-02)
  spec-lint            trace IDs, tiers, requirement coverage (CON-16)
  accept <milestone>   run milestone acceptance, e.g. M0; M4 also judges the perf logs of a streaming window (--logs DIR, default perf/logs) and refuses without a valid one
  repro <bundle>       replay a repro bundle (format arrives in M1)
  perf [--machine-ready]   performance harness on Rasierklinge; the harness measures readiness and refuses (ADR 0020);
                           --machine-ready is the owner's override. Only the architect session runs it
  perf stream [SCENE|all] --store DIR   the streaming scenes dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km (ADR 0032) and dive-cold (the dive over a reader that ignores the pre-filled coarse tiles, report-only) over a stored 16384-cell bake; --allow-memory-bake runs over an in-memory bake labelled NOT the production source; `all` runs one child process per scene (the scene's own RAM peak, gate per child);
                           --smoke proves the scenes on the software adapter (never a measurement); also --global-res N, --out PATH, --machine-ready, --gen-workers N (tile generation threads, default the machine's), --gen-ahead N, --gen-reserve N (STRM-020)
  perf global-bake [--global-res N] [--threads N] [--machine-ready]   the GEN-014 global bake (default 16384 cells per face edge) under the same gate
  cache-verify <dir> [--world <file>|--seed N] [--sample N] [--allow-stale]   regenerate a sample of a tile store from the definition and compare (STRM-004)
  cache-compact <dir> [--world <file>|--seed N] [--cap-bytes N]   rewrite a tile store keeping the records the definition still produces, evicting the oldest packs first under a cap (STRM-013)
  det-diff <a> <b>    compare two determinism manifests (GEN-003; CI diffs Windows against Linux)
  golden-review [--proposal <file>] [--extra <dir>]... [--apply]   classify golden candidates under ADR 0017 (auto-bless or escalate)
  bless                list candidate goldens (agents promote under ADR 0017 via golden-review; the rest is the owner's /bless)

options for test: --strict (a missing golden fails), --real (use the discrete GPU), extra args after `--` go to nextest";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = util::repo_root();
    let result = match args.first().map(String::as_str) {
        Some("test") => testcmd::test(&root, &args[1..]),
        Some("check") => testcmd::check(&root),
        Some("layering") => layering::run(&root),
        Some("spec-lint") => speclint::run(&root),
        Some("accept") => accept::run(&root, &args[1..]),
        Some("repro") => testcmd::repro(&root, &args[1..]),
        Some("perf") => testcmd::perf(&root, &args[1..]),
        Some("cache-verify") => testcmd::cache_verify(&root, &args[1..]),
        Some("cache-compact") => testcmd::cache_compact(&root, &args[1..]),
        Some("det-diff") => detdiff::run(&root, &args[1..]),
        Some("golden-review") => {
            let mut a = vec!["run", "-q", "-p", "planet-testkit", "--bin", "golden-review", "--"];
            a.extend(args[1..].iter().map(String::as_str));
            util::run(&root, "cargo", &a, &[])
        }
        Some("bless") => testcmd::bless_list(&root),
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
