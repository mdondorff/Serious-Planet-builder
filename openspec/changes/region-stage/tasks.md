## Tasks
1. [ ] world-def: the surface table, `StampKind::Region`, params, `mask_layer`, put refusals, inside test, footprint rows incl. mode 1 (WDEF-012); tests first.
2. [ ] generators: the mask factor in `apply_stamps` and the bounds (GEN-029); tests first (inside/outside, other layers, several regions, bounds).
3. [ ] contract: pass-through, recorded footprint, re-key only the clipped layer's tiles.
4. [ ] frame: `Overlay::Regions`, the plan's polygon list and text (VIEW-030); tests first (round trip, hash, order).
5. [ ] render: the lit fragment tint (analytic), F3 cycle in the editor, tier C images in `target/review/regions/`; no golden.
6. [ ] numerics-reviewer (generators, shader), spec-guardian, verifier; archive.
