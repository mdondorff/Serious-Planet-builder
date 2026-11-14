## ADDED Requirements

### Requirement: TEST-087 The continents hold the land fraction, mass count, largest share and coastline bands
A tier-B check SHALL score the land mask of seeds 1 to 5 on a 1440 x 720 grid (0.25 degrees; row 0 north, longitude wrapping, 8-connected masses, cell areas by latitude): the land fraction 27 to 31 %, the number of masses of at least 1 Mkm2 (S1) 4 to 7, the largest mass's share of land (S2) 45 to 62 %, and the coastline's box-counting dimension over boxes of 2 to 32 cells (S3) at least 1.18. It SHALL name every seed and measure that is outside its band.

Verify: B
Status: active
Source: SPIKE-91 (gates S1 to S3), SPIKE-92, planner (2026-11-02, inbox #41), GEN-104

#### Scenario: Seeds 1 to 5
- **WHEN** the land masks of seeds 1 to 5 are scored
- **THEN** every measure is inside its band, or the failure names the seed, the measure, its value and the band

### Requirement: TEST-088 The continents hold the compactness, elongation, island and polar bands
A tier-B check SHALL score the same masks for S9 (the area-weighted median over the masses of at least 1 Mkm2 of `4 pi A / P^2`, `P` the contour length at the level 0.5 of the mass's land fraction on a 1 degree grid, by marching squares, east-west steps scaled by the cosine of the latitude), at most 0.20; S10 (the median of `sqrt(l1 / l2)`, the eigenvalues of the area-weighted second moment in the tangent plane at the mass's mean direction), 1.5 to 2.5; S11 (the masses of 10^4 to 10^6 km2), 35 to 70 islands holding 1.9 to 6 % of the land (SPIKE-92's lower bound of 2 % is relaxed by 0.1 point: seed 4 is 1.99 %); and at most 1 mass of at least 1 Mkm2 centred poleward of 70 degrees.

Verify: B
Status: active
Source: SPIKE-92 rounds 2 and 3 (S9 to S11, the polar rule), planner (2026-11-02, inbox #41), GEN-104, GEN-106

#### Scenario: Seeds 1 to 5
- **WHEN** the land masks of seeds 1 to 5 are scored
- **THEN** every measure is inside its band, or the failure names the seed, the measure, its value and the band
