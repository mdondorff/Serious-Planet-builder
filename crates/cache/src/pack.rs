//! The tile store: append-only packs with a rebuildable Hilbert-ordered index (ADR 0019, STRM-008).
//!
//! A pack (`pack-<seq>.tpack`) is a header and records, appended and never updated in place. Its index
//! (`pack-<seq>.tidx`) is a sorted array written atomically and always rebuildable from the data by one scan; a missing
//! or stale index is rebuilt, never trusted, and an index whose entries point outside the data is stale. One writer per
//! directory holds an advisory lock on `writer.lock` (the OS releases it when the process dies, so a crashed writer
//! never blocks the cache); any number of readers need no lock. A cache degrades to misses, never to refusing: a pack
//! that is unreadable, from an older hash version or still being created is skipped by readers and reported.
//!
//! Record (all integers little endian): `key (16) | kind (u8) | tile id (u64) | generator version (u32) | hash version
//! (u32) | node set id (16) | payload length (u32) | payload CRC-32C (u32) | header CRC-32C (u32) | payload`. The header
//! CRC covers the 57 bytes before it, so a zero-filled or bit-flipped header is detected and never read as a record. The
//! tile id is not in the ADR's list; the index must be rebuildable from the data alone and its sort key needs the tile
//! (ADR 0024).
//!
//! Damage rules: a record that is cut short, or a zero-filled remainder, is a torn tail: the writer truncates it,
//! readers ignore it. A complete record whose payload fails its CRC is skipped, and the records after it stay readable.
//! A header that fails its CRC in the middle of a pack cannot be skipped (its length is unknown): readers stop there and
//! the writer leaves the pack untouched and continues in a new one. A file that ends before the length a scan took from
//! its metadata (cut under the scan) is a torn tail at that position. A reader that finds an indexed record unusable
//! reports it once; the entry is then a miss for that store, and it hides an older pack's copy (ADR 0024 points 7, 8).

mod compact;
pub use compact::{compact, rank_by_pack_age, CompactPolicy, CompactReport};

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use planet_core::TileId;

use crate::crc32c::{crc32c, update};
use crate::hilbert::{sort_key, SortKey};
use crate::key::CacheKey;

pub const PACK_MAGIC: [u8; 8] = *b"PLNTPACK";
pub const INDEX_MAGIC: [u8; 8] = *b"PLNTTIDX";
/// Version of the pack and index layouts. History: 1 = first layout; 2 = a CRC over the record header.
pub const PACK_FORMAT_VERSION: u32 = 2;
/// A pack file rolls over at 4 GiB (offsets are `u64` regardless).
pub const DEFAULT_MAX_PACK_BYTES: u64 = 4 << 30;

const PACK_HEADER_LEN: u64 = 24;
const RECORD_FIXED_LEN: usize = 57;
const RECORD_HEADER_LEN: usize = 61;
const INDEX_HEADER_LEN: usize = 40;
const INDEX_ENTRY_LEN: usize = 40;
/// How much of the end of the data the index's tail CRC covers.
const TAIL_BYTES: u64 = 4096;
/// Read size when checking a payload that is not needed in memory.
const CHUNK: usize = 64 * 1024;

/// What identifies a store: the hash version every record was made under and the body it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreParams {
    pub hash_version: u32,
    pub body: u32,
}

/// One stored artefact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub key: CacheKey,
    pub kind: u8,
    pub tile: TileId,
    pub generator_version: u32,
    pub hash_version: u32,
    /// BLAKE3-128 of the sorted paths of the read set, so compaction can decide liveness without regenerating.
    pub node_set_id: [u8; 16],
    pub payload: Vec<u8>,
}

/// Why a store operation failed. Every variant names the file and what to do or what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackError {
    Io(String),
    BadFile {
        path: String,
        why: String,
    },
    WrongStore {
        path: String,
        found: StoreParams,
        wanted: StoreParams,
    },
    Locked {
        path: String,
        holder: String,
    },
    /// A payload failed its CRC or its entry points outside the pack: treat as a cache miss, regenerate and log.
    Corrupt {
        path: String,
        offset: u64,
        key: CacheKey,
    },
    TooLarge {
        len: usize,
    },
    /// The pack sequence numbers are exhausted.
    NoRoom {
        dir: String,
    },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Io(m) => write!(f, "{m}"),
            PackError::BadFile { path, why } => write!(f, "{path}: {why}"),
            PackError::WrongStore { path, found, wanted } => write!(
                f,
                "{path} belongs to another body (body {}, wanted body {}): use another cache directory per body",
                found.body, wanted.body
            ),
            PackError::Locked { path, holder } => {
                write!(f, "{path} is locked by another writer ({holder}): wait for it or use another pack directory")
            }
            PackError::Corrupt { path, offset, key } => {
                write!(f, "{path}: payload of key {key} at offset {offset} fails its CRC; regenerate the tile")
            }
            PackError::TooLarge { len } => write!(f, "a payload of {len} bytes does not fit a record (u32 length)"),
            PackError::NoRoom { dir } => write!(f, "{dir}: the pack sequence numbers are exhausted"),
        }
    }
}

impl std::error::Error for PackError {}

fn io<T>(r: std::io::Result<T>, what: &str, path: &Path) -> Result<T, PackError> {
    r.map_err(|e| PackError::Io(format!("{what} {}: {e}", path.display())))
}

/// Like [`io`], but an unexpected end of file is `Ok(None)`: the file is shorter than the length the caller took from its
/// metadata, so something cut it while it was being read. For a scan that is a torn tail at that position (ADR 0024
/// point 4), not a failure; every other I/O error stays an error.
fn io_or_eof<T>(r: std::io::Result<T>, what: &str, path: &Path) -> Result<Option<T>, PackError> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(PackError::Io(format!("{what} {}: {e}", path.display()))),
    }
}

/// An index entry: where a record is and the sort key that orders it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct IndexEntry {
    sort: SortKey,
    key: CacheKey,
    offset: u64,
    length: u32,
}

impl IndexEntry {
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.sort);
        out.extend_from_slice(&self.key.0);
        out.extend_from_slice(&self.offset.to_le_bytes());
        out.extend_from_slice(&self.length.to_le_bytes());
        out.push(0); // reserved, zero: entries are 40 bytes, 8-byte aligned
    }

    fn decode(b: &[u8; INDEX_ENTRY_LEN]) -> IndexEntry {
        IndexEntry {
            sort: b[0..11].try_into().expect("11 bytes"),
            key: CacheKey(b[11..27].try_into().expect("16 bytes")),
            offset: u64::from_le_bytes(b[27..35].try_into().expect("8 bytes")),
            length: u32::from_le_bytes(b[35..39].try_into().expect("4 bytes")),
        }
    }

    /// Whether the record this entry names lies inside `data_len` bytes of data.
    fn inside(&self, data_len: u64) -> bool {
        self.offset >= PACK_HEADER_LEN
            && self.offset.checked_add(RECORD_HEADER_LEN as u64 + u64::from(self.length)).is_some_and(|end| end <= data_len)
    }
}

/// Encode a record (the wire format too: key, kind, tile, versions, length, CRCs, payload).
pub fn encode_record(r: &Record) -> Result<Vec<u8>, PackError> {
    let len = u32::try_from(r.payload.len()).map_err(|_| PackError::TooLarge { len: r.payload.len() })?;
    let mut out = Vec::with_capacity(RECORD_HEADER_LEN + r.payload.len());
    out.extend_from_slice(&r.key.0);
    out.push(r.kind);
    out.extend_from_slice(&r.tile.raw().to_le_bytes());
    out.extend_from_slice(&r.generator_version.to_le_bytes());
    out.extend_from_slice(&r.hash_version.to_le_bytes());
    out.extend_from_slice(&r.node_set_id);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&crc32c(&r.payload).to_le_bytes());
    debug_assert_eq!(out.len(), RECORD_FIXED_LEN);
    let header_crc = crc32c(&out);
    out.extend_from_slice(&header_crc.to_le_bytes());
    out.extend_from_slice(&r.payload);
    Ok(out)
}

/// A record header as found in a pack, before its payload is read.
struct RecordHeader {
    key: CacheKey,
    kind: u8,
    tile: TileId,
    generator_version: u32,
    hash_version: u32,
    node_set_id: [u8; 16],
    length: u32,
    crc: u32,
}

/// Decode a header; `None` if it is too short, fails its own CRC or names no tile.
fn decode_header(b: &[u8]) -> Option<RecordHeader> {
    let b = b.get(..RECORD_HEADER_LEN)?;
    if crc32c(&b[..RECORD_FIXED_LEN]) != u32::from_le_bytes(b[57..61].try_into().ok()?) {
        return None;
    }
    Some(RecordHeader {
        key: CacheKey(b[0..16].try_into().ok()?),
        kind: b[16],
        tile: TileId::from_raw(u64::from_le_bytes(b[17..25].try_into().ok()?))?,
        generator_version: u32::from_le_bytes(b[25..29].try_into().ok()?),
        hash_version: u32::from_le_bytes(b[29..33].try_into().ok()?),
        node_set_id: b[33..49].try_into().ok()?,
        length: u32::from_le_bytes(b[49..53].try_into().ok()?),
        crc: u32::from_le_bytes(b[53..57].try_into().ok()?),
    })
}

/// Decode one whole record from `bytes` (header and payload), checking both CRCs.
pub fn decode_record(bytes: &[u8]) -> Option<Record> {
    let h = decode_header(bytes)?;
    let end = RECORD_HEADER_LEN.checked_add(h.length as usize)?;
    let payload = bytes.get(RECORD_HEADER_LEN..end)?;
    (crc32c(payload) == h.crc).then(|| Record {
        key: h.key,
        kind: h.kind,
        tile: h.tile,
        generator_version: h.generator_version,
        hash_version: h.hash_version,
        node_set_id: h.node_set_id,
        payload: payload.to_vec(),
    })
}

fn pack_header(p: StoreParams) -> [u8; PACK_HEADER_LEN as usize] {
    let mut h = [0u8; PACK_HEADER_LEN as usize];
    h[0..8].copy_from_slice(&PACK_MAGIC);
    h[8..12].copy_from_slice(&PACK_FORMAT_VERSION.to_le_bytes());
    h[12..16].copy_from_slice(&p.hash_version.to_le_bytes());
    h[16..20].copy_from_slice(&p.body.to_le_bytes());
    h
}

/// What a pack header says about the pack, relative to the store that wants to use it.
enum HeaderCheck {
    /// Same store: usable.
    Ours,
    /// A valid pack of this body made under another hash version: all its records are dead.
    OtherHashVersion,
}

fn check_pack_header(path: &Path, h: &[u8; PACK_HEADER_LEN as usize], wanted: StoreParams) -> Result<HeaderCheck, PackError> {
    let bad = |why: String| PackError::BadFile { path: path.display().to_string(), why };
    if h[0..8] != PACK_MAGIC {
        return Err(bad("not a tile pack (bad magic)".into()));
    }
    let format = u32::from_le_bytes(h[8..12].try_into().expect("4 bytes"));
    if format != PACK_FORMAT_VERSION {
        return Err(bad(format!("pack format version {format}, this build reads {PACK_FORMAT_VERSION}")));
    }
    let found = StoreParams {
        hash_version: u32::from_le_bytes(h[12..16].try_into().expect("4 bytes")),
        body: u32::from_le_bytes(h[16..20].try_into().expect("4 bytes")),
    };
    if found.body != wanted.body {
        return Err(PackError::WrongStore { path: path.display().to_string(), found, wanted });
    }
    Ok(if found.hash_version == wanted.hash_version { HeaderCheck::Ours } else { HeaderCheck::OtherHashVersion })
}

/// Read the header of an opened pack: `None` if the file is shorter than a header (a pack still being created, or cut by a
/// crash before its first record).
fn read_pack_header(file: &mut File, path: &Path) -> Result<Option<[u8; PACK_HEADER_LEN as usize]>, PackError> {
    let mut h = [0u8; PACK_HEADER_LEN as usize];
    io(file.seek(SeekFrom::Start(0)), "seeking in", path)?;
    match file.read_exact(&mut h) {
        Ok(()) => Ok(Some(h)),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(PackError::Io(format!("reading {}: {e}", path.display()))),
    }
}

/// How a scan ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tail {
    /// Every byte was a record.
    Clean,
    /// A record cut short, or a zero-filled remainder, from this offset: the writer may truncate here.
    Torn(u64),
    /// A header that fails its CRC with data after it: cannot be skipped, must not be truncated.
    Damaged(u64),
}

/// A record a scan could not use: where it is and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damage {
    pub pack: String,
    pub offset: u64,
    /// The record's key, if its header could be read.
    pub key: Option<CacheKey>,
    pub why: &'static str,
}

impl fmt::Display for Damage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.key {
            Some(k) => write!(f, "{}: key {k} at offset {}: {}", self.pack, self.offset, self.why),
            None => write!(f, "{}: offset {}: {}", self.pack, self.offset, self.why),
        }
    }
}

struct Scan {
    entries: Vec<IndexEntry>,
    tail: Tail,
    /// Records skipped (payload CRC failed) and the damaged header that ended the scan, if any.
    damage: Vec<Damage>,
}

/// CRC-32C of `len` bytes at `pos`, streamed in fixed chunks; `None` if the file ends before `len` bytes.
fn stream_crc(file: &mut File, pos: u64, len: u64, path: &Path) -> Result<Option<u32>, PackError> {
    io(file.seek(SeekFrom::Start(pos)), "seeking in", path)?;
    let mut crc = 0u32;
    let mut left = len;
    let mut buf = vec![0u8; CHUNK.min(usize::try_from(len).unwrap_or(CHUNK)).max(1)];
    while left > 0 {
        let n = usize::try_from(left).unwrap_or(usize::MAX).min(buf.len());
        if io_or_eof(file.read_exact(&mut buf[..n]), "reading", path)?.is_none() {
            return Ok(None);
        }
        crc = update(crc, &buf[..n]);
        left -= n as u64;
    }
    Ok(Some(crc))
}

/// Whether every byte from `pos` to `len` is zero; `None` if the file ends before `len`.
fn all_zero_from(file: &mut File, pos: u64, len: u64, path: &Path) -> Result<Option<bool>, PackError> {
    io(file.seek(SeekFrom::Start(pos)), "seeking in", path)?;
    let mut left = len - pos;
    let mut buf = vec![0u8; CHUNK];
    while left > 0 {
        let n = usize::try_from(left).unwrap_or(usize::MAX).min(CHUNK);
        if io_or_eof(file.read_exact(&mut buf[..n]), "reading", path)?.is_none() {
            return Ok(None);
        }
        if buf[..n].iter().any(|b| *b != 0) {
            return Ok(Some(false));
        }
        left -= n as u64;
    }
    Ok(Some(true))
}

/// Scan a pack's records. A read error is an error, never a torn tail: only the data itself decides what is torn. A file
/// that ends before the length it was given (cut under the scan) is data too: a torn tail at that position.
fn scan(file: &mut File, len: u64, path: &Path) -> Result<Scan, PackError> {
    let mut entries = Vec::new();
    let mut damage = Vec::new();
    let pack = path.display().to_string();
    let mut pos = PACK_HEADER_LEN;
    let mut head = [0u8; RECORD_HEADER_LEN];
    let tail = loop {
        if pos >= len {
            break Tail::Clean;
        }
        if len - pos < RECORD_HEADER_LEN as u64 {
            break Tail::Torn(pos);
        }
        io(file.seek(SeekFrom::Start(pos)), "seeking in", path)?;
        if io_or_eof(file.read_exact(&mut head), "reading", path)?.is_none() {
            break Tail::Torn(pos);
        }
        let Some(h) = decode_header(&head) else {
            if all_zero_from(file, pos, len, path)?.unwrap_or(true) {
                break Tail::Torn(pos);
            }
            damage.push(Damage {
                pack: pack.clone(),
                offset: pos,
                key: None,
                why: "the record header fails its CRC; the records after it are unreachable",
            });
            break Tail::Damaged(pos);
        };
        let Some(end) = pos.checked_add(RECORD_HEADER_LEN as u64 + u64::from(h.length)).filter(|end| *end <= len) else {
            break Tail::Torn(pos);
        };
        let Some(crc) = stream_crc(file, pos + RECORD_HEADER_LEN as u64, u64::from(h.length), path)? else {
            break Tail::Torn(pos);
        };
        if crc == h.crc {
            entries.push(IndexEntry { sort: sort_key(h.kind, h.tile), key: h.key, offset: pos, length: h.length });
        } else {
            damage.push(Damage { pack: pack.clone(), offset: pos, key: Some(h.key), why: "the payload fails its CRC and is skipped" });
        }
        pos = end;
    };
    Ok(Scan { entries: normalise(entries), tail, damage })
}

/// Sort entries by `(sort key, key, offset)` and keep only the last record of each `(sort key, key)`: the later offset
/// wins within a pack.
fn normalise(mut entries: Vec<IndexEntry>) -> Vec<IndexEntry> {
    entries.sort_unstable();
    let mut out: Vec<IndexEntry> = Vec::with_capacity(entries.len());
    for e in entries {
        match out.last_mut() {
            Some(last) if last.sort == e.sort && last.key == e.key => *last = e,
            _ => out.push(e),
        }
    }
    out
}

/// CRC of the last bytes of the data; `None` if the file is shorter than `data_len` (cut under the caller).
fn tail_crc(file: &mut File, data_len: u64, path: &Path) -> Result<Option<u32>, PackError> {
    let start = data_len.saturating_sub(TAIL_BYTES).max(PACK_HEADER_LEN.min(data_len));
    stream_crc(file, start, data_len - start, path)
}

fn encode_index(entries: &[IndexEntry], data_len: u64, tail: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(INDEX_HEADER_LEN + entries.len() * INDEX_ENTRY_LEN);
    out.extend_from_slice(&INDEX_MAGIC);
    out.extend_from_slice(&PACK_FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&tail.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    for e in entries {
        e.encode(&mut out);
    }
    out
}

/// Read an index file: its entries, if it is well formed, was built from exactly the data of length `data_len` whose tail
/// has CRC `tail`, is strictly sorted by (sort key, key) and names only records inside the data. Anything else is stale and the caller
/// rebuilds.
fn read_index(path: &Path, data_len: u64, tail: u32) -> Option<Vec<IndexEntry>> {
    let b = fs::read(path).ok()?;
    if b.len() < INDEX_HEADER_LEN || b[0..8] != INDEX_MAGIC || u32::from_le_bytes(b[8..12].try_into().ok()?) != PACK_FORMAT_VERSION {
        return None;
    }
    if u64::from_le_bytes(b[16..24].try_into().ok()?) != data_len || u32::from_le_bytes(b[24..28].try_into().ok()?) != tail {
        return None;
    }
    let count = usize::try_from(u64::from_le_bytes(b[32..40].try_into().ok()?)).ok()?;
    if b.len() != INDEX_HEADER_LEN.checked_add(count.checked_mul(INDEX_ENTRY_LEN)?)? {
        return None;
    }
    let entries: Vec<IndexEntry> = b[INDEX_HEADER_LEN..].as_chunks::<INDEX_ENTRY_LEN>().0.iter().map(IndexEntry::decode).collect();
    // Strictly increasing by (sort key, key): two entries for one record would make `find` return the earlier offset,
    // and the later record wins.
    (entries.windows(2).all(|w| (w[0].sort, w[0].key) < (w[1].sort, w[1].key)) && entries.iter().all(|e| e.inside(data_len)))
        .then_some(entries)
}

fn write_index_atomic(path: &Path, entries: &[IndexEntry], data_len: u64, tail: u32) -> Result<(), PackError> {
    let tmp = path.with_extension("tidx.tmp");
    io(fs::write(&tmp, encode_index(entries, data_len, tail)), "writing", &tmp)?;
    // Another process (an indexer, a scanner) can briefly hold the target on Windows: retry a few times.
    let mut last = None;
    for attempt in 0..4 {
        match fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(std::time::Duration::from_millis(10 * (attempt + 1)));
    }
    Err(PackError::Io(format!("renaming into place {}: {}", path.display(), last.expect("at least one attempt"))))
}

fn pack_paths(dir: &Path, seq: u32) -> (PathBuf, PathBuf) {
    (dir.join(format!("pack-{seq:08}.tpack")), dir.join(format!("pack-{seq:08}.tidx")))
}

/// The sequence numbers of the packs in `dir`, ascending (the newest last). Only the canonical names count: zero-padded to eight
/// digits (more for large numbers). A missing directory has none.
fn pack_sequences(dir: &Path) -> Result<Vec<u32>, PackError> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(PackError::Io(format!("listing {}: {e}", dir.display()))),
    };
    let mut seqs = Vec::new();
    for e in rd {
        let name = io(e, "listing", dir)?.file_name().to_string_lossy().into_owned();
        if let Some(n) = name.strip_prefix("pack-").and_then(|r| r.strip_suffix(".tpack")) {
            // Only the canonical spelling counts (zero-padded to at least eight digits, no sign): a name that would be
            // formatted differently names a file that `pack_paths` would never open.
            if n.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(seq) = n.parse::<u32>() {
                    if format!("{seq:08}") == n {
                        seqs.push(seq);
                    }
                }
            }
        }
    }
    seqs.sort_unstable();
    Ok(seqs)
}

/// One pack opened for reading.
struct PackFile {
    path: PathBuf,
    entries: Vec<IndexEntry>,
    damage: Vec<Damage>,
    /// Read-only after open, and read by position, so concurrent readers never queue on it.
    file: File,
    /// Offsets of entries a read found unusable: `get` answers a miss for them from then on (ADR 0019 validation,
    /// ADR 0024 point 7), so a long-lived store reports one `Corrupt` per entry and then a miss.
    dropped: Mutex<Vec<u64>>,
}

impl PackFile {
    /// `Ok(None)` for a pack to skip (still being created, or made under another hash version); `problems` collects
    /// what a person should know.
    fn open(dir: &Path, seq: u32, params: StoreParams, problems: &mut Vec<String>) -> Result<Option<PackFile>, PackError> {
        let (data_path, index_path) = pack_paths(dir, seq);
        let mut file = io(File::open(&data_path), "opening", &data_path)?;
        let len = io(file.metadata(), "reading the size of", &data_path)?.len();
        let Some(header) = read_pack_header(&mut file, &data_path)? else { return Ok(None) };
        match check_pack_header(&data_path, &header, params) {
            Ok(HeaderCheck::Ours) => {}
            Ok(HeaderCheck::OtherHashVersion) => return Ok(None),
            Err(PackError::BadFile { path, why }) => {
                problems.push(format!("{path}: {why}; skipped"));
                return Ok(None);
            }
            Err(e) => return Err(e),
        }
        // A file shorter than its metadata said (cut under us) has no usable index: the scan below finds the torn tail.
        let tail = tail_crc(&mut file, len, &data_path)?;
        let mut damage = Vec::new();
        let entries = match tail.and_then(|t| read_index(&index_path, len, t)) {
            Some(e) => e,
            // A missing or stale index is rebuilt by one scan, never trusted. A reader leaves the files alone.
            None => {
                let s = scan(&mut file, len, &data_path)?;
                if let Tail::Damaged(at) = s.tail {
                    problems.push(format!(
                        "{}: damaged record header at offset {at}; the records after it are unreachable until a compaction",
                        data_path.display()
                    ));
                }
                let skipped = s.damage.iter().filter(|d| d.key.is_some()).count();
                if skipped > 0 {
                    problems.push(format!("{}: {skipped} record(s) fail their payload CRC and are skipped", data_path.display()));
                }
                damage = s.damage;
                s.entries
            }
        };
        Ok(Some(PackFile { path: data_path, entries, damage, file, dropped: Mutex::new(Vec::new()) }))
    }

    fn find(&self, sort: &SortKey, key: CacheKey) -> Option<&IndexEntry> {
        let start = self.entries.partition_point(|e| e.sort < *sort);
        self.entries[start..].iter().take_while(|e| e.sort == *sort).find(|e| e.key == key)
    }

    fn is_dropped(&self, e: &IndexEntry) -> bool {
        self.dropped.lock().expect("dropped entries mutex").contains(&e.offset)
    }

    /// Take an entry whose record failed its checks out of the in-RAM index. The pack file is not touched.
    fn drop_entry(&self, e: &IndexEntry) {
        self.dropped.lock().expect("dropped entries mutex").push(e.offset);
    }

    fn corrupt(&self, e: &IndexEntry) -> PackError {
        PackError::Corrupt { path: self.path.display().to_string(), offset: e.offset, key: e.key }
    }

    fn read(&self, e: &IndexEntry) -> Result<Record, PackError> {
        let total = RECORD_HEADER_LEN + e.length as usize;
        let mut buf = vec![0u8; total];
        match read_exact_at(&self.file, &mut buf, e.offset) {
            Ok(()) => {}
            // The file is shorter than the index says: the entry is bad, not the disk.
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Err(self.corrupt(e)),
            Err(err) => return Err(PackError::Io(format!("reading a record of {}: {err}", self.path.display()))),
        }
        decode_record(&buf).filter(|r| r.key == e.key).ok_or_else(|| self.corrupt(e))
    }
}

/// Fill `buf` from `file` at `offset` without moving or locking a shared cursor (`pread` / `ReadFile` with an offset).
#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, offset) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// The read side of a store: every pack of a directory, newest first on lookup.
pub struct TileStore {
    packs: Vec<PackFile>,
    problems: Vec<String>,
    gets: AtomicU64,
    found: AtomicU64,
    get_ns: AtomicU64,
}

/// What the read side of a store did so far (recorded only, STRM-022): lookups, those that found a record, and the time they took
/// in all threads together (index search, the positional read and the record's checks). Never an input of a decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoreReadStats {
    pub gets: u64,
    pub found: u64,
    pub get_ns: u64,
}

impl TileStore {
    /// Open the packs in `dir` for reading. A missing directory is an empty store. A cache degrades to misses: a pack that
    /// is still being created, made under another hash version (all its records are dead) or unreadable is skipped and
    /// reported by [`problems`](TileStore::problems). A pack of another body is an error naming it.
    pub fn open(dir: &Path, params: StoreParams) -> Result<TileStore, PackError> {
        let mut problems = Vec::new();
        let mut packs = Vec::new();
        for seq in pack_sequences(dir)? {
            if let Some(p) = PackFile::open(dir, seq, params, &mut problems)? {
                packs.push(p);
            }
        }
        Ok(TileStore { packs, problems, gets: AtomicU64::new(0), found: AtomicU64::new(0), get_ns: AtomicU64::new(0) })
    }

    /// What a person should know about packs that were skipped or damaged; empty for a healthy store.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Records a scan found unusable while opening (a payload that fails its CRC, a damaged header): reported with pack,
    /// offset and key so a verifier can fail on them. Damage that only shows when an entry is read is `get`'s `Corrupt`.
    pub fn damaged(&self) -> Vec<Damage> {
        self.packs.iter().flat_map(|p| p.damage.iter().cloned()).collect()
    }

    /// The record of `kind` for `tile` with content key `key`, if any pack holds it. If two packs hold the same key the
    /// newest pack wins (within a pack the later offset already won). A payload that fails its CRC, or an entry that points
    /// outside its pack, is [`PackError::Corrupt`] once: the caller treats it as a miss, regenerates and logs, and the entry leaves the
    /// in-RAM index, so asking again is a plain miss (the file stays as it is until a compaction).
    pub fn get(&self, kind: u8, tile: TileId, key: CacheKey) -> Result<Option<Record>, PackError> {
        let started = Instant::now();
        let out = self.get_unmeasured(kind, tile, key);
        self.gets.fetch_add(1, Ordering::Relaxed);
        self.found.fetch_add(u64::from(matches!(out, Ok(Some(_)))), Ordering::Relaxed);
        self.get_ns.fetch_add(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX), Ordering::Relaxed);
        out
    }

    /// The lookups made so far and the time they took (recorded only).
    pub fn read_stats(&self) -> StoreReadStats {
        StoreReadStats {
            gets: self.gets.load(Ordering::Relaxed),
            found: self.found.load(Ordering::Relaxed),
            get_ns: self.get_ns.load(Ordering::Relaxed),
        }
    }

    fn get_unmeasured(&self, kind: u8, tile: TileId, key: CacheKey) -> Result<Option<Record>, PackError> {
        let sort = sort_key(kind, tile);
        for pack in self.packs.iter().rev() {
            if let Some(e) = pack.find(&sort, key) {
                // A dropped copy stays a miss: a corrupt newest copy hides an older pack's intact one (ADR 0024 point 8).
                if pack.is_dropped(e) {
                    return Ok(None);
                }
                let read = pack.read(e);
                if matches!(read, Err(PackError::Corrupt { .. })) {
                    pack.drop_entry(e);
                }
                return read.map(Some);
            }
        }
        Ok(None)
    }

    /// Number of distinct index entries across packs (a key in two packs counts twice).
    pub fn len(&self) -> usize {
        self.packs.iter().map(|p| p.entries.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// The keys stored for `kind` at `tile` (several definitions or versions can exist per tile), newest pack first.
    pub fn keys_for(&self, kind: u8, tile: TileId) -> Vec<CacheKey> {
        let sort = sort_key(kind, tile);
        let mut out = Vec::new();
        for pack in self.packs.iter().rev() {
            let start = pack.entries.partition_point(|e| e.sort < sort);
            out.extend(pack.entries[start..].iter().take_while(|e| e.sort == sort).map(|e| e.key));
        }
        out
    }

    /// Every stored key in index order (kind, face, level, Hilbert), for verification and compaction.
    pub fn all_entries(&self) -> Vec<(u8, CacheKey)> {
        let mut out = Vec::new();
        for pack in &self.packs {
            out.extend(pack.entries.iter().map(|e| (e.sort[0], e.key)));
        }
        out
    }
}

/// The `i`-th of `count` evenly spaced picks among `total` entries (`i < count < total`), without overflow on any target.
pub fn pick(i: usize, count: usize, total: usize) -> usize {
    (i as u128 * total as u128 / count as u128) as usize
}

/// A record read from a store together with where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub record: Record,
    pub pack: String,
    pub offset: u64,
}

impl TileStore {
    /// `count` evenly spaced entries of the store in index order (all of them when `count` is 0 or at least the number of
    /// entries), read and CRC-checked. An entry that cannot be read is an `Err` in its place, so a verifier can report it.
    pub fn sample_records(&self, count: usize) -> Vec<Result<Stored, PackError>> {
        let total = self.len();
        let picks: Vec<usize> =
            if count == 0 || count >= total { (0..total).collect() } else { (0..count).map(|i| pick(i, count, total)).collect() };
        let mut out = Vec::with_capacity(picks.len());
        for n in picks {
            let mut left = n;
            for pack in &self.packs {
                if left < pack.entries.len() {
                    let e = &pack.entries[left];
                    out.push(pack.read(e).map(|record| Stored { record, pack: pack.path.display().to_string(), offset: e.offset }));
                    break;
                }
                left -= pack.entries.len();
            }
        }
        out
    }
}

/// The pack a writer is appending to.
struct Current {
    seq: u32,
    data_path: PathBuf,
    index_path: PathBuf,
    file: File,
    len: u64,
    entries: Vec<IndexEntry>,
    index_current: bool,
}

/// The write side: one writer per pack directory, holding an advisory lock on `writer.lock`.
pub struct PackWriter {
    dir: PathBuf,
    params: StoreParams,
    max_bytes: u64,
    cur: Current,
    lock: File,
    holder: PathBuf,
}

/// Open pack `seq` of `dir` for appending, or the next one that can take records: a pack of another hash version (its
/// records are dead) or with a damaged header in the middle is left alone and the writer moves on. A torn tail is cut.
fn open_pack(dir: &Path, params: StoreParams, mut seq: u32) -> Result<Current, PackError> {
    loop {
        let (data_path, index_path) = pack_paths(dir, seq);
        let mut file = io(OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&data_path), "opening", &data_path)?;
        let mut len = io(file.metadata(), "reading the size of", &data_path)?.len();
        let mut entries = Vec::new();
        let mut index_current = false;
        match read_pack_header(&mut file, &data_path)? {
            // New, or a crash before the header was written: no record can exist yet, so write the header again.
            None => {
                io(file.set_len(0), "truncating", &data_path)?;
                io(file.seek(SeekFrom::Start(0)), "seeking in", &data_path)?;
                io(file.write_all(&pack_header(params)), "writing", &data_path)?;
                len = PACK_HEADER_LEN;
            }
            Some(header) => match check_pack_header(&data_path, &header, params)? {
                HeaderCheck::OtherHashVersion => {
                    seq = seq.checked_add(1).ok_or_else(|| PackError::NoRoom { dir: dir.display().to_string() })?;
                    continue;
                }
                HeaderCheck::Ours => {
                    let tail = tail_crc(&mut file, len, &data_path)?;
                    if let Some(e) = tail.and_then(|t| read_index(&index_path, len, t)) {
                        entries = e;
                        index_current = true;
                    } else {
                        let s = scan(&mut file, len, &data_path)?;
                        match s.tail {
                            Tail::Clean => {}
                            // The torn tail of a crashed writer: cut it off. Nothing before it is touched.
                            Tail::Torn(at) => {
                                // A file cut under the scan is shorter than `at`: never grow it.
                                let at = at.min(io(file.metadata(), "reading the size of", &data_path)?.len());
                                io(file.set_len(at), "truncating", &data_path)?;
                                len = at;
                            }
                            // A damaged header with data after it: leave the pack exactly as it is and continue in a new one.
                            Tail::Damaged(_) => {
                                seq = seq.checked_add(1).ok_or_else(|| PackError::NoRoom { dir: dir.display().to_string() })?;
                                continue;
                            }
                        }
                        entries = s.entries;
                    }
                }
            },
        }
        io(file.seek(SeekFrom::End(0)), "seeking in", &data_path)?;
        return Ok(Current { seq, data_path, index_path, file, len, entries, index_current });
    }
}

fn holder_text(path: &Path) -> String {
    fs::read_to_string(path).map(|s| s.trim().to_string()).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "holder unknown".into())
}

/// Take the directory's writer lock (creating the directory): a held lock is [`PackError::Locked`], naming the holder. Returns
/// the locked file and the holder note's path (the caller removes the note and releases the lock when it is done).
fn take_dir_lock(dir: &Path) -> Result<(File, PathBuf), PackError> {
    io(fs::create_dir_all(dir), "creating", dir)?;
    let lock_path = dir.join("writer.lock");
    let holder = dir.join("writer.holder");
    let lock = io(OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&lock_path), "opening", &lock_path)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(PackError::Locked { path: lock_path.display().to_string(), holder: holder_text(&holder) });
        }
        Err(TryLockError::Error(e)) => return Err(PackError::Io(format!("locking {}: {e}", lock_path.display()))),
    }
    // The Windows lock is mandatory, so the locked file cannot be read by others: the note lives in a file of its own.
    let taken = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    io(fs::write(&holder, format!("pid {} took the lock at unix time {taken}\n", std::process::id())), "writing", &holder)?;
    Ok((lock, holder))
}

impl PackWriter {
    /// Take the directory's writer lock (a held lock is [`PackError::Locked`], naming the holder), then open the newest
    /// pack for appending, creating the first one if there is none. A torn tail is truncated; the index is loaded if it
    /// is current and rebuilt otherwise.
    pub fn open(dir: &Path, params: StoreParams, max_bytes: u64) -> Result<PackWriter, PackError> {
        let (lock, holder) = take_dir_lock(dir)?;
        let seq = pack_sequences(dir)?.last().copied().unwrap_or(1);
        let cur = open_pack(dir, params, seq)?;
        Ok(PackWriter { dir: dir.to_path_buf(), params, max_bytes, cur, lock, holder })
    }

    /// Append a record. If it would take the pack over the size limit, the pack is finished (its index written) and a new
    /// one with the next sequence number is started. A failed write is undone: the pack is cut back to its last good
    /// record, so the records appended before and after stay where the index says.
    pub fn append(&mut self, record: &Record) -> Result<(), PackError> {
        let bytes = encode_record(record)?;
        if self.cur.len > PACK_HEADER_LEN && self.cur.len + bytes.len() as u64 > self.max_bytes {
            self.rollover()?;
        }
        if let Err(e) = self.cur.file.write_all(&bytes) {
            let _ = self.cur.file.set_len(self.cur.len).and_then(|()| self.cur.file.seek(SeekFrom::Start(self.cur.len)).map(|_| ()));
            return Err(PackError::Io(format!("appending to {}: {e}", self.cur.data_path.display())));
        }
        self.cur.entries.push(IndexEntry {
            sort: sort_key(record.kind, record.tile),
            key: record.key,
            offset: self.cur.len,
            length: record.payload.len() as u32,
        });
        self.cur.len += bytes.len() as u64;
        self.cur.index_current = false;
        Ok(())
    }

    /// Write the index of the current pack atomically (temp file and rename).
    pub fn write_index(&mut self) -> Result<(), PackError> {
        let c = &mut self.cur;
        c.entries = normalise(std::mem::take(&mut c.entries));
        io(c.file.flush(), "flushing", &c.data_path)?;
        // The writer holds the lock, so a pack shorter than the bytes it wrote is damage, not a race.
        let tail = tail_crc(&mut c.file, c.len, &c.data_path)?
            .ok_or_else(|| PackError::Io(format!("{} is shorter than the {} bytes written to it", c.data_path.display(), c.len)))?;
        write_index_atomic(&c.index_path, &c.entries, c.len, tail)?;
        io(c.file.seek(SeekFrom::End(0)), "seeking in", &c.data_path)?;
        c.index_current = true;
        Ok(())
    }

    fn rollover(&mut self) -> Result<(), PackError> {
        self.write_index()?;
        let next = self.cur.seq.checked_add(1).ok_or_else(|| PackError::NoRoom { dir: self.dir.display().to_string() })?;
        self.cur = open_pack(&self.dir, self.params, next)?;
        Ok(())
    }

    /// Sequence number of the pack being written.
    pub fn seq(&self) -> u32 {
        self.cur.seq
    }

    /// Write the index and release the lock.
    pub fn finish(mut self) -> Result<(), PackError> {
        self.write_index()
    }
}

impl Drop for PackWriter {
    fn drop(&mut self) {
        if !self.cur.index_current {
            let _ = self.write_index();
        }
        let _ = fs::remove_file(&self.holder);
        let _ = self.lock.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::Face;

    fn record(i: u8) -> Record {
        Record {
            key: CacheKey([i; 16]),
            kind: 1,
            tile: TileId::new(Face(0), 3, u32::from(i) % 8, 0).unwrap(),
            generator_version: 1,
            hash_version: 1,
            node_set_id: [0; 16],
            payload: vec![i; 40],
        }
    }

    /// A pack file of `n` records, returning its path and the offset of each record.
    fn make_pack(name: &str, n: u8) -> (PathBuf, Vec<u64>) {
        let path = std::env::temp_dir().join(format!("planet-pack-unit-{}-{name}.tpack", std::process::id()));
        let mut bytes = pack_header(StoreParams { hash_version: 1, body: 0 }).to_vec();
        let mut at = Vec::new();
        for i in 0..n {
            at.push(bytes.len() as u64);
            bytes.extend(encode_record(&record(i)).unwrap());
        }
        fs::write(&path, bytes).unwrap();
        (path, at)
    }

    // spec: STRM-008
    #[test]
    fn a_file_cut_under_a_scan_is_a_torn_tail_at_that_position() {
        let (path, at) = make_pack("cut", 4);
        let full = fs::metadata(&path).unwrap().len();
        // The scan was told the file is `full` long. Cut it inside the header of record 3, inside its payload and at its
        // start: each time the records before the cut stay and the tail is torn at the first incomplete record.
        for cut in [at[3] + 10, at[3] + RECORD_HEADER_LEN as u64 + 5, at[3]] {
            let mut f = OpenOptions::new().read(true).write(true).open(&path).unwrap();
            f.set_len(cut).unwrap();
            let s = scan(&mut f, full, &path).unwrap_or_else(|e| panic!("cut at {cut}: {e}"));
            assert_eq!(s.tail, Tail::Torn(at[3]), "cut at {cut}");
            assert_eq!(s.entries.len(), 3, "cut at {cut}");
        }
        let _ = fs::remove_file(&path);
    }

    // spec: STRM-008
    #[test]
    fn a_zero_run_cut_under_a_scan_is_a_torn_tail_and_an_unknown_tail_checksum_is_not_an_error() {
        let (path, at) = make_pack("zero", 3);
        let full = fs::metadata(&path).unwrap().len();
        // Zero the last record, then cut the file inside the zero run: the zero check meets the end of the file.
        let mut bytes = fs::read(&path).unwrap();
        bytes[at[2] as usize..].fill(0);
        bytes.truncate(at[2] as usize + 70);
        fs::write(&path, &bytes).unwrap();
        let mut f = File::open(&path).unwrap();
        let s = scan(&mut f, full, &path).unwrap();
        assert_eq!(s.tail, Tail::Torn(at[2]));
        assert_eq!(s.entries.len(), 2);
        assert_eq!(tail_crc(&mut f, full, &path).unwrap(), None);
        let _ = fs::remove_file(&path);
    }
}
