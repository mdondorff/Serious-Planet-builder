## Why
- Decision 9 (valley widths, G3 gate R9) needs the width at 10 m by river size, for RG, B and GLO-30 alike (lead #198, #200).
- #198 measured it with a spike-only environment dump in `river_suite.rs` (spike/rg-prototype). Lead #192 says the gate lives on main, so the measure moves here.

## What changes
- BUILD-057: the widths at 10 m are split by Strahler order (1, 2, 3, 4+) on the river cells and by drained-area class (under 75, 75-300, 300-1,000, 1,000-3,000, from 3,000 km2).
  - Printed as a `RIVER-GATE widths:` line.
  - `--table` writes one row per run to `target/review/river-gate-widths.md` and `.csv` beside the table.
- Report only: no column of the table and no bar changes; decision 9 sets any bar.
- Once this lands, the spike copy (`PLANET_WIDTH_DUMP`) is removed from spike/rg-prototype (lead #200).

## Expected golden changes:
- none: the gate is a CPU measure; no rendered output changes.

## Impact
- Crates: cache (`river_suite.rs`), xtask (`rivergate.rs`).
- Spec delta: BUILD-057 (one Detail bullet, one scenario).
- Tests (tier A):
  - `the_width_is_split_by_strahler_order_and_area_class_and_the_split_covers_every_measured_cell` (TEST-137's fixtures; the split's p50 equals `width10_p50`; a two-branch valley gives orders 1 and 2);
  - `the_widths_line_parses_into_its_own_table_and_a_run_without_it_gives_an_empty_row`.
