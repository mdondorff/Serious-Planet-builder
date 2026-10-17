---
name: adr
description: Record an architecture decision as a new ADR in docs/adr. Use when a decision is made or changed, or when a spike concludes.
---
Create `docs/adr/NNNN-<kebab-title>.md`, using the next free four-digit number:

```
# NNNN <Title>
Status: proposed | accepted | superseded by NNNN
Date: <today>
Source: <spike, learnings entry or spec requirement, if any>

## Context            (at most 5 bullets: the problem and the facts that force a choice)
## Options            (only when more than one was viable; one line each with its trade-off)
## Decision           (numbered points, one or two lines each)
## Consequences       (what becomes easier or harder; what must be tested)
## Licences of adopted code or techniques
```

Length and style (ADR 0055): at most about one page. Definitions, not narration; no asides. An ADR records **why**. The current value of a number, budget or limit goes in the spec (`openspec/specs/`), which cites the ADR in its `Source:` line. Do not write an ADR to record how an accepted decision was built (put that in the change's design notes); write one only for a new decision or a departure from an accepted one.

Rules:
- New ADRs written by Claude always start as `proposed`. Only the owner sets `accepted`.
- Link the ADR from the affected spec(s) in `openspec/specs/`.
- If it supersedes an ADR, change only the old ADR's status line to `superseded by NNNN`. If it amends one point, the old status line says `point N amended by NNNN`, and the spec carries the new value in the same PR.
- If it decides an open point of an earlier ADR ("accepted in part", "point N open"), update that earlier ADR's status line in the same PR (for example "point 5 decided by NNNN").
- Update the status index table in `docs/adr/README.md` (number, title, status, open points) in the same PR.
- If the decision would change a constitution rule, say so in Context and name the rule ID; do not edit the constitution.
- Add a fragment `docs/log.d/<YYYY-MM-DD>-adr-NNNN.md` with one line: date, ADR number, title, status.

Topic: $ARGUMENTS
