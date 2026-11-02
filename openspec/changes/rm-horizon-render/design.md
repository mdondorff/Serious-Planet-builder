## Decisions
- Delete first, follow the compiler. The render crate keeps no horizon type; the app keeps only the bake path (PRs 7 and 8 remove it).
- View indices are renumbered, not left with a gap: `ShadeView::index` is the position in `ALL`, and the shaders compare against 5 (shadow) and 6 and above (driver views).
- The shader's shadow view (5) is a plain lit grey; the march's visibility is drawn over it by `draw_visibility` (VIEW-054), as before.
- `MeasureVariant::NoShadow` has been the product source since REND-051 and `Neither` equalled `NoMaterial`; both removed. `shadow-cost` keeps its scene name and its `compiled_out_ms` and `together_ms` fields (read by `accept_m8`).
- The coarse horizon read (`read_coarse`) no longer packs for the GPU (`pack_coarse` went with `horizonview`); PR 8 removes the read.
- The gate's horizon counters stay in `gate.rs` (PR 7); a window run has no horizon jobs now, so they read zero and the "horizon maps baked: 0" rule does not apply.
