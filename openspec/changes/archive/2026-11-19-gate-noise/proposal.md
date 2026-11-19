## Why
- Reading 30 (lead #173, 2026-11-19): the prototype and B run on GEN-140's turned base for g1f8; the gate computes a window's start and a store's base itself, so it has to know the noise.
- Two defects of #1605: `#timing=` split a stage name holding commas; the bars against B also scored start-surface rows.

## What changes
- BUILD-060: `--noise axis | turned` / `#noise=` (default axis).
- BUILD-060: the timing file's numbers read from the line's right end; the bars against B only on rows with a graph.

## Impact
the `river_gate` example, xtask (`rivergate.rs`, help); tests. The test strings of #1602 and #1605 with real line breaks rewritten as escapes.
