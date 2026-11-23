## ADDED Requirements

### Requirement: BUILD-067 The look sheet: a switch row's crops beside the defaults row
A chain run with `--probe-crops <crops.json>` SHALL write a crop store: each pre-registered crop of the window cut, after every stage, into a box aligned to the H1 grid and one H1 cell larger on each side, as a chain directory of BUILD-060's layout. `cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]` SHALL render the store at one L cell a pixel with the chain sheet's fixed-sun shade and colours: without `--against` one sheet an azimuth with a row a crop; with it one sheet a crop and azimuth with the rows defaults, switch and the switch tinted by its difference to the defaults, and `look.txt`.

Verify: A
Status: active
Source: Lead 2026-11-23 (#183; owner 2026-11-23: every switch run comes with its sheet, combined with the stage manifest, the cockpit only where the link is obvious), SPIKE-143 (the crops and the sheet's layout), BUILD-046, BUILD-048, BUILD-060, BUILD-064, TEST-150
Detail:
- crops: SPIKE-143's `crops.json` form (`size` and named `[x, y]` pairs in window L cells, other keys skipped), in the file's order; a side of 1 to 128 L cells; chosen before any stage of the run is looked at;
- the store: `crops.txt` (`look-crops 1`, then `crop NAME X Y SIZE BX BY` a crop) and a directory a crop: its `chain.txt` (the chain's, with the box's window line and the crop's name after the label) and the box's cut of every L field, H1 field and `stages/` field the chain wrote; kilobytes a stage, so the store outlives a deletion of the full dumps;
- columns: B, `S0`, `S_KH`, KS0, KS1 with connect, band6 with connect, KV and the final surface, in that order; a stage the run did not write stays grey and says so;
- the shade: BUILD-060's (central differences, sun 10 degrees high, grey `0.5 + 2 (shade - sin 10)`), at each azimuth given (default 315 and 135); the difference: red lower, blue higher, full at 10 m; scales fixed, never stretched per sheet;
- `look.txt`: per crop and stage the cells, the RMS and the largest absolute difference (m); a pair needs the same crops;
- output: `target/review/look/<run>/`, 8-bit RGB PNG; a sheet over BUILD-048's per-sheet limit is refused, naming it, its size and the limit, and is not written; sheets are committed only at a gate, under `docs/stages/sheets/<gate>/`;
- the cockpit (BUILD-064) indexes `target/review/look/` with its other image directories.
- every switch row a lane reports carries its look sheet pair beside its gate table.

#### Scenario: A switch row's sheet
- **WHEN** a defaults run and a switch run of one window write crop stores of the same crops and `cargo xtask look-sheet SWITCH --against DEFAULTS` runs
- **THEN** each crop has one sheet an azimuth with the defaults, switch and difference rows at one L cell a pixel, a stage the switch did not change has a difference panel equal to its switch panel, and `look.txt` names each stage's RMS and largest difference
