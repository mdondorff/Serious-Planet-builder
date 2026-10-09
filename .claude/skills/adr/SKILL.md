---
name: adr
description: Record an architecture decision as a new ADR in docs/adr. Use when a decision is made or changed, or when a spike concludes.
---
Create `docs/adr/NNNN-<kebab-title>.md`, using the next free four-digit number:

```
# NNNN <Title>
Status: proposed | accepted | superseded by NNNN
Date: <today>
Source: <background report section, spike, or learnings entry, if any>

## Context
## Options (with trade-offs)
## Decision
## Consequences (what becomes easier or harder; what must be tested)
## Licences of adopted code or techniques
```

Rules:
- New ADRs written by Claude always start as `proposed`. Only the owner sets `accepted`.
- Link the ADR from the affected spec(s) in `openspec/specs/`.
- If it supersedes an ADR, change only the old ADR's status line to `superseded by NNNN`.
- If the decision would change a constitution rule, say so in Context and name the rule ID; do not edit the constitution.
- Add one line to `docs/status.md`: date, ADR number, title, status.

Topic: $ARGUMENTS
