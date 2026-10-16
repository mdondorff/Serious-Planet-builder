---
name: learning
description: Append a finding to docs/learnings.md when something surprising happens or an assumption turns out wrong. Use proactively during implementation.
---
Append to `docs/learnings.md` (never rewrite earlier entries; the `housekeeping` skill moves or removes them at milestone boundaries):

```
## <today> <short title>
Scope: project | milestone <M>
What happened: <observation, with evidence: numbers, file:line, image>
Assumption it changes: <spec ID, ADR number, constitution rule ID, or "background report §N">
Proposed follow-up: <spec delta / new ADR / test / nothing>
Obsolete when: <the condition that makes this entry irrelevant, e.g. "the scheduler is incremental (F2)", or "never">
```

- **Scope**: `project` if it stays true and useful beyond the current milestone (a property of a library, the machine, a numeric trap); `milestone <M>` if it explains one milestone's work (an ablation, a measured bottleneck that a change fixed).
- If the new finding disproves an earlier entry, append to that entry only one line: `Superseded by: <date> <title>`.

Then:
- If a spec must change, create or extend an OpenSpec change for it (don't edit `openspec/specs/` directly).
- If a decision must change, run `/adr`.
- If a constitution rule is affected, note it in `docs/status.md` for the owner; do not edit the constitution.

Finding: $ARGUMENTS
