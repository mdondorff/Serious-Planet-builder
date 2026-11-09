## ADDED Requirements
### Requirement: BUILD-045 The class-source store tests take slots in proportion to their work
The nextest profile SHALL give every test of `planet-cache`'s `it` binary whose name starts `class_source_store::` `threads-required = 4`, and SHALL NOT change their slow-timeout (the default's 180 s) or any test body. The comment of the override SHALL state the measured durations.

Verify: A
Status: active
Source: Architect 2026-11-09 (inbox #96: the keys test of the store hit the 180 s cap in three full `test fast` runs; 23 s alone), BUILD-003

#### Scenario: The override
- **WHEN** `.config/nextest.toml` is read
- **THEN** the override for those tests has `threads-required = 4` and no slow-timeout of its own
