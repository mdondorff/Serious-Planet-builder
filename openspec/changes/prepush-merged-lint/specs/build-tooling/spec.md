## ADDED Requirements

### Requirement: BUILD-022 Prepush lints the branch merged onto main
`cargo xtask prepush` SHALL run, before its other steps, `spec-lint` on the tree that merging `HEAD` with the fetched `origin/main` makes (`git merge-tree --write-tree`, checked out in a temporary worktree that is removed afterwards), so that a requirement ID, a spec or a change that another PR added meanwhile collides in prepush and not on main. The branch, its index and its working tree SHALL NOT be changed. A branch that already holds `origin/main` SHALL pass this step without a merge; a fetch that fails SHALL print that the step is skipped and pass; a branch that does not merge cleanly SHALL fail naming `origin/main` and the merge to make first. The step SHALL be in every prepush plan, docs-only ones included.

Verify: A
Status: active
Source: opus-planner (2026-10-20), BUILD-016, BUILD-021

#### Scenario: The plan
- **WHEN** the plan of a docs-only diff, of a crate diff and of a diff with a golden path is listed
- **THEN** each starts with `merged spec-lint`, followed by the steps BUILD-016 and BUILD-021 name

#### Scenario: The tree id
- **WHEN** the first line of `git merge-tree --write-tree` is a 40 or 64 digit hexadecimal id, and when it is a conflict message or empty
- **THEN** the first is taken as the tree and the others are not
