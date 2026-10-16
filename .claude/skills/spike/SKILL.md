---
name: spike
description: Open or close a time-boxed spike in docs/spikes.md. Use when a question must be answered by experiment before a decision, e.g. erosion strategy or wgpu feature fit.
---
**Opening a spike** (`/spike open <question>`): add an entry to `docs/spikes.md`:

```
## SPIKE-NN <short title>
Status: open
Opened: <today>
Question: <one sentence>
Why it matters: <which decision or milestone depends on it>
Time box: <e.g. 1 day of agent work>
Success criteria: <measurable>
Fallback if it fails: <the plan B>
Code location: spikes/<name>/ (throwaway; not part of the main crates)
```

**Closing a spike** (`/spike close SPIKE-NN`): set `Status: closed`, add `Closed:`, `Result:` (with numbers, images in `target/review/` or committed under `spikes/<name>/results/`), and `Recommendation:`. Then run `/adr` for the resulting decision and, if an assumption turned out wrong, `/learning`.

Spike code is never merged into the main crates as-is; the real implementation goes through an OpenSpec change.

**Spikes in a parallel session** (owner, 2026-10-16). A spike may run in its own session beside the main coder when it does not depend on the main queue. Rules:
- Its own worktree (`.claude/worktrees/spike-<milestone>`) and its own branch per spike (`spike/<id>`); spike code stays on that branch.
- It writes only its own files: its `docs/spikes.md` entry and, if needed, its `docs/learnings.md` entries, plus results under `spikes/<name>/results/`. One docs-only PR per spike. It never edits `docs/status.md`, `docs/log.md`, specs, ADRs or the main crates; the architect writes the log and status lines and coordinates the merge order.
- Shared data (bakes, stores, goldens) is read-only for it; it copies what it must modify into its own `target/`.
- It stops for the architect's perf windows like every coder (no builds, tests or runs during a window) and avoids long CPU or GPU runs while the main coder measures ad hoc.
- A result worth keeping is rebuilt by the main coder through an OpenSpec change, never merged from the spike branch.

Arguments: $ARGUMENTS
