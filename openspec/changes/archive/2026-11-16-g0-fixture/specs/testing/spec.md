## ADDED Requirements
### Requirement: TEST-139 The G0 minimum fixture
A tier-A fixture SHALL provide the G0 minimum landscape of the river-graph plan (G0 contract addendum section 2.7) as terrain and expectations only: `B`, `z_b`, `z_s` and `S0` on a cube sphere whose H1 cell is 814 m and whose L cell is an eighth of it, two partition layouts as data, and a plain-table expectations file; it SHALL build with the same bits on any thread count, and its measured features SHALL equal the recorded table.

Verify: A
Status: active
Source: Lead 2026-11-16 (crusader #154), owner 2026-11-16 (G0 approved)
Detail: the features are one trunk and two tributaries, one explicit confluence, one antecedent river crossing a ridge that `z_s` cuts and `B` keeps, one closed basin meant as a protected lake (at least 2 m deep and 0.25 km2 at L), one divide with a region boundary of the two-region layout within one H1 column, one reach crossing a tile boundary (64 L cells a tile) and one crossing a cube-face edge; they are checked with the round-rules routing and the graded flood, never with graph code; the recorded values (outlets, confluences, crossings, the basin's floor, spill and cells, the ridge's rise in `S0`) match within 0.006; the one-region layout puts every H1 cell in region 0 and the two-region layout splits at a column boundary between the divide's two sides.

#### Scenario: Same bits, recorded features
- **WHEN** the fixture is built on 1, 2, 4 and 7 threads and measured
- **THEN** every surface has the same bits, each feature holds, and every measured value equals the recorded table
