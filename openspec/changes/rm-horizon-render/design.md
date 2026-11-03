## Decisions
- Delete first, follow the compiler. The render crate keeps no horizon type; the app keeps only the bake path (PRs 7 and 8 remove it).
- View indices are renumbered, not left with a gap: `ShadeView::index` is the position in `ALL`, and the shaders compare against 5 (shadow) and 6 and above (driver views).
- The shader's shadow view (5) is a plain lit grey; the march's visibility is drawn over it by `draw_visibility` (VIEW-054), as before.
- `MeasureVariant::NoShadow` has been the product source since REND-051 and `Neither` equalled `NoMaterial`; both removed. `shadow-cost` keeps its scene name and its `compiled_out_ms` and `together_ms` fields (read by `accept_m8`).
- The coarse horizon read (`read_coarse`) no longer packs for the GPU (`pack_coarse` went with `horizonview`); PR 8 removes the read.
- The gate's horizon counters stay in `gate.rs` (PR 7); a window run has no horizon jobs now, so they read zero and the "horizon maps baked: 0" rule does not apply.

## Text that stays stale until PRs 7 and 8 (spec-guardian, 2026-11-03)
- TEST-058 (the per-frame horizon step of the viewer and the stills' settling loop) and TEST-065 (`HorizonRig::decide`, the gate's horizon counters): the viewer runs no horizon step and the window gate lands no map; the horizon counters read zero. PR 7 removes TEST-065, TEST-066 and rewords TEST-054; TEST-058's viewer rows go with it.
- GEN-044 and GEN-046 cite REND-038 (removed here); GEN-044, 045, 046, 085 and 086 are removed in PR 8. The code is right, the spec text transitional.
- Comments citing REND-038 and REND-043 (`envelopeperf.rs`, `streamperf.rs`, `generators/horizon.rs`) go with PRs 7 and 8.
- Coverage: the byte-exact `lit_ground_shadow_synthetic` goldens went with `gpu_horizon.rs` (the test produced them); the shadow view's picture is covered by `gpu_shadow_frame` (REND-050, greyness over the surface) and VIEW-054's cycle by the viewer unit tests. A tier-C check that the shadow view shows both lit and shadowed terrain is worth adding before PR 9.
