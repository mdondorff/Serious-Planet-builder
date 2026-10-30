## Why
ADR 0083 point 5: the share table of the runtime compute budgets is a spec, so that the current value of every share has one home, the perf harness can check each share as a regression, and a share changes by a spec change and not by a subsystem. Option B1 of `docs/briefings/week-2026-11-02.md`.

## What changes
A new capability `compute-budgets` (BUDG-001 to BUDG-007): the shares of the main-thread frame time, the GPU time, the cores, the RAM and the VRAM, with reserves for sky and weather, physics, game logic and networking; the job-system rule; the reserve test; the rule that an overrun names its share.

## Initial values
Drafted from SPIKE-44 (`spikes/compute-inventory/results/results.md`: about 9 ms main thread and 4.3 ms GPU in a 16.8 ms frame, RAM 3.1 GB, VRAM 1.7 GB, 18 tile workers) and CON-21 (16.6 ms, RAM 8 GB, VRAM 8 and 14 GB, terrain's VRAM share from STRM-006). Terrain's shares are the measured figures plus a margin; the reserves split the remainder. The values are for the owner and opus-planner to ratify; changing one is a spec change.

## Status
All requirements are `planned`: no share is checked yet. The checks arrive with B2 (one job system) and B3 (reserve test); each requirement turns `active` with the change that makes its check pass.
