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

Arguments: $ARGUMENTS
