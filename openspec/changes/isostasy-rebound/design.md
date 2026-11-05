## Physics
- Local (Airy) isostasy: removing a thickness `e` of crust (density 2,700 kg/m3) lets the column rise by `e * rho_c / rho_m` = 0.82 `e` for a mantle of 3,300 kg/m3.
- Flexural isostasy: the response is the load convolved with the flexure kernel. Its width is the flexural parameter `alpha = (E Te^3 / (12 (1 - nu^2) rho_m g))^(1/4)`, with `E` 1e11 Pa and `nu` 0.25: about 23 km for `Te` 10 km and 46 km for `Te` 25 km. A removal narrower than about `alpha` is spread out, so a valley's unloading lifts its peaks and the region, not only the valley floor.
- FastScape's form is the same (a flexural response of the eroded load, `Te` a parameter); here the kernel is a Gaussian of `sigma = alpha`, three box passes, which keeps the sum exactly the mass removed (a box blur conserves it) and needs no FFT.

## Where it acts
- In `bake::carve::carve` (pass 1), after the implicit update of each step: `lost = max(0, z_before - z_after)` per cell (f32), smoothed by the Gaussian, then `z += rebound * smoothed` on every cell that is not an outlet (the sea and the face's border keep their heights).
- Cost on a face of 12,288 cells plus the halo: one f32 copy of the heights, one f32 plane of what was lost, three box passes on each axis a step (running f64 sums in a fixed order, so the bits do not depend on the thread count): about 1.2 GB more memory than the carve's peak and a few seconds a step (the measured pass 1 took 1,942 s with it, 2,262 s with pass 2 of the tile).
- The edges: zero outside the plane (what is lost beyond the halo is unknown); a face border is an outlet and does not rebound.
- Pass 2 (the local carve, incise only) needs none. Its tile reads the face's change, which already holds the rebound.

## Measurement (seed 14, tile 2/12/10, production grid; rebound 0.82, sigma 50 km = 61 cells)
Relief on the spike's measure (SPIKE-99: p95 - p5 in blocks that are at least 95 % land and 90 % one class), base + change, p50:

| Measure | Without | With |
|---|---|---|
| 3 km, mountains | 534 m | 540 m |
| 3 km, high mountains | 695.2 m | 694.8 m |
| 3 km, plain | 59 m | 72 m |
| 10 km, mountains | 1,215 m | 1,226 m |
| 10 km, high mountains | 1,545 m | 1,545 m |

The filled surface (the runtime look) has 519 m (0.96 of the Alps' 543 m) and 675 m (0.84 of Everest's 804 m) without it: the relief does not change by more than 2 %.

Elevation (land cells):

| Class | Without p50 / p95 / p99.9 / max | With p50 / p95 / p99.9 / max |
|---|---|---|
| plain | 104 / 791 / 1,091 / 1,321 m | 166 / 927 / 1,250 / 1,510 m |
| mountains | 1,841 / 3,303 / 4,082 / 4,515 m | 2,085 / 3,571 / 4,356 / 4,789 m |
| high mountains | 2,899 / 4,281 / 4,953 / 5,215 m | 3,192 / 4,571 / 5,241 / 5,505 m |

- The rebound at 50 km is a regional uplift of 0.82 of the regional mean erosion: +244 m (mountains) and +293 m (high mountains) at the median, +290 m at the peaks.
- The elevations are already high against GLO-30 (Everest tile p50 2,157 m against 2,899 m here, Alps 1,076 m against 1,841 m): the rebound adds to that bias unless the base's mean is lowered with it.
- Not measured: widths other than 50 km. At the local limit (sigma toward 0) 82 % of each cell's own erosion returns and the relief falls by up to that share; above 50 km it stays a uniform uplift.

## Determinism and keys
- Sums in f64 in a fixed order, one thread per pass in the probe (rows or columns can be parallel with the same bits since each line is summed alone); CON-14 holds.
- The new `CarveParams` fields enter `LayerSpec::bytes` (CON-07), so a layer with and without rebound have other keys.

## Open
- `Te` per class or per province (a thinner lithosphere in young belts): a parameter by class would join the lithology field (GEN-116).
- Whether to retune the base's mean down with the rebound (SPIKE-102 section 6 and the GLO-30 hypsometry).
