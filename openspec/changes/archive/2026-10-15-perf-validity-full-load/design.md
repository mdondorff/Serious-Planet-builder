## Decisions (owner, 2026-10-15, ADR 0036)

- Power cap (0x4) recorded and printed, never invalidating; other throttle bits as before; "unknown" means an unparseable reading (bits outside the listed set are ignored as before).
- GPU validity: all samples of the view's final batch at or above 1,400 MHz, whatever the P-state; a missing sample is unknown.
- Equal clock state: both views GPU-valid and clocks within 5 % of the larger; the P-state is printed. The frame-median comparison uses the same rule for its "unequal states" line, so two views below the floor are never "equal".
- The floor is a named constant; another reference machine needs its own measured floor (CON-21).

## Choices

- Reason text for an invalid view names the lowest sample (`P2 1150 MHz, below the 1400 MHz floor`), not the median, so a single low sample among good ones is visible.
- `gpu_clock_state` of a valid view with mixed P-states joins them (`P0/P2 1590 MHz`).
- Log format: one added field (`power_cap`); nothing removed.
