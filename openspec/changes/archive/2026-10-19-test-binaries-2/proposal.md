## Why
BUILD-015 listed `frame` and `render` to follow after the removal of the vertex render path (#304, merged 2026-10-19). Both are above `generators`, `cache` and `streaming` in the chain, so every edit of those crates, and every edit of `frame` itself, relinks their per-file test binaries (45 and 34).

## What Changes
- `frame` builds `it` (tier A, 40 files) and `oracle_all` (tier B, 5 files); `render` builds `it` (7), `oracle_all` (7) and `gpu_all` (20); `autotests = false`, files included as `#[path]` modules.
- `.config/nextest.toml`: the `real-gpu-heavy` group names the heavy render tests by package, binary and module (`planet-render::gpu_all`: `gpu_lit`, `gpu_water`, `gpu_atmosphere`, `gpu_compact_blocks`; `planet-render::oracle_all`: `oracle_lit`).
- The xtask converted-crates guard lists `frame` and `render` (the per-tier layout test and the test-count oracle read them from `autotests = false`).
- BUILD-015: the crates done now include `frame` and `render`, with the measurement below.

## Measured (`cargo nextest run --no-run -p planet-frame -p planet-render -p planet-editor -p planet` after an edit of `crates/frame/src/lib.rs`; three edits per side after a warm-up; same machine, one after the other)
| | Runs | Median |
|---|---|---|
| Before (one binary per file) | 33.5, 34.8, 32.6 s | 33.5 s |
| After (`it`, `oracle_all`, `gpu_all`) | 10.7, 11.2, 10.0 s | 10.7 s |

-68 %, far over the 10 % rule.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling. No golden moves, no test is lost (the layout oracle compares the test lists).
