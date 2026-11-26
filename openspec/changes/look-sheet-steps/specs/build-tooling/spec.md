## ADDED Requirements
### Requirement: BUILD-071 The look sheet's step rows: each stage beside its step from the previous stage
`cargo xtask look-sheet RUN --steps [--az A,B] [--out DIR]` SHALL render, for each crop of a crop store (BUILD-067) and each azimuth, one sheet `<crop>-step-azNNN.png` with one row per stage of the run in the run's order. Each row SHALL show the stage under the chain sheet's fixed-sun shade at one L cell a pixel, beside that shade tinted by the stage's difference to the previous stage. The command SHALL also write `steps.txt`. The stages SHALL be the store's own list, so the command reads any pipeline's stages and holds no candidate's code.

Verify: A
Status: active
Source: Lead 2026-11-26 (the river-graph park plan, step 4: the owner's item to keep sheet generation on main), lead #184i (`step_row.py`, spike branch 7d6d11c3), BUILD-067, BUILD-048, TEST-158
Detail:
- the stage list: the store's `stages.txt`:
  - its form: `look-stages 1`, then one `stage NAME LABEL` a stage, in the pipeline's order;
  - it is written by `CropWriter::stage(NAME, LABEL, surface)`, which cuts an L surface of the window as `NAME.f32` into every crop, as the writer gets it;
  - NAME takes letters, digits, '-' and '_'; LABEL takes no whitespace; a name given twice is refused;
- a store without `stages.txt` (a chain run's written before this requirement) takes BUILD-067's columns, those its first crop holds, in their order;
- refused, each naming the store:
  - a listed stage that a crop does not hold, naming both;
  - a store with fewer than two stages;
  - `--steps` together with `--against` (a pair's sheets are BUILD-067's);
- the shade and colours are BUILD-067's (BUILD-060's shade at each azimuth, by default 315 and 135);
- the difference: the stage minus the previous stage, red lower, blue higher, full at 2 m (`STEP_M`); no scale is stretched per sheet; the first row's difference panel is grey, labelled `first`;
- `steps.txt`: `look-steps 1` and `run LABEL`, then one line a crop and step: `CROP NAME PREV cells N mean_m M rms_m R max_abs_m X`;
- output: BUILD-067's directory (`target/review/look/<run>/`) and form (8-bit RGB PNG); a sheet over BUILD-048's per-sheet limit is refused, naming it, its size and the limit, and is not written.

#### Scenario: A run's stages
- **WHEN** a crop store holds three stages written by `CropWriter::stage` and `cargo xtask look-sheet RUN --steps` runs
- **THEN** each crop has one sheet an azimuth with three rows, a lowered step is tinted red and a raised one blue, an unchanged stage's difference panel equals its stage panel, and `steps.txt` names each step's signed mean, RMS and largest difference
