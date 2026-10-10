## MODIFIED Requirements

### Requirement: COORD-009 Pure-Rust transcendentals
Code that produces tile data SHALL call `libm` for transcendental functions and SHALL NOT call the platform implementations (`f64::sin`, `cos`, `tan`, `atan2`, `exp`, `ln`, `powf`, `powi`, `mul_add` and similar, on `f64` and `f32`, in any call form). The ban SHALL be enforced over the whole workspace by clippy `disallowed-methods` in `cargo xtask check`, with a text scan as a fallback.

Verify: A
Status: active
Source: CON-14, ADR 0010

#### Scenario: Banned call
- **WHEN** a crate contains a banned method call in any form (method, path, `<f64>::` or as a function value)
- **THEN** `cargo xtask check` fails with the offending file, line and the libm replacement, and the scan test fails for the forms it covers
