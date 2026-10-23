Opened: 2026-10-23
Question: Can the engine run a regional field per block of a fixed grid (ADR 0077) inside streaming, the cache, the plan and the editor without breaking CON-22, bounded edits or the frame, whatever the block's compute is?
Why it matters: M8 implementation starts on 2026-10-25 (owner); ADR 0077 needs the integration risks measured alongside SPIKE-33's method question.
Time box: about half a day of agent work, indicative timings, no perf window; report by 2026-10-24 evening.
Success criteria: the measurements of the brief below with a fit verdict per rule (CON-06, CON-08, CON-22, edit invalidation, LOD stability, memory).
Fallback if it fails: a list of what in streaming, the cache or the plan blocks a block field, with the change each needs.
Code location: branch `spike/block-stage` (never merged); a prototype patch against main in this folder.

# SPIKE-34 brief: a block stage in the engine (block-stage)

## Set-up
- A block field as a `HeightField` over the store: `Scope::Block` on a fixed grid of whole coarse cells, a halo in metres, keyed like a stage output (ADR 0022's key plus the block grid).
- The compute is a stand-in: SPIKE-32's `RegionStage` (droplet erosion, 4 m cells) run per block with a halo, or a timed sleep of the measured cost per km2; the method is SPIKE-33's question, not this one's.
- Block jobs run on the streaming workers through the injected interfaces (CON-04); a tile whose block is not ready draws from the coarse field.

## Measurements
| # | What | Output |
|---|---|---|
| 1 | Flight from 30 km to 1.8 m over SPIKE-32's site and a scripted pan at 300 m: blocks requested, ready latency, tiles drawn from the coarse stand-in, frame CPU time | table |
| 2 | CON-22: no frame waits for a block; the switch from stand-in to block result (a visible pop: height difference at the switch, p99) | numbers; a still pair |
| 3 | Seams at block borders and between a fine tile and its parent (heights, normals) | numbers |
| 4 | Plan and bounds: `height_bounds` from a block's run; plan refusals or capped tiles over blocks | counts |
| 5 | Edits: a stamp near a block border invalidates which blocks; time until the area is exact again | table |
| 6 | Memory and cache: blocks resident at 300 m and 30 m, bytes per block, eviction | table |
| 7 | Block size: 5, 10 and 20 km blocks against items 1, 5 and 6 | recommendation |

## Rules for this spike
- Own worktree and branch; the owner's 16384 store is read-only (copy what a run modifies).
- No `cargo xtask perf`; timings indicative, other sessions and the display setup noted.
- Report: numbers in tables, the changes each subsystem needs (streaming, cache key, plan, editor), and a Visuals section.
