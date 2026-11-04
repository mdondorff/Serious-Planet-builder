## ADDED Requirements
### Requirement: VIEW-062 The viewer draws the erosion layer on request
The viewer SHALL draw the erosion layer of a bake over a stored world when `--erosion on` is given, and not otherwise (off by default; no definition key, no key, id or golden moves with it off):
- `--erosion-store DIR` names the layer (a bake's output folder or its `tiles/`; the world's store when not given), `--erosion-face-res` and `--erosion-res` the bake's pass-1 and pass-2 cells (12,288 and 98,304 by default);
- it needs `--store` and an eroded field, and refuses a store that does not say it was baked from the class source unless `--erosion-unchecked`;
- the layer's keys follow the bake's shape and the definition's seed: a layer made under other keys is refused, naming both;
- the log and the title say the layer is on, and the terrain's id is the layered field's.

Verify: A
Status: planned
Source: Owner 2026-11-04 (integrated preview), inbox #56 item 1b, GEN-112

#### Scenario: Options
- **WHEN** the options are parsed, and given without `--erosion on` or outside the viewer
- **THEN** the defaults are off and the production bake, bad values are refused, and the options without `on` are refused

#### Scenario: On, and the refusals
- **WHEN** a stored world and a tiny layer made under the shape's keys are opened with `--erosion on`, with and without the marker, without a store, with a pass-1 size the layer was not made for, and over a missing folder
- **THEN** the unmarked store, the missing store, the other keys and the missing folder are refused with their reasons, and the layer's field has the layered id, says so in `describe()` and moves land heights
