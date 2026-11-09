## ADDED Requirements
### Requirement: GEN-158 The resident build runs as one phase plan with the lifetimes of its buffers fixed
The build after the landmass SHALL run the rounds, H1 and the sediment pass in one process in that order, resident, and SHALL give the bits of the same steps run one after another.
- The pre-carve surface SHALL be written to a spill file before the rounds and read back only for the sediment pass; the file SHALL be deleted after the read-back and its size reported.
- H1's weighted route SHALL return the filled surface it was made on, and H1 SHALL read that surface (one flood).
- H1's per-cell discharges (`Q`, `Q_w`) SHALL be checked against G-CON (`hydro::verify`) and then released before the sediment pass; the result SHALL keep the channel graph, the basins and the labels.
- Every allocation boundary of the plan SHALL be marked for a caller to read the process's memory.
- The stage probe SHALL be called at the landmass and at H1, and with it on no bit SHALL move.
- `verify_with_area` SHALL give the bits of `verify` when it is given the sum `weighted_cell_area` returned.
- The runs of `examples/landmass_scale plan` (the peak working set and commit, the per-phase seconds and the hashes) are measurements, not gates; the gates are the tier-A tests.

Verify: A
Status: planned
Source: A3 section 6.6 rule 1, inbox #109 and #116, G-SCALE
- G-SCALE's complete-build row stays open until two runs on this path read at most 11,444 MiB of peak commit.

#### Scenario: a compact landmass built in one plan
- **WHEN** a landmass of a sphere of 24 cells a face edge is built by the plan, by the steps run in order, and by the plan with a stage probe installed
- **THEN** the surface, the labels, the channel flags and the basins are the same bits in all three, the spill file does not exist after the plan, `Q` and `Q_w` are released, every conservation error is at most 1e-9, and the probe saw the landmass, each round, H1, thermal, the deposition and the pit spreading once and in order
