## ADDED Requirements
### Requirement: TEST-158 The look sheet's step rows on fixtures
Tier-A fixtures SHALL check BUILD-071's step rows on a crop store of three stages written by `CropWriter::stage`.

Verify: A
Status: active
Source: Lead 2026-11-26 (the river-graph park plan, step 4), BUILD-071
Detail:
- the fixture: TEST-150's 256 x 192 window and three crops; the stages `k5`, `k6` and `k7`, where `k6` lowers the hills by 5 m and `k7` raises them by 5 m;
- `stages.txt` lists the stages in the order written;
- each crop and azimuth has one sheet named `<crop>-step-az<A>.png`, of the fixed size of three rows;
- a step past 2 m is the full lower colour when it lowers and the full raise colour when it raises; an unchanged cell's difference pixel is its stage pixel; the first row's difference panel is grey;
- `steps.txt` has one line a crop and step, with the step's signed mean;
- the same store gives the same bytes;
- a store without `stages.txt` takes BUILD-067's columns that it holds;
- a listed stage missing from a crop, a store of one stage, a surface of another size and a name given twice are refused;
- xtask refuses `--steps` with `--against`.

#### Scenario: The step rows' fixtures
- **WHEN** the tests of `look_steps` and xtask's `looksheet` tests run
- **THEN** each check above has an assertion that cites BUILD-071 and TEST-158
