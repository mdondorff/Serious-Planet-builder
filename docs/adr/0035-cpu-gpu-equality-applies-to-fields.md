# 0035 CPU/GPU equality applies to fields, not to presentation data
Status: accepted (owner, 2026-10-15, opus-planner session; amends constitution CON-07 and CON-14, v1.3)
Date: 2026-10-15
Source: owner's question of 2026-10-15 on the intent of CPU/GPU equality; SPIKE-06 and SPIKE-07 (`docs/spikes.md`); ADR 0033 (normal encoding); ADR 0013 (instances behind a CPU reference); ADR 0028 (`height(dir)` for collision and picking); constitution CON-05, CON-06, CON-07, CON-11, CON-13, CON-14, CON-18, CON-19

## Context
CON-07 and CON-14 require every GPU output to match a canonical CPU implementation "per field" (heights rel 1e-6 / 1 mm, normals 0.01 degrees, masks 1e-4), but neither defines a field. The rule serves five purposes:
1. cache keys name results, so CPU- and GPU-made tiles are interchangeable and cracks cannot appear at seams (CON-05, CON-06);
2. the surface that collision, picking and authored anchors use (`height(dir)`, ADR 0028) is the surface that is drawn;
3. a seed and a definition give the same world on every machine and adapter (shared worlds, repro bundles, CON-19);
4. a field can move from CPU to GPU for speed without changing the world;
5. agents verify GPU work against a numeric CPU oracle, headless.

None of these applies to data the GPU derives only for display and nothing reads back. Applying the bound there anyway has already cost: SPIKE-06's compact tile needed 16-bit normal components (8.8 KB per tile instead of 4.5 KB, ADR 0033) only to meet 0.01 degrees on a shading normal; SPIKE-07's detail term needed a 300-cell octave rule for the same bound. M5 (atmosphere, FFT ocean waves, foam, shoreline blending, emissive liquids) and ground cover would each need an f64 CPU twin under a literal reading.

## Options (with trade-offs)
1. Literal reading, no change: every GPU output gets a CPU twin. Uniform oracle; doubles the cost of every visual feature and pushes render formats to wider encodings.
2. A reading ADR only ("field" means world data). No constitution change; agents and reviewers read the constitution literally, so the reading would be re-argued.
3. **Amend CON-07 and CON-14 to define a field and carve out presentation data, with a declared bound per attribute** (chosen).

## Decision
1. **A field** is data that a generator produces, that the cache stores or keys, or that CPU systems (collision, picking, placement, anchors, application logic) or other fields read. Fields keep CON-07 and CON-14 unchanged: one implementation per cache, CPU canonical, GPU within the stated bounds, bit-exact CPU across runs, threads and operating systems.
2. **Presentation data** is derived by the GPU only for display: never read back by CPU systems, never stored or keyed as world data, never an input to a field. Examples: shading normals and their stored encoding, the detail term below the finest level, lighting, atmosphere scattering, exposure, water surface motion, foam and shoreline blending, emission, texture blending, wind animation, LOD morphing.
3. **Presentation data still has rules.** Each attribute's spec states its class (field or presentation) and its own precision bound (for example, a stored shading normal within 1 degree), verified by a test (an f64 CPU computation where that is cheap, as a check, not as a second implementation) and by golden images through golden-review (CON-18, ADR 0017). CON-11 (no planet-radius values in shaders) and CON-13 (jitter, vertex quantization) apply in full.
4. **Changing class.** An attribute that becomes an input to a field or to a CPU system (waves that carry boats, snow depth that takes footprints, grass that becomes collidable trees) needs a CPU reference and an ADR before the change. Reviewers check that a declared bound follows from the attribute's purpose, not from the measured result.
5. **Constitution text (v1.3):** CON-07 gains the definition of a field; CON-14 states that its CPU/GPU bounds apply to fields, that presentation data declares its own bound and keeps CON-11 and CON-13, and the class-change rule.

## Consequences
- Easier: GPU-only visual features (M5 atmosphere and ocean, lava glow, ground cover animation) need no CPU twin. The compact tile's normal encoding may return to octahedral snorm8 (4.5 KB per tile) if its look is acceptable; decided after the first M4 perf window (GPU time at P0) and a look check by the owner, with the debug golden re-blessed.
- Harder: presentation attributes lose the uniform CPU oracle; each needs a sensible declared bound, and reviewers must watch that bounds are not fitted to results.
- Unchanged: world data, cache keys (render formats are covered by `MESH_VERSION`), cross-machine determinism of the world, collision and picking through `height(dir)`.
- Specs: render-side specs (rendering, the compact tile format GEN-019 and REND-016) state the class and bound of each attribute they produce, when next touched; spec-guardian checks new attributes for a declared class.

## Licences of adopted code or techniques
None.
