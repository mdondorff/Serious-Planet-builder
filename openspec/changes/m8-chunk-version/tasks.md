## 1. Read set
- [ ] 1.1 `anet::chunk_read_set` and tests (equal to `segments_reaching` on the existing network tests; the upstream window included)
- [ ] 1.2 `segments_reaching` uses it

## 2. Version and Under
- [ ] 2.1 `ChunkVersion` from the items touching the read set; part of `ChunkSpec` and the record
- [ ] 2.2 `WorldBase` reads base plus stamps under `Under`; GEN-039 default `Under`
- [ ] 2.3 Test: re-solve == cold solve of the new version, bit for bit; outside-halo-inside-routing-reach edit changes version and bits; outside the read set changes neither

## 3. Stale and re-solve
- [ ] 3.1 `ChunkManager`: stale mark, version in the want, replace on landing, order and worker cap
- [ ] 3.2 `worldfield.rs` edit hook; the viewer test on a fake clock (edit, old version draws, new lands)

## 4. Close
- [ ] 4.1 verifier, spec-guardian, numerics-reviewer (generators), archive, log fragment
