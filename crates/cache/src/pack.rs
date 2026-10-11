//! The tile store: append-only packs with a rebuildable Hilbert-ordered index (ADR 0019, STRM-008).
//!
//! A pack (`pack-<seq>.tpack`) is a header and records, appended and never updated in place. Its index
//! (`pack-<seq>.tidx`) is a sorted array written atomically and always rebuildable from the data by one scan; a missing
//! or stale index is rebuilt, never trusted. One writer process holds an advisory lock on `pack-<seq>.lock` (the OS
//! releases it when the process dies, so a crashed writer never blocks the cache); any number of readers need no lock.
//! A torn tail is detected by length and CRC and truncated by the writer; readers ignore it. A cache needs correctness
//! (content keys, CRC), not deterministic bytes, so the bytes of a pack written by streaming are not specified.
//!
//! Record (all integers little endian): `key (16) | kind (u8) | tile id (u64) | generator version (u32) | hash version
//! (u32) | node set id (16) | payload length (u32) | payload CRC-32C (u32) | payload`. The tile id is not in the ADR's
//! list; the index must be rebuildable from the data alone and its sort key needs the tile (proposed ADR 0024).

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use planet_core::TileId;

use crate::crc32c::crc32c;
use crate::hilbert::{sort_key, SortKey};
use crate::key::CacheKey;

pub const PACK_MAGIC: [u8; 8] = *b"PLNTPACK";
pub const INDEX_MAGIC: [u8; 8] = *b"PLNTTIDX";
/// Version of the pack and index layouts.
pub const PACK_FORMAT_VERSION: u32 = 1;
/// A pack file rolls over at 4 GiB (offsets are `u64` regardless).
pub const DEFAULT_MAX_PACK_BYTES: u64 = 4 << 30;

const PACK_HEADER_LEN: u64 = 24;
const RECORD_HEADER_LEN: usize = 57;
const INDEX_HEADER_LEN: usize = 40;
const INDEX_ENTRY_LEN: usize = 40;
/// How much of the end of the data the index's tail CRC covers.
const TAIL_BYTES: u64 = 4096;

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
    /// A payload failed its CRC: treat as a cache miss, regenerate and log (ADR 0019).
    Corrupt {
        path: String,
        offset: u64,
        key: CacheKey,
    },
    TooLarge {
        len: usize,
    },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Io(m) => write!(f, "{m}"),
            PackError::BadFile { path, why } => write!(f, "{path}: {why}"),
            PackError::WrongStore { path, found, wanted } => write!(
                f,
                "{path} belongs to another store (hash version {}, body {}; wanted hash version {}, body {}): use another cache directory",
                found.hash_version, found.body, wanted.hash_version, wanted.body
            ),
            PackError::Locked { path, holder } => {
                write!(f, "{path} is locked by another writer ({holder}): wait for it or use another pack directory")
            }
            PackError::Corrupt { path, offset, key } => {
                write!(f, "{path}: payload of key {key} at offset {offset} fails its CRC; regenerate the tile")
            }
            PackError::TooLarge { len } => write!(f, "a payload of {len} bytes does not fit a record (u32 length)"),
        }
    }
}

impl std::error::Error for PackError {}

fn io<T>(r: std::io::Result<T>, what: &str, path: &Path) -> Result<T, PackError> {
    r.map_err(|e| PackError::Io(format!("{what} {}: {e}", path.display())))
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

    fn decode(b: &[u8]) -> IndexEntry {
        IndexEntry {
            sort: b[0..11].try_into().expect("11 bytes"),
            key: CacheKey(b[11..27].try_into().expect("16 bytes")),
            offset: u64::from_le_bytes(b[27..35].try_into().expect("8 bytes")),
            length: u32::from_le_bytes(b[35..39].try_into().expect("4 bytes")),
        }
    }
}

/// Encode a record (the wire format too: key, kind, versions, length, CRC, payload).
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

fn decode_header(b: &[u8]) -> Option<RecordHeader> {
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

/// Decode one whole record from `bytes` (header and payload), checking the CRC.
pub fn decode_record(bytes: &[u8]) -> Option<Record> {
    let h = decode_header(bytes.get(..RECORD_HEADER_LEN)?)?;
    let payload = bytes.get(RECORD_HEADER_LEN..RECORD_HEADER_LEN + h.length as usize)?;
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

fn check_pack_header(path: &Path, h: &[u8], wanted: StoreParams) -> Result<(), PackError> {
    let bad = |why: &str| PackError::BadFile { path: path.display().to_string(), why: why.to_string() };
    if h.len() < PACK_HEADER_LEN as usize || h[0..8] != PACK_MAGIC {
        return Err(bad("not a tile pack (bad magic)"));
    }
    let format = u32::from_le_bytes(h[8..12].try_into().expect("4 bytes"));
    if format != PACK_FORMAT_VERSION {
        return Err(bad(&format!("pack format version {format}, this build reads {PACK_FORMAT_VERSION}")));
    }
    let found = StoreParams {
        hash_version: u32::from_le_bytes(h[12..16].try_into().expect("4 bytes")),
        body: u32::from_le_bytes(h[16..20].try_into().expect("4 bytes")),
    };
    if found != wanted {
        return Err(PackError::WrongStore { path: path.display().to_string(), found, wanted });
    }
    Ok(())
}

/// Scan a pack's records: the entries of every intact record and the end of the intact prefix. A record that is cut
/// short or fails its CRC ends the scan; everything after it is a torn tail.
fn scan(file: &mut File, len: u64) -> Result<(Vec<IndexEntry>, u64), PackError> {
    let mut entries = Vec::new();
    let mut pos = PACK_HEADER_LEN;
    let mut head = [0u8; RECORD_HEADER_LEN];
    loop {
        if pos + RECORD_HEADER_LEN as u64 > len {
            break;
        }
        let read = file.seek(SeekFrom::Start(pos)).and_then(|_| file.read_exact(&mut head));
        if read.is_err() {
            break;
        }
        let Some(h) = decode_header(&head) else { break };
        let end = pos + RECORD_HEADER_LEN as u64 + u64::from(h.length);
        if end > len {
            break;
        }
        let mut payload = vec![0u8; h.length as usize];
        if file.read_exact(&mut payload).is_err() || crc32c(&payload) != h.crc {
            break;
        }
        entries.push(IndexEntry { sort: sort_key(h.kind, h.tile), key: h.key, offset: pos, length: h.length });
        pos = end;
    }
    Ok((normalise(entries), pos))
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

fn tail_crc(file: &mut File, data_len: u64) -> Result<u32, PackError> {
    let start = data_len.saturating_sub(TAIL_BYTES).max(PACK_HEADER_LEN.min(data_len));
    let mut buf = vec![0u8; (data_len - start) as usize];
    let r = file.seek(SeekFrom::Start(start)).and_then(|_| file.read_exact(&mut buf));
    r.map_err(|e| PackError::Io(format!("reading the tail of a pack: {e}")))?;
    Ok(crc32c(&buf))
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

/// Read an index file: its entries, if it is well formed and was built from exactly the data of length `data_len` whose
/// tail has CRC `tail`. Anything else is stale and the caller rebuilds.
fn read_index(path: &Path, data_len: u64, tail: u32) -> Option<Vec<IndexEntry>> {
    let b = fs::read(path).ok()?;
    if b.len() < INDEX_HEADER_LEN || b[0..8] != INDEX_MAGIC || u32::from_le_bytes(b[8..12].try_into().ok()?) != PACK_FORMAT_VERSION {
        return None;
    }
    if u64::from_le_bytes(b[16..24].try_into().ok()?) != data_len || u32::from_le_bytes(b[24..28].try_into().ok()?) != tail {
        return None;
    }
    let count = usize::try_from(u64::from_le_bytes(b[32..40].try_into().ok()?)).ok()?;
    if b.len() != INDEX_HEADER_LEN + count.checked_mul(INDEX_ENTRY_LEN)? {
        return None;
    }
    let entries: Vec<IndexEntry> = b[INDEX_HEADER_LEN..].as_chunks::<INDEX_ENTRY_LEN>().0.iter().map(|c| IndexEntry::decode(c)).collect();
    entries.windows(2).all(|w| w[0] < w[1]).then_some(entries)
}

fn write_index_atomic(path: &Path, entries: &[IndexEntry], data_len: u64, tail: u32) -> Result<(), PackError> {
    let tmp = path.with_extension("tidx.tmp");
    io(fs::write(&tmp, encode_index(entries, data_len, tail)), "writing", &tmp)?;
    io(fs::rename(&tmp, path), "renaming into place", path)
}

fn pack_paths(dir: &Path, seq: u32) -> (PathBuf, PathBuf, PathBuf) {
    (dir.join(format!("pack-{seq:08}.tpack")), dir.join(format!("pack-{seq:08}.tidx")), dir.join(format!("pack-{seq:08}.lock")))
}

/// The sequence numbers of the packs in `dir`, ascending (the newest last).
fn pack_sequences(dir: &Path) -> Result<Vec<u32>, PackError> {
    let mut seqs = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return Ok(seqs) };
    for e in rd {
        let name = io(e, "listing", dir)?.file_name().to_string_lossy().into_owned();
        if let Some(n) = name.strip_prefix("pack-").and_then(|r| r.strip_suffix(".tpack")) {
            if let Ok(seq) = n.parse::<u32>() {
                seqs.push(seq);
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
    file: Mutex<File>,
}

impl PackFile {
    fn open(dir: &Path, seq: u32, params: StoreParams) -> Result<PackFile, PackError> {
        let (data_path, index_path, _) = pack_paths(dir, seq);
        let mut file = io(File::open(&data_path), "opening", &data_path)?;
        let len = io(file.metadata(), "reading the size of", &data_path)?.len();
        let mut header = [0u8; PACK_HEADER_LEN as usize];
        if io(file.read(&mut header), "reading", &data_path)? < header.len() {
            return Err(PackError::BadFile { path: data_path.display().to_string(), why: "shorter than a pack header".into() });
        }
        check_pack_header(&data_path, &header, params)?;
        let tail = tail_crc(&mut file, len)?;
        let entries = match read_index(&index_path, len, tail) {
            Some(e) => e,
            // A missing or stale index is rebuilt by one scan, never trusted. A reader leaves the files alone.
            None => scan(&mut file, len)?.0,
        };
        Ok(PackFile { path: data_path, entries, file: Mutex::new(file) })
    }

    fn find(&self, sort: &SortKey, key: CacheKey) -> Option<&IndexEntry> {
        let start = self.entries.partition_point(|e| e.sort < *sort);
        self.entries[start..].iter().take_while(|e| e.sort == *sort).find(|e| e.key == key)
    }

    fn read(&self, e: &IndexEntry) -> Result<Record, PackError> {
        let total = RECORD_HEADER_LEN + e.length as usize;
        let mut buf = vec![0u8; total];
        {
            let mut f = self.file.lock().expect("pack file mutex");
            let r = f.seek(SeekFrom::Start(e.offset)).and_then(|_| f.read_exact(&mut buf));
            io(r, "reading a record of", &self.path)?;
        }
        decode_record(&buf).filter(|r| r.key == e.key).ok_or_else(|| PackError::Corrupt {
            path: self.path.display().to_string(),
            offset: e.offset,
            key: e.key,
        })
    }
}

/// The read side of a store: every pack of a directory, newest first on lookup.
pub struct TileStore {
    packs: Vec<PackFile>,
}

impl TileStore {
    /// Open the packs in `dir` for reading. A missing directory is an empty store. A pack of another store (another hash
    /// version or body) is an error naming it.
    pub fn open(dir: &Path, params: StoreParams) -> Result<TileStore, PackError> {
        let packs = pack_sequences(dir)?.into_iter().map(|s| PackFile::open(dir, s, params)).collect::<Result<Vec<_>, _>>()?;
        Ok(TileStore { packs })
    }

    /// The record of `kind` for `tile` with content key `key`, if any pack holds it. If two packs hold the same key the
    /// newest pack wins (within a pack the later offset already won). A payload that fails its CRC is
    /// [`PackError::Corrupt`]: the caller treats it as a miss, regenerates and logs.
    pub fn get(&self, kind: u8, tile: TileId, key: CacheKey) -> Result<Option<Record>, PackError> {
        let sort = sort_key(kind, tile);
        for pack in self.packs.iter().rev() {
            if let Some(e) = pack.find(&sort, key) {
                return pack.read(e).map(Some);
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

/// The write side: one writer per pack directory, holding an advisory lock on the newest pack.
pub struct PackWriter {
    dir: PathBuf,
    params: StoreParams,
    max_bytes: u64,
    seq: u32,
    data_path: PathBuf,
    index_path: PathBuf,
    lock_path: PathBuf,
    file: File,
    lock: File,
    len: u64,
    entries: Vec<IndexEntry>,
    index_current: bool,
}

/// The note a writer leaves about itself next to its lock. It is a separate file because the lock file itself cannot be
/// read by anyone else while it is locked (the Windows lock is mandatory).
fn holder_path(lock_path: &Path) -> PathBuf {
    lock_path.with_extension("holder")
}

fn lock_holder(lock_path: &Path) -> String {
    fs::read_to_string(holder_path(lock_path))
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "holder unknown".into())
}

impl PackWriter {
    /// Open the newest pack of `dir` for appending, creating the first one if there is none. Takes the pack's advisory
    /// lock (a held lock is [`PackError::Locked`], naming the holder), truncates a torn tail and loads or rebuilds the
    /// index.
    pub fn open(dir: &Path, params: StoreParams, max_bytes: u64) -> Result<PackWriter, PackError> {
        io(fs::create_dir_all(dir), "creating", dir)?;
        let seq = pack_sequences(dir)?.last().copied().unwrap_or(1);
        PackWriter::open_seq(dir, params, max_bytes, seq)
    }

    fn open_seq(dir: &Path, params: StoreParams, max_bytes: u64, seq: u32) -> Result<PackWriter, PackError> {
        let (data_path, index_path, lock_path) = pack_paths(dir, seq);
        let lock = io(OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&lock_path), "opening", &lock_path)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(PackError::Locked { path: lock_path.display().to_string(), holder: lock_holder(&lock_path) });
            }
            Err(TryLockError::Error(e)) => return Err(PackError::Io(format!("locking {}: {e}", lock_path.display()))),
        }
        // Say who holds the lock (for the message another writer sees), now that we hold it.
        let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let note = holder_path(&lock_path);
        io(fs::write(&note, format!("pid {} started at unix time {started}\n", std::process::id())), "writing", &note)?;

        let mut file = io(OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&data_path), "opening", &data_path)?;
        let mut len = io(file.metadata(), "reading the size of", &data_path)?.len();
        let (entries, index_current);
        if len == 0 {
            io(file.write_all(&pack_header(params)), "writing", &data_path)?;
            len = PACK_HEADER_LEN;
            entries = Vec::new();
            index_current = false;
        } else {
            let mut header = [0u8; PACK_HEADER_LEN as usize];
            io(file.seek(SeekFrom::Start(0)), "seeking in", &data_path)?;
            if io(file.read(&mut header), "reading", &data_path)? < header.len() {
                return Err(PackError::BadFile { path: data_path.display().to_string(), why: "shorter than a pack header".into() });
            }
            check_pack_header(&data_path, &header, params)?;
            let (scanned, valid_end) = scan(&mut file, len)?;
            if valid_end < len {
                // The torn tail of a crashed writer: cut it off. Nothing before it is touched.
                io(file.set_len(valid_end), "truncating", &data_path)?;
                len = valid_end;
            }
            let tail = tail_crc(&mut file, len)?;
            index_current = read_index(&index_path, len, tail).is_some();
            entries = scanned;
        }
        io(file.seek(SeekFrom::End(0)), "seeking in", &data_path)?;
        Ok(PackWriter {
            dir: dir.to_path_buf(),
            params,
            max_bytes,
            seq,
            data_path,
            index_path,
            lock_path,
            file,
            lock,
            len,
            entries,
            index_current,
        })
    }

    /// Append a record. If it would take the pack over the size limit, the pack is finished (its index written) and a new
    /// one with the next sequence number is started.
    pub fn append(&mut self, record: &Record) -> Result<(), PackError> {
        let bytes = encode_record(record)?;
        if self.len > PACK_HEADER_LEN && self.len + bytes.len() as u64 > self.max_bytes {
            self.rollover()?;
        }
        io(self.file.write_all(&bytes), "appending to", &self.data_path)?;
        let entry =
            IndexEntry { sort: sort_key(record.kind, record.tile), key: record.key, offset: self.len, length: record.payload.len() as u32 };
        self.entries.push(entry);
        self.len += bytes.len() as u64;
        self.index_current = false;
        Ok(())
    }

    /// Write the index of the current pack atomically (temp file and rename).
    pub fn write_index(&mut self) -> Result<(), PackError> {
        self.entries = normalise(std::mem::take(&mut self.entries));
        io(self.file.flush(), "flushing", &self.data_path)?;
        let tail = tail_crc(&mut self.file, self.len)?;
        write_index_atomic(&self.index_path, &self.entries, self.len, tail)?;
        io(self.file.seek(SeekFrom::End(0)), "seeking in", &self.data_path)?;
        self.index_current = true;
        Ok(())
    }

    fn rollover(&mut self) -> Result<(), PackError> {
        self.write_index()?;
        let next = PackWriter::open_seq(&self.dir, self.params, self.max_bytes, self.seq + 1)?;
        // Dropping the old writer releases its lock; the new one already holds its own.
        *self = next;
        Ok(())
    }

    /// Sequence number of the pack being written.
    pub fn seq(&self) -> u32 {
        self.seq
    }

    /// Write the index and release the lock.
    pub fn finish(mut self) -> Result<(), PackError> {
        self.write_index()
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for PackWriter {
    fn drop(&mut self) {
        if !self.index_current {
            let _ = self.write_index();
        }
        let _ = self.lock.unlock();
    }
}
