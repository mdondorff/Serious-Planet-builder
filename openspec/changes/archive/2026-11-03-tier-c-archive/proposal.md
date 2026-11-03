## Why
Every Windows tier-C part compiled the workspace's test binaries itself, 5-6 min of each part's job (#1125: 5m15s to 6m07s). With four parts (#1128) the worst part still reached about 16 min. Every merge in the queue costs the next PR a CI round, so a shorter round speeds the whole queue. Option (4) of the planner's plan (opus-planner, 2026-11-03).

## What Changes
- `cargo xtask test gpu --archive-out FILE`: one build of the tier-C binaries (`gpu_all` of `planet` and `planet-render`, with the binaries they start) into a nextest archive.
- `cargo xtask test gpu --archive-file FILE`: runs from it with no build and no package selection (`--workspace-remap` to this checkout); the filter, profile config and logs are as before.
- `ci.yml`:
  - `gpu-build` (Windows) makes and uploads the archive;
  - the four Windows parts need it, download it and run their part;
  - the Linux lavapipe leg is its own job (`gpu-linux`), so it does not wait for the Windows build.
- `gpu_window` starts the `planet` binary from `NEXTEST_BIN_EXE_planet` when nextest sets it, since the compile-time `CARGO_BIN_EXE_planet` path belongs to the build job.
- BUILD-018 and its repository check test follow, with a tier-A test of the archive arguments.

## Impact
- `.github/workflows/ci.yml`, `xtask/src/testcmd.rs`, `xtask/src/repocheck.rs`, `crates/app/tests/gpu_window.rs`; BUILD-018 is modified and renamed.
- The job names of the parts are unchanged; one more job (`windows (WARP) C build`).
- Tests read files through compile-time `CARGO_MANIFEST_DIR` paths, which hold because every windows-latest job checks out to the same workspace path.
