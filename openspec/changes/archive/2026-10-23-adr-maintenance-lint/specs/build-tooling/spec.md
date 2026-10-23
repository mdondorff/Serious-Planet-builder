## ADDED Requirements

### Requirement: BUILD-025 A maintained accepted ADR carries a Maintained line
`cargo xtask spec-lint` SHALL fail when the branch changes an accepted ADR (`docs/adr/NNNN-*.md` whose status line starts with `accepted` at the merge base with `origin/main`) beyond its status line and the ADR gains no line `Maintained: <YYYY-MM-DD>, <links|values|terms|erratum>: <what>` (ADR 0078 point 7), naming the ADR. The lint reads the files at the merge base and at `HEAD`. It SHALL pass when git or the base is missing (a fresh clone), for an ADR new on the branch, for one that is not accepted at the base, and when only the status line changed.

Verify: A
Status: active
Source: opus-planner (2026-10-23), ADR 0078

#### Scenario: What passes and what fails
- **WHEN** an accepted ADR changes only its status line, changes its body and gains a well-formed Maintained line, changes its body without one, or gains a Maintained line with a bad date or kind; and a proposed ADR changes its body
- **THEN** the first two pass, the third and fourth fail naming the ADR, the proposed ADR is not checked, and no base gives no error
