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

## G-SURF-I at the vertices (GEN-154; `k7_region 200 4`, the tile (65, 7) of face 1, `L` quantised as the kind 13 stream stores it)
| Item | With the channels | Without |
|---|---|---|
| gate at the cell centres | 5.7e-14 m unquantised, 0.1250 m quantised | the same |
| vertices read | 28,900 | 28,900 |
| p50 / p95 / p99 / max | 0.040 / 0.134 / 0.225 / 2.554 m | 0.039 / 0.134 / 0.228 / 2.554 m |
| over 0.25 m | 192 (0.66 %) | 209 (0.72 %) |
| vertices by local step (up to 5 m, 5-20 m, over 20 m) | 3,333 / 10,895 / 14,672 | 3,365 / 10,896 / 14,639 |
| over 0.25 m by that step | 0 / 20 / 172 | 0 / 25 / 184 |
| largest difference over step height (step of at least 1 m) | 0.098 | 0.109 |
| bins of 64 cells with vertices over 0.25 m (of 256 with vertices), most in one bin | 84, 8 | 95, 8 |
| unclamped read of L overshoots the clamped by up to | 2.28 m | 2.28 m |
- No vertex over 0.25 m stands where the bake's surface moves 5 m or less within 2 cells; 90 % of them stand beside a step of over 20 m. The difference stays under about 11 % of the step height. The vertices are spread over 84 of 256 bins with at most 8 in one: no cluster.
- The gate at the centres holds; the vertices stay a report. The clamp stays.
- The stored change decodes to the gate's quantised values (the test `the_stored_change_reads_back_as_the_quantisation_of_the_surface_gate`).
