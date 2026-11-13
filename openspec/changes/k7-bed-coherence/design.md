# k7-bed-coherence: design notes

## Evidence
| Probe | Store or fixture | Result |
|---|---|---|
| `k7_rises` (app example) | r3a-e1, 52 tiles | 219,390 line steps in cores; 63,167 rise > 0.25 m; p50 1.29, p99 5.68, max 44.70 m |
| channel 2545, tile 1/89/7 | r3a-e1 | line cell 12 at 393.8 m (a crossed trench), cells 13-23 at 432-437 m, joins at 393.4 m: no carry from cell 12 |
| SPIKE-113 cross-section, frame-43680 | r3a-e1 | line 15-20 m above the carved low, low 130-150 m off; bank 10-15 m at -80 m |
| TEST-134 draft | rugged fixture region | 9,369 land steps, 44 rises (max 5 m, coastal); level - ground p50 +4.9 m, p99 +194 m |

## Open
- Where the stored lines and K7's burnt lines diverge: `k7_line_check` (cache example) reruns K7 on one R3a tile with the bake's inputs and compares the in-memory lines, the stored lines, K7's z and the stored B + L. Held for a quiet machine.
- The fixture does not reproduce the inland rises; the R3a check is the evidence for GEN-185.
