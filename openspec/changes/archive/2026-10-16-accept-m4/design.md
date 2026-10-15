## Decisions

- The verdicts are not re-implemented: the per-frame log carries every column `case_verdicts` reads except the tile ids of blank nodes (counts only), so `accept` parses `frames.txt` into `FrameRecord`s and calls the harness's own function. A change to a budget or rule in the scene changes the acceptance with it.
- Validity is the session's `valid_for_budgets` (readiness gate, throttling, smoke, adapter, power scheme, override), plus the source and field the scene recorded. The clock state decides only GPU verdicts, and the pipelined scenes give none; it is printed. Validity is checked on all five scenes, and one session's figures cannot be mixed with another's (source and field must agree).
- The log's frame statistics (median, p99, max) must equal those recomputed from `frames.txt`; this catches a `frames.txt` left from another run, which a per-file check could not.
- RAM from a process that ran several scenes is refused (`scene_alone`), not shown with a caveat: the criterion is gated.
- Driver VRAM is the whole GPU, an upper bound; absent it is reported "not measured" and does not fail (the allocator's slots and bytes gate); present and over the tier cap it fails, as in the scene.
- The report is written only from valid logs, never over an existing `M4-report.md`; the findings no log carries are slots marked TO FILL.
