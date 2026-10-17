# Briefing for sonnet-spikes: SPIKE-12 Retiring the vertex tile format (opus-planner, 2026-10-17)

Your session was cleared, so everything you need is below or in the files it names. opus-planner is the architect; message it (`opus-planner`) with results and questions. The owner asked for this spike (2026-10-17) to prepare one decision at the M5 gate: **retire the vertex tile format from rendering, or keep both formats.**

## Read first
1. `docs/constitution.md` and `CLAUDE.md`.
2. The `spike` skill (`.claude/skills/spike/SKILL.md`), especially "Spikes in a parallel session" and the worktree check (`git rev-parse --show-toplevel` before any branch switch).
3. ADR 0033 (points 3-6: slots, the compact format, the CON-13 guard, the finest level) and ADR 0054 (compact tiles in storage buffers, accepted by the owner).
4. Your two earlier explorations: `spikes/tile-format-duality/results/results.md` (PR #218) and `spikes/compact-storage-buffer/results/results.md` (PR #225), and their learnings in `docs/learnings.md`.
5. `docs/status.md`: item 7c, and the gate item "Retire the vertex tile format from rendering".

## Where things stand
- sonnet-explorer is building ADR 0054 (item 7c: compact texels in storage-buffer blocks) in production code. Do not touch production code or its branch. Build on your own prototype branch `explore/compact-storage-buffer`.
- The vertex format (48 B per vertex, 58,608 B per tile) is used for levels 0-2, for CON-13 fallback tiles (a finest tile with more than 65.535 m of relief), and as the CPU reference. The compact format (8 B texel plus 128 B record, 8,840 B per tile) is used for everything else.
- Your duality exploration estimated retirement at about 5-7 coder days. It needs (a) a way to draw levels 0-2 compactly, where the 3x3 biquadratic base surface is too coarse (3.6 px at level 0), and (b) a texel for cliff tiles: a 32-bit height with 15-bit normals, the floor for CON-14. Neither was prototyped or timed.

## Questions (time box 1.5 days)
1. **Levels 0-2.** Prototype both options and compare:
   - **(a)** a 5x5 base surface, for at least levels 0-2. Measure its position error against the CPU mesh per level in pixels at the standard views.
   - **(b)** a render floor at level 3: orbit and aerial views draw level-3 tiles instead of levels 0-2. Count the draws and slots at the ten standard views, and the indicative RTX GPU time at orbit and aerial-200km.
   - For both: the image difference against the current vertex path (debug views, WARP and RTX), and what changes for LOD selection, residency, the pre-fill and the goldens.
2. **Cliff tiles.** Prototype a wider texel on the buffer layout: a 32-bit height and two 15-bit normal components. Either one layout for all tiles, or a per-slot variant where only cliff tiles use the wide texel; say which and why.
   - Check CON-13 (1 mm at the finest level) and CON-14 (normals within 0.01 degrees of the CPU normal) at tier-B depth on the steep stress field.
   - Measure the indicative RTX GPU time against the current 8 B texel at ground-1m and on the steep field, and the bytes per slot.
3. **Per-tile data in the slot.** Does the buffer slot hold the climate field (today in the spare 16 bits) and a future per-tile block without a second resource? Give a concrete slot layout with byte counts against ADR 0033's shares.
4. **What remains.**
   - What is left of the vertex format afterwards (the CPU reference and oracle only?).
   - Which production lines go (count them), which tests change and why (none may be weakened: a test of a removed path is replaced by one of the new path), which goldens move and by how much, and the MESH_VERSION bump.
5. **Plan and cost.** An ordered step plan, with each step a small OpenSpec change of one subsystem, a firmer estimate than 5-7 days, the risks, and a recommendation: retire or keep, and if retire, in M5 (before item 8's perf window) or early M6.

## Rules
- Write only in your worktree (`.claude/worktrees/sonnet-spikes`), on a branch `spike/spike-12-vertex-retirement` cut from `explore/compact-storage-buffer`. Open the register entry with `/spike open` (SPIKE-12), and close it with `/spike close` when done.
- Write only your `docs/spikes.md` entry, `docs/learnings.md` entries and `spikes/vertex-retirement/results/`. Never edit `docs/status.md`, `docs/log.md`, specs, ADRs or the main crates.
- **RTX timings** are allowed as indicative numbers, without a separate go from opus-planner. Before each timing run, ask sonnet-explorer whether the GPU is free, and tell it when you're done. Label every number indicative (not a perf window), with its time. Never run `cargo xtask perf`. If opus-planner announces a perf window, stop all runs.
- The WARP device removal with compact tiles plus atmosphere is known and accepted as test-only. Don't chase it; use the debug views without atmosphere on WARP.
- Write files with LF line endings and forward slashes in paths (the PR #224 lesson).
- No production code. Prototype code stays on the spike branch.

## When you finish
Make one docs-only PR (entry, results, learning) and message opus-planner with:
- the PR number;
- the numbers for questions 1-3;
- the plan and cost;
- the recommendation;
- anything only the owner can decide.

Then go idle. SPIKE-11 (mouse look) stays open, waiting for the owner's local check: leave `spikes/mouse-look` as it is.
