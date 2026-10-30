## ADDED Requirements

### Requirement: TEST-074 The owner package says what ran
`cargo xtask package` SHALL run the `capture stills` of the default look of each look-review site and `capture flights` into `target/review/owner-package/<date>/` and SHALL write `package.md` there with one row per item: PASS, FAIL or NOT RUN, with its reason, and its files. An item with no store SHALL be NOT RUN, a failed command FAIL with its message, a command that wrote no file FAIL. A failed item SHALL NOT stop the others, and the command SHALL fail when an item failed.

Verify: A
Status: active
Source: opus-planner (2026-10-30), B4-5

Detail: The command is a tool, never a measurement. `package.md` ends with the counts and says a NOT RUN item is not a pass. `--dry-run` prints the steps without running them. The directory is sent to the owner by the planner; the command has no upload.

#### Scenario: No store
- **WHEN** no `--store` is given
- **THEN** every item is NOT RUN, none is PASS, and no capture command runs

#### Scenario: A failed step
- **WHEN** one capture command fails
- **THEN** its row is FAIL with its message, the other steps run, and the command fails
