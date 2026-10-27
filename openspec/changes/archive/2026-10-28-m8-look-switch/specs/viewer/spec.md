## ADDED Requirements

### Requirement: VIEW-060 F8 switches the ground look
The lit window SHALL show the ground look `LookChoice` names: F8 cycles the defaults (the Earth curves at the strength F6 names) and the look file's `ground` table (LOOK-011; its curves, and its detail strength in place of the natural mode's, while full and off stay what F6 says); the window starts on the defaults. The window's title SHALL name the choice when it is not the default (`look look file`), the key table SHALL list F8 with its state (`default` or `look file`), and `LitPlan::with_ground_look` SHALL make the plan so that a test can read it. A look file without a ground table shows the same in both. Nothing is stored or keyed (ADR 0035).

Verify: A
Status: active
Source: owner (2026-10-28, via opus-planner): compare the default look and the look file's curves live, VIEW-046, VIEW-055, LOOK-011

#### Scenario: F8
- **WHEN** F8 is pressed three times in a viewer at its start and plans are made for each choice and mode with a ground table that has a detail of 0.3 and a soil moisture curve
- **THEN** the choice is the look file, the defaults, the look file; the defaults plan has the Earth curves and the mode's strength; the look file plan has the table's curves and 0.3 at the natural mode and 1 and 0 at full and off; the other switches are unchanged, the help names F8 and the ground look
