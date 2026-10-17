## Why
Two tier-B/C tests are flaky by slowness on the Windows CI runner (nextest timeout 60 s x 3 = 180 s): `oracle_frustum_cut no_tile_with_a_visible_triangle_is_cut_in_the_standard_views` (about 27 s on 20 cores locally, 180 s timeouts on CI in F11, #209 and twice on #210) and `gpu_window` (about 60 s locally for its two smokes, 73 s on WARP, one 180 s timeout under load). A test that times out says nothing about the code, and every retry costs a CI cycle.

## What Changes
- LOD-007 oracle (test support only): the depth oracle projected every tile through `tile_mesh_field`, which evaluates the height field five times per vertex for normals plus climate and a mesh key, none of which the oracle reads. It now computes only the vertex positions (same arithmetic, one height per lattice point), and a self-check inside the test asserts the positions and origin equal the real mesh's bit for bit for three tiles of every view. The depth pass and the ownership pass, serial before, run on all cores (per-thread depth buffers merged by per-pixel maximum, which does not depend on thread count or triangle order). Same views, same 1280x720 depth pass, same assertions.
- VIEW-005 `gpu_window`: its two smokes (default look; a moonlit look file) are separate processes that judge frame counts, never wall-clock; they now run at the same time instead of one after the other. Same assertions, same frame counts.
- No change to requirements, tiers, tolerances or resolutions. No spec delta.

## Impact
Tests only (`crates/frame/tests/oracle_frustum_cut.rs`, `crates/frame/tests/support/depth_oracle.rs`, `crates/app/tests/gpu_window.rs`). Before/after times are in the log line.
