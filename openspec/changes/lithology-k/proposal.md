## Why
SPIKE-102 (2026-11-04), finding 1: the generator has no erodibility field. K varies only by the class carve factor (0.5 for sand sea, 1 elsewhere), so badlands, mesas, cuestas, the cliff-or-beach choice and alpine contrast cannot form. Owner D2 (2026-11-04): lithology K(x, z) in the carve, in M9, with a preview.

## What Changes
- New requirement GEN-116: an analytic lithology field, CPU-canonical (libm, integer hashing): hardness H per plate province (craton, old margin, active margin, rift) times noise; strata K(z) as horizontal or folded bands with a caprock flag; a soluble flag.
- Pass 1 and pass 2 read K per cell (pass 2 per sample every 4 cells) in place of the class carve factor.
- Kernel and field versions; the field has no stored channel (keyed by its version).
- Whole-planet preview maps of H, strata and the soluble flag at 16 km.
- No values in this proposal: they come from the preview round.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-116 (new), GEN-100 and GEN-101 (K source)

Depends on: SPIKE-102 plan rows 3 and 4 (PR 3 field, PR 4 carve). Expected golden changes: none until the carve switches.
