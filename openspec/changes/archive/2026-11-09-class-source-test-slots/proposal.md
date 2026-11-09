# class-source-test-slots (BUILD-045)

## What
- `.config/nextest.toml`: the tests of `class_source_store` take 4 slots (`threads-required = 4`); the slow-timeout stays 180 s; no test body changes.
- BUILD-045 and a tier-A test that reads the override.

## Why
The keys test of the store timed out at 180.1 s in three full `cargo xtask test fast` runs on 2026-11-09 and takes 23 s alone (inbox #96).
