## Decision (owner, 2026-10-15, on ADR 0034)

ADR 0034 point 4 as proposed made a view outside P0 NOT VALID for `gpu_ms` and `frame_ms`. The owner accepted the ADR with a change: the frame-time median and p99 verdicts of `frame_ms` (CON-21) are judged as measured at any clock state, because that is what a user sees; the state is printed beside them and logged. `gpu_ms`, the GPU budget and GPU comparisons still need P0 and, across sessions, equal clock state. Frame-time medians are compared across sessions as CPU figures are, with both clock states printed.

ADR 0034's own text (point 4 "Amended by the owner" and the status line) already records this; this change only brings code and TEST-012 in line.

## Choices

- Frame verdict: the only frame budget is the p99 one (20 ms); the line prints median and p99 and judges the p99. No separate frame-median budget is introduced.
- Frame comparison uses the CPU baseline (`terrain.last-valid.json`), not the GPU baseline: a P2 session therefore stays a baseline for frame medians. Unequal or unknown states are printed, never a reason to skip.
- Clock state label: the state when all samples agree, else the reason (`mixed P0/P2`, `no clock sample`), the same text as `gpu_clock_state`.
- Log format unchanged; `view_clock_state` is recomputed from the samples when a log is read.
