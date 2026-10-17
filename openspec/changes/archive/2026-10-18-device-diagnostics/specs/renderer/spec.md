## ADDED Requirements

### Requirement: REND-029 Device diagnostics
A `GpuContext` SHALL install on its device, before any resource exists, a device-lost callback and an uncaptured-error handler.
- A lost device (any reason other than a deliberate destroy) SHALL print `wgpu device lost (<reason>): <message>` on stderr and be readable as `GpuContext::device_lost`.
- The first uncaptured error SHALL print `first wgpu error: <description>` on stderr and be readable as `GpuContext::first_error`; later ones SHALL not replace it.
- The uncaptured-error handler SHALL then panic with wgpu's own message (`wgpu error: <description>`), as the default handler does.
- The handlers SHALL change no pixel, draw, pipeline or plan.

Verify: A, C
Status: active
Source: perf window 8 (dive on the lit path, `Buffer with 'globals' label is invalid`), TEST-012

#### Scenario: A destroy is no loss
- **WHEN** the lost-device line is built for the reason `Destroyed` and for another reason
- **THEN** the first gives no line and the second a line with the reason and the message

#### Scenario: The first error is kept
- **WHEN** a buffer with an invalid descriptor is created on a context
- **THEN** `first_error` holds the description, stays the same after a second error, and the creating call panicked with `wgpu error:`
