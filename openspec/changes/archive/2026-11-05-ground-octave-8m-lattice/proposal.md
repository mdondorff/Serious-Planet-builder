## Why
- The look sheet of slice 2a (2026-11-05) showed a regular diamond pattern on the steep cone with the 8 m octave on (rock tilt 0.18): the octave's lattice was axis-aligned (hash salt 7), and the gradient of smoothstep value noise vanishes along lattice lines, so an axis-aligned lattice shows as a grid of diamonds on a steep flank (inbox #64).

## What Changes
- The 8 m octave takes hash salt 6, the turned lattice (the even salts turn it by 45 degrees, `gd_shear`; the 16 m octave is turned, the 2 m one is not): the octave no longer lines up its grid with the axis-aligned 2 m octave (a factor 4) and with the 8 m patch octave (the same lattice), and the pattern reads as irregular rock. A turned lattice is as regular as an axis-aligned one; what changed is which lattices it coincides with. Same sheet, before and after: `docs/dev_screenshots/2026-11-05-ground-octave-8m-lattice-before-after.jpg`.
- Nothing else moves: the octave is still 0 in the Earth default and skipped, so no golden and no plan text changes (REND-057 does not name a salt).

## Impact
- Code: `render` (terrain shader, one argument). The 8 m octave's lattice now nests with the 16 m one at a factor 2 (both turned, sides 5.7 m and 11.3 m), instead of the 2 m one at a factor 4 (both axis-aligned); the sheet shows no pattern from it at the starting coefficients.
- Open trade-off (numerics review): the 8 m lattice now nests with the 16 m one at a factor 2, so a look with strong 16 m and 8 m rock tilts and little 2 m and 64 m tilt could show diagonal lines 11.3 m apart under a low sun; a tier-A guard on the salt parity of octaves within a factor 2 of each other is a candidate for the tuning pass.
