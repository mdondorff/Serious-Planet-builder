# Tasks: m8-horizon-shadows

Step 1 landed with the first PR; the rest follows. Each step is one PR of at most about a day. The Rasierklinge timings are the architect's window.

- [x] 0. Decisions recorded in design.md (planner, 2026-10-24); requirement IDs GEN-044 to GEN-046, STRM-045, REND-038 are this change's.
- [x] 1. GEN-044: the horizon bake of a raster with halo (`generators::horizon`), tier A on synthetic terrain; indicative CPU timing in design.md.
- [x] 2a. GEN-045: the coarse horizon from the coupled heights (`generators::coarsehorizon`), tier A on a small body.
- [x] 2b. STRM-045: the coarse horizon as a `TileChannel` of kind 9 beside the coupled one (`cache::horizon_store`: codec, key with the march and the coupled run, round trip). Loaded or made with the world: with the chunk wiring (step 4).
- [x] 3. GEN-046: the combination, the interpolation, the soft edge and the azimuth in each frame (`generators::horizon_lookup`, the CPU reference of the shader).
- [x] 4a. STRM-048: the horizon job kind (`streaming::chunks::HorizonJobs`: a second job after the heights, state word at dispatch, settle window, eviction, disk trait), tier A with a fake baker.
- [ ] 4b. STRM-049: the app baker (chunk field heights over chunk + halo to `bake_horizon`), the pack-backed disk, the landing refresh; a flight test with the re-bake count.
- [ ] 5. REND-038: the horizon textures in the terrain bind group and the shadow term in the lit fragment stage; the shader against the CPU twin (tier C).
- [ ] 6. CON-18: the horizon debug view and stills at sunset and midday on the fixed poses; goldens through `golden-review`.
- [ ] 7. Perf (architect's window): the shadow term's GPU time at 1440p, the chunk job time with the bake; judged against criterion 5 together with M8-6.
- [ ] 8. Spec-guardian, numerics-reviewer (core, generators, shaders), verifier; archive when all requirements pass and the window has recorded the figures.
