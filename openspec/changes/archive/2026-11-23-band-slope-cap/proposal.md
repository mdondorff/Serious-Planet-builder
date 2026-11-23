## Why
- SPIKE-109 (2026-11-12): the start surface z0 holds closed basins over 30 cells on 32 / 22 / 22 % of the land of R1h / R3a / R3b against 3.7 % in the DEM pool; the class band is the largest component (whole band off: 12.2 / 11.8 / 10.9 %), and the 8 km hill octave alone holds 4.5 to 6.9 points.
- Round 1 cuts these basins' outlet paths: the 5.001 m cells and G-PROFILE's sub-sea rises follow (correlation 0.77 / 0.71). Architect, 2026-11-12: the fix is at the source (ADR 0118 point 4), switched off until previewed (point 2).

## What changes
- GEN-179: a slope cap on every band octave, `amp_o <= c x slope(e0) x wavelength_o / 2`, and a weight on the 8 km hill octave, each its own switch, both off by default (the bits of today's base and today's keys).
- The mountain band's centre falls with the capped octaves so the band stays zero-mean.
- The value of `c` is swept on R1h, R3a and R3b in a separate measurement; the switch-on PR waits for the owner's look approval.

## Not in this change
- The coast-plain tilt and the plateau (the next two PRs of the start-surface fix); a cap on the belt term itself (its relief is e0's own slope, not a band octave).

## Impact
generators (classes.rs, global.rs, bake/inputs.rs). Default path unchanged: every elevation and every key are the bits of the build before.
