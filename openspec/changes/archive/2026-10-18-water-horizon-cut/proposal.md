## Why
Since the coastal band (ADR 0056) every node of the plan near a coast carries water: the water pass draws 2.9 to 3.7 times the nodes of window 6 for the same sea pixels (docs/perf/water-attribution.md). Most of the added nodes lie beyond the horizon of the sea or over terrain above the level, and the shader floods nowhere on them.

## What Changes
- The plan leaves out of the water node list the nodes that cannot show water from the camera: beyond the horizon of the tile's own level (with the waves' rise), or over terrain whose lowest point is at or above the level plus the rise (ATMO-020).
- The cut is made where the plan assembles the node list. The tile's answer, the `WaterBook`, every key and the stored data are unchanged (no `MESH_VERSION` or `GLOBAL_VERSION` change).
- The viewer and the lit perf scenes hand the plan the terrain's height field; a request without one cuts nothing.

## Impact
Spec: ATMO-020 added (atmosphere-ocean). Plan data only: no shader, pipeline or golden change; the picture is pixel-identical. Refinement within ADR 0056, no ADR. The shore filter and occlusion by terrain are out of scope.
