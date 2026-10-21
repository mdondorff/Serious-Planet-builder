## ADDED Requirements

### Requirement: REND-035 Specular term of the ground
With the ground detail on (REND-034, strength above 0) the lit fragment stages SHALL add the sun's specular reflection to the detailed ground: GGX distribution with Smith-Schlick visibility and Schlick Fresnel of the plan's `f0` (LOOK-010), times the cosine, with the roughness of the point from the plan's gloss table and the same weights as the detail (LOOK-008), scaled by the strength. The sun's illuminance is one unit without an atmosphere and the transmitted, shadowed sun with one. The term SHALL be added to the lit view only (not the albedo, lighting or climate views), SHALL never darken a pixel, and SHALL be absent when the strength is 0 or no material has weight, so a picture with the detail off is what it was bit for bit. Presentation only (ADR 0028, ADR 0035).

Verify: C
Status: active
Source: M7 G2b (opus-planner, 2026-10-21; owner: roughness and shine are in M7), REND-034, LOOK-010

#### Scenario: The highlight
- **WHEN** a ground-level scene is drawn with the detail on and `f0` 0, with the default `f0`, with a smooth and with a rough table, with the detail off and `f0` 1, and in the albedo view with `f0` 1
- **THEN** no channel of the default-`f0` picture is below the `f0` 0 picture and some pixels are brighter; the smooth and the rough tables give pictures that differ from each other and from the default; the detail-off picture equals the one with `f0` 0 and the albedo view equals its picture with `f0` 0
