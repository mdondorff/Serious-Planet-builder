# Change: edit-health-scale

## Why
- SPIKE-29 (`spikes/edit-scale/results/results.md`): the A5 lists (steep, stacked, problems) rescan every stroke of every brush after each change; they were the first thing to break, at about 100 items. opus-planner (2026-10-22) asked for fix 2: incremental and lazy, the work per change bounded.

## What changes
- `commands.rs`: a change log of item ids (`change_seq`, `changes_since`).
- `health.rs`: the grid index and the incremental steep and stacked lists, equal to the full scans.
- `session.rs`: the lists, the row warnings, the problems tab and the inspector read `Health`; the group warning from a group index; the problems rows by lookup; the merge test remembered per item.
- VIEW-048.

## Tests
- Tier A (`crates/editor/tests/health.rs`): equal to the full scans over a random edit sequence; the work per change at 5,041 items.

Expected golden changes: none.
