//! The 128-bit identity of an authored item (ADR 0066 point 6, ADR 0023 point 2): the id (a ULID, 16 bytes) and the item's content hash
//! (BLAKE3-128) are `u128`s, so their big-endian bytes sort as the numbers do and every encoding of them is 16 bytes.
//!
//! The hashes that were written when ids and content hashes were `u64` stay what they were for values below `2^64`. The 64-bit-word
//! hashes (definition, cell, footprint) write the low words where the old value stood and the high words, when any is not zero, in
//! a tail after the rows, whose length the header already fixes; the byte hashes write the low 8 bytes and a tail read from its end
//! (see [`wide_tail`] and [`wide_tail_bytes`]).

/// The id of an authored item: 128 bits, big-endian bytes in every key, ULID text in the item's file (WDEF-009).
pub type ItemId = u128;

/// The content hash of an authored item: 128 bits.
pub type ItemHash = u128;

/// The low 64 bits of a 128-bit value.
pub fn lo(v: u128) -> u64 {
    v as u64
}

/// The high 64 bits of a 128-bit value.
pub fn hi(v: u128) -> u64 {
    (v >> 64) as u64
}

/// The tail of a 64-bit-word hash for rows whose id or content hash does not fit 64 bits: `[TAIL_MARK, count, (row index, id high
/// word, content high word)...]`, empty when every row fits, so a set of items whose ids and hashes are all below `2^64` hashes
/// exactly as it did before they were 128 bits. `rows` gives `(id, content)` of each row in the order the hash wrote them. The
/// header of the hash fixes the number of rows and their width, so the tail cannot be taken for rows.
pub fn wide_tail(rows: impl Iterator<Item = (u128, u128)>) -> Vec<u64> {
    let mut tail = Vec::new();
    let mut count = 0u64;
    for (index, (id, content)) in rows.enumerate() {
        if hi(id) != 0 || hi(content) != 0 {
            tail.extend([index as u64, hi(id), hi(content)]);
            count += 1;
        }
    }
    if count == 0 {
        return Vec::new();
    }
    let mut out = vec![TAIL_MARK, count];
    out.extend(tail);
    out
}

/// The first word of a tail (`"wide128"` as ASCII).
pub const TAIL_MARK: u64 = 0x0077_6964_6531_3238;

/// The tail of a byte hash made of 40-byte rows (an id and a content hash as 8 bytes each and 24 bytes of direction): for each wide row
/// its index as a `u32` and the high 8 bytes of its id and of its content hash (little-endian), then the count of wide rows (`u32`), the
/// count of rows (`u32`) and the byte `0x01`; empty when every row fits. The hash has no header, so the tail is read from its end: when the
/// whole input is not a multiple of 40 bytes it has a tail, the last 9 bytes give the row count `k` and the wide count `c`, and the
/// input is `40 k + 20 c + 9` bytes long, so the rows end where the entries begin. The tail's own length `20 c + 9` is never a multiple of
/// 40, so an input without one is told apart by its length, and two different sets of rows never give the same bytes.
pub fn wide_tail_bytes(rows: impl ExactSizeIterator<Item = (u128, u128)>) -> Vec<u8> {
    let row_count = u32::try_from(rows.len()).expect("fewer than 2^32 rows");
    let mut entries = Vec::new();
    let mut count = 0u32;
    for (index, (id, content)) in rows.enumerate() {
        if hi(id) != 0 || hi(content) != 0 {
            entries.extend_from_slice(&(index as u32).to_le_bytes());
            entries.extend_from_slice(&hi(id).to_le_bytes());
            entries.extend_from_slice(&hi(content).to_le_bytes());
            count += 1;
        }
    }
    if count == 0 {
        return Vec::new();
    }
    entries.extend_from_slice(&count.to_le_bytes());
    entries.extend_from_slice(&row_count.to_le_bytes());
    entries.push(1u8);
    entries
}
