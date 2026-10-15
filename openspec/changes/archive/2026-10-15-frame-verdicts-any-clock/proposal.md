## Why

The owner accepted ADR 0034 on 2026-10-15 with one change: the frame-time verdicts stay valid at whatever clock state the driver chose (that is what a user sees), with the clock state printed beside them; only `gpu_ms`, the GPU budget and GPU comparisons need P0. The code of #131 marks `frame_ms` NOT VALID outside P0 and withdraws the frame p99 verdict. Queue item 6d, before the first M4 perf window.

## What Changes

- perf: `GPU_METRICS` is `gpu_ms` only; the regression report compares `frame_ms` against the normal baseline at any clock state and prints both states (`[clock state X vs Y]` on a regression line, a `frame medians of <view> compared across unequal clock states` line when they differ or one is unknown). `LogSummary` gains the per-view clock state.
- perf: `terrain_view_line` (pure, tier A) is the console line of a view: outside P0 `NOT VALID for GPU budgets (<state>): GPU median ... (no verdict)`, the frame median and p99 with the verdict, and `[clock state <state>]` on every line; `skeleton.rs` prints it.
- Spec: TEST-012 text, `View outside P0`, `Equal clock state`, new `Frame medians at any clock state`.
- No log format change: `gpu_valid` and `gpu_clock_state` stay (they already carry the state), old logs (no samples) still read, their frame medians are compared and the line says `no clock sample`.

## Not in this change

- ADR 0034's text (accepted; only its status line says the owner's change and that this code follows). Not run on the reference GPU.
