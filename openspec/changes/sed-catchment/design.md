## Decisions
- A catchment is the set of cells whose route in the sweep's own routing ends at one outlet cell. Round r's catchments are those of the routing of its result, not the round's cut at its start.
- The balance per catchment: eroded column (land cells, `original - z` where positive) = rise of the heights (`z` after minus before the sweep, times the area) + the load exported at the outlet. The identity checks the sweep's carried load against the heights and against the catchment border: no load crosses a catchment.
- The round solver's carve has no sediment term, so there is no per-round sediment balance of the carve itself. This is a gap in the contract of A3 section 6.7 ("D8-routed sediment" inside a round): where sediment is produced and deposited is a decision for the planner.

## Measurements (`examples/landmass_scale 1 8 sedround`: 3,072 cells a face edge, 9.41e6 cells, 8 threads; examples, not gates)
| Round | Catchments | Eroded (m3) | Raised (m3) | Exported (m3) | Max relative error, a catchment | Region | Largest catchment's share of the eroded |
|---|---|---|---|---|---|---|---|
| 1 | 76,249 | 5.4638e15 | 1.8751e15 | 3.5887e15 | 3.3e-14 | 6.8e-15 | 5.8 % |
| 2 | 48,522 | 3.6978e15 | 1.8066e15 | 1.8912e15 | 4.2e-14 | 3.3e-15 | 6.1 % |
| 3 | 46,536 | 3.3094e15 | 1.8139e15 | 1.4955e15 | 3.2e-14 | 1.4e-14 | 5.8 % |
| 4 | 45,653 | 3.2314e15 | 1.8250e15 | 1.4064e15 | 2.8e-14 | 2.7e-15 | 5.5 % |

- G-SED holds for the deposition sweep per catchment and per round at this scale, from the heights.
- The rise is nearly the same in every round (1.8e15 m3) while the eroded column falls from 5.5e15 to 3.2e15 m3: the share that settles grows from 34 % (round 1) to 56 % (round 4).

## The pits thermal makes (`examples/landmass_scale 1 8 thdiag`, after 4 rounds)
- 96,656 pits of 0.5 m or deeper after thermal; 95,318 were pits before (max 5.5 m). Thermal adds 1,338; 534 are 50 m or deeper (0.006 % of land cells); none of the 534 was a pit before. None is next to the sea; 65 of all have fewer than 8 neighbours.
- Mechanism (cell examples): a canyon floor cell between walls 1,400 to 1,900 m high rises by hundreds of metres (72 m to 711 m), the next floor cells less (to 232 m and 107 m); the cell upstream (79 m) then sits 610 m under the spill. Thermal infills a one-cell canyon with talus cell by cell, unevenly, and the highest infill dams the channel.
- Not measured: the share of the 534 explained by this mechanism. The count of land cells steeper than the unperturbed critical slope before thermal is 163 (0.002 %); the walls exceed the critical slope only under the per-pair perturbation (down to 0.75 times), and the count with that factor was not run.
- Fix options, none built (the end state after the deposition, 0.26 % of cells at 3 to 5 m, is acceptable at this scale): (1) cap a receiver at the giver's level less the repose; (2) run thermal before the carve's last round; (3) deposit before thermal. Re-check at 814 m and 102 m when those runs include thermal.

## Not done
- A sediment step inside the round solver.
- The balance on a round's own cut (its catchments at the start of the round).
- 12,288 cells.
- Earth-specific values (ADR 0108): the table is the prototype landmass at 3,072 cells; examples.
