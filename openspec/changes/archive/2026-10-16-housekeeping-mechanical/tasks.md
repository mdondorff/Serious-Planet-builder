## 1. Check first (failing before the fix)
- [x] 1.1 BUILD-008: unit test over fixtures plus a repository test (xtask repocheck) for finished unarchived changes
- [x] 1.2 BUILD-005: repository test that both workflows list the same docs-only paths including `spikes/**/results/**`

## 2. Mechanical edits
- [x] 2.1 (a) spike and learnings references in code comments and manifests
- [x] 2.2 (b) stale "proposed" / "until exists" phrases; STRM-006 source line
- [x] 2.3 (c) the removed "Recent" section
- [x] 2.4 (e) both CI workflows

## 3. Finish
- [x] 3.1 spec-guardian and verifier; test, spec-lint, fmt, clippy; archive; log line; delete the queue item text (with the budget change); PR; CI; merge
