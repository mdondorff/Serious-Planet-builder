## Why
The owner raised the `test fast` budget to 20 s on 2026-10-16 (M4 acceptance; M5 queue item 0).

## What Changes
- xtask: `FAST_BUDGET_SECS` = 20 (testcmd; `accept`'s timing step uses it) and the comments.
- spec: BUILD-003 says 20 s, naming the owner's decision.
- No test changes.
