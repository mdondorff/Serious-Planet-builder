## Measure
`Win32_PerfRawData_PerfProc_Process` (`Timestamp_Sys100NS`, `IDProcess`, `PercentProcessorTime` = cumulative CPU time in 100 ns, `Name`): the raw class gives exact differences whatever the interval; the formatted class returns a stale or zero first reading in a fresh process. The machine's busy share is the sum over all processes except `Idle` and `_Total` of the CPU time used between two readings, over the interval times the logical processors. A process that appeared in the interval counts all its time, one that exited in it is not seen (an under-count, never an over-count).

## Harness versus competitor
Own process and helpers (`powershell`, `pwsh`, `nvidia-smi`, `wmiprvse`, `conhost`, the sampler's own spawns) form the harness's share. The rest is the competing share; its top five are listed by name (instances of one name add: twenty `chrome` processes are one competitor). The cargo and shell ancestors use no CPU while waiting and are not special-cased.

## Notes, not verdicts
The brief: record, and add a note if the load exceeds a threshold, using the ADR's numbers. ADR 0020's gate has two relevant numbers, the 15 % mean CPU load and the busy-process list; both are applied to the competing share, during the case. Neither voids a session. The first window that shows a note tells the architect whether it should.

## Store check
The smoke run over a store is already a run of the real path on the software adapter. It now starts with `WorldTerrain::store_check`: `TileStore::open` is read-only (no `PackWriter`), `prefill_present` looks up and decodes every pre-filled tile through the pack's CRC and the codec, the ocean probe asks `WaterField::water_at` for a fixed 12 x 24 grid. The check fails the run on a missing or all-land or all-sea ocean channel and on an incomplete compact pack; the vertex form is shown for information because the production run draws the compact form.
