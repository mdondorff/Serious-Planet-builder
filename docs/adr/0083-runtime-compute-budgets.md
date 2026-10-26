# 0083 Runtime compute budgets
Status: proposed (drafted by opus-planner, 2026-10-26; owner's targets 2026-10-26)
Date: 2026-10-26
Source: owner, 2026-10-26 (1440p at 60 fps on Rasierklinge; weaker machines degrade later); SPIKE-44 (`spikes/compute-inventory/results/results.md`, #737, #738)

## Context
- Every subsystem sizes its own threads: tile pool 18 (cores − 2), chunk pool 9, plan 1, horizon-bake jobs, and the plan job spawns up to 20 scoped height workers per call outside every pool.
- Measured on Rasierklinge (i7-12800H, 14 cores / 20 threads; RTX 3080 Ti 16 GB) at 1 m: 59.6 fps vsync-paced, about 9 ms of main-thread work in a 16.8 ms frame, GPU 4.3 ms (terrain 3.4); workers 4.7 to 5.4 busy cores, up to 17.5 of 20 in a flight; RAM 3.1 GB, VRAM 1.7 GB.
- The risk is the tail: 3.2 % of frames over 30 ms (plan and residency, chunk landing up to 729 ms in one frame).
- Sky and weather, physics and game logic will need CPU, GPU and cores later; nothing reserves them.

## Decision
1. **Frame budget.** Game and viewer mode target 16.6 ms at 1440p on the reference machine; the frame's main-thread CPU and GPU time are split into named shares per subsystem, with reserved shares for sky and weather, physics and game logic. Editor mode may exceed terrain's share; its tools have their own share.
2. **One job system.** Background work runs on one shared job system with a core budget per subsystem and priority classes: frame-critical, visible streaming and edits, prefetch, idle (bakes). No subsystem creates threads outside it.
3. **Terrain's share.** Terrain (streaming, chunks, horizon, plan) gets a stated fraction of the frame and of the cores; the rest is reserved, unused until a later subsystem claims it.
4. **Memory budgets.** RAM and VRAM shares per subsystem, the same way.
5. **Where the numbers live.** The share table is a spec (the current value of every share); the perf harness checks each share as a regression, and a share is changed by a spec change, not by a subsystem.
6. **Weaker machines** degrade through quality settings (LOD factor, pool sizes) later; not part of this decision.

## Consequences
- The tile, chunk and plan pools and the plan job's scoped workers move onto the job system (one change, before M8's acceptance perf window if it fits); criterion 7 is then measured inside terrain's share.
- A frame over budget names the share that overran (the stall probe's phase marks, VIEW-056).
- New subsystems claim a share before they add work.

## Licences of adopted code or techniques
None.
