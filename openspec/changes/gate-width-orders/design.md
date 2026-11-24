## Design
- `measure_suite` computes the Strahler order once per call (`strahler`), over the river cells by their receivers: upstream first by drained area, ties by cell id. A river cell drains less than its receiver, so this order is a topological one and the result does not depend on the scan.
- Each measured width (the 10 m width BUILD-057 already takes) is pushed to its order's and its area class's vectors beside `wf`; `spread` gives each one's p50, p90 and count. No width is measured twice.
- The line is separate from `RIVER-GATE fields:`, so the table's columns and the bars are unchanged; xtask parses it per row (`parse_widths`) and renders it with the table's own `render_table` into `river-gate-widths.md` and `.csv`.
- The area classes are fixed in km2 (`WIDTH_AREA_KM2`), as #198's table; a class under 75 km2 holds the cells of a run at a lower channel threshold.
