## ADDED Requirements
### Requirement: BUILD-070 The planet library's bake tests take slots in proportion to their work
The nextest profile SHALL give each of the planet library's bake tests that take 10 s or more in `test fast` (named one by one in one override, filter `package(=planet) and kind(lib)`) `threads-required = 4`. It SHALL NOT give them a test group, change their slow-timeout (the default's 180 s) or change any test body.

Verify: A
Status: active
Source: Lead 2026-11-26 (after four prepushes of #1662 timed them out; tracked in #1666), BUILD-003, BUILD-045
Detail:
| Run (`planet-render`, `planet-editor`, `planet`, `xtask`; 816 tests; idle machine) | Total | Slowest bake test |
|---|---|---|
| before (no override) | 106.5 s | 95.6 s (`generate_bakes_the_class_source...`) |
| after (`threads-required = 4`) | 99.9 s | 68.1 s (`the_base_under_a_commit...`) |
- Before, with other work on the machine, two to four of them ran past 180 s (2026-11-25).
- A test group is not used: tier A and B are not capped by groups (BUILD-003's test).
- The list is the planet tests of 10 s or more in the before run; a new heavy bake test joins it.

#### Scenario: The override
- **WHEN** `.config/nextest.toml` is read
- **THEN** one override names the bake tests, sets `threads-required = 4`, and sets no slow-timeout and no test group
