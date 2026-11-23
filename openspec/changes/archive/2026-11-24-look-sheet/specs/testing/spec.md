## ADDED Requirements

### Requirement: TEST-150 The look sheet on fixtures
Tier-A fixtures SHALL check the look sheet (BUILD-067): a crop store's bytes and shade against the full window's, the sheet's fixed layout and scales, the pair's difference, the per-sheet cap and the cockpit's index.

Verify: A
Status: active
Source: Lead 2026-11-23 (#183), BUILD-067
Detail:
- the fixture: a 256 x 192 chain window with H1 cells of 8 L cells and three crops, at the window's corner, inside it and at its far corner;
- every L, H1 and `stages/` field of a crop is the full field's cut of the box, byte for byte;
- a crop's shade is the full window's over the same cells, bit for bit, under both suns;
- a field of no known size and a crop outside the window are refused;
- SPIKE-143's crops file reads its pairs in order;
- a sheet has the fixed size of its rows, and the same store gives the same bytes;
- a flat cell is the shade's mid grey under both suns;
- in a pair, a stage raised past 10 m is the full raise colour, and an unchanged stage's difference panel is its switch panel;
- `look.txt` has the numbers, and none for a stage not dumped;
- a pair of other crops, or of the same crops of another window, is refused;
- three rows of the largest crops of noise stay under the per-sheet limit; eight rows are refused, naming the limit, and not written;
- the cockpit indexes a look sheet with the stage sheets.

#### Scenario: The look sheet's fixtures
- **WHEN** the tests of `look_sheet` and the cockpit's look test run
- **THEN** each check above has an assertion that cites BUILD-067 and TEST-150
