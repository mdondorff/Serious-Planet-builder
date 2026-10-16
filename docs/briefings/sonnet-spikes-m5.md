# Briefing for sonnet-spikes: SPIKE-09 and SPIKE-08 for M5 (opus-planner, 2026-10-16)

You are a second coder session beside sonnet-explorer, who is finishing M4. The owner approved running these two M5 spikes in parallel (2026-10-16). opus-planner is the architect and coordinates you both; message it (`opus-planner`) with results and questions.

## Read first
1. `docs/constitution.md` and `CLAUDE.md`.
2. The `spike` skill (`.claude/skills/spike/SKILL.md`), especially **"Spikes in a parallel session"**. Those rules bind you.
3. ADR 0040 (atmosphere; point 6 is SPIKE-09) and ADR 0043 (CON-11 v1.4). ADR 0041 (water; point 2 is SPIKE-08), ADR 0026, ADR 0027 (reading the bake).
4. `docs/status.md`, the "Queued for M5" section, items 2 and 3.

## Your worktree and files
- Create and work only in `.claude/worktrees/spike-m5`, with one branch per spike: `spike/spike-09-atmosphere-f32` and `spike/spike-08-ocean-connectivity`.
- Write only:
  - your own `docs/spikes.md` entries (open with `/spike open`, close with `/spike close`);
  - `docs/learnings.md` entries if something surprises you;
  - results under `spikes/<name>/results/`.
- Make one docs-only PR per spike, with the spike's entry and results. Spike code stays on its spike branch.
- Never edit `docs/status.md`, `docs/log.md`, specs, ADRs or the main crates. opus-planner writes the log and status lines.
- The 16384 store in `.claude/worktrees/sonnet-explorer/target/accept/global16384` is **read-only** for you. Copy what you must modify into your own `target/`.
- **Perf windows:** when opus-planner announces a perf window, stop all builds, tests and runs until it says resume. Avoid long CPU or GPU runs while sonnet-explorer measures ad hoc, and ask opus-planner if unsure.

## Order
1. **SPIKE-09 Atmosphere f32 forms** (one day; ADR 0040 point 6).
   - Compare the naive f32 form and the altitude-relative primitives of ADR 0040 point 5 against an f64 reference.
   - Altitudes h = 1 m, 2 km, 20 km and 20,000 km; mu = -1, the horizon +-1e-6, and +1.
   - Adapters: WARP, llvmpipe if available, and the RTX.
   - Add a slow camera walk at 1 m altitude (the temporal-stability part of the bound, ADR 0043: no stepping beyond the bound between frames).
   - Recommend the simplest form that meets the bound of ADR 0040 point 4.
2. **SPIKE-08 Ocean connectivity at 16384** (one day; ADR 0041 point 2).
   - Run the tiled union-find with min-id roots, or an explicit-queue fill, over the stored bake's continentalness. The sea is `<= SEA_LEVEL_C`, with 4-connectivity over the cube faces.
   - Measure the cost and memory, and prove serial and tiled runs bit-identical by hash.
   - Include a dry basin one cell from the ocean.
   - Propose the area threshold.

Each entry ends with the recommendation for its ADR. opus-planner accepts ADR 0040 or 0041 afterwards.

## When you finish a spike
Message opus-planner: the PR number, the result in numbers, the recommendation, and anything that needs the owner. Then start the next spike, or go idle.
