# Status log

Newest entries at the top. One line per event (change archived, ADR drafted, spike opened or closed, quarantine, milestone report). Stop reports get a short section.

## Waiting for the owner
- **Accept or change the proposed ADRs** 0001, 0003 to 0012 (ADR 0002 accepted 2026-10-10). Work proceeds on their leanings. ADR 0003 departs from the report's leaning (tangent-adjusted warp first, equal-area evaluated at the M2 gate).
- **Bless the six terrain goldens** (adapter dx12 WARP; listed in tests/goldens/pending.txt; candidates in target/review/candidates/): terrain_orbit_tile_id, terrain_aerial20km_level, terrain_ground200m_height, terrain_cube_corner_normals, terrain_ground1m_depth, terrain_low2km_morph. `/bless` must also remove the names from pending.txt (`cargo xtask check` fails otherwise). Two of them (ground200m height, cube-corner normals) carry little information; say if you prefer other views.
- **Bless the Linux goldens** (adapter `vulkan-llvmpipe-llvm-20-1-2-256-bits`): the four CI candidates are byte-identical to the blessed WARP goldens. They are in the `review-Linux` artifact of the latest CI run on `main` (download with `gh run download <id> -n review-Linux`).
- **Performance harness on Rasierklinge**: skeleton only; a real run needs your go (charger, performance power profile, discrete GPU mode), then `cargo xtask perf --machine-ready`. Also run `cargo xtask test gpu --real` once to get the real-GPU candidates.
- Tool note: `gh` is installed at `C:\Program Files\GitHub CLI\gh.exe` but is not on the Git Bash PATH.

## Log
- 2026-10-10 m2-terrain-render archived (LOD-005, LOD-007, REND-003..006, REND-009 active). Six terrain goldens wait for /bless (tests/goldens/pending.txt).
- 2026-10-10 m2-node-selection archived (LOD-001..004 active).
- 2026-10-10 M1 accepted automatically (docs/milestones/M1-report.md); verifier: all 5 exit criteria pass. CI green on Windows (WARP) and Linux (lavapipe) on `main`.
- 2026-10-10 m1-golden-strictness archived (TEST-002: missing golden fails for blessed adapters; bless listing ignores stale candidates).
- 2026-10-10 Goldens blessed by the owner for dx12-microsoft-basic-render-driver (hello_triangle, debug_view_face, debug_view_tile_id, debug_view_height). PR #1 merged by the owner's request to bootstrap CI; PR #4 merged after green CI (#2 and #3 superseded by #4).
- 2026-10-10 ADR 0002 (depth) accepted by the owner.
- 2026-10-10 m1-frameplan-skeleton archived (REND-001, 006, TEST-008, 009, 010, BUILD-007, LOD-006 active).
- 2026-10-10 m1-generator-first-tile archived (GEN-001..006, 009 active; GENERATOR_VERSION 2).
- 2026-10-10 m1-coordinates archived (COORD-001..010 active; libm pinned =0.2.16).
- 2026-10-10 ADRs 0001-0012 drafted (proposed).
- 2026-10-10 M0 archived (docs/milestones/M0-report.md).
- 2026-10-09 Repository seeded from the starter bundle (constitution v1, CLAUDE.md, agents, skills).
