## Context
M2 gate, owner request to understand the draw-count risk.

## Decisions
- Frames re-plan nothing: the camera is static, planning cost is measured separately (20 samples) so CPU selection cost and GPU cost are reported side by side.
- Meshes are generated in parallel (std threads) and uploaded once per tile across views.
- Budget verdicts use GPU median <= 8 ms (report §15 M2) and wall p99 <= 20 ms (CON-21); sessions under a non-performance power scheme or with an idle GPU are flagged invalid (m2-real-gpu-findings).
- Smoke mode: same code, tiny frame size and counts, any adapter, labelled, no log by default.

## Observation (software adapter, not a measurement)
Planning 3,078 nodes at the 1 m view with cells 16 / tau 4 took about 11 ms of CPU per full recompute: selection must become frame-coherent or be cached before a 60 FPS loop re-plans every frame.

## Review follow-ups (verifier)

- A smoke run can no longer pass for a measurement: `--perf` with `--perf-smoke` is rejected, smoke needs `--scene terrain`, its log (only written with `--out`) is `valid_for_budgets: false` with a "smoke run" warning, and every session is also invalid when the adapter is not the reference GPU (the check moved into the session warnings, not just the nvidia-smi view of the machine).
- A zero or non-monotonic timestamp interval is reported as no GPU time instead of 0 ms; the budget comparison is a tested pure function (`budget_verdict`, inclusive limits, no timestamps means no GPU verdict).
- Noise is reported per view; smoke uses 1280x720 so the default cells/tau are valid; a plan for another frame size gets a `SizeMismatch` error; `--machine-ready` is checked before the adapter; `cargo xtask perf` now runs the terrain workload.
- Left as noted: `plan_ms` is sampled once per view and copied to its three runs; wall time includes a device wait per frame (a serialised frame, an upper bound for a pipelined one); GPU time covers the render pass only; no VRAM cap is applied (counts report resident vertex bytes); `render` panics on a poll or map failure.
