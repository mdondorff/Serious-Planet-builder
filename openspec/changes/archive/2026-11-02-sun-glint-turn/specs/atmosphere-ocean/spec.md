## MODIFIED Requirements

### Requirement: ATMO-013 Gerstner waves with phases reduced on the CPU
The waves SHALL be plane waves in the planet-fixed frame (ADR 0041 point 5): each liquid has up to four, with the wave vectors `k_i = 2 pi / lambda_i x D_i` (the directions the vertices of a regular tetrahedron, wavelengths 1, 0.62, 0.38 and 0.23 of the liquid's, amplitudes 1, 0.55, 0.3 and 0.15 of its wave height, dispersion `omega = sqrt(g k) / (1 + viscosity)`); the CPU SHALL reduce `(k_i . O + phi_i - omega_i t) mod 2 pi` in f64 per tile origin `O` and plan time `t` (the injected clock, CON-04) and the shader SHALL add only `k_i . local`, `local` the tile-local position of the surface point, so a planet-sized number meets a wave vector nowhere in a shader (CON-11). Two tiles that hold one point therefore agree about its phase, across face edges too (no per-tile tangent projection). A wave's weight is the product of four continuous factors: its tangent share (a smoothstep from 0.25 to 0.6 of the length of its direction's part along the surface, so a wave pointing out of the surface has none), its resolution (a smoothstep from four to eight vertices per wavelength at the vertex's own spacing, the node's sample spacing times one plus the vertex's morph factor, so a wave the grid cannot hold fades out continuously across LOD borders; in the vertex stage only, so the shading normal does not jump at a stand-in tile's border) the pixel footprint (a smoothstep from two to four pixels per wavelength, `PIXEL_FADE`, of the wavelength over the size of the pixel on the surface, the larger of the screen-axis derivatives of the tile-local position, in the fragment stage only: a wave under about two pixels per wavelength fades out of the shading instead of aliasing; the vertex stage has no footprint and no fade, ADR 0056) and the shore (a smoothstep from 0 to four times the sum of the amplitudes of the depth, so the surface never dips below the terrain where the water is thin: the vertical displacement is at most half the depth). Liquids with no wave height (lava) have no waves. Class: presentation data (ADR 0035; no GPU twin, never stored or keyed). Bound: displacement and slope against the f64 reference on the same f32-rounded inputs and CPU-reduced phases within `1 mm + 1e-3 x (sum of the amplitudes)` per component (ATMO-014), with the footprint included (footprints from 2 cm to 500 m, tier B); the largest `|k . local|` of a wave with weight over the scripted views from orbit to 1 m is asserted below 250 rad against a limit of 1,000, and by construction it is at most about 1.3 cells (so the f32 argument keeps its digits).

Verify: A
Status: active
Source: ADR 0041 point 5, ADR 0043, CON-11, CON-14

#### Scenario: Known answers
- **WHEN** the tile phase of a wave of `k = 0.1` rad/m at a planet-sized origin is reduced at t = 0, 1.5 s and 1e6 s
- **THEN** it equals the 50-digit value within 1e-8 rad, lies in `[0, 2 pi)`, and repeats after a thousand periods within 1e-6

#### Scenario: Seams
- **WHEN** a point of the shared border of two tiles, also across a face edge, is evaluated from each tile's origin with f32-rounded local numbers
- **THEN** displacement agrees within the bound and slope within 1e-3

#### Scenario: A tile's grid does not shape the normal
- **WHEN** the sea of one plan is drawn twice in the normal view, the second time with every other node's sample spacing 64 times larger (stand-in tiles of a turn beside finer ones)
- **THEN** the mean difference of the shading normal over the water pixels is under 0.004 (0.0130 before, 0.0020 after: the rest is the displaced geometry)

#### Scenario: Pixel footprint
- **WHEN** the weights are taken at footprints from a fraction of a wavelength to many wavelengths
- **THEN** a wave is whole from four pixels per wavelength, gone under two, monotone and continuous between, and a footprint of 0 changes nothing; the shader agrees with the reference, and a far grazing pixel (115 m along the view) has no wave left

#### Scenario: Weights and the shore
- **WHEN** the weights are taken at spacings around four to eight vertices per wavelength, under an up that is a wave's own direction, and at depths from 1 mm to 40 m
- **THEN** they are continuous, a wave on a grid of fewer than four vertices per wavelength or out of the surface has none, and the vertical displacement never exceeds half the depth

#### Scenario: Known limit of the waves (ADR 0053)
- **WHEN** a liquid has a sum of `choppy x k x amplitude` above 1
- **THEN** its surface can fold; the Earth row is far below 1
