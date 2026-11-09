## Measurement (2026-11-09, `k7_region 200 4 store`, then `l_measure`)
| Item | Value |
|---|---|
| tiles | 36 (1,024 cells at 102 m; a 6 x 6 block of tile ids 62..=67 x 5..=10) |
| cells, land cells | 37,748,736; 28,614,379 |
| encoded bytes (payload, no record header) | 26,198,711 |
| bytes a land cell, pooled | 0.916 |
| bytes a land cell, lowest, median, highest tile | 0.222, 0.901, 1.164 |
| worst round-trip error | 0.125 m (half the 0.25 m step) |
| sea-only tile | 14 B |
| Earth-scale local land cells (1.49e8 km2 over (0.102 km)^2) | 1.43e10 |
| Earth-scale size, pooled rate | 13.1 GB |
| Earth-scale size, highest tile's rate (unrounded rate) | 16.7 GB |

## Reading
- The legacy chain's kind 13 was about 24 GB at Earth scale; A3's target is 16-24 GB. The new chain's L is 13.1 GB pooled and 16.7 GB at the highest tile, at or under the target's lower end.
- The payload measured here excludes the pack record header (61 B), the kind 13 payload header (73 B) and the index entry (40 B): 174 B a tile, about 0.0002 B a land cell at 76 % land.
- The region is one mountain-and-coast area of the prototype landmass; flat plains and deserts may compress differently. The pooled figure is a measurement of that area.
- The stored change here is f32 metres from `K7Result::change`, quantised by `layer_tiles`; the stored i16 form is that quantisation (GEN-156 reserves the requirement for the measurement, not for the layout).
- G-STORE stays open: field bake on the new chain, temporary and replacement space and edit overrides are unmeasured.
