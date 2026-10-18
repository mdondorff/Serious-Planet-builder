## Why
The flights of the lit scenes logged no cut counters, so a regression of their mesh building (no water step, no minimum) would not show in the smoke test (follow-up to #293).

## What Changes
- Each flight's log line carries the cut counters summed over its plans, in the held views' format.
- The lit smoke test asserts a nonzero exact cut and 0 kept with no minimum for the held views and the four flight lines.
- ATMO-020 scenario widened to the flights.

## Impact
Log text and a test; the planning code is unchanged. No ADR.
