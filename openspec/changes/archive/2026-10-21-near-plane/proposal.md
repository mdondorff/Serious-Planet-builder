# Change: near-plane

## Why
- M7 G0 (ground detail, Q11): a first-person walk and a crouch: the eye about 0.5 m above the ground and the near plane about 5 cm, over the finest tiles, with no z-fighting and no clipping into the ground, and the depth precision measured.

## What changes
- `freecam::FREE_NEAR_M` (0.05 m) and `FREE_EYE_CLEARANCE_M` (0.5 m); the viewer's free camera and the camera of a click flight's landing use them (the scripted flight, goldens and perf scenes keep 0.1 m and the controller's clearance).
- VIEW-004 (text): the clearance, the near plane, the depth resolution.

## Tests
- `height_aware.rs`: a crouched eye (0.5 m, near 0.05 m, horizon and down) stays above the drawn surface by the near plane and no drawn surface comes nearer, on the reference and the steep field (LOD-008's check on the free camera); the depth step of the f32 reversed-Z value from the camera's own projection: 7e-8 m at 1 m, 1.5e-5 m at 100 m, 1e-4 m at 1 km.

Expected golden changes: none (the scripted flight keeps its near plane).
