## Why
- The owner wants sheet generation kept on main after the river graphs were parked (owner, 2026-11-26; lead's park plan, step 4).
- Switches are already compared by `cargo xtask look-sheet RUN --against DEFAULTS` (BUILD-067).
- Pipeline stages with their differences were drawn only by the spike branch's `step_row.py` (lead #184i, 7d6d11c3), which is Python and stays off main.

## What changes
- BUILD-071: `cargo xtask look-sheet RUN --steps [--az A,B] [--out DIR]` writes, per crop and azimuth, `<crop>-step-azNNN.png`.
  - The sheet has one row per stage of the run, in the run's order.
  - Each row shows the stage under the chain sheet's fixed-sun shade at one L cell a pixel, beside that shade tinted by the stage's difference to the previous stage.
  - The command also writes `steps.txt`.
- The stages are the store's own list, `stages.txt`, written by `CropWriter::stage` as the writer gets each surface.
  - So the command draws B's stages (K5, K6, K7) and a chain run's alike, and holds no candidate's code.
  - A store without the list takes BUILD-067's chain columns that it holds.
- TEST-158: a tier-A fixture of a three-stage crop store checks the row count, the difference's signs, the file names and the refusals.
- The README's xtask table and xtask's usage text name the command.

## Expected golden changes:
- none: a library module, an example and xtask; no render path

## Impact
- `crates/cache/src/look_sheet.rs`, `crates/cache/examples/look_sheet.rs`, `crates/cache/tests/look_steps.rs`, `crates/cache/tests/it.rs`; `xtask/src/looksheet.rs`, `xtask/src/main.rs`; `README.md`.
- Specs: BUILD-071 (build-tooling) and TEST-158 (testing).
