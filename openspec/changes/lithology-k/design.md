## Decisions
- Analytic, no store: the field is a pure function of position, seed and version (SPIKE-102 section 3.2, F1).
- Province data comes from the plate model (SPIKE-81's H); the class source's plate mask is the input.
- Two PRs: the field and its preview (crusader lane), then K in both carves (explorer lane, after the fill-depth change).
- Cost (SPIKE-102 estimate): pass 1 about 0.25 thread-h, pass 2 about 0.35 thread-h.

## Open
- Hardness and strata parameters, soluble share: set in the preview round.
- Relief changes everywhere: R7 re-tunes class amplitudes after the carve switches.
