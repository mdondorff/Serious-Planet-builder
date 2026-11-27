---
name: spike
description: Open or close a time-boxed spike (an R row's research package) in docs/spikes.md and spikes/<name>/results/. Use when a question must be answered by experiment before a decision, e.g. erosion strategy or wgpu feature fit.
---
**On closing, also add the spike's row to the catalogue in `spikes/README.md`** (owner, 2026-11-04: `Id | Folder | Question | Result | Pick up | Archive`; the housekeeping skill checks it).

**The register is an index (ADR 0075):** `docs/spikes.md` stays at most 10 KB, one table row per spike (`Id | Title | Status | Results`); details live in the spike's results file.

**A spike is a research package** (ADR 0130, proposed, applied on the owner's word of 2026-11-27). It answers an R row of the active sprint file (`docs/milestones/<M>.md`); a spike without an R row waits for one. Spikes from SPIKE-145 on use this header; older ones keep theirs.

**Opening a spike** (`/spike open <question>`): add a row to `docs/spikes.md` with status `open`, and write the header at the top of `spikes/<name>/results/results.md`, one line per field, every field filled before the first run:

```
Opened: <today>
Exit: <M> E<n> (mode R)
Question: <one sentence>
Hypothesis: <the expected answer, one sentence>
Constraints: <what the work must respect: data, budgets, the machine, the time box (e.g. 1 day of agent work)>
Out of scope: <one line each>
Evaluation (quantitative): <each metric, its reference outside the candidate, and its value, set today>
Evaluation (visual): <the sheet, crops, sun and poses, or "none: <why nothing can be looked at>">
Falsifier: <the result that would reject the hypothesis>
Code location: spikes/<name>/ or branch spike/<name> (throwaway; not part of the main crates)
```

- A value in an Evaluation line is fixed when the spike opens. A changed value needs a dated line under the header naming who changed it and why; the old value stays beside it.
- No requirement IDs, no accepted ADR, no default change and no `crates/*` PR on main come from an R row.

**Closing a spike** (`/spike close SPIKE-NN`): add to the header

```
Finding: <the answer, with the numbers, one or two sentences>
Decision: continue | revise | reject | promote: <one line why>
Research-Review: ready (<reviewer>, <date>, <sha>)   # only when the Decision is promote
```

- `promote` needs the `research-review` agent's verdict "ready" on the results at that sha, and the owner's `decision:` line; until both exist the row stays open. `continue`, `revise` and `reject` the lead signs.
- Set the register row's status to `closed <date>` with a clause for the result; the numbers go in `results.md`.
- Then run `/adr` only if a decision changed (ADR 0055) and `/learning` if an assumption turned out wrong.

**Spike output: one place** (owner, 2026-10-17). Everything a spike or an exploration keeps goes to `spikes/<name>/results/`:
- `results.md`: the header above, then the report (method, numbers, the finding's evidence). It ends with a **Visuals** section that embeds the renders with relative links (`![caption](images/<file>.png)`) and a one-line caption each: what the image shows, the adapter, and the view. Use before/after or side-by-side comparisons where the question has two options.
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
