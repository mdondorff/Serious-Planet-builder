## ADDED Requirements
### Requirement: AUTH-006 The item store's revisit trigger is counted and timed
The editor SHALL count the items of each body and time each load of a body's items, and show both in its diagnostics. A body holding more than 20,000 items, or a load over 2 s (ADR 0066 point 6), SHALL log one warning per crossing naming the count or the time, the threshold and "ADR 0066 point 6: revisit the backend". The count SHALL follow every addition and removal, and a fall back under the limit SHALL re-arm the warning. The time of the store's read alone SHALL come from an injected timer; it is a signal, and only loads on Rasierklinge decide, which the warning says.

Verify: A
Status: active
Source: ADR 0066 point 6, planner (2026-11-01, item-store-trigger)

#### Scenario: The count
- **WHEN** stores of 19,999, 20,000 and 20,001 items are loaded
- **THEN** only the last warns, once, naming 20,001 items and the limit 20,000

#### Scenario: The time
- **WHEN** a load is timed at 2,000 ms and at 2,100 ms by an injected timer
- **THEN** only the second warns, naming 2,100 ms and the limit 2,000 ms

#### Scenario: Adding past the limit
- **WHEN** the 20,001st item is put, then removed by undo, then put back by redo
- **THEN** the first crossing warns, the fall back clears the mark and the second crossing warns again

#### Scenario: The diagnostics
- **WHEN** the status bar of an editor with a store is drawn
- **THEN** it shows the largest body's count and the last load time with their limits, marked when one is over
