# Tasks: m8-horizon-shadows

Not started; the planner assigns the start. Each step is one PR of at most about a day. The Rasierklinge timings are the architect's window.

- [ ] 0. Decisions: design questions 1 (texture plumbing), 2 (bake cost) and 4 (debug view) with the planner; claim the requirement IDs (GEN-044 to GEN-046, STRM-045, REND-038) when the change starts, since explorer's M8-3 may use numbers in the same range.
- [ ] 1. GEN-044: the horizon bake of a raster with halo (`generators`), tier A on synthetic terrain; indicative CPU timing on a 4 m chunk raster (answers question 2).
- [ ] 2. GEN-045: the coarse horizon from the stored coupled heights; a `TileChannel` kind beside the coupled one (STRM-045 part 1); tier A and the store round trip.
- [ ] 3. GEN-046: the combination and the interpolation twin of the shader (CPU reference, CON-07).
- [ ] 4. STRM-045 part 2: the horizon as a chunk output (key, disk cache, eviction with the chunk, stand-in = coarse only, landing refresh); one registration line in the explorer-owned chunk path, agreed with the planner.
- [ ] 5. REND-038: the horizon textures in the terrain bind group and the shadow term in the lit fragment stage; the shader against the CPU twin (tier C).
- [ ] 6. CON-18: the horizon debug view and stills at sunset and midday on the fixed poses; goldens through `golden-review`.
- [ ] 7. Perf (architect's window): the shadow term's GPU time at 1440p, the chunk job time with the bake; judged against criterion 5 together with M8-6.
- [ ] 8. Spec-guardian, numerics-reviewer (core, generators, shaders), verifier; archive when all requirements pass and the window has recorded the figures.
