## ADDED Requirements

### Requirement: VIEW-046 F6 cycles the ground detail
The editor window SHALL show the procedural ground detail and gloss (REND-034, REND-035) at the strength `GroundDetailMode` names: F6 cycles natural, full and off (`planet_frame::look::materials::GroundDetailMode`; strengths `NATURAL_STRENGTH` 0.6, 1 and 0); the window starts at natural. The window's title SHALL name the mode (`detail natural`, `detail full`, `detail off`), and the keys help (`KEYS_HELP`, the editor's help text) SHALL name F6. The plan's strength is 0 for every plan the window does not make (`test-render` and the goldens), so no golden moves.

Verify: A
Status: active
Source: owner (2026-10-21, via opus-planner): the detail must be visible in the editor and viewer without code changes, VIEW-022, VIEW-014

#### Scenario: F6
- **WHEN** F6 is pressed four times in a viewer at its start
- **THEN** the mode is natural at the start, then full, off, natural, full; the strengths are 0.6, 1 and 0 for natural, full and off; the other switches are unchanged; the help names F6 and the ground detail, F6 is bound to the command and no other key is
