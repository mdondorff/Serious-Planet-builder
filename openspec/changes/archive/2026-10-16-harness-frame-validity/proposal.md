## Why

Perf window 1 (2026-10-16): `cargo xtask accept M4` refused because every session was INVALID. The harness treated a GPU that started in P8 (idle clock) as voiding the whole session, but CPU-bound streaming scenes never clock the GPU up, and the owner decided on 2026-10-15 (ADR 0034 as accepted) that frame verdicts stand at any clock state. The console also said `SESSION VALID` in one place and `not valid for time budgets` in a later line.

## What Changes

- perf: `planet_perf::validity` (pure): each cause of invalidity with what it voids (frame verdicts, GPU verdicts), the session verdict and its console lines, and the reading of old logs. The idle-start check voids GPU verdicts only.
- log: `frame_valid`, `gpu_session_valid`, `gpu_only_causes` added; `valid_for_budgets` keeps its meaning (both valid).
- app: the three harness paths print the one verdict from it; the `WARNING (not valid for time budgets)` lines are gone for real sessions (the causes are in the verdict).
- accept: judges frame verdicts from a frame-valid log, refuses on every other cause, prints the GPU state as a fact.
- compare: a frame-valid session is valid for the frame, CPU and count figures and baselines.
- spec: TEST-012, TEST-021 (modified; no new ids).

Not in this change: GPU time in the pipelined scenes, any perf run.
