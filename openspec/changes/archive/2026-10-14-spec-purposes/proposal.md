## Why

M3 close-out item 4. All ten specs in `openspec/specs/` still carry OpenSpec's archive placeholder ("TBD - created by archiving change m0-foundations") as their Purpose, so a reader of a spec cannot tell what the subsystem owns or which ADRs govern it.

## What Changes

- Each of the ten specs gets a one-to-three-sentence Purpose naming what the subsystem owns and its governing ADRs and constitution rules.
- spec-lint (`xtask/src/speclint.rs`) refuses a main spec whose Purpose is missing, empty or contains "TBD" (any case). Change deltas carry no Purpose and are not checked.
- Tier-A test `a_main_spec_needs_a_real_purpose` (TEST-001); the lint fixtures in the module gain a real Purpose.
- TEST-001's text states the rule.
- `docs/learnings.md`: tier-A sweep tests share a warm `SelectionCache`.
