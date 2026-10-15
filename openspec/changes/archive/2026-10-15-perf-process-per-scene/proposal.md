## Why

The first M4 perf window's RAM figure was the process peak working set over the process life, so `perf stream all` (six scenes in one process) reported a number that included the bake and every earlier scene; CON-21's 8 GB limit needs the scene's own peak (opus-planner, 2026-10-15). The VRAM line was the whole GPU with no allocator figure beside it, and the CPU clock was not recorded although the compact-vs-vertex plan-time difference of the first window might have been a slower CPU.

## What Changes

- `cargo xtask perf stream all` (and `perf stream` without a scene) runs one child process per scene, sequentially, each the same binary with the session's options passed through (`--out DIR` becomes `DIR/<scene>.json`); a failing child does not stop the rest; a combined summary lists all six scenes; the exit is non-zero if any failed. The orchestration is a pure function over a `ChildRunner` in `planet-perf::session`, tested on fakes; `xtask` only supplies the process spawn.
- The readiness gate (ADR 0020) runs in each child, before that child's measured case. The gate needs the adapter and runs inside the harness; each scene is thereby gated on the machine as it is then, and no state is passed down.
- RAM verdict: `scene's own peak working set (child process)` against 8 GiB; a process that ran several scenes (`--scene streaming` by hand) is labelled as not the scene's own.
- New verdict line `terrain slots`: `N slots allocated of M share slots` (the pool's slots against the slots the tier's share holds at the pool's slot size, mid tier 2 GiB as the scenes use), and the driver VRAM line says whole GPU, upper bound, beside the same counts. Counts in the log: `pool_slots`, `share_slots`, `max_resident_slots`.
- CPU clock and power plan: the effective clock (base frequency times `% Processor Performance`, through the locale-independent WMI class) before and after each case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz`), printed with the power plan's name and GUID. Recorded, never judged, never compared between logs.
- Each child prints one `SCENE-MEMORY` line the parent reads for the summary.

## Not in this change

- Per-frame GPU timestamps of retired frames (gpu_ms stays unmeasured in the pipelined scenes; tracked).
- Any change to the verdict rules or budgets; no real perf run.
