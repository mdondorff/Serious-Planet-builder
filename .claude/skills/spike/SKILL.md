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

**Closing a spike** (`/spike close SPIKE-NN`): set `Status: closed`, add `Closed:`, `Result:` (with numbers, linking `spikes/<name>/results/results.md`), and `Recommendation:`. Then run `/adr` for the resulting decision and, if an assumption turned out wrong, `/learning`.

**Spike output: one place** (owner, 2026-10-17). Everything a spike or an exploration keeps goes to `spikes/<name>/results/`:
- `results.md`: the report (question, method, numbers, recommendation). It ends with a **Visuals** section that embeds the renders with relative links (`![caption](images/<file>.png)`) and a one-line caption each: what the image shows, the adapter, and the view. Use before/after or side-by-side comparisons where the question has two options.
- `images/`: the renders as PNG or JPG, each under 1 MB (a contact sheet under 1 MB beats many files). Never commit `target/review/` paths as the only copy.
- Raw numbers and logs as `.txt`/`.md` next to `results.md`.
- A spike whose question has nothing to look at (a pure CPU cost, a crash bisection) says so in one line under **Visuals** instead of leaving the section out.
- Every exploration gets a register entry (`SPIKE-NN`) like a spike, so `docs/spikes.md` lists everything.

`docs/spikes/` holds only the frozen images of SPIKE-03 to 07, which accepted ADRs link to; nothing new goes there.

Spike code is never merged into the main crates as-is; the real implementation goes through an OpenSpec change.

**Spikes in a parallel session** (owner, 2026-10-16). A spike may run in its own session beside the main coder when it does not depend on the main queue. Rules:
- Its own worktree (`.claude/worktrees/spike-<milestone>`) and its own branch per spike (`spike/<id>`); spike code stays on that branch.
- Before any git command that switches or creates a branch, check `git rev-parse --show-toplevel` names your own worktree. A worktree directory can vanish (session cleanup); git then falls back to the main checkout, which must stay on `main` (2026-10-16). If it vanished, recreate it with `git worktree add` and tell the architect.
- It writes only its own files: its `docs/spikes.md` entry and, if needed, its `docs/learnings.md` entries, plus results under `spikes/<name>/results/`. One docs-only PR per spike. It never edits `docs/status.md`, `docs/log.md`, `docs/log.d/`, specs, ADRs or the main crates; the architect writes the log fragment (`docs/log.d/<YYYY-MM-DD>-<id>.md`) and the status lines and coordinates the merge order.
- Shared data (bakes, stores, goldens) is read-only for it; it copies what it must modify into its own `target/`.
- It stops for the architect's perf windows like every coder (no builds, tests or runs during a window) and avoids long CPU or GPU runs while the main coder measures ad hoc.
- A result worth keeping is rebuilt by the main coder through an OpenSpec change, never merged from the spike branch.

Arguments: $ARGUMENTS
