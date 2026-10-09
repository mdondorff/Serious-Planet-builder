---
name: bless
description: Owner-only review of candidate golden images; promotes approved candidates to goldens.
disable-model-invocation: true
---
1. List candidate images waiting for approval (`target/review/candidates/`), grouped by test and adapter.
2. For each candidate, show the candidate, the current golden (if any) and the diff image side by side, plus the test name, requirement ID and what changed in the code since the current golden.
3. Ask the owner, one group at a time: approve, reject, or skip.
4. Copy only approved candidates to `tests/goldens/<adapter>/`. Leave rejected ones in place and add a line to `docs/status.md` naming the test and the reason given.
5. Commit approved goldens on the current branch with the message `bless: <tests>`.

Never approve anything without an explicit answer from the owner in this session.

Scope: $ARGUMENTS
