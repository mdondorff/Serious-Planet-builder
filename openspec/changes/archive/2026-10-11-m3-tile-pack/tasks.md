## 1. Pieces
- [x] 1.1 Hilbert curve and sort key, CRC-32C, with known answers and properties
- [x] 1.2 Record format, pack writer with lock, torn-tail truncation, atomic index, rollover; store reader with rebuild

## 2. Tests
- [x] 2.1 Round trip, versions per tile, torn tail, CRC failure, index rebuild and staleness, order, newest wins, rollover, concurrent readers, locks, wrong store, wire format

## 3. Close-out
- [x] 3.1 Spec STRM-008, proposed ADR 0024, reviews, verifier, archive, status line
