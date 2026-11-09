## ADDED Requirements
### Requirement: BUILD-046 The sheet probe writes fixed-scale sheets and numbers in a bounded buffer
The sheet probe SHALL write for each field of a stage a PNG `<run>/<stage>-<field>.png` whose colour scale is fixed by the field's kind (the same value is the same colour in every run and at every scale) and a line in `<run>/stages.txt`: cells, minimum, 5th, 50th and 95th percentile (from a 4,096-bin histogram), maximum, a 64-bit hash of the data, the numbers the stage gave, and the probe's own milliseconds and bytes. A sheet SHALL have at most `max_bins` bins and the probe's buffers SHALL be bounded by them and the histogram, whatever the grid. The sheets are review aids, not goldens.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #100, briefing item 18: the gate margin is 1 %, so the probe must not raise H1's peak), GEN-161

#### Scenario: The sheet
- **WHEN** a 900 x 700 field is probed with 10,000 bins, two sheets of different data show the same value, and a bit of one cell changes
- **THEN** the sheet has at most 10,000 bins with the aspect kept, the value has one colour on both sheets, the percentiles are within a bin of the exact ones, and the data hash differs
