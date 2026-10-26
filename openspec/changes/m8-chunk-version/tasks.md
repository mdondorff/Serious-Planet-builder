## 1. Read set
- [x] 1.1 `anet::chunk_read_set` and tests (equal to `segments_reaching` on the existing network tests; the upstream window included)
- [x] 1.2 `region_segments` and `main_donor_of` shared by the build and the read set

## 2. Version and Under
- [x] 2.1 `ChunkVersion` from the items touching the read set (the disk key carries it, done)
- [x] 2.2 `WorldBase` reads base plus stamps under `Under`; GEN-039 default `Under`
- [x] 2.3 Test: re-solve == cold solve of the new version, bit for bit; an edit outside the read set starts no job (app); the read set covers the routing reach (generators, GEN-081); a job that straddles an edit is neither stored nor trusted (streaming)

## 3. Stale and re-solve
- [x] 3.1 `ChunkManager`: stale mark, version in the want, replace on landing, order and worker cap
- [x] 3.2 `worldfield.rs` edit hook; the viewer test on a fake clock (edit, old version draws, new lands)

## 4. Close
- [ ] 4.1 verifier, spec-guardian, numerics-reviewer (generators), archive, log fragment
