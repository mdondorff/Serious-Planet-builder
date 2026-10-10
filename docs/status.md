# Status log

Newest entries at the top. One line per event (change archived, ADR drafted, spike opened or closed, quarantine, milestone report). Stop reports get a short section.

## Waiting for the owner
- **M2 go/no-go gate** (docs/milestones/M2-report.md): recommendation Go with changes. Nothing in M3 has been started.
- **Accept or change the proposed ADRs** 0003 to 0014 (ADRs 0001 and 0002 accepted 2026-10-10). Work proceeds on their leanings. ADR 0003 departs from the report's leaning (tangent-adjusted warp first, equal-area evaluated at the M2 gate).
- **Bless the six terrain goldens** (adapter dx12 WARP; listed in tests/goldens/pending.txt; candidates in target/review/candidates/): terrain_orbit_tile_id, terrain_aerial20km_level, terrain_ground200m_height, terrain_cube_corner_normals, terrain_ground1m_depth, terrain_low2km_morph. `/bless` must also remove the names from pending.txt (`cargo xtask check` fails otherwise). Two of them (ground200m height, cube-corner normals) carry little information; say if you prefer other views.
- **Bless the Linux goldens** (adapter `vulkan-llvmpipe-llvm-20-1-2-256-bits`): the four CI candidates are byte-identical to the blessed WARP goldens. They are in the `review-Linux` artifact of the latest CI run on `main` (download with `gh run download <id> -n review-Linux`).
- **Performance on Rasierklinge**: set the Windows power profile to High performance (your first run used Balanced and an idle GPU), connect the charger, then run `cargo xtask perf --machine-ready` (terrain workload at 1440p, four views, 3 runs each, GPU timestamps; takes a minute or two plus mesh generation). Read the verdict lines (GPU median at most 8 ms, frame p99 at most 20 ms) and send me perf/logs/terrain.json; the session is marked invalid if the machine is not ready. Real-GPU candidates (adapter vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu) can be blessed next to the WARP goldens: debug views are byte-identical, three terrain views differ by at most 1 LSB.
- Tool note: `gh` is installed at `C:\Program Files\GitHub CLI\gh.exe` but is not on the Git Bash PATH.

## Queued for agents
- **ADR 0006 review follow-ups** (2026-10-10, owner asked for these to be picked up by the M2 agent; ADR 0006 updated, still proposed):
  1. GPU vertex buffers are keyed by `TileId` alone; `upload` keeps a stale buffer when the mesh changes (`crates/render/src/terrain.rs:115`, `:309`). Key by mesh key or replace on mismatch; test: upload seed 1 then seed 2 for one tile.
  2. Meshes have no key, version or known answers: add `MESH_VERSION`, `mesh_key(height_key, cells, radius bits, MESH_VERSION)` and known-answer hashes for 2-3 meshes (`crates/generators/src/mesh.rs:49`; inputs `radius_m`, `cells`, `SKIRT_FRACTION`, normal stencil, tile origin).
  3. Tie the known-answer tile table to its version: `assert_eq!(GENERATOR_VERSION, 2)` beside `KNOWN_TILE_HASHES` (`crates/generators/tests/pipeline.rs:16`), as `hash_known_answers.rs` does for `HASH_VERSION`.
  4. M3 (not M2): minimal `GenContext`; `replay` rejects a non-zero `definition_hash` once `world-def` exists (`crates/app/src/skeleton.rs:112`).

## Log
- 2026-10-10 m2-seam-normals archived (from the ADR 0003 review): probe measured 0.076 degrees of cross-face normal mismatch, now 0; TileId::corner_neighbors added; partition test independent of from_direction. terrain_cube_corner_normals may be blessed (its WARP candidate is unchanged; the normals view carries little information).
- 2026-10-10 m2-terrain-perf archived (REND-013, TEST-012): persistent terrain renderer with GPU timestamps and a terrain perf workload; first measurement is the owner's run. Observation: planning 3,078 nodes takes about 11 ms of CPU per full recompute (software-adapter smoke, cells 16, tau 4).
- 2026-10-10 m2-real-gpu-findings archived (hello-triangle colour exact on NVIDIA; perf sessions flag Balanced power scheme and idle GPU). Owner real-GPU run: 15 of 16 passed before the fix, 16 of 16 after.
- 2026-10-10 M2 report written (docs/milestones/M2-report.md): go/no-go gate reached; stopped for the owner. Automated acceptance steps pass; timing and real-GPU criteria need Rasierklinge.
- 2026-10-10 m2-camera-graph-instances archived (REND-010, 011, 012 active; SPIKE-01 and SPIKE-02 closed; ADRs 0013, 0014 proposed). Process note: a PR was once merged while its Windows job was still pending (it finished green); merges now wait for explicit green on both jobs.
- 2026-10-10 m2-async-streaming archived (STRM-001, 002, 005, 007 active).
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
