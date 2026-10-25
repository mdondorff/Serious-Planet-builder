# Tasks: m8-horizon-shadows

Step 1 landed with the first PR; the rest follows. Each step is one PR of at most about a day. The Rasierklinge timings are the architect's window.

- [x] 0. Decisions recorded in design.md (planner, 2026-10-24); requirement IDs GEN-044 to GEN-046, STRM-045, REND-038 are this change's.
- [x] 1. GEN-044: the horizon bake of a raster with halo (`generators::horizon`), tier A on synthetic terrain; indicative CPU timing in design.md.
- [x] 2a. GEN-045: the coarse horizon from the coupled heights (`generators::coarsehorizon`), tier A on a small body.
- [x] 2b. STRM-045: the coarse horizon as a `TileChannel` of kind 9 beside the coupled one (`cache::horizon_store`: codec, key with the march and the coupled run, round trip). Loaded or made with the world: with the chunk wiring (step 4).
- [x] 3. GEN-046: the combination, the interpolation, the soft edge and the azimuth in each frame (`generators::horizon_lookup`, the CPU reference of the shader).
- [x] 4a. STRM-048: the horizon job kind (`streaming::chunks::HorizonJobs`: a second job after the heights, state word at dispatch, settle window, eviction, disk trait), tier A with a fake baker.
- [x] 4b. STRM-049: the world wiring (`app::horizonworld`: baker from the world's field, `enable_horizon`, `horizon_frame`, `horizon_map`), flight test with the re-bake count. The pack-backed disk is STRM-050 (planned; it needs a record kind of its own: coordinate the number with explorer).
- [x] 5a. REND-038 part (a) (REND-041): `HorizonFrame` and the per-node table builder in `frame` (`frame::horizonview`, design.md "Render design" point 1, 8a).
- [x] 5b. REND-038 part (b): the render plumbing, bindings 4 to 7, dummies, `set_horizon`, uploads; frames identical with an all -1 table. (`render::horizon`, `TerrainRenderer::set_horizon`; `gpu_horizon`)
- [x] 5c. REND-038 part (c): the shader term and its test against the CPU reference. (`render::horizon_wgsl`, `probe_horizon`, `frame::horizonview::pack_coarse`, `set_horizon_coarse`; `gpu_horizon`)
- [ ] 6. REND-038 part (d), CON-18: `ShadeView::Horizon` / `Shadow`, the viewer call sites, stills at sunset and midday on the fixed poses; goldens through `golden-review`.
- [ ] 7. Perf (architect's window): the shadow term's GPU time at 1440p, the chunk job time with the bake; judged against criterion 5 together with M8-6.
- [ ] 8. Spec-guardian, numerics-reviewer (core, generators, shaders), verifier; archive when all requirements pass and the window has recorded the figures.
