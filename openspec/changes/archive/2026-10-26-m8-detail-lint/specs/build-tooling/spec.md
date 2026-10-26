## ADDED Requirements

### Requirement: BUILD-027 No new Detail blocks under a requirement
`cargo xtask spec-lint` SHALL fail a requirement with a `#### Detail` block unless its ID is in `xtask/src/detail_blocks.txt`, fail a listed ID whose requirement has no such block (delete its line), and fail the list when it holds more than the 10 entries it started with: the list only shrinks. A removed requirement is not read.

Verify: A
Status: active
Source: opus-planner (2026-10-26), BUILD-024

#### Scenario: A Detail block
- **WHEN** a requirement with a `#### Detail` block is linted with and without its ID in the list, and a listed ID has no block
- **THEN** only the unlisted block fails, naming the requirement, and the listed ID without a block fails with the instruction to delete its line

#### Scenario: The list only shrinks
- **WHEN** the list holds more entries than it started with
- **THEN** the lint fails
