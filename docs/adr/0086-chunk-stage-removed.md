# 0086 Chunk stage removed
Status: accepted (owner, 2026-10-29: remove the chunk stage from the code; recorded 2026-10-30 by sonnet-crusader for opus-planner); done 2026-10-31 in six PRs (#935, #944, #948, #951, #953, #957)
Date: 2026-10-30
Source: owner decisions 2026-10-29 (queue row CS) and 2026-10-30 (M8 closed); `openspec/changes/archive/2026-10-31-remove-chunk-stage/proposal.md`; ADR 0077, 0081, 0082, 0084; SPIKE-57 (M9 erosion)

## Context
- The A-net chunk stage (fine erosion and river routing per 5 km chunk) has been off by default since 2026-10-28 (STRM-046, TEST-064); with it on, the no-degradation gate fails (held-back tiles, level steps, slow chunk frames).
- M9 builds erosion into the bake (SPIKE-57), so the stage has no successor in the plan.
- The horizon maps are keyed per chunk and stay until the ADR 0085 build replaces them.
- Git keeps the history; no spec or code is kept for a feature that is off.

## Decision
1. The chunk stage is removed from the code, specs and tests in seven PRs, one area each (plan: `openspec/changes/archive/2026-10-31-remove-chunk-stage/`). It covers the A-net incision and routing, the chunk field and pyramid, the chunk manager, the stand-in share and the fade, the re-solve after an edit, and their flags, scenes and tests.
2. This supersedes ADR 0077 (except its global coupled run and CON-08's wording), ADR 0081, ADR 0082 and ADR 0084 points 3 and 4. Those ADRs keep their text; their status lines say so (ADR 0078).
3. Edits under the erosion (D3, owner 2026-10-26) are deferred to M9's erosion design; the chunk re-solve is retired.
4. The driver channels of the stage (GEN-070 to GEN-076) are removed if and only if the default path draws identical pixels without them; M9 re-adds drivers from the stored bake.
5. The definition hash stays stable: the removed `chunk_stage` setting hashes as its old default, and a test shows that the 16384 store's definition opens without a rebuild.
6. Kept until the ADR 0085 build: `HorizonJobs`, `ChunkKey`, the chunk lattice, the global coupled run with its store and the coarse horizon.

## Consequences
- Every PR of the removal leaves main green and changes no golden on the default (stage-off) path; a golden diff stops the PR.
- Criterion 3's gate has no stage-on run; `gate --chunks` and its ceilings go.
- The Tracked items about the stage (gate failures with it on, chunk-fade limits, re-solve, repro) are deleted with their code.

## Licences of adopted code or techniques
None.
