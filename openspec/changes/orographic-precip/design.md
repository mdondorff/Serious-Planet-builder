## Decisions
- Fixed-order upwind sweep, CPU f64, libm; thread-count independent (CON-14).
- The sweep reads the pre-carve base, so it runs in the field bake before pass 1.
- Cost (SPIKE-102 estimate): under 0.1 thread-h for 6 faces of 4.2 M cells, about 10 passes.

## Open
- Wind bands, rain-out rate, shadow length, arid thresholds: set in the preview round.
- Class shares against Earth (badlands 3.3 %, sand sea 7.9 %) are tuned after the arid rules exist.
