# M1 Coordinates, harness and walking skeleton: report (2026-10-10)

## Summary
Four changes archived: `m1-coordinates` (COORD-001..010), `m1-generator-first-tile` (GEN-001..006, 009), `m1-frameplan-skeleton` (REND-001, REND-006, TEST-008/009/010, BUILD-007, LOD-006) and `m1-golden-strictness` (TEST-002). `main` has 70 requirements, of which the M0 and M1 ones are active.

## Criteria (report §15, M1)
| Criterion | Result | Evidence |
|---|---|---|
| Round trip <= 1 mm | pass | surface contract suite (tolerance 1 mm, measured about 4e-9 m), regression bound 1e-6 m |
| Seam property tests | pass | neighbour symmetry over all faces and edges (levels 0..4), bit-identical border samples (levels 0..3 exhaustive, 20..28 random), deep cross-face generator borders, net continuity |
| Tile hashes identical on Windows and Linux CI | pass | known-answer tile hashes, float libm bit patterns and hash KATs pass on both CI jobs (run 38027899959, Linux = llvmpipe LLVM 20.1.2) |
| Golden harness live, exact-match debug views | pass (Windows WARP); Linux goldens not yet blessed | 4 goldens committed; lavapipe output is byte-identical to the WARP goldens |
| Skeleton runs in all three run modes | pass, with a note | `test-render`, `editor --offscreen` (no window until M2) and `generate`; repro bundles replay in the first two |

## Verifier
All five criteria pass on the automated evidence (verifier report 2026-10-10). Findings fixed afterwards: golden strictness per adapter, stale candidate listing, status cleanup.

## Open risks
- Only the software adapters (WARP, lavapipe) have run; the real GPU (RTX 3080 Ti) is unverified, including the WGSL integer texel selection from `@builtin(position)`.
- Coarse tiles cannot use f32 tile-local offsets (1 mm limit at level 9); the M2 terrain change must build border positions from shared data (learning 2026-10-10).
- Height debug view has low contrast; generator is a test noise only.

## Learnings
See docs/learnings.md (wgpu 30 API, nextest filter rule, bit-identical directions vs crack-free tiles, `@builtin(position)`).

## Proposed ADRs awaiting the owner
0001, 0003 to 0012.

## Recommendation
Go to M2 (CDLOD node selection on the cube sphere, camera-relative rendering, reversed-Z, minimal async tile generation). M2 ends at a go/no-go gate; the report there needs a real-GPU run, which needs your go.
