---
name: bless
description: Owner review of candidate golden images that the golden-review policy (ADR 0017) escalates; promotes approved candidates to goldens.
disable-model-invocation: true
---
1. List candidate images waiting for approval (`target/review/candidates/`), grouped by test and adapter.
2. For each candidate, show the enlarged golden | candidate | diff image and its crop of the differing pixels (`target/review/golden-review/<adapter>/<name>.png` and `<name>.crop.png`, TEST-038; not the raw thumbnails), plus the test name, requirement ID and what changed in the code since the current golden. If no enlarged image exists for a candidate (a first golden has no golden to compare), show the candidate itself enlarged.
3. Ask the owner, one group at a time: approve, reject, or skip.
4. Copy only approved candidates to `tests/goldens/<adapter>/`. Leave rejected ones in place and add a line to `docs/status.md` naming the test and the reason given.
5. Commit approved goldens on the current branch with the message `bless: <tests>`.

Never approve anything without an explicit answer from the owner in this session. Agents bless only through the `golden-review` skill (ADR 0017); this skill is for what that policy escalates.

Scope: $ARGUMENTS
