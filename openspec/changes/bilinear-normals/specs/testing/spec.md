## ADDED Requirements
### Requirement: TEST-153 The lit shading against the cell diagonal through the real renderer
A tier-C test SHALL render a smooth scarp through the centre of face 0 through the compact renderer's lit path, at 45 and at 135 degrees to the tile grid, each lit by a sun 10 degrees high across it, and compare the 45-degree picture with the 135-degree one turned back by a quarter turn.

Verify: C
Status: active
Source: Lead 2026-11-24 (#200: the evidence row through the real renderer, before and after), REND-068
Detail:
| Item | Value |
|---|---|
| field | `30 tanh(d / 60 m)`, `d` the distance from the line through the face centre, in face 0's gnomonic plane |
| view | a nadir camera at 530 m with its up on face 0's v axis, 256 x 256 px, levels capped at 14 (cells of about 38 m, about 16 px), fixed exposure EV 11, no ground detail |
| measure | the absolute luminance difference over the picture's central half |
| bounds | mean at most 0.3 and p99 at most 5 (0-255); with the vertex normals: mean 1.94, p99 22.6 (WARP, 2026-11-24) |
- At the face centre the equiangular grid, the camera and the field are symmetric under the quarter turn, which maps each cell's `a-c` diagonal to `b-d`: a difference measures the triangulation's direction in the shading.
- The pictures and the difference go to `target/review/quad-split/`.

#### Scenario: The bias is gone
- **WHEN** the test runs on the software adapter
- **THEN** the difference is within the bounds, and the scarp shows (a lit and a shaded side)
