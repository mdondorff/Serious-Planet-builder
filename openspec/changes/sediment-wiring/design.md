## Decisions
- The order is K7's: thermal, deposition, pit spreading. The deposition reads the eroded column after thermal, so thermal's moves onto cells above `original` count as load (K7 does the same); the report gives the column before and after thermal.
- The steps take a landmass's boundary cells (fewer neighbours than on the cube: the coast ring) as outlets, with the sea and the grid's border; thermal still treats them as land (`is_edge` is false on a cube grid). The deposition's `original` is the surface before the rounds, so the eroded column includes what the breach lowered (the departure of GEN-148).
- `breach_lowered_m3` runs the carve with no incision on the starting surface with the first round's parameters. It is the first round's breach volume; it is neither a bound on the breach's later lowering nor on its effect on the incision, so its quotient with the all-round eroded column is a reference figure, not a bound.
- The steps run on the whole landmass here. A3 section 6.7 puts them per tile with a halo; that form is not built.

## Measurements (`examples/landmass_scale 1 8 sed`: 3,072 cells a face edge, 9.41e6 cells, 4 rounds then the steps, 8 threads; examples, not gates)
| Quantity | Value |
|---|---|
| Eroded / deposited / exported (m3) | 1.5686e16 / 4.3553e15 / 1.1331e16 |
| (eroded - deposited - exported) / eroded | -3.5e-14 (whole landmass, once; an accounting identity of the sweep; per catchment and per round not measured; gate not closed) |
| Eroded column before / after thermal (m3) | 1.5686e16 / 1.5686e16 (thermal's phantom load is under the printed digits) |
| Height-volume change over the deposition against deposited | 4.3553e15 / 4.3553e15 (the heights moved by what the sweep reports) |
| Thermal: passes, volume change | 96, 3.6e-16 |
| Pit spreading: passes, volume change | 1, 1.2e-15 |
| First-round breach alone on the starting surface, quotient with the all-round eroded column | 1.68e15 m3, 10.7 % (a reference figure, not a bound) |
| Time of the steps | 53 s (breach alone 18 s) |
| Peak memory | 654 MiB (616 after the rounds) |

| Pits of 0.5 m or deeper | Share of land cells | p50 (m) | p99 (m) | Max (m) |
|---|---|---|---|---|
| After the rounds | 1.024 % | 3.18 | 5.36 | 5.5 |
| After thermal | 1.038 % | 3.22 | 29.8 | 609.7 |
| After deposition | 0.260 % | 4.52 | 4.87 | 5.0 |
| After pit spreading | 0.261 % | 4.49 | 4.87 | 5.0 |

- Thermal, run before the deposition, makes pits up to 610 m deep (p99 30 m); the deposition fills them to at most 5 m. The pit spreading does nothing of note at this cell size (1 pass).
- The first-round breach volume is 10.7 % of the eroded column the deposition reads: a share of what it redeposits is breach-lowered ground, not incision.
- Why 0.26 % of cells stay pits (3 to 5 m) is not isolated.
- Why thermal makes pits of 610 m is not isolated. Two causes are excluded by the run: taking the coast ring as outlets changes no pit count, and thermal's phantom load is under 5 digits of the eroded column.

## Not done
- Per-tile form with a halo, and the wiring into the bake's tile pass.
- G-SED per catchment and per round (here: the whole landmass, once).
- The breach's share over all rounds, and its effect on the pits that return, are not isolated.
- Whole-landmass run at 12,288 cells.
- Earth-specific values (ADR 0108): the table is the prototype landmass at 3,072 cells; examples.
