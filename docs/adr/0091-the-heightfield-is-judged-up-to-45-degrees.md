# 0091 The heightfield is judged up to 45 degrees; steeper faces are a separate technology
Status: accepted (owner, 2026-11-01; drafted by opus-planner from SPIKE-85)
Date: 2026-11-01
Source: owner, 2026-11-01 ("cliffs and steep mountains are not suitable for this algorithm of terrain LOD"); SPIKE-85 (`spikes/m9-slope-ceiling/results/results.md`); ADR 0089; TEST-081

## Context
- A heightfield with LOD cannot render cliffs, overhangs or near-vertical faces well; this holds by design, not by tuning.
- GLO-30 at 30 m: the share of area above 45° is 4.3 % in the Alps, 0.1 % in the Highlands and the Black Forest, 0 in Valley and Ridge (8.7 % at Everest).
- A 45° limit keeps 0.997-1.000 of the 1 km relief p50 at all four criterion-1 sites; 35° keeps 0.985 in the Alps.
- The faces above 45° hold 14 % of the Alps' total ascent: visible, but not judged by criterion 1.

## Decision
1. **Realism targets of the heightfield stop at 45°.** Slope statistics that judge the heightfield (criterion 1's slope p50, TEST-081) count only cells at or below 45° on both the generated and the real side. The value lives in the testing spec.
2. **Steeper faces are a separate technology**, not a heightfield target: in order, steep-face texturing and displacement in the heightfield's shading, then instanced cliff meshes; voxel patches only for caves and overhangs. Each needs its own decision.
3. **The steep share is a coverage goal** for that technology: the share of area above 45° per landform, measured against the real tiles, once the technology exists.

## Consequences
- TEST-081's slope p50 changes (the Alps' real p50 about x0.95); the relief ratio, TEST-082 to TEST-084 and GEN-096 to GEN-098 do not.
- No work on optimising the steepest parts of the heightfield (more LOD, steeper amplitude) to meet slope targets.
- The cliff technology is not scheduled; it enters a milestone by its own ADR.

## Licences of adopted code or techniques
None.
