## ADDED Requirements

### Requirement: TEST-038 Golden review evidence is enlarged for judging
For every candidate whose golden changed on an adapter, `golden-review` SHALL write to `target/review/golden-review/<adapter>/` two images of golden | candidate | diff (changed pixels red, the others dimmed) enlarged by nearest-neighbour: `<name>.png` at the largest factor of 8, 6 and 4 for which the three panels and their 4-pixel gaps stay within 2,400 px of width (4 where none does), and `<name>.crop.png` of the bounding box of the differing pixels widened by 4 pixels on every side (clipped to the picture, at least 16 pixels in each direction where the picture allows), at the largest factor of 32, 24, 16 and 8 for which the width stays within 2,400 px (8 where none does). `report.md` SHALL name both files for each such candidate and give the bounding box of its differing pixels as `x0..x1, y0..y1`. A full-size inspection render is not part of the default run.

Verify: A
Status: active
Source: owner (2026-10-18), ADR 0017, TEST-022

#### Scenario: Scale
- **WHEN** the pictures are 128 px, 256 px and 1,000 px wide
- **THEN** the factors of `<name>.png` are 6, 4 and 4, and every image panel is an exact nearest-neighbour enlargement (each source pixel a factor-square block)

#### Scenario: Crop
- **WHEN** the differing pixels span x 43..84 and y 27..68 of a 128x96 picture, or are a single pixel in a corner
- **THEN** the crop is x 39..88, y 23..72 and the single-pixel crop is 16 pixels square inside the picture, and a picture smaller than 16 pixels is cropped to the whole picture
