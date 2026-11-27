//! `cargo xtask`: the only command surface (CON-25). std only, so it builds in about a second.

mod accept;
mod agentguard;
mod archivems;
mod bakecmd;
mod blesscmd;
mod cistats;
mod cockpit;
mod detdiff;
mod disk;
mod gate;
mod gates;
mod heavy;
mod k7layout;
mod knowntierc;
mod lanes;
mod layering;
mod logcmd;
mod looksheet;
mod nextid;
mod package;
mod perfab;
mod perfwindow;
mod plancmd;
mod prbody;
mod prepush;
mod preview;
mod process;
mod repocheck;
mod retired;
mod retiredids;
mod rivergate;
mod scratch;
mod speclint;
mod sprintfile;
mod sprintlint;
mod stages;
mod storecmd;
mod terrainscore;
mod testcmd;
mod testlog;
mod threadscan;
mod transects;
mod util;
mod windowrepro;
mod windowsdraw;
mod worktrees;

use std::process::ExitCode;

const USAGE: &str = "cargo xtask <command>

  test fast            tier A for changed crates (and the crates above them)
  test                 tiers A + B on the software adapter
  test gpu             tier C goldens and debug views (software adapter; --real for the GPU, which also runs the tier-B oracles)
  check                rustfmt --check, clippy -D warnings, layering
  gate [--small] [--site frame-N]... [--paths DIR] [--store DIR]   the no-degradation gate (TEST-061 to TEST-063): the editor window over the real store, a scripted 400 m hill and the camera path of each capture site of the owner test (outputs under target/gate/), judged on frames over 100 ms, held-back tiles no edit reaches, horizon maps, caught panics and the step between levels; --small is the same counters on a generated world on the software adapter
  prepush              before every push to a PR branch: check, test fast, and test gpu when the diff against origin/main touches a golden-producing path (render, frame, shaders, generators, ...); prints each step's time and one verdict line (BUILD-016); refuses a working tree that differs from HEAD, since the push sends the commit (BUILD-037) `--declared <proposal.md>`: tier C also passes when its golden candidates are exactly the proposal's `Expected golden changes:` (BUILD-068)
  layering             crate layering and GPU-free dependency check (CON-02)
  k7-layout [--write-baseline]  the layout report of K7 (BUILD-052): the fixture's tile layout against an alternative, compared with docs/tools/gates/k7-layout-baseline.txt; REVIEW above twice the baseline p99 or max; exit 0; prepush runs it when K7 files change
  river-gate <store dir | face,i0,i1,j0,j1,seed | r1h | r3a | r3b | m1 | f1 | grid=file,w,h,sea_level_m> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--network-km2 A,B] [--graph FILE] [--stages DIR] [--partition ONE,TWO] [--surface FILE] [--start FILE] [--noise axis|turned] [--corridor-bar PCT] [--no-key-check]   the river gate (BUILD-055): closed basins on river length (0.5 m, 16 cells), the valley depth (1-2 cells) and the burn (K7's cut), ring 2-4 and the erosion depth, lakes at D, relief at 200 m and 1 km against the start surface, edge agreement with H1, and the measurement suite (BUILD-057: gradient, valley width, asymmetry, drainage density, catchment ratio, seam, narrowness, slope-area, relief spectrum) and the channel network at two thresholds (BUILD-059: density, Strahler streams, R_b; default 1 and 75 km2), and with a canonical river graph, stage fields or a partition pair the G2 columns with their gate classes and the G2 bars, pass or fail (BUILD-060; the budget's relief on S0, reading 28; --corridor-bar default 97.5, 95 on m1), on a store (its commit's tiles), a window's start surface or a raw f64 grid of 101.8 m cells (a GLO-30 tile); release, under the heavy lock; exit 0
  river-gate --table <window | store dir@window | store dir | grid=file,w,h,sea>[#graph=F][#stages=D][#partition=A,B][#surface=F][#start=F][#noise=N][#timing=F][#chain=F]... [--windows W,W...] [options]   one row per run of every gate and suite number, counts beside percentages (BUILD-057); target/review/river-gate-table.md and .csv
  spec-lint            trace IDs, tiers, requirement coverage (CON-16)
  log [--since YYYY-MM-DD] [--limit N]   the project log: fragments of docs/log.d/ and the entries of docs/log.md, newest first (BUILD-010)
  archive-milestone <M> [--before YYYY-MM-DD] [--dry-run]   housekeeping's mechanical archive steps (BUILD-072): the fragments of docs/log.d/ (before the date) into docs/archive/<M>/log.md in the log's order, then deleted; the closed spikes' rows of docs/spikes.md into docs/archive/<M>/spikes.md; the header pointers; a closed sprint file, its report and its records folder into docs/archive/<M>/ (BUILD-077), naming the live files that still point at them; nothing else deleted; --dry-run prints the plan only
  accept <milestone>   run milestone acceptance, e.g. M0; M4 also judges the perf logs of a streaming window (--logs DIR, default perf/logs) and refuses without a valid one
  store [--res N] [--relief-scale S] [--link DIR]   build the production store (default 16384 cells, into target/stores/global<N>) and point the editor at it, or only link an existing one (VIEW-052); with --relief-scale S (1 to 4) an exaggerated world into its own store, and the editor command line for it (WDEF-017)
  repro <bundle> [--real] [--every N] [--store DIR]   replay a repro bundle or an F9 path; --store DIR replays a bundle of a stored bake read-only (TEST-071)
  repro <frame-N-path.txt> --window [--real] [-- OPTIONS]   replay the path in a window over the bundle's world (its --commit and --site), count the stalls (BUILD-054)
  capture stills --poses FILE [--store DIR | --allow-memory-bake --global-res N] [--out-dir DIR] [--size WxH] [--views A,B] [--smoke]   one PNG per pose of the poses file over the base terrain of a world, with the horizon maps near the eye baked (TEST-048; default directory target/review/m8-stills)
  perf [--machine-ready] [--loaded]   performance harness on Rasierklinge; the harness measures readiness and refuses (ADR 0020); --loaded records the load instead of refusing on it (ADR 0087, BUILD-030);
                           --machine-ready is the owner's override. Only the architect session runs it
  perf --ab REF [--rounds N] [--retries N] [--metrics a,b] [--log FILE] PERF-COMMAND...   the A/B helper (BUILD-033, ADR 0087): the working tree and REF run alternately (a worktree under target/ab), a gate refusal is retried, one table of the medians, the change and the verdict
  perf reserve [--rounds N] [--retries N] [--lit] [--store DIR] [--loaded] ...   the reserve test (BUDG-012): the five standard flights without and with the synthetic loads of the reserved shares, alternating, one table and the reserve report per flight; the planner gives the go
  perf stream [SCENE|all] --store DIR   the streaming scenes dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km (ADR 0032) and dive-cold (the dive over a reader that ignores the pre-filled coarse tiles, report-only) over a stored 16384-cell bake; --allow-memory-bake runs over an in-memory bake labelled NOT the production source; `all` runs one child process per scene (the scene's own RAM peak, gate per child);
                           --smoke proves the scenes on the software adapter (never a measurement) and, with --store, first checks the store read-only (ocean channel, pre-filled compact tiles of levels F-4 (F the render floor, default 3), TEST-024; --prefill-levels N changes N and --floor-level F the floor, both passed through test-render); also --global-res N, --out PATH, --machine-ready, --gen-workers N (tile generation threads, default the terrain share, BUDG-003), --gen-ahead N, --gen-reserve N, --gen-priority below-normal|normal (STRM-020)
  package [--store DIR] [--date YYYY-MM-DD] [--out DIR] [--dry-run]   the owner package (TEST-074): the default-look stills of the two look-review sites (hills-b, river-r1) and the coastal flights over the store, and a package.md with PASS, FAIL or NOT RUN per item, in target/review/owner-package/<date>/; never a measurement; delivery: the planner sends the directory to the owner with SendUserFile (B4-6)
  capture flights [--store DIR | --allow-memory-bake --global-res N] [--out-dir DIR] [--size WxH] [--flight-frames N] [--smoke]   the two coastal flights as PNG sequences at 1440p with shore_filter 1 and 0 in target/review/m5-flights (and mp4 videos when ffmpeg is on the PATH) for the owner's look review (TEST-027); never a measurement, never during a perf window; --smoke uses the software adapter
  perf lit [--store DIR] [--out PATH] [--frames N] [--flight-frames N] [--views a,b,c] [--smoke] [--machine-ready]   the M5 lit perf scenes (TEST-025): per-pass GPU time over the viewer's views and the coastal flights
  perf m5 --store DIR [--out DIR] [--machine-ready]   the first M5 window in order: lit and the seven streaming scenes with --lit (one child per streaming scene); `cargo xtask accept M5` judges the logs
  perf wallclock [--machine-ready]   the wall-clock checks that left tier A (BUILD-003): builds them, runs the readiness gate (ADR 0020; the adapter condition does not apply, CPU only), then each ignored check alone in release, PASS or FAIL per check; a refused gate is NOT RUN (exit code 3), no verdict; run at the start of every perf window and by `cargo xtask accept`
  perf window PLAN [--no-preflight]   a declared perf window in one process (BUILD-034, ADR 0087 point 7): one release build, the preflight, rotated rounds of the plan's configurations and flights with the duty idle between runs (BUILD-035), bounded retries, the stop rule, and every attempt timed in target/perf-window/<time>/; the architect authorizes the window, and --machine-ready in a flight stays the owner's override
  perf preflight [--out DIR] [--smoke] [--machine-ready]   the check before a perf window (about 70 s): the readiness gate, three CPU calibrations, the top 5 CPU processes, AC state and power plan; refuses with the causes (TEST-040)
  perf envelope [--store DIR | --allow-memory-bake --global-res N] [--share-bound F] [--steady-window-s S] [--altitudes-m M,M] [--only-steady] [--places K] [--out FILE] [--smoke] [--machine-ready]   the M8 speed envelope (TEST-047, TEST-054): the highest level-flight speed per altitude (30 m, 100 m, 300 m, 1 km) at which the coarse share of the screen (over a `--store` world with the horizon jobs cold per flight) stays within the bound (default 0.01) over every 1 s window, plus 300 m with the horizon jobs off, written to perf/logs/envelope.json; the architect authorizes the window
  perf preview [N] [--threads N] [--seed N] [--smoke]   the latency of the level-1 preview at the H0 grid (TEST-099): a height stamp, a rainfall region and both, with the phases, the process peak and the resident bytes; a timing read needs a quiet machine and a perf window authorised by the lane orders; --smoke (n = 96) is no measurement
  perf coupled [--global-res N] [--machine-ready]   the GEN-042 coupled run (default 2048 cells per face edge, 100 steps) under the same gate
  perf global-bake [--global-res N] [--threads N] [--machine-ready]   the GEN-014 field bake (default 16384 cells per face edge) under the same gate: its wall time reported against terrain's world-build share (BUDG-014), its resident memory judged against 8 GB
  cache-verify <dir> [--world <file>|--seed N] [--sample N] [--allow-stale]   regenerate a sample of a tile store from the definition and compare (STRM-004)
  cache-compact <dir> [--world <file>|--seed N] [--cap-bytes N]   rewrite a tile store keeping the records the definition still produces, evicting the oldest packs first under a cap (STRM-013)
  ci-stats [--runs N] | --record --since YYYY-MM-DD [--until YYYY-MM-DD]   median and p90 of each CI job over the last N (default 20) completed main runs, read with gh; a median over 16 min is flagged (BUILD-014)
  heavy [--exclusive] <command> [args]   run any command (a diagnostic example, a replay) in one of the heavy-run lock's slots (PLANET_HEAVY_SLOTS, default 2); --exclusive waits for every slot and holds them all (timing runs; perf, gate and repro --window are exclusive by default) (BUILD-026)
  preview world [--seed N] [--out DIR]   (default seed 14, the reference world) the whole-planet preview of the default class source (GEN-103): equirectangular PNGs at 2048x1024 (land and sea; elevation with hillshade, today's and the new side by side; the plate mask; plateau and back-arc; relief classes, argmax and blended; k) with the seed, versions and a legend, and a report of the class shares; CPU only, no store, about 10 s; default out target/preview/world-seed-N
  bake layer [--face-res N] [--planet-res N] [--tile N] [--halo N] [--region FILE] [--keep-pass1] [--seed N] [--out DIR] [--threads N] [--map-width W]   (m9-bake PR 5, GEN-101) pass 1 then pass 2 of the erosion layer: every land tile of the --planet-res grid (default 3072 cells a face edge, tiles of 3072) or the tiles of a region file (`face tile_i tile_j` a line, at most 64), checkpointed per face and per tile (a killed run resumes), the maps and tiles.txt in DIR; pass 1 is dropped after a complete planet pass unless --keep-pass1; above 12,288 only with a region
  bake pass1 [--planet-res N] [--seed N] [--out DIR] [--threads N] [--faces LIST] [--map-width W]   (m9-bake, GEN-100) pass 1 of the erosion layer at a reduced resolution (default 768 cells per face edge; above 4096 refused: a full bake needs the planner's go), one face at a time into a layer store in DIR/layer (a killed run resumes), then the maps of the carved base; prints the wall time and the peak memory of each face
  terrain-score [--store DIR] [--dem DIR] [--sites FILE] [--out DIR] [--maps]   the terrain scorecard (TEST-079, BUILD-029): seven descriptors on nine GLO-30 tiles and the generator's two matched sites each (world.field() of the store, read only), distances and a phone-readable report.md and report.json in target/review/terrain-score/<date>/ with the Copernicus notice; CPU only, about 1 minute; --maps adds one 512 px PNG per descriptor per raster
  pr-body-check        the pr-gate check (BUILD-044, BUILD-075): reads PR_BODY, PR_TITLE, HEAD_REF, BASE_SHA and HEAD_SHA; a PR with a non-docs path needs `Verifier: PASS <sha>` of a commit of the PR, plus `Numerics: PASS <sha>` (crates/core, crates/generators, *.wgsl) and `Spec-Guardian: PASS <sha>` (openspec/) by path; a record needs `Research-Review: READY <sha>` and `Owner-Look:`; an active sprint needs `Sprint: <M>`; then `plan --pr`
  lanes                per lane of docs/tools/lanes.txt the last commit and its age, the open PR, the latest inbox number and the running cargo or example processes (BUILD-043)
  plan [--pr TITLE [--branch B] [--body-file F] [--files F] PATH...]   the current milestone's exit items (docs/milestones/, Status line `active`; a `draft` between sprints) with their state, queue rows (docs/status.md) and open PRs naming them; fails for a queue row off the milestone and for an open item without a row (BUILD-053); a sprint file (an `Opened:` line, ADR 0130) is checked for its header, at most 7 days and 5 exits with every cell, its records, at most 3 open decisions and A1's safe defaults, and prints `overdue`; --pr checks one PR against the R/P firewall and the overdue rule, with the exits its title and its body's `Exit:` lines name and the paths given or listed one a line in --files (BUILD-074)
  stages [--update]    the stage manifest (BUILD-047): runs the stage probe's fixture and checks docs/tools/stages/manifest.txt against it (a stale manifest fails naming the stage, field and column); --update rewrites it from the run; the sheets go to target/review/stages/ci/
  stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]   the chain sheet (BUILD-060): one PNG of a chain directory's twelve stage panels (base, z_s, H1 rivers, S0, KH, KS, trunks, tributaries and basins, KV, KC, final, final - S0) at a 10-degree sun and one fixed scale; default the whole window at 12 L cells per pixel to target/review/stages/chain/<label>.png
  look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]   the look sheet (BUILD-067): a chain run's crop store (--probe-crops) at one L cell a pixel under a fixed 10-degree sun, beside the defaults run and their difference; target/review/look/<run>/
  look-sheet RUN --steps [--az A,B] [--out DIR]   the step rows (BUILD-071): per crop, each stage of the store's stages.txt (or the chain columns it holds) beside its step from the previous stage, red lower, blue higher, full at 2 m; <crop>-step-azNNN.png and steps.txt
  transects --chain DIR [--face F] [--out FILE]   the cross-section sheet (BUILD-061): per SPIKE-119 river transect inside a chain directory's window, S0, the final surface and the matched GLO-30 valley on one vertical scale, with each profile's valley depth and its width at 10 m and 50 m above the river; to target/review/stages/transects/<label>.png and a .txt table (the face from chain.txt's `switch window`, or --face)
  windows-draw [--out FILE] [--threads N]   G3-8's out-of-sample windows (BUILD-066): a seeded stratified draw of 10 windows over the seed-14 world's coarse field (cube edges and a corner, flat coasts, high mountains, arid or endorheic land, mixed mid-latitude), none overlapping the five windows; written as data to crates/cache/data/g3-windows.toml
  cockpit [--seed N | --world F] [--erosion on|off [--channels on|off] | --commit DIR] [--pipeline-json F] [--run <world|drivers|stages|all>] [--compare-seed N] [--chain DIR [--face F]] [--km N] [--threads N] [--out-dir DIR]   assemble the offline pipeline DAG, switch simulator, Global Authoring preview inspector, and pin/probe ledger under target/review/cockpit/index.html (BUILD-064)
  gates                the gate ledger (BUILD-041): per gate of A3 12.1 the newest verdict, rows and lanes from docs/tools/gates/ledger.md; spec-lint rejects a gate with no row
  worktrees [--prune] [--no-size]   every git worktree with size, branch, uncommitted count, last commit and merged PR (BUILD-039); --prune removes with `git worktree remove` (never --force) only the clean ones whose PR is merged or that are detached at an ancestor of origin/main, never the current one or the root; prepush warns above 12 worktrees; prepush, store and bake refuse below 20 GB free on the drive and warn below 50 GB
  quiet on|off|status   the owner's flag file: while it is up every heavy command (test, prepush, perf, capture, accept, store, heavy) waits (BUILD-026)
  det-diff <a> <b>    compare two determinism manifests (GEN-003; CI diffs Windows against Linux)
  golden-review [--proposal <file>] [--extra <dir>]... [--tests <junit.xml>]... [--apply]   classify golden candidates under ADR 0017, and ADR 0037 for first goldens of M5 classes (auto-bless or escalate)
  bless [--extra <dir>]... [--apply <name>[,<name>]... --approved-by <who>]   list candidate goldens of every adapter (agents promote under ADR 0017 via golden-review; the rest is the owner's /bless, which applies what the owner approved with --apply; --apply runs only after the owner's /bless approval in the same session and --approved-by names the owner, never on an agent's own judgement)
  agent-guard [--refresh-bin]   Antigravity PreToolUse guardrail (reads hook JSON on stdin, emits JSON decision; BUILD-062)
  next-id <PREFIX|ADR|SPIKE> [--lanes-dir DIR] [--no-lanes] [--gh]   print next contiguous max+1 requirement, ADR, or spike ID (BUILD-062)

options for test: --strict (a missing golden fails), --real (use the discrete GPU), extra args after `--` go to nextest";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = util::repo_root();
    let result = match args.first().map(String::as_str) {
        Some("test") => {
            heavy::with(&format!("test {}", args[1..].join(" ")), || scratch::begin(&root).and_then(|_| testcmd::test(&root, &args[1..])))
        }
        Some("check") => testcmd::check(&root),
        Some("prepush") => {
            // BUILD-068: `--declared <proposal.md>`, the change's declared golden changes; any other argument is refused
            let declared: Result<Option<std::path::PathBuf>, String> = match &args[1..] {
                [] => Ok(None),
                [flag, p] if flag == "--declared" => {
                    let p = root.join(p);
                    if p.is_file() {
                        Ok(Some(p))
                    } else {
                        Err(format!("--declared {}: no such proposal", p.display()))
                    }
                }
                other => Err(format!("prepush takes no argument but `--declared <proposal.md>`, not {other:?}")),
            };
            match declared {
                Err(e) => Err(e),
                Ok(declared) => {
                    if prepush::needs_slot(&testcmd::changed_paths(&root)) {
                        heavy::with("prepush", || {
                            disk::check(&root, "prepush")
                                .and_then(|_| scratch::begin(&root))
                                .and_then(|_| prepush::run(&root, declared.as_deref()))
                        })
                    } else {
                        // A docs-only diff runs spec-lint alone, in seconds: it takes no heavy slot and waits for no bake (BUILD-026; lead #156). It does not
                        // prepare the scratch folder either: that empties `target/test-tmp`, which a heavy test of this worktree may be using.
                        eprintln!("prepush: spec-lint only, so no heavy slot is taken");
                        disk::check(&root, "prepush").and_then(|_| prepush::run(&root, declared.as_deref()))
                    }
                }
            }
        }
        // The gate times frames in a window: exclusive; `--small` runs headless on the software adapter in a slot.
        Some("gate") if args.iter().any(|a| a == "--small") => {
            heavy::with(&format!("gate {}", args[1..].join(" ")), || gate::run_gate(&root, &args[1..]))
        }
        Some("gate") => heavy::with_exclusive(&format!("gate {}", args[1..].join(" ")), || gate::run_gate(&root, &args[1..])),
        Some("layering") => layering::run(&root),
        Some("k7-layout") => heavy::with("k7-layout", || k7layout::run(&root, &args[1..])),
        Some("river-gate") if args.get(1).map(String::as_str) == Some("--table") => {
            heavy::with(&format!("river-gate {}", args[1..].join(" ")), || rivergate::run_table(&root, &args[2..]))
        }
        Some("river-gate") => heavy::with(&format!("river-gate {}", args[1..].join(" ")), || rivergate::run_gate(&root, &args[1..])),
        Some("spec-lint") => speclint::run(&root),
        Some("log") => logcmd::run(&root, &args[1..]),
        Some("archive-milestone") => archivems::run(&root, &args[1..]),
        Some("accept") => {
            heavy::with(&format!("accept {}", args[1..].join(" ")), || scratch::begin(&root).and_then(|_| accept::run(&root, &args[1..])))
        }
        // A window replay times frames: the machine alone (BUILD-026 exclusive); a headless replay stays unlocked as before.
        Some("repro") if args.iter().any(|a| a == "--window") => {
            heavy::with_exclusive(&format!("repro {}", args[1..].join(" ")), || testcmd::repro(&root, &args[1..]))
        }
        Some("repro") => testcmd::repro(&root, &args[1..]),
        Some("store") => heavy::with("store", || disk::check(&root, "store").and_then(|_| storecmd::run_store(&root, &args[1..]))),
        Some("perf") => heavy::with_exclusive(&format!("perf {}", args[1..].join(" ")), || testcmd::perf(&root, &args[1..])),
        Some("capture") => heavy::with(&format!("capture {}", args[1..].join(" ")), || testcmd::capture_cmd(&root, &args[1..])),
        Some("package") => heavy::with(&format!("package {}", args[1..].join(" ")), || package::run(&root, &args[1..])),
        Some("preview") => heavy::with(&format!("preview {}", args[1..].join(" ")), || preview::run_cmd(&root, &args[1..])),
        Some("bake") => heavy::with(&format!("bake {}", args[1..].join(" ")), || {
            disk::check(&root, "bake").and_then(|_| bakecmd::run_cmd(&root, &args[1..]))
        }),
        Some("terrain-score") => heavy::with("terrain-score", || terrainscore::run_cmd(&root, &args[1..])),
        Some("pr-body-check") => prbody::run(&root),
        Some("lanes") => lanes::run(&root),
        Some("plan") => plancmd::run(&root, &args[1..]),
        Some("gates") => gates::run(&root),
        Some("stages") => heavy::with(&format!("stages {}", args[1..].join(" ")), || stages::run_cmd(&root, &args[1..])),
        Some("look-sheet") => heavy::with(&format!("look-sheet {}", args[1..].join(" ")), || looksheet::run_cmd(&root, &args[1..])),
        Some("transects") => heavy::with(&format!("transects {}", args[1..].join(" ")), || transects::run_cmd(&root, &args[1..])),
        Some("windows-draw") => heavy::with(&format!("windows-draw {}", args[1..].join(" ")), || windowsdraw::run_cmd(&root, &args[1..])),
        Some("cockpit") if cockpit::needs_heavy_slot(&args[1..]) => {
            heavy::with(&format!("cockpit {}", args[1..].join(" ")), || cockpit::run_cmd(&root, &args[1..]))
        }
        Some("cockpit") => cockpit::run_cmd(&root, &args[1..]),
        Some("worktrees") => worktrees::run(&root, &args[1..]),
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
        Some("agent-guard") => agentguard::run(&root, &args[1..]),
        Some("next-id") => nextid::run(&root, &args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            // Checks NOT RUN on a locked desktop: incomplete, not a pass, and not a failure (TEST-073).
            if e.starts_with(util::INCOMPLETE) {
                ExitCode::from(3)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
