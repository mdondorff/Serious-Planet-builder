## ADDED Requirements

### Requirement: REND-043 The coarse horizon fades out with distance
The elevation the coarse horizon map gives SHALL be multiplied by a strength that is 1 up to 1,000 km from the camera to the surface point, falls as a smoothstep to 0 at 3,000 km and stays 0 beyond (`planet_generators::horizon::coarse_fade`; the shader's `horizon_coarse_fade` is the same function); the chunk horizon maps SHALL NOT be faded. The probe of the shader SHALL take the distance.

Verify: A, C
Status: active
Source: REND-038, GEN-045, the owner's frame of 2026-10-25 (dark spots near the terminator at 3,904 km)

#### Scenario: Near and far
- **WHEN** a coarse map with a horizon of 10 degrees is probed for a sun at 5 degrees at distances from 0 to 6,000 km
- **THEN** the factor is 0 near, 1 far, monotone between, and equal to the CPU reference with `coarse_fade` within 0.02
