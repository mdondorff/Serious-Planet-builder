## Why
A lost GPU device reaches the program only through a callback; every buffer made afterwards is invalid with no error of its own, and the first visible failure is `Buffer with 'globals' label is invalid` at a later write (window 8, dive on the lit path). An uncaptured error is fatal by default, so the panic shows the last error, not always the first. Neither reason was printed.

## What Changes
- `GpuContext` installs a device-lost callback and an uncaptured-error handler on its device (REND-029).
- Lost device: a line on stderr and a reading (`GpuContext::device_lost`); a deliberate destroy is not a loss.
- Uncaptured error: the first one is printed on stderr and kept (`GpuContext::first_error`); the handler then panics with wgpu's own message, so the default behaviour is unchanged.

## Impact
Spec: REND-029 added (renderer). Diagnostics only: no pixel, pipeline, draw or plan changes; no golden changes.
