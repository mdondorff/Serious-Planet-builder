## Why

ADR 0010 review (queued in docs/status.md, owner asked): the platform-maths ban was only a text scan that misses `f64::powi(`, `f64::mul_add(`, `.map(f64::sin)`, `<f64>::sin(`, a space before `(`, and did not scan `app`.

## What Changes

- `clippy.toml`: `disallowed-methods` for every f64/f32 transcendental (sin, cos, tan, asin, acos, atan, atan2, sin_cos, exp, exp2, exp_m1, ln, ln_1p, log, log2, log10, powf, powi, hypot, cbrt, the hyperbolic family, mul_add) over the whole workspace, enforced by `cargo xtask check` (clippy -D warnings). Each entry names its libm replacement.
- The text scan stays as a fallback and now also scans `app`.
- Four test-only uses (`powi`, `powf` in frame tests) replaced by exact multiplications and `libm::pow`.
- Probe (verifier): 24 banned calls in many forms and locations (generators, render, app, xtask, cfg(test)) all fail `cargo xtask check`. A test keeps clippy.toml in step with the scan list. Known gap: calls through third-party traits (num_traits::Float, glam, nalgebra) are not seen by clippy; none is a dependency today.
- Modified: COORD-009 (enforced by clippy).
