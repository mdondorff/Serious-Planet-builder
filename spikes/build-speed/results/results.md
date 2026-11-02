# SPIKE-15 edit-to-test build time

Question: where do the 50-100 s after a one-line edit go, and which of a faster linker, fewer test binaries and test-profile tuning bring the edit to the first test result to 20-30 s?
Prototype: branch `spike/spike-15-build-speed` (commit d534ca4, never merged). Machine: Rasierklinge (20 threads), `CARGO_BUILD_JOBS=10`, MSVC toolchain, rustc 1.99.0.

## Method
- Edit: one `std::hint::black_box(N);` line, N changed each run, in a function body of `crates/frame/src/lod.rs`, `crates/generators/src/height.rs` or `crates/app/src/lib.rs`.
- Timed: `cargo nextest run --no-run -p <the edited crate and every crate above it> --timings` (the build half of `test fast`; the tests are timed apart).
- Per configuration: one warm-up edit (not counted), then 3 timed edits per crate; the table gives the median. CPU load (instantaneous, before each run) is in `raw-timings.txt`; every run but three started at 0-12 %.
- Steps are cumulative: each row keeps the changes of the rows above it.

## Step 1: where the time goes (baseline)
| Edit | Median wall | Runs |
|---|---|---|
| frame (frame, render, editor, app) | 88.8 s | 89.3 / 88.8 / 84.8 |
| generators (7 crates) | 120.6 s | 118.2 / 121.2 / 120.6 |
| app (planet only) | 31.4 s | 33.7 / 31.4 / 30.0 |

`cargo xtask test fast` after a frame edit: 120 s wall (build 102.9 s, tests 14.1 s, 439 tests).

Breakdown of the frame edit (`--timings`, 107 units, 75.5 s inside cargo):

| Part | Time |
|---|---|
| Compiling the library crates (planet-frame lib 2.3 s, render 1.1 s, app 1.4 s) | 3 s |
| Test-profile build of each library's unit-test binary (render 10.9 s, app 12.2 s, editor 6.2 s) | 6-12 s each |
| About 100 integration-test and example binaries, each 1-17 s (compile of the test file plus a link with full debug info) | 60+ s of the wall, 10 at a time |

The dominant cost is the number of binaries: every one is compiled and linked on its own, and the dependent chain of one edit relinks all of them. Codegen of the edited function is under 3 s. Contention with other sessions was not the cause (load 4-7 % before the runs).

## Steps 2-4: results (median of 3, wall seconds of the build)
| Configuration | frame edit | generators edit | app edit |
|---|---|---|---|
| 1 Baseline | 88.8 | 120.6 | 31.4 |
| 2 `rust-lld` (`[target.x86_64-pc-windows-msvc] linker = "rust-lld"`) | 78.6 (66-99) | 96.9 | 15.8 |
| 3 plus one test binary per tier in frame and render | 41.6 | 60.5 (one run at 93 % load) | 18.2 |
| 4a plus `debug = "line-tables-only"`, dependencies `debug = false` | 23.2 | 41.7 | 15.2 |
| 4b (instead of 4a) workspace crates at `opt-level = 0`, dependencies at 3 | 32.1 | 49.0 | 33.0 |

| Step | Gain against the row above (frame edit) | Cost to apply properly | Risks |
|---|---|---|---|
| 2 `rust-lld` | 10 s; app edit halves (31 to 16 s) | 1 h plus a CI check | The frame figure is noisy (66-99 s). Linux CI: rustc already links with lld on x86_64 Linux since 1.90, so no change there. `link.exe` fallback needs no change. |
| 3 binaries per tier | 37 s (frame) | 4-6 h for all crates (generated `it.rs`, `oracle_all.rs`, `gpu_all.rs` per crate; 150 test files) | Test names gain a module prefix (`lod::balancing...`), binary names change to `it`, `oracle_all`, `gpu_all`. Tier filters by `binary(/^gpu_/)` still select the tiers (kept by the naming). The `real-gpu` override and the xtask test `the_real_gpu_profile_groups_existing_binaries_only_and_leaves_the_default_profile_alone` name individual binaries and need a new grouping; the group of 2 would now cover the whole `gpu_all`. `#[path]` modules keep `support/` and `data/` paths working. A failing build in one file stops the whole tier's binary. |
| 4a debug info | 18 s (frame) | 30 min | Debuggers and backtraces keep file and line, lose local variables and types; a debug session needs the full profile (`--profile` with `debug = true`). No change in code generation. |
| 4b `opt-level = 0` | slower than 4a by 9 s (frame), tests 28.4 s against 21.2 s for the whole tier A | | Rejected: the build is slower (unoptimised code is larger to link) and the tests cost 34 % more. |

Tests after 4a: `cargo nextest run --workspace` tier A and B: 1121 tests, 21-22 s, 1 failure (the xtask guard named above, caused by the nextest group edit of the prototype). Debug-info settings do not change code generation, so the test time of 4a equals the baseline's.

## Step 5 (dynamic library of heavy dependencies)
Not run. After steps 2-4a the largest unit of a frame edit takes 5.4 s and the generators edit is 41.7 s with 91 units still built (generators 24, cache 14, streaming 9, app 15 test files not yet merged). The brief's trigger (linking above about 10 s per edit) is not met for a frame or app edit.

## Step 6: CI
- Linux runners already use lld; Windows would gain from `rust-lld` (the CI runner builds the same 100+ binaries).
- Fewer binaries and less debug info shorten the cold build of the CI run too (fewer links, smaller artefacts to cache); the per-binary link is the same cost cold or warm. Expected gain: a few minutes of the median 16 min; not measured.
- Smaller debug info shrinks `target/` and so the cache upload and restore. Not measured.
- The CI `.github` files were not changed.

## Recommendation
Apply, in this order:
1. Steps 4a and 2 (1.5 h, small risk): frame edit 89 to about 55 s, app edit 31 to about 15 s.
2. Step 3 for every crate with more than 5 test files: frame 42, render 32, generators 24, cache 14, app 15, streaming 9 (4-6 h; requires the nextest group and xtask guard rework).
3. Skip 4b; skip step 5 unless a measured edit still links above 10 s after step 3.

Expected edit-to-first-test after steps 1-3 on a quiet machine: frame edit about 23 s of build plus 1-2 s of test start (target 20-30 s met); generators edit about 40 s now, an estimated 30 s once generators, cache, streaming and app are merged too (the estimate is from the unit list, not a measurement); app edit about 15 s.

## Visuals
`--timings` charts (HTML, open in a browser; each under 150 kB):
- `images/timings-1-baseline-frame-edit.html` against `images/timings-4-final-frame-edit.html`: the baseline chart has 107 units and ends at 75.5 s of cargo time; the final chart has 38 units and ends at 17.3 s.
- `images/timings-1-baseline-generators-edit.html` against `images/timings-4-final-generators-edit.html`: the same for the generators edit (160 units ending at 99.3 s of cargo time, against 91 units at 30.2 s).
