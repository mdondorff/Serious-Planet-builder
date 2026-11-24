## ADDED Requirements

### Requirement: BUILD-067 The look sheet: a switch row's crops beside the defaults row
`planet_cache::look_sheet::CropWriter` SHALL write a crop store of a chain window: each pre-registered crop cut into a box aligned to the H1 grid and one H1 cell larger on each side, as a chain directory of BUILD-060's layout, for every field the chain hands it. `cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]` SHALL render a store at one L cell a pixel with the chain sheet's fixed-sun shade and colours: without `--against` one sheet an azimuth with a row a crop; with it one sheet a crop and azimuth with the rows defaults, switch and the switch tinted by its difference to the defaults, and `look.txt`.

Verify: A
Status: active
Source: Lead 2026-11-23 (#183; owner 2026-11-23: every switch run comes with its sheet, combined with the stage manifest, the cockpit only where the link is obvious), SPIKE-143 (the crops and the sheet's layout), BUILD-046, BUILD-048, BUILD-060, BUILD-064, TEST-150
Detail:
- the chain hook: `--probe-crops <crops.json>` of the spike branch's chain runner hands `CropWriter` every field as the chain writes it, so the store outlives a deletion of the full dumps (kilobytes a stage);
- crops: SPIKE-143's `crops.json` form (`size` and named `[x, y]` pairs in window L cells; other keys skipped), in the file's order; a side of 1 to 128 L cells; chosen before any stage of the run is looked at;
- the store: `crops.txt` (`look-crops 1`, then `crop NAME X Y SIZE BX BY` a crop);
- a crop's directory: the chain's `chain.txt` with the box's window line and the crop's name after the label, and the box's cut of every L field, H1 field and `stages/` field;
- columns, in order: B (`base.f32`), `S0` (`s0.f32`), `S_KH` (`kh.f32`), KS0 (`ks0.f32`), KS1 with connect (`ks1c.f32`), band6 with connect (`band6c.f32`), KV (`kv.f32`), the final surface (`final.f32`);
- `ks0`, `ks1c` and `band6c` are chain surfaces beyond BUILD-060's layout, which the chain sheet does not draw;
- a stage the run did not write stays grey, labelled `not dumped`;
- the shade: BUILD-060's (central differences, sun 10 degrees high, grey `0.5 + 2 (shade - sin 10)`) at each azimuth given, by default 315 and 135;
- the difference: red lower, blue higher, full at 10 m; no scale is stretched per sheet;
- `look.txt`: per crop and stage the cells, the RMS and the largest absolute difference (m);
- a pair needs the same crops of the same window (each box's place, size and H1 grid);
- output: `target/review/look/<run>/`, 8-bit RGB PNG;
- a sheet over BUILD-048's per-sheet limit is refused, naming it, its size and the limit, and is not written;
- sheets are committed only at a gate, under `docs/stages/sheets/<gate>/`;
- the cockpit (BUILD-064) indexes `target/review/look/` with the stage sheets, the run as the variant;
- the lanes: every switch row reported carries its look sheet pair beside its gate table.

#### Scenario: A switch row's sheet
- **WHEN** a defaults run and a switch run of one window write crop stores of the same crops and `cargo xtask look-sheet SWITCH --against DEFAULTS` runs
- **THEN** each crop has one sheet an azimuth with the defaults, switch and difference rows at one L cell a pixel, a stage the switch did not change has a difference panel equal to its switch panel, and `look.txt` names each stage's RMS and largest difference
