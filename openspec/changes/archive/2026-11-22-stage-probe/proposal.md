# stage-probe (BUILD-046, GEN-161; briefing item 18, PR 1)

## What
- `bake::probe`: a process-wide optional sink (`install`, `uninstall`; off by default) and `emit(stage, |s| StageView)`: off, one atomic load (the closure does not run, nothing is allocated); on, the sink receives a view that only borrows the stage's output.
- `SheetProbe`: a PNG per field with a colour scale fixed by the field's kind (height, area on a log scale, delta, mask) and a line of key numbers in `stages.txt` (percentiles from a 4,096-bin histogram, a data hash, the stage's numbers, the probe's cost); at most `max_bins` bins a sheet, so the probe's memory is bounded whatever the grid.
- Hooks (one `emit` each, read only): after each round of `solve_rounds_with` and `solve_rounds_memo` (heights and routing area), after thermal, deposition and pit spreading in `sediment_pass` (heights and the change against the original, evaluated cell by cell).
- Tier A: off the closure does not run; the rounds and the sediment pass give the same output bits and reports with the probe on and off and write every stage's sheets and numbers; the bin bound; fixed colours; percentiles within a bin and the data hash.

## Not in this change
- The landmass and H1 boundaries: the explorer's phase plan (#109) calls `probe::emit` there. K7: one line after the explorer's #114 merges (a follow-up PR).
- The manifest and CI (PR 2) and the advisory agent (PR 3).
- The probe's cost at the real sizes: #109's runs report it; the gate is read with the probe as run.
