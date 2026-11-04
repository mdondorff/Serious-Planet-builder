## Decisions
- Values are SPIKE-102 run (a)'s: band x4.5, ridged, centre 0.45, breach 500 m. The spike's noise layer id differs from the port's (a new hash layer), so the realisation differs, the statistics do not.
- The centred band can be negative; a land point is kept at least 1 m above the sea (the continent stage's shore, tested by `global_class`).
- The relief-scaled breach is a proxy: the receiver's height above the sea scales the limit (50 m at the sea to `breach_max_m` over `breach_height_scale_m`); off by default, in the layer key.
- Pass 2's breach stays 0 (its local carve only incises).

## Open
- Relief against Everest and the Alps on 2/12/10 and the stills come with the preview re-run after the other M9 items (owner: no layer re-run yet).
