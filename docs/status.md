# Status log

Newest entries at the top. One line per event (change archived, ADR drafted, spike opened or closed, quarantine, milestone report). Stop reports get a short section.

## Waiting for the owner
- **ADR 0004 seam wording** (from the m2-wgpu-seam review): render's public API is now the seam to wgpu (enforced by `xtask layering`), but ADR 0004 also describes an internal device/buffer/pipeline interface inside render, due 'at the first real pass (M2)'. Decide: revise ADR 0004 (proposed) so the seam is render's public API and the internal interface waits for a second backend, or schedule the interface. Also decide who owns the window surface and egui painting (an egui-wgpu integration in editor would pull wgpu above render).
- **M2 go/no-go gate** (docs/milestones/M2-report.md): recommendation Go with changes. Nothing in M3 has been started.
- **Accept or change the proposed ADRs** 0007, 0009, 0013 and 0014 (ADRs 0001-0006, 0008, 0010-0012 accepted 2026-10-10). Work proceeds on their leanings.
- **Bless the six terrain goldens** (adapter dx12 WARP; listed in tests/goldens/pending.txt; candidates in target/review/candidates/): terrain_orbit_tile_id, terrain_aerial20km_level, terrain_ground200m_height, terrain_cube_corner_normals, terrain_ground1m_depth, terrain_low2km_morph. `/bless` must also remove the names from pending.txt (`cargo xtask check` fails otherwise). Two of them (ground200m height, cube-corner normals) carry little information; say if you prefer other views.
- **Bless the Linux goldens** (adapter `vulkan-llvmpipe-llvm-20-1-2-256-bits`): the four CI candidates are byte-identical to the blessed WARP goldens. They are in the `review-Linux` artifact of the latest CI run on `main` (download with `gh run download <id> -n review-Linux`).
- **Performance on Rasierklinge**: set the Windows power profile to High performance (your first run used Balanced and an idle GPU), connect the charger, then run `cargo xtask perf --machine-ready` (terrain workload at 1440p, four views, 3 runs each, GPU timestamps; takes a minute or two plus mesh generation). Read the verdict lines (GPU median at most 8 ms, frame p99 at most 20 ms) and send me perf/logs/terrain.json; the session is marked invalid if the machine is not ready. Real-GPU candidates (adapter vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu) can be blessed next to the WARP goldens: debug views are byte-identical, three terrain views differ by at most 1 LSB.
- Tool note: `gh` is installed at `C:\Program Files\GitHub CLI\gh.exe` but is not on the Git Bash PATH.

## Queued for agents
- **ADR 0006 review follow-ups** (items 1-3 DONE in m2-mesh-keys, 2026-10-10; only item 4 remains, for M3; owner asked for these to be picked up by the M2 agent; ADR 0006 accepted):
  1. GPU vertex buffers are keyed by `TileId` alone; `upload` keeps a stale buffer when the mesh changes (`crates/render/src/terrain.rs:115`, `:309`). Key by mesh key or replace on mismatch; test: upload seed 1 then seed 2 for one tile.
  2. Meshes have no key, version or known answers: add `MESH_VERSION`, `mesh_key(height_key, cells, radius bits, MESH_VERSION)` and known-answer hashes for 2-3 meshes (`crates/generators/src/mesh.rs:49`; inputs `radius_m`, `cells`, `SKIRT_FRACTION`, normal stencil, tile origin).
  3. Tie the known-answer tile table to its version: `assert_eq!(GENERATOR_VERSION, 2)` beside `KNOWN_TILE_HASHES` (`crates/generators/tests/pipeline.rs:16`), as `hash_known_answers.rs` does for `HASH_VERSION`.
  4. M3 (not M2): minimal `GenContext`; `replay` rejects a non-zero `definition_hash` once `world-def` exists (`crates/app/src/skeleton.rs:112`).

- **ADR 0010 review follow-ups** (2026-10-10, owner asked; ADR 0010 accepted):
  1. Enforce the platform-maths ban with clippy `disallowed-methods` in `clippy.toml` (all `f64::`/`f32::` transcendentals plus `powi`, `powf`, `mul_add`, `sin_cos`, `exp_m1`, `ln_1p`, the hyperbolic family) over the whole workspace; keep or retire `crates/core/tests/no_platform_math.rs` (today it misses `f64::powi(`, `f64::mul_add(`, `.map(f64::sin)`, `<f64>::sin(`, a space before `(`, and does not scan `app`). Sent to the M2 agent 2026-10-10.
- **ADR 0004 review follow-up** (2026-10-10, owner asked; sent to the M2 agent): render's public API is the seam to wgpu. Remove `pub use wgpu` (`crates/render/src/lib.rs:6`), make the `GpuContext` fields private with accessors for adapter facts (used by `facts()` and the adapter log line in `crates/app/src/lib.rs`), and extend `cargo xtask layering` so only `render` depends on wgpu. Goldens stay byte-identical.

## Log
- 2026-10-10 m2-golden-adapter-family archived (TEST-002 modified; ADR 0009 follow-up 1): golden keys by adapter family (vulkan-llvmpipe), driver.txt beside every golden directory and printed on mismatch, CI requires adapter goldens, runner pinned to ubuntu-24.04 (Mesa package not pinned: runner apt index; Mesa 25.2.8 recorded). Linux goldens moved with git mv, byte-identical. ADR 0009 follow-ups 2-6 still open.
- 2026-10-10 m2-wgpu-seam archived (BUILD-001 modified): render no longer re-exports wgpu, GpuContext fields private, accessors, layering bans graphics-API crates outside render (table form, quoted keys, renames) and wgpu types in render's pub signatures. Goldens byte-identical.
- 2026-10-10 ADR 0004 updated from review and accepted by the owner: render's public API is the seam (no internal HAL), M2 gate outcome recorded (wgpu kept), wgpu upgrades keep goldens byte-identical, DX12 and Vulkan only.
- 2026-10-10 ADRs 0005, 0006, 0008, 0010, 0011, 0012 accepted by the owner; CON-14 amended (constitution v1.1, per-field CPU/GPU bounds); con14-tolerance archived (GEN-008 modified).
- 2026-10-10 m2-clippy-math-ban archived (COORD-009 modified): clippy disallowed-methods over the workspace, verifier probed 24 forms/locations. Known gap: third-party float traits (num_traits::Float, glam, nalgebra) are not seen by clippy; a layering rule or dependency review must keep them out of generator crates (none present today). ADR 0010 follow-up 1 done.
- 2026-10-10 det-manifest archived (GEN-003 modified): Windows/Linux determinism manifest and `cargo xtask det-diff`; CI `determinism` job. ADR 0003 accepted; ADR 0010 updated with a proposed CON-14 tolerance amendment.
- 2026-10-10 m2-key-aware-residency archived (REND-013 Stale mesh): plan draws carry mesh_key, render() fails with StaleMesh on a mismatch, has_mesh replaces has_tile. Known gap: no test changes the seed on a live app renderer (the editor will); StaleMesh makes a regression loud instead of silent.
- 2026-10-10 m2-mesh-keys archived (GEN-010, REND-013): MESH_VERSION 2, mesh_key, known-answer mesh hashes, renderer replaces buffers whose mesh key differs, GENERATOR_VERSION asserted beside KNOWN_TILE_HASHES. Open note: perf_terrain decides which meshes to generate by has_tile (ignores the key); fine while the seed is fixed per run.
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
- 2026-10-11 m2-readable-terrain-views archived (REND-006): depth bands, height contours, morph blue-red ramp, normals golden from 3,000 km above the cube corner; height golden renamed terrain_aerial200km_height. Four reworked candidates await /bless.
- 2026-10-11 m2-contour-artifacts archived: height contours fade per decade over a 2.5-octave window (no per-triangle switching); terrain_aerial200km_height candidate regenerated.
