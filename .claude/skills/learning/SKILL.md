---
name: learning
description: Append a finding to docs/learnings.md when something surprising happens or an assumption turns out wrong. Use proactively during implementation.
---
Append (never edit earlier entries) to `docs/learnings.md`:

```
## <today> <short title>
What happened: <observation, with evidence: numbers, file:line, image>
Assumption it changes: <spec ID, ADR number, constitution rule ID, or "background report §N">
Proposed follow-up: <spec delta / new ADR / test / nothing>
```

Then:
- If a spec must change, create or extend an OpenSpec change for it (don't edit `openspec/specs/` directly).
- If a decision must change, run `/adr`.
- If a constitution rule is affected, note it in `docs/status.md` for the owner; do not edit the constitution.

Finding: $ARGUMENTS
