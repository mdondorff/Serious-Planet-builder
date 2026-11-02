## Results (preview report, 2048 x 1024)
| Measure | Round 1, seed 1 | Round 1b, seed 1 | Round 1b, seed 2 | Reference |
|---|---|---|---|---|
| Plains / hills / mountains | 72.7 / 16.6 / 5.5 % | 74.2 / 10.5 / 8.6 % | 69.8 / 9.2 / 6.1 % | 55-67 / 10-25 / 14-30 % |
| Land above 2 / 4 km | 11.3 / 1.1 % | 7.7 / 2.1 % | 4.5 / 0.6 % | 5-10 / 1-3 % |
| Highest land | 6.57 km | 6.87 km | 6.74 km | 7-9 km |
| Largest step of `k`; share over 2.0 | 4.93; 0.22 % | 6.03; 0.19 % | 5.38; 0.18 % | at most 2.0 |
| S4 width median / p10 | 235 / 39 km | 235 / 55 km | 235 / 39 km | target median about 310 km |
| S5 longest span | 6,318 km | 4,360 km | 6,569 km | at most 7,500 km |
| S6 enclosed area | 0.0 | 0.0 | 0.0 | about 0-1 Mkm2 |

## Decisions
- **Where the highs came from (seed 1, land at least 2 km under 60 degrees):** the belt and the coast reach 2 km alone over 70 % of that land, the plateau field leads over 19 %, the back-arc over 2 %. The back-arc is not the cause; the plateau uplift fell to 800 m, and the mountains' membership rose by a gain of 2 on the core, so the belt flanks above 2 km read as mountains.
- **Segments end in a taper** of width and height; the narrowest half-width is 120 km. The ends of a segment still make thin tips of high ground where the height crosses 2 km, so the p10 of S4 stays near the pixel floor (39 km, two cells); S4's median is the measure.
- **The trace bends** by a signed distance to the margin (positive on the side of the plate with the lower index), shifted by the bend; the signed distance is continuous where the nearest plate changes.
- **The step of `k` rose** with the gain on the mountains' membership (4.93 to 6.03 on seed 1): the limit of 2.0 needs wider transitions than the belts have. Reported, not weakened.

## Open
- Mountains are 6-9 % against 14-30 %; the highest land is under 7 km; S4's median is 235 km against about 310 km; seed 2's belts are fewer (its land above 2 km is 4.5 %).
- The continents are round 2.
