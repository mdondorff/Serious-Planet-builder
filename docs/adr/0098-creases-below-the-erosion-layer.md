# 0098 Creases only below the erosion layer's grid; the layer owns erosion at its scales
Status: accepted (owner, 2026-11-03: "creases like above are better suited for lower scale noise and the erosion term needs to be having control over the erosion effects at its scale"; "Creases then become however very similar to the 'fine detail' term"); the fade band of point 2 and the coast blend of point 3 are start values; points 3 and the version consequence amended 2026-11-03 (GEN-109 reviews, #1143)
Date: 2026-11-03
Source: owner, 2026-11-03; ADR 0088 points 2 and 5; GEN-060, GEN-061, GEN-099; erosion-bake preview and diagnosis (explorer, 2026-11-03)

## Context
- ADR 0088 point 2 kept the analytic creases in the runtime base: `m + k (e - m)` plus `k` times the creases, and point 5 adds the field's octaves and the shader detail below the layer's cells. The creases were the field's stand-in for erosion before the bake existed.
- The crease octaves run from 25 km down to about 12 m (12 octaves, halving); the erosion layer covers about 0.2-10 km (pass 1 on 814 m cells, pass 2 on 101.8 m cells).
- The bake carves a base without the creases, so at runtime the creases would add a second, unrelated valley pattern over the layer's drainage, amplified by `k` (5-7.5 in hills and mountains).
- ADR 0088 point 5 states only that the field's octaves apply below the layer's cells; it does not say what happens to the crease octaves above them.

## Decision
1. Where the erosion layer exists, the layer alone shapes erosion at its scales; there is no separate crease term above the layer's grid.
2. The crease octaves of wavelength at most 2 layer cells (about 200 m) are the fine-detail term of ADR 0088 point 5 (one term, not two); octaves above 4 cells (about 400 m) are dropped; between, a smoothstep fade in log wavelength. The band is a parameter of the field, tuned on the preview.
3. Where no layer is read (the relief switch off, a world without a layer, the sea floor below the definition's sea level), the creases stay as today; above the sea level the two join over a blend band (about 200 m of elevation), so no ledge forms along the shore.
4. The bounds of point 3 of ADR 0088 take the crease bound of the remaining octaves only.

## Consequences
- Amends ADR 0088 points 2 and 5. The base the bake carves does not change, so no bake has to be rerun for this.
- The implementation id and inputs hash change when the band is on (the runtime layer read switches it on); the field's version does not; without the band the field is unchanged bit for bit, so no golden moves.
- Whether the remaining fine creases keep the factor `k` is decided on the preview stills (open point).
- The spec text is a new generator requirement (GEN-109) in the implementing change.
