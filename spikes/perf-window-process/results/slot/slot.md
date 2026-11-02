# Quiet slot: SPIKE-95's proofs, the 2° shadow scene and the flight scenes

The planner schedules the slot (inbox #72, opus-planner 2026-11-02); the spikes lane runs it from the native-UI session.
- Conditions: the desktop is unlocked, the other lanes hold CPU work, the readiness gate decides, never `--machine-ready`.
- Estimate: 75 to 90 min. Steps 3 to 4b skip the window preflight (`--no-preflight`): step 2 opened the slot with one, and each child keeps its own readiness gate (ADR 0020).

| Step | What | Command | Minutes |
|---|---|---|---|
| 0 | One release build (the window runner builds again, a no-op) | `cargo build --release -p planet` | 2 |
| 1 | PR 1 proof: the calibration estimator on the quiet machine | `cargo xtask perf preflight --out target/slot/pf-N` for N = 1 to 7 (21 calibration rounds) | 9 |
| 1b | PR 1 proof: the slow state is still refused | the planner runs 3 preflights from its VS Code terminal | 4 |
| 2 | PR 2 and PR 3 proof: duty idle, no cool start, window runner | `cargo xtask perf window spikes/perf-window-process/results/slot/proof-duty-idle.plan` | 20 |
| 3 | The 2° shadow scene, default configuration (march on, quarter-res below 5°, bilinear ground at mip-0) | PLACEHOLDER: `perf stream shadow-cost` line and 2° poses file from crusader (asked 2026-11-02) | 10 |
| 4 | ADR 0092: flight scenes at the terrain share; teleports recorded | `cargo xtask perf window spikes/perf-window-process/results/slot/flights-share6.plan --no-preflight` | 25 |
| 4b | The lit flight coast-glide once (frame-time check) | `cargo xtask perf window spikes/perf-window-process/results/slot/lit-glide.plan --no-preflight` | 3 |

## What each step reports
1. The share of the 21 rounds over a TEST-039 limit, against the single-timing record: SPIKE-90 window 1, 36 % of unloaded attempts; SPIKE-58 phase 2, 2 of 4 quiet preflights. Plus the spread of the readings, as input for re-deriving the limits (ADR 0093 point 5).
   - 1b passes when every round is refused.
2. The valid share, the cool-down and duty-idle minutes, and minutes per valid run, against window 2 block 2 (35 min, 0 valid) and windows 1 and 2 (3.3 and 10 min per valid run).
   - Confound: window 2 ran thermally limited (60-64 C) with loaded configurations.
   - Also the gate's share of the window: the remaining lever, an ADR 0020 amendment if it is large.
3. Readings of that one run: the shadow-pass ms at 2° against #1085's 1.50-1.54 ms, and the frame total. No separate switches for the bilinear or level checks.
4. Flight verdicts (frame median and p99, tile-ready) of dive, dive-fast, look-1m and look-2km at 6 workers (0.6 x B, B = 10 here: ADR 0092's worked example). Teleport-1m and teleport-2km are recorded, never a verdict. coast-glide runs once as the lit flight's frame-time check.
   - A flight miss goes back to the owner.

## After the slot
- Results go into `spikes/perf-window-process/results/results.md`, section "Proofs".
- The verdicts of steps 3 and 4 go to the planner for `docs/status.md`.
- The raw window folders stay under `target/perf-window/`; only the tables are committed.
