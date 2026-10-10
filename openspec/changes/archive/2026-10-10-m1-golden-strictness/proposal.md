## Why

After the first goldens were blessed, CI would still pass if a golden of a blessed adapter were deleted or never added, and `cargo xtask bless` listed stale candidates identical to the goldens.

## What Changes

- A missing golden now fails whenever the adapter already has blessed goldens (strict mode still fails always); adapters with none still report pending, so Linux can be blessed later without a red CI.
- `cargo xtask bless` and the `test gpu` summary list only candidates that are new or differ from the golden.

## Capabilities

### Modified Capabilities
- `testing`: TEST-002.

## Impact

`crates/testkit`, `xtask`.
