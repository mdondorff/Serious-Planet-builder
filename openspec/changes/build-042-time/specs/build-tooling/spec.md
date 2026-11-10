## MODIFIED Requirements
### Requirement: BUILD-042 An item that becomes done moves the re-estimate
Spec-lint SHALL fail a branch that turns an item of the table under `## Items` of `docs/briefings/terrain-prototype.md` to a State starting with `done`, against the merge base with `origin/main`, unless the briefing's `Re-estimate: YYYY-MM-DD[ HH:MM] (...)` line holds a valid stamp later than the base's.
- A stamp SHALL be a date with an optional time `HH:MM` (24 hours, local); a date alone SHALL read as 00:00 on that day, and a time that is not a valid `HH:MM` SHALL read as no time.
- Stamps SHALL be compared as a whole: the same stamp SHALL be refused (the re-estimate is the base's), an earlier stamp SHALL be refused (a re-estimate moves forward), a later one SHALL pass, including a later time on the same day.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (items were marked done without the remaining work re-estimated), inbox #122 (a second re-estimate on one day)

#### Scenario: An item is done
- **WHEN** an item becomes done with the re-estimate stamp unchanged, earlier, with no line, or later (a later date, or a later time on the same day)
- **THEN** the first three are refused naming the item and the last passes

#### Scenario: A date alone
- **WHEN** the base's re-estimate is a date alone and the new line is the same day at 00:00, at 00:01, or the date alone again; or the base has a time and the new line is the date alone
- **THEN** 00:01 passes and the others are refused (a date alone is 00:00)
