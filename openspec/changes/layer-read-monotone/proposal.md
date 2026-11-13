## Why
- The owner's flight of 2026-11-13 (F9 frame-13715, over the R3a commit) showed a straight terrain step that appears and disappears along one line.
- Diagnosis (spikes lane, order #108): the step is in the runtime read of the erosion layer, not in the drawing. `ErosionLayer::read_face` clamps its 2D Catmull-Rom result to the range of the four cells around the point; the four change at every cell-centre line, so where the curve along the line overshoots on one side only, the read steps.
- On r3a-e1, 20,000 random crossings of cell-centre lines: 1.5 % of crossings jump more than 0.1 m, up to 12.5 m; half a cell off the lines, and with the clamp off, at most 0.001 m.

## What changes
- GEN-188: the layer read is a monotone C1 cubic per axis (Hermite with harmonic-mean slopes, Fritsch-Butland), rows first, then across the rows. It never overshoots, so no clamp; it is continuous with its first derivative across every cell-centre line; at a cell centre it is the stored sample bit for bit (a position within 1e-9 cells of a centre is the centre).
- `LAYER_READ_VERSION` 1 to 2: the keys of a field that reads a layer change.
- The range over a cap (GEN-102) is unchanged: the read stays within the four cells around the point, as before.

## Not in this change
- The bake's twin reads: `map::change_in_face` and K7's regional field (`k7.rs`) clamp a Catmull-Rom read to the four cells the same way, so the baked data may carry the same steps along the regional cell-centre lines. Changing them changes every store's bits; reported to the lead (order #110), not changed here.
- The edge blend at cube edges (`EDGE_BLEND_CELLS`), the coast fade (GEN-113) and the pyramid ranges.

## Impact
generators (erosion_layer.rs). Spec: generator-pipeline. Tests: tier A (`runtime_layer.rs`). Lit goldens drawn over a layer can move (a declared change, golden-review).
