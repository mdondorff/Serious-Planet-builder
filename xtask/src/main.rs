//! `cargo xtask`: the only command surface (CON-25). std only, so it builds in about a second.

mod accept;
mod layering;
mod repocheck;
mod speclint;
mod testcmd;
mod util;

use std::process::ExitCode;

const USAGE: &str = "cargo xtask <command>

  test fast            tier A for changed crates (and the crates above them)
  test                 tiers A + B on the software adapter
  test gpu             tier C goldens and debug views (software adapter; --real for the GPU)
  check                rustfmt --check, clippy -D warnings, layering
  layering             crate layering and GPU-free dependency check (CON-02)
  spec-lint            trace IDs, tiers, requirement coverage (CON-16)
  accept <milestone>   run milestone acceptance, e.g. M0
  repro <bundle>       replay a repro bundle (format arrives in M1)
  perf [--machine-ready]   performance harness on Rasierklinge (owner confirms the machine first)
  bless                list candidate goldens (promotion is the owner's /bless skill)

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
