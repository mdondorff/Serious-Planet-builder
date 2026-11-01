## Results on seed 1 (the preview report, 2048 x 1024)
| Measure | Iteration 0 | Round 1 | Reference |
|---|---|---|---|
| Plains / hills / mountains | 57.2 / 27.7 / 10.8 % | 72.7 / 16.6 / 5.5 % | 55-67 / 10-25 / 14-30 % |
| Land above 2 / 4 km | 11.3 / 4.2 % | 11.3 / 1.1 % | 5-10 / 1-3 % |
| Highest land | 6.95 km | 6.57 km | 7-9 km |
| Largest step of `k`; share of steps over 2.0 | 6.50; not reported | 4.93; 0.22 % | at most 2.0 |
| S4 high-ground width, median / p10 | not measured | 244 / 39 km | target median about 310 km |
| S5 longest span | not measured | 6,249 km | at most 7,500 km |
| S6 enclosed area | not measured | 0.0 Mkm2 | about 0-1 |

## Decisions
- **Smoothness is the limit on the belts.** `k` runs from 1 to 7.5, and 39 km is one lattice cell, so a step of 2.0 needs a transition over about 130 km (190 km for a smoothstep). A Gaussian belt of 130 km half-width cannot meet it; a tent profile of 220 to 500 km half-width comes close, and the remaining 0.22 % of steps are at the strongest belt flanks and the polar strip.
- **The mountains membership is the core itself** (a tent), not a smoothstep of it, for the same reason; the class decision reads the elevation without the ridge factor (0.75 for `0.5 + 0.5 r`) so the weights do not follow the noise of the ridges.
- **The belt amplitude is 7000 m** (5500 m before): the strike factor lowers most belts, and the highest land fell under 7 km.
- **Pairs of the three nearest plates** replace the nearest and second nearest. A pair's term does not change when its two plates swap, so the mask is continuous where the nearest plate changes; the weight of the third plate falls to 0 where it trades places with the fourth; the pair (second, third) counts only near the nearest margin (it would otherwise draw the extension of its bisector).
- **The estimates** `d_c` and the distance to the coast are first order, `|f| / |gradient|` with a central difference; the radius is Earth's in the preview.

## Open
- The belts' amplitude is low in the middle: mountains 5.5 % against 14-30 %, plains 72.7 %. R7 or the next round.
- The continents (S1 2 masses of 120 and 31 Mkm2, S2 79 %, S3 1.00) are round 2.
