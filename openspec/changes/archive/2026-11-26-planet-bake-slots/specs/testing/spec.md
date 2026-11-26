## ADDED Requirements
### Requirement: TEST-156 The planet bake tests' slot rule is checked
A tier-A test SHALL check BUILD-070's override: `threads-required = 4`, no slow-timeout, no test group, at least 20 names, no name twice, and each name's function a test function of `crates/app/src`.

Verify: A
Status: active
Source: Lead 2026-11-26, BUILD-070

#### Scenario: A renamed test
- **WHEN** a name in the override is not a function of `crates/app/src`
- **THEN** the test fails, naming it
