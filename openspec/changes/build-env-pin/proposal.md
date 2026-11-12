# build-env-pin

Why: ADR 0114 point 6 and condition 2 of the external review (2026-11-11) require the build environment pinned before the world build: toolchain, `Cargo.lock`, profile, compiler flags, CPU features and floating-point settings. Before this change CI installed a moving `stable` and no file pinned the toolchain.

What: `rust-toolchain.toml` pins one exact release; every CI job installs that release; tests of the repository files check that the pin is exact, that CI agrees with it, that `Cargo.lock` is committed and that no file sets a target CPU, a CPU feature or a fast-math style flag. The determinism fixture across thread counts and repeated runs (condition 3) is the next change.

Owner of the release choice: the architect (a release change is a deliberate PR). Requirement ID: BUILD-050 (bulldozer, 2026-11-12).
